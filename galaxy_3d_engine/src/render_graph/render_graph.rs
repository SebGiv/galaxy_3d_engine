/// Render graph — a DAG of render passes selected for execution this frame.
///
/// Owns a ring of `CommandList`s (one per frame in flight) and the scratch
/// buffers used by `execute()` for topological sort, previous-access
/// resolution, and per-pass image-access lists. All scratch is `Vec` /
/// `FxHashMap` reused across frames via `clear()` — zero heap allocation
/// in steady state.

use std::collections::VecDeque;
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use crate::error::Result;
use crate::engine_bail;
use crate::engine::Engine;
use crate::graphics_device;
use super::access_type::{AccessType, TargetOps};
use super::frame_buffer::{Framebuffer, FramebufferKey};
use super::graph_resource::{GraphResource, GraphResourceKey};
use super::render_pass::{RenderPass, RenderPassKey};

slotmap::new_key_type! {
    /// Stable key for a `RenderGraph` in the `RenderGraphManager`.
    pub struct RenderGraphKey;
}

pub struct RenderGraph {
    name: String,
    command_lists: Vec<Box<dyn graphics_device::CommandList>>,
    current_frame: usize,

    // ===== Per-execute scratch (all reused via clear(), zero alloc steady-state) =====
    sorted_passes: Vec<RenderPassKey>,
    prev_access: FxHashMap<GraphResourceKey, AccessType>,
    image_accesses: Vec<graphics_device::ImageAccess>,
    buffer_accesses: Vec<graphics_device::BufferAccess>,

    // Topological sort scratch
    in_degree: FxHashMap<RenderPassKey, u32>,
    successors: FxHashMap<RenderPassKey, Vec<RenderPassKey>>,
    writers: FxHashMap<GraphResourceKey, RenderPassKey>,
    topo_queue: VecDeque<RenderPassKey>,
}

impl RenderGraph {
    /// Internal — created via `RenderGraphManager::create_render_graph()`.
    pub(crate) fn new(
        name: String,
        graphics_device: &dyn graphics_device::GraphicsDevice,
        frames_in_flight: usize,
    ) -> Result<Self> {
        if frames_in_flight == 0 {
            engine_bail!("galaxy3d::RenderGraph",
                "frames_in_flight must be at least 1");
        }
        let mut command_lists = Vec::with_capacity(frames_in_flight);
        for _ in 0..frames_in_flight {
            command_lists.push(graphics_device.create_command_list()?);
        }
        Ok(Self {
            name,
            command_lists,
            current_frame: frames_in_flight - 1,
            sorted_passes: Vec::new(),
            prev_access: FxHashMap::default(),
            image_accesses: Vec::new(),
            buffer_accesses: Vec::new(),
            in_degree: FxHashMap::default(),
            successors: FxHashMap::default(),
            writers: FxHashMap::default(),
            topo_queue: VecDeque::new(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Borrow the command list recorded by the most recent `execute()` call.
    pub fn command_list(&self) -> Result<&dyn graphics_device::CommandList> {
        if self.command_lists.is_empty() {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderGraph '{}' has no command lists", self.name);
        }
        Ok(&*self.command_lists[self.current_frame])
    }

    /// Execute the graph for one frame.
    ///
    /// `passes` is the set of `RenderPass`es to run. The graph topologically
    /// orders them from their `ResourceAccess`es, recompiles dirty passes,
    /// resolves `previous_access_type` per access, then records every pass
    /// into the current frame's command list. `post_passes` runs after the
    /// last pass (typically a swapchain blit) inside the same command list.
    ///
    /// # Borrow split
    ///
    /// This method receives the pass map and the graph-resource map as
    /// separate borrows (instead of `&mut RenderGraphManager`) because
    /// `self` itself is owned by the manager — splitting borrows is the
    /// only way to mutate passes while reading graph resources.
    pub(crate) fn execute<F>(
        &mut self,
        passes_map: &mut SlotMap<RenderPassKey, RenderPass>,
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        framebuffers: &SlotMap<FramebufferKey, Framebuffer>,
        passes: &[RenderPassKey],
        post_passes: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut dyn graphics_device::CommandList) -> Result<()>,
    {
        // The ResourceManager is locked only briefly to materialise each
        // pass's image/buffer accesses (Arc clones are stashed in the
        // scratch lists), and the lock is released before any user
        // callback (`PassAction::execute`, `post_passes`). Otherwise a
        // drawer that re-locks `Engine::resource_manager()` deadlocks.
        let rm_arc = Engine::resource_manager()?;
        if self.command_lists.is_empty() {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderGraph '{}' has no command lists", self.name);
        }

        // 1. Validate keys.
        for &k in passes {
            if !passes_map.contains_key(k) {
                engine_bail!("galaxy3d::RenderGraph",
                    "RenderGraph '{}': RenderPassKey not found", self.name);
            }
        }

        // 2. (Pass caches are eager — no recompile here.)

        // 3. Topological sort.
        self.topological_sort(passes_map, passes)?;

        // 4. Advance ring command list.
        let frame = (self.current_frame + 1) % self.command_lists.len();
        self.current_frame = frame;
        self.command_lists[frame].begin()?;

        // Wrap pass execution in a closure so we can always end() the
        // command list, even on error — otherwise the next frame's
        // begin() would fail on a still-recording list.
        self.prev_access.clear();
        let result = (|| -> Result<()> {
            for i in 0..self.sorted_passes.len() {
                let pass_key = self.sorted_passes[i];

                // Build the per-pass image/buffer access lists with
                // resolved `previous_access_type` into the scratch buffers.
                // The `prev_access` map is updated as we go so subsequent
                // passes in the same frame see the right source state.
                self.image_accesses.clear();
                self.buffer_accesses.clear();
                // Materialise image/buffer accesses (Arc clones) under
                // a brief RM lock — the lock is dropped at the end of
                // this scope before pass.action_mut().execute() runs.
                {
                    let resource_manager = rm_arc.lock().unwrap();
                    let pass = passes_map.get(pass_key).unwrap();
                    for access in pass.accesses() {
                        let prev = self.prev_access
                            .get(&access.graph_resource_key)
                            .copied();
                        match graph_resources.get(access.graph_resource_key).copied() {
                            Some(GraphResource::Texture { texture_key, .. }) => {
                                if let Some(tex) = resource_manager.texture(texture_key) {
                                    self.image_accesses.push(graphics_device::ImageAccess {
                                        texture: tex.graphics_device_texture().clone(),
                                        access_type: access.access_type,
                                        previous_access_type: prev,
                                    });
                                }
                            }
                            Some(GraphResource::Buffer(buf_key)) => {
                                if let Some(buf) = resource_manager.buffer(buf_key) {
                                    self.buffer_accesses.push(graphics_device::BufferAccess {
                                        buffer: buf.graphics_device_buffer().clone(),
                                        access_type: access.access_type,
                                        previous_access_type: prev,
                                    });
                                }
                            }
                            None => {}
                        }
                        self.prev_access.insert(access.graph_resource_key, access.access_type);

                        // An MSAA color attachment with a `resolve_target`
                        // also writes to the resolve texture at end-of-pass.
                        // Track it in `prev_access` so the next reader of
                        // the resolved texture sees `ColorAttachmentWrite`
                        // as the source state.
                        if let Some(TargetOps::Color { resolve_target: Some(rt), .. })
                            = access.target_ops
                        {
                            self.prev_access.insert(rt, AccessType::ColorAttachmentWrite);
                        }
                    }
                }

                // Begin → action → end. Skip passes without attachments
                // (compute-only paths are not yet recorded here).
                let pass = passes_map.get_mut(pass_key).unwrap();
                let (rp, fb_key) = match (pass.gd_render_pass(), pass.framebuffer_key()) {
                    (Some(rp), Some(fb_key)) => (rp.clone(), fb_key),
                    _ => continue,
                };
                let fb = framebuffers.get(fb_key).ok_or_else(|| {
                    crate::engine_err!("galaxy3d::RenderGraph",
                        "Pass '{}': framebuffer_key not found in manager", pass.name())
                })?;
                let gd_fb = fb.gd_framebuffer().clone();
                self.command_lists[frame].begin_render_pass(
                    &rp,
                    &gd_fb,
                    pass.clear_values(),
                    &self.image_accesses,
                    &self.buffer_accesses,
                )?;
                let pass_info_clone = pass.pass_info().cloned().ok_or_else(|| {
                    crate::engine_err!("galaxy3d::RenderGraph",
                        "Pass '{}' has attachments but no PassInfo", pass.name())
                })?;
                pass.action_mut().execute(
                    &mut *self.command_lists[frame],
                    &pass_info_clone,
                )?;
                self.command_lists[frame].end_render_pass()?;
            }

            // 5. Post-passes hook (e.g. swapchain blit).
            post_passes(&mut *self.command_lists[frame])?;
            Ok(())
        })();

        self.command_lists[frame].end()?;
        result
    }

    /// Kahn's algorithm: writer-before-reader on shared `GraphResourceKey`s.
    ///
    /// Result lands in `self.sorted_passes`. All scratch maps are cleared
    /// at entry, populated, then the queue is drained.
    fn topological_sort(
        &mut self,
        passes_map: &SlotMap<RenderPassKey, RenderPass>,
        passes: &[RenderPassKey],
    ) -> Result<()> {
        self.in_degree.clear();
        self.successors.clear();
        self.writers.clear();
        self.topo_queue.clear();
        self.sorted_passes.clear();

        for &k in passes {
            self.in_degree.insert(k, 0);
            self.successors.insert(k, Vec::new());
        }

        // Map each shared resource to its (last) writer in this batch.
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                if access.access_type.is_write() {
                    self.writers.insert(access.graph_resource_key, k);
                }
            }
        }

        // Reader → depends on writer (if any, and not itself).
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                if !access.access_type.is_write() {
                    if let Some(&writer) = self.writers.get(&access.graph_resource_key) {
                        if writer != k {
                            *self.in_degree.get_mut(&k).unwrap() += 1;
                            self.successors.get_mut(&writer).unwrap().push(k);
                        }
                    }
                }
            }
        }

        for &k in passes {
            if self.in_degree[&k] == 0 {
                self.topo_queue.push_back(k);
            }
        }

        while let Some(k) = self.topo_queue.pop_front() {
            self.sorted_passes.push(k);
            let succs = self.successors.get(&k).unwrap().clone();
            for s in succs {
                let d = self.in_degree.get_mut(&s).unwrap();
                *d -= 1;
                if *d == 0 {
                    self.topo_queue.push_back(s);
                }
            }
        }

        if self.sorted_passes.len() != passes.len() {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderGraph '{}': cycle detected ({} of {} passes ordered)",
                self.name, self.sorted_passes.len(), passes.len());
        }

        Ok(())
    }
}
