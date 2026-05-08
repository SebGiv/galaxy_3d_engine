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

    // Per-GPU-key writer history for the topological sort. Indexed by
    // the underlying TextureKey / BufferKey; each Vec lists every
    // (sub_range, writer) pair so that overlap-based dependency
    // detection (correction C) handles distinct GraphResources that
    // alias the same GPU resource. Persisted across calls; inner Vecs
    // cleared per topological_sort run (capacity preserved).
    //
    // Two distinct buckets per resource type to mirror the two-pass
    // strategy:
    //   - `image_writers` / `buffer_writers`     : Pass 1, global
    //     last-writer table, source of RAW dep edges.
    //   - `streaming_image_writers` / `streaming_buffer_writers`
    //     : Pass 2, streaming view in user-declared order, source of
    //     WAW dep edges. Walking and updating happens in lockstep.
    image_writers: FxHashMap<TextureKey, Vec<(ImageSubRange, RenderPassKey)>>,
    buffer_writers: FxHashMap<BufferKey, Vec<(BufferSubRange, RenderPassKey)>>,
    streaming_image_writers: FxHashMap<TextureKey, Vec<(ImageSubRange, RenderPassKey)>>,
    streaming_buffer_writers: FxHashMap<BufferKey, Vec<(BufferSubRange, RenderPassKey)>>,

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
            // Pre-size the topo-sort scratch maps. 32 passes covers our
            // typical graphs without rehash; subsequent calls reuse this
            // capacity. Inner Vecs of `successors` are reused across
            // calls (cleared but not dropped — see `clear_topo_state`).
            in_degree: FxHashMap::with_capacity_and_hasher(
                32, Default::default(),
            ),
            successors: FxHashMap::with_capacity_and_hasher(
                32, Default::default(),
            ),
            // Pre-size the writer-history maps. 64 textures / 32 buffers
            // covers our typical graphs without rehash; subsequent
            // topological_sort calls reuse this capacity.
            image_writers: FxHashMap::with_capacity_and_hasher(
                64, Default::default(),
            ),
            buffer_writers: FxHashMap::with_capacity_and_hasher(
                32, Default::default(),
            ),
            streaming_image_writers: FxHashMap::with_capacity_and_hasher(
                64, Default::default(),
            ),
            streaming_buffer_writers: FxHashMap::with_capacity_and_hasher(
                32, Default::default(),
            ),
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
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
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
        self.topological_sort(passes_map, graph_resources, passes)?;

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
                // this scope before the PassAction::execute() runs.
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
                // Split-borrow the three fields needed by execute() so we
                // can hand `pass_info` to the action without cloning it.
                // The borrow checker accepts the projection because the
                // method signature distinguishes the three field origins.
                let (pass_name, pass_info_opt, action) = pass.execute_components_mut();
                let pass_info_ref = pass_info_opt.ok_or_else(|| {
                    crate::engine_err!("galaxy3d::RenderGraph",
                        "Pass '{}' has attachments but no PassInfo", pass_name)
                })?;
                action.execute(
                    &mut *self.command_lists[frame],
                    pass_info_ref,
                    graphics_device,
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

    /// Kahn's algorithm with overlap-based Read-After-Write (RAW) and
    /// Write-After-Write (WAW) dependencies on shared GPU resources
    /// (correction C).
    ///
    /// Each access is resolved to its underlying `(TextureKey,
    /// ImageSubRange)` or `(BufferKey, BufferSubRange)`. Two passes
    /// touching the same GPU resource on overlapping sub-ranges therefore
    /// get a dependency edge even when they reference the resource through
    /// distinct `GraphResource`s.
    ///
    /// Two-pass strategy with distinct semantics:
    ///   - **Pass 1** fills `image_writers` / `buffer_writers` with the
    ///     full list of writers per GPU key, irrespective of user order.
    ///     A reader pulls RAW deps from this table — every overlapping
    ///     writer in the frame becomes a predecessor.
    ///   - **Pass 2** streams `passes` in user order, walking and
    ///     updating `streaming_image_writers` / `streaming_buffer_writers`
    ///     in lockstep. A writer pulls WAW deps from this table — only
    ///     the writers already seen in user order are predecessors.
    ///     Multiple writes on the same sub-range therefore stay in
    ///     user-supplied order without injecting reverse-direction edges.
    ///
    /// Result lands in `self.sorted_passes`. All four writer maps are
    /// kept across calls; only their inner Vecs are cleared at entry
    /// (capacity preserved).
    fn topological_sort(
        &mut self,
        passes_map: &SlotMap<RenderPassKey, RenderPass>,
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        passes: &[RenderPassKey],
    ) -> Result<()> {
        self.clear_topo_state();

        for &k in passes {
            self.in_degree.insert(k, 0);
            // `entry().or_default()` is a no-op when the entry already
            // exists (steady state: every pass key has been seen before),
            // so the inner Vec — emptied by `clear_topo_state` — is
            // reused with its capacity preserved. Only the very first
            // appearance of a pass key allocates here.
            self.successors.entry(k).or_default();
        }

        // ===== Pass 1: collect ALL writers per GPU key. =====
        // Unlike the legacy "single last writer" table, we accumulate
        // every write — distinct sub-ranges on the same texture all
        // coexist in the same Vec, and the lookup later filters by
        // overlap.
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                if !access.access_type.is_write() { continue; }
                match graph_resources.get(access.graph_resource_key).copied() {
                    Some(GraphResource::Texture {
                        texture_key, base_mip_level, mip_count,
                        base_array_layer, layer_count,
                    }) => {
                        let r = ImageSubRange {
                            base_mip_level, mip_count,
                            base_array_layer, layer_count,
                        };
                        self.image_writers
                            .entry(texture_key)
                            .or_insert_with(|| Vec::with_capacity(8))
                            .push((r, k));
                    }
                    Some(GraphResource::Buffer { buffer_key, offset, size }) => {
                        let r = BufferSubRange { offset, size };
                        self.buffer_writers
                            .entry(buffer_key)
                            .or_insert_with(|| Vec::with_capacity(8))
                            .push((r, k));
                    }
                    None => {}
                }
            }
        }

        // ===== Pass 2: stream passes in user order, build edges. =====
        for &k in passes {
            let pass = passes_map.get(k).unwrap();
            for access in pass.accesses() {
                match graph_resources.get(access.graph_resource_key).copied() {
                    Some(GraphResource::Texture {
                        texture_key, base_mip_level, mip_count,
                        base_array_layer, layer_count,
                    }) => {
                        let r = ImageSubRange {
                            base_mip_level, mip_count,
                            base_array_layer, layer_count,
                        };
                        if access.access_type.is_write() {
                            // WAW: every prior streaming writer that overlaps.
                            if let Some(entries) = self.streaming_image_writers.get(&texture_key) {
                                for (prev_r, prev_k) in entries {
                                    if *prev_k != k && prev_r.overlaps(&r) {
                                        *self.in_degree.get_mut(&k).unwrap() += 1;
                                        self.successors.get_mut(prev_k).unwrap().push(k);
                                    }
                                }
                            }
                            // Push self into streaming writers.
                            self.streaming_image_writers
                                .entry(texture_key)
                                .or_insert_with(|| Vec::with_capacity(8))
                                .push((r, k));
                        } else {
                            // RAW: every global writer that overlaps.
                            if let Some(entries) = self.image_writers.get(&texture_key) {
                                for (writer_r, writer_k) in entries {
                                    if *writer_k != k && writer_r.overlaps(&r) {
                                        *self.in_degree.get_mut(&k).unwrap() += 1;
                                        self.successors.get_mut(writer_k).unwrap().push(k);
                                    }
                                }
                            }
                        }
                    }
                    Some(GraphResource::Buffer { buffer_key, offset, size }) => {
                        let r = BufferSubRange { offset, size };
                        if access.access_type.is_write() {
                            if let Some(entries) = self.streaming_buffer_writers.get(&buffer_key) {
                                for (prev_r, prev_k) in entries {
                                    if *prev_k != k && prev_r.overlaps(&r) {
                                        *self.in_degree.get_mut(&k).unwrap() += 1;
                                        self.successors.get_mut(prev_k).unwrap().push(k);
                                    }
                                }
                            }
                            self.streaming_buffer_writers
                                .entry(buffer_key)
                                .or_insert_with(|| Vec::with_capacity(8))
                                .push((r, k));
                        } else {
                            if let Some(entries) = self.buffer_writers.get(&buffer_key) {
                                for (writer_r, writer_k) in entries {
                                    if *writer_k != k && writer_r.overlaps(&r) {
                                        *self.in_degree.get_mut(&k).unwrap() += 1;
                                        self.successors.get_mut(writer_k).unwrap().push(k);
                                    }
                                }
                            }
                        }
                    }
                    None => {}
                }
            }
        }

        // ===== Kahn's drain (unchanged). =====
        for &k in passes {
            if self.in_degree[&k] == 0 {
                self.topo_queue.push_back(k);
            }
        }

        while let Some(k) = self.topo_queue.pop_front() {
            self.sorted_passes.push(k);
            // Disjoint-fields borrow: `self.successors` (immut) and
            // `self.in_degree` / `self.topo_queue` (mut) are distinct fields,
            // the borrow checker accepts the split — no Vec clone needed.
            for &s in &self.successors[&k] {
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

    /// Reset the per-call topological-sort state without freeing any
    /// internal Vec buffer. Each writer-history Vec keeps its capacity
    /// for the next call; zero allocation in steady state.
    fn clear_topo_state(&mut self) {
        self.in_degree.clear();
        // Clear inner Vec contents but keep the outer HashMap entries
        // alive — each Vec retains its capacity for the next call. Same
        // pattern as the writer-history maps below.
        for entries in self.successors.values_mut() { entries.clear(); }
        self.topo_queue.clear();
        self.sorted_passes.clear();
        for entries in self.image_writers.values_mut() { entries.clear(); }
        for entries in self.buffer_writers.values_mut() { entries.clear(); }
        for entries in self.streaming_image_writers.values_mut() { entries.clear(); }
        for entries in self.streaming_buffer_writers.values_mut() { entries.clear(); }
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

    /// Test helper — return every direct predecessor of `target` in the
    /// last topological sort, by inverting the `successors` adjacency.
    /// Sorted by `RenderPassKey` for stable comparisons. Useful for
    /// asserting that correction C did or did not insert an edge.
    #[cfg(test)]
    pub(crate) fn predecessors_of(&self, target: RenderPassKey) -> Vec<RenderPassKey> {
        let mut preds: Vec<RenderPassKey> = self
            .successors
            .iter()
            .filter_map(|(&pred, succs)| {
                if succs.contains(&target) { Some(pred) } else { None }
            })
            .collect();
        preds.sort_unstable_by_key(|k| format!("{:?}", k));
        preds
    }

    /// Test helper — return the position of `pass` in the last
    /// topological sort result. Used to assert relative ordering between
    /// two passes.
    #[cfg(test)]
    pub(crate) fn sorted_position_of(&self, pass: RenderPassKey) -> Option<usize> {
        self.sorted_passes.iter().position(|&k| k == pass)
    }

    /// Test helper — snapshot the capacities of every inner Vec across
    /// all four topological-sort writer history maps. Like
    /// `image_access_history_capacities` but for the topo-sort scratch.
    #[cfg(test)]
    pub(crate) fn topo_writer_history_capacities(&self) -> Vec<usize> {
        let mut caps: Vec<usize> = Vec::new();
        caps.extend(self.image_writers.values().map(|v| v.capacity()));
        caps.extend(self.buffer_writers.values().map(|v| v.capacity()));
        caps.extend(self.streaming_image_writers.values().map(|v| v.capacity()));
        caps.extend(self.streaming_buffer_writers.values().map(|v| v.capacity()));
        caps.sort_unstable();
        caps
    }
}

#[cfg(test)]
#[path = "render_graph_tests.rs"]
mod tests;
