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
use crate::resource::resource_manager::{BufferKey, TextureKey};
use super::access_type::{AccessType, TargetOps};
use super::frame_buffer::{Framebuffer, FramebufferKey};
use super::graph_resource::{
    BufferSubRange, GraphResource, GraphResourceKey, ImageSubRange,
};
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
    image_accesses: Vec<graphics_device::ImageAccess>,
    buffer_accesses: Vec<graphics_device::BufferAccess>,

    // Per-frame access history, indexed by the underlying GPU
    // TextureKey / BufferKey (NOT by GraphResourceKey). Two distinct
    // GraphResources pointing at the same texture share their history,
    // so the second-pass barrier sees the right `previous_access_type`
    // and avoids the bug-tracking discard.
    //
    // Persisted across frames: `clear_access_history` clears the inner
    // Vecs (preserving their capacity) without dropping the outer
    // HashMap. Zero allocation in steady state.
    prev_image_accesses: FxHashMap<TextureKey, Vec<(ImageSubRange, AccessType)>>,
    prev_buffer_accesses: FxHashMap<BufferKey, Vec<(BufferSubRange, AccessType)>>,

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
            image_accesses: Vec::new(),
            buffer_accesses: Vec::new(),
            // Pre-size the access-history maps to a comfortable capacity
            // so the steady-state regime is reached quickly with no
            // rehash. 64 textures / 32 buffers covers our typical graphs.
            prev_image_accesses: FxHashMap::with_capacity_and_hasher(
                64, Default::default(),
            ),
            prev_buffer_accesses: FxHashMap::with_capacity_and_hasher(
                32, Default::default(),
            ),
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
        //
        // Per-frame access history is reset *without* freeing the inner
        // Vec buffers — they keep their capacity, so the steady-state
        // run does no heap allocation here.
        self.clear_access_history();
        let result = (|| -> Result<()> {
            for i in 0..self.sorted_passes.len() {
                let pass_key = self.sorted_passes[i];

                // Build the per-pass image/buffer access lists with
                // resolved `previous_access_type` into the scratch buffers.
                // The access history is updated as we go so subsequent
                // passes in the same frame see the right source state —
                // including when they reference the same TextureKey via
                // a different GraphResourceKey (correction B).
                self.image_accesses.clear();
                self.buffer_accesses.clear();
                // Materialise image/buffer accesses (Arc clones) under
                // a brief RM lock — the lock is dropped at the end of
                // this scope before pass.action_mut().execute() runs.
                {
                    let resource_manager = rm_arc.lock().unwrap();
                    let pass = passes_map.get(pass_key).unwrap();
                    for access in pass.accesses() {
                        match graph_resources.get(access.graph_resource_key).copied() {
                            Some(GraphResource::Texture {
                                texture_key, base_mip_level, mip_count,
                                base_array_layer, layer_count,
                            }) => {
                                let sub_range = ImageSubRange {
                                    base_mip_level, mip_count,
                                    base_array_layer, layer_count,
                                };
                                let prev = self.find_previous_image_access(
                                    texture_key, &sub_range,
                                );

                                if let Some(tex) = resource_manager.texture(texture_key) {
                                    self.image_accesses.push(graphics_device::ImageAccess {
                                        texture: tex.graphics_device_texture().clone(),
                                        access_type: access.access_type,
                                        previous_access_type: prev,
                                        base_mip_level,
                                        mip_count,
                                        base_array_layer,
                                        layer_count,
                                    });
                                }

                                self.record_image_access(
                                    texture_key, sub_range, access.access_type,
                                );
                            }
                            Some(GraphResource::Buffer { buffer_key, offset, size }) => {
                                let sub_range = BufferSubRange { offset, size };
                                let prev = self.find_previous_buffer_access(
                                    buffer_key, &sub_range,
                                );

                                if let Some(buf) = resource_manager.buffer(buffer_key) {
                                    self.buffer_accesses.push(graphics_device::BufferAccess {
                                        buffer: buf.graphics_device_buffer().clone(),
                                        access_type: access.access_type,
                                        previous_access_type: prev,
                                        offset,
                                        size,
                                    });
                                }

                                self.record_buffer_access(
                                    buffer_key, sub_range, access.access_type,
                                );
                            }
                            None => {}
                        }

                        // An MSAA color attachment with a `resolve_target`
                        // also writes to the resolve texture at end-of-pass.
                        // Track it in the image access history so the next
                        // reader of the resolved texture sees
                        // `ColorAttachmentWrite` as the source state.
                        // The resolve target points (by construction) at a
                        // GraphResource::Texture, so we resolve it the same
                        // way we resolved the primary attachment.
                        if let Some(TargetOps::Color { resolve_target: Some(rt), .. })
                            = access.target_ops
                        {
                            if let Some(GraphResource::Texture {
                                texture_key, base_mip_level, mip_count,
                                base_array_layer, layer_count,
                            }) = graph_resources.get(rt).copied() {
                                let resolve_sub_range = ImageSubRange {
                                    base_mip_level, mip_count,
                                    base_array_layer, layer_count,
                                };
                                self.record_image_access(
                                    texture_key,
                                    resolve_sub_range,
                                    AccessType::ColorAttachmentWrite,
                                );
                            }
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

    /// Kahn's algorithm with both Read-After-Write (RAW) and Write-After-Write
    /// (WAW) dependencies on shared `GraphResourceKey`s.
    ///
    /// Two-pass strategy with distinct semantics:
    ///   - Pass 1 fills `self.writers` with the *global last writer* of each
    ///     resource (irrespective of user order) — used for RAW deps so a
    ///     read on a resource that some other pass writes anywhere in the
    ///     batch is still correctly ordered (and mutual reads form the
    ///     classic cycle).
    ///   - Pass 2 streams `passes` in user order, maintaining a separate
    ///     `streaming_writers` map. Writes create WAW deps against the most
    ///     recent writer encountered so far; reads use the global table
    ///     from Pass 1.
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

        // Pass 1: global "last writer" per resource (for RAW deps).
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                if access.access_type.is_write() {
                    self.writers.insert(access.graph_resource_key, k);
                }
            }
        }

        // Pass 2: stream user order. Reads pull RAW deps from the global
        // table built above; writes pull WAW deps from a streaming local
        // table, then update it. The locality is what ensures multiple
        // writers of the same resource stay in user-supplied order without
        // injecting fake reverse-direction deps.
        let mut streaming_writers: FxHashMap<GraphResourceKey, RenderPassKey> =
            FxHashMap::default();
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                if access.access_type.is_write() {
                    if let Some(&prev) = streaming_writers.get(&access.graph_resource_key) {
                        if prev != k {
                            *self.in_degree.get_mut(&k).unwrap() += 1;
                            self.successors.get_mut(&prev).unwrap().push(k);
                        }
                    }
                    streaming_writers.insert(access.graph_resource_key, k);
                } else {
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

    // ============================================================
    // Per-frame access history (correction B)
    // ============================================================
    //
    // The history is indexed by the GPU TextureKey / BufferKey, NOT by
    // GraphResourceKey. Two distinct GraphResources targeting the same
    // texture share their access history — the second-pass barrier
    // therefore sees the right `previous_access_type`, even when its
    // sub-range overlaps the first pass's.

    /// Clear the per-frame access history without freeing any internal
    /// `Vec` buffer. Each inner Vec keeps its capacity for the next
    /// frame; zero allocation in steady state.
    fn clear_access_history(&mut self) {
        for entries in self.prev_image_accesses.values_mut() {
            entries.clear();
        }
        for entries in self.prev_buffer_accesses.values_mut() {
            entries.clear();
        }
    }

    /// Find the most recent `AccessType` that wrote/read a sub-range
    /// overlapping `query` on the given texture.
    ///
    /// Returns `None` if no overlapping entry exists, or if multiple
    /// overlapping entries disagree on `AccessType` (conservative
    /// fallback — the caller will use `oldLayout = UNDEFINED`, which is
    /// safe; see §2 of `.claude/notes/barrier_subrange_concurrency.md`).
    fn find_previous_image_access(
        &self,
        texture_key: TextureKey,
        query: &ImageSubRange,
    ) -> Option<AccessType> {
        let entries = self.prev_image_accesses.get(&texture_key)?;
        let mut found: Option<AccessType> = None;
        for (range, ty) in entries {
            if range.overlaps(query) {
                match found {
                    None => found = Some(*ty),
                    Some(prev) if prev == *ty => {}
                    Some(_) => {
                        crate::engine_warn!("galaxy3d::RenderGraph",
                            "RenderGraph '{}': multiple overlapping previous \
                             image accesses on same TextureKey with conflicting \
                             AccessTypes; falling back to None",
                            self.name);
                        return None;
                    }
                }
            }
        }
        found
    }

    /// Append an entry to the texture's access history. The first time
    /// a `TextureKey` is recorded, we allocate its inner Vec with
    /// `with_capacity(8)` — enough for typical mip-chain / cubemap
    /// usages without growth. Subsequent frames reuse the same Vec
    /// (cleared, capacity preserved).
    fn record_image_access(
        &mut self,
        texture_key: TextureKey,
        sub_range: ImageSubRange,
        access_type: AccessType,
    ) {
        self.prev_image_accesses
            .entry(texture_key)
            .or_insert_with(|| Vec::with_capacity(8))
            .push((sub_range, access_type));
    }

    /// Buffer counterpart of `find_previous_image_access`.
    fn find_previous_buffer_access(
        &self,
        buffer_key: BufferKey,
        query: &BufferSubRange,
    ) -> Option<AccessType> {
        let entries = self.prev_buffer_accesses.get(&buffer_key)?;
        let mut found: Option<AccessType> = None;
        for (range, ty) in entries {
            if range.overlaps(query) {
                match found {
                    None => found = Some(*ty),
                    Some(prev) if prev == *ty => {}
                    Some(_) => {
                        crate::engine_warn!("galaxy3d::RenderGraph",
                            "RenderGraph '{}': multiple overlapping previous \
                             buffer accesses on same BufferKey with conflicting \
                             AccessTypes; falling back to None",
                            self.name);
                        return None;
                    }
                }
            }
        }
        found
    }

    /// Buffer counterpart of `record_image_access`.
    fn record_buffer_access(
        &mut self,
        buffer_key: BufferKey,
        sub_range: BufferSubRange,
        access_type: AccessType,
    ) {
        self.prev_buffer_accesses
            .entry(buffer_key)
            .or_insert_with(|| Vec::with_capacity(8))
            .push((sub_range, access_type));
    }

    /// Test helper — snapshot the capacity of every inner Vec in the
    /// per-texture access-history map. Used to assert the
    /// zero-allocation-in-steady-state contract.
    #[cfg(test)]
    pub(crate) fn image_access_history_capacities(&self) -> Vec<usize> {
        let mut caps: Vec<usize> = self
            .prev_image_accesses
            .values()
            .map(|v| v.capacity())
            .collect();
        caps.sort_unstable();
        caps
    }
}

#[cfg(test)]
#[path = "render_graph_tests.rs"]
mod tests;
