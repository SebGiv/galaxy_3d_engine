/// Render graph — a DAG describing how a frame is rendered.
///
/// A render graph defines the sequence of render passes (nodes) and the
/// render targets (edges) that connect them. It is the high-level description
/// of a complete rendering pipeline.
///
/// Passes and targets are stored in contiguous `Vec`s for cache-friendly
/// iteration, with `FxHashMap<String, usize>` for name-based lookup.
///
/// Each pass declares how it accesses each resource via `AccessType`.
/// The backend handles synchronization and layout transitions internally
/// via dynamic barrier tracking (Vulkan 1.3 synchronization2).
///
/// Render graphs can only be created via `RenderGraphManager::create_render_graph()`.

use std::collections::VecDeque;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use crate::error::Result;
use crate::engine_bail;
use crate::graphics_device;
use crate::resource;
use super::access_type::{AccessType, ResourceAccess};
use super::pass_action::PassAction;
use super::render_target;
use super::render_pass::RenderPass;
use super::render_target::{RenderTarget, TargetOps};

pub struct RenderGraph {
    /// Render passes (nodes) stored by index
    passes: Vec<RenderPass>,
    /// Pass name to index mapping
    pass_names: FxHashMap<String, usize>,
    /// Render targets (edges) stored by index
    targets: Vec<RenderTarget>,
    /// Target name to index mapping
    target_names: FxHashMap<String, usize>,
    /// Sequential execution order (filled by compile)
    execution_order: Vec<usize>,
    /// Command lists for double/triple buffering (created by compile)
    command_lists: Vec<Box<dyn graphics_device::CommandList>>,
    /// Current frame index (points to the active command list)
    current_frame: usize,
}

impl RenderGraph {
    /// Internal only — created via RenderGraphManager::create_render_graph()
    pub(crate) fn new() -> Self {
        Self {
            passes: Vec::new(),
            pass_names: FxHashMap::default(),
            targets: Vec::new(),
            target_names: FxHashMap::default(),
            execution_order: Vec::new(),
            command_lists: Vec::new(),
            current_frame: 0,
        }
    }

    // ===== ADD =====

    /// Add a named render pass (node) to the graph
    ///
    /// Returns the index of the newly added pass.
    ///
    /// # Errors
    ///
    /// Returns an error if a pass with the same name already exists.
    pub fn add_pass(&mut self, name: &str) -> Result<usize> {
        if self.pass_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderPass '{}' already exists in this graph", name);
        }

        let id = self.passes.len();
        self.passes.push(RenderPass::new());
        self.pass_names.insert(name.to_string(), id);
        Ok(id)
    }

    /// Add a render target (edge) to the graph
    ///
    /// References a resource texture at a specific array layer and mip level.
    /// The GPU render target view is created immediately from the texture.
    /// Load/store/clear ops are auto-detected from the texture usage.
    ///
    /// Returns the index of the newly added target.
    ///
    /// # Errors
    ///
    /// Returns an error if a target with the same name already exists,
    /// or if the GPU render target view creation fails.
    pub fn add_target(
        &mut self,
        name: &str,
        texture: Arc<resource::Texture>,
        layer: u32,
        mip_level: u32,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<usize> {
        if self.target_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderTarget '{}' already exists in this graph", name);
        }

        let id = self.targets.len();
        self.targets.push(RenderTarget::new(texture, layer, mip_level, graphics_device)?);
        self.target_names.insert(name.to_string(), id);
        Ok(id)
    }

    // ===== ACCESS =====

    /// Declare how a pass accesses a target.
    ///
    /// Replaces the old `set_output()` / `set_input()` API.
    /// The compiler uses these declarations to:
    /// - Determine execution order (topological sort)
    /// - Generate pipeline barriers (layout transitions + memory sync)
    /// - Infer final_layout for render pass creation
    ///
    /// # Errors
    ///
    /// Returns an error if the pass or target name is not found.
    pub fn add_access(
        &mut self,
        pass_name: &str,
        target_name: &str,
        access_type: AccessType,
    ) -> Result<()> {
        let pass_id = self.pass_names.get(pass_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderPass '{}' not found", pass_name))?;

        let target_id = self.target_names.get(target_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderTarget '{}' not found", target_name))?;

        self.passes[pass_id].add_access(ResourceAccess {
            target_id,
            access_type,
            previous_access_type: None,
        });

        Ok(())
    }

    // ===== ACCESS BY INDEX (PRIMARY) =====

    /// Get a render pass by index
    pub fn pass(&self, id: usize) -> Option<&RenderPass> {
        self.passes.get(id)
    }

    /// Get a render target by index
    pub fn target(&self, id: usize) -> Option<&RenderTarget> {
        self.targets.get(id)
    }

    // ===== ACCESS BY NAME (SECONDARY) =====

    /// Get a render pass by name
    pub fn pass_by_name(&self, name: &str) -> Option<&RenderPass> {
        self.pass_names.get(name).and_then(|&id| self.passes.get(id))
    }

    /// Get a render target by name
    pub fn target_by_name(&self, name: &str) -> Option<&RenderTarget> {
        self.target_names.get(name).and_then(|&id| self.targets.get(id))
    }

    // ===== NAME → INDEX RESOLUTION =====

    /// Get a pass index by name
    pub fn pass_id(&self, name: &str) -> Option<usize> {
        self.pass_names.get(name).copied()
    }

    /// Get a target index by name
    pub fn target_id(&self, name: &str) -> Option<usize> {
        self.target_names.get(name).copied()
    }

    // ===== ACTION =====

    /// Set the action for a pass by index
    ///
    /// # Errors
    ///
    /// Returns an error if the pass index is out of bounds.
    pub fn set_action(&mut self, pass_id: usize, action: Box<dyn PassAction>) -> Result<()> {
        let pass_count = self.passes.len();
        let pass = self.passes.get_mut(pass_id)
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderPass index {} out of bounds (count: {})", pass_id, pass_count))?;

        pass.set_action(action);
        Ok(())
    }

    // ===== TARGET OPS (PER-TARGET CLEAR/LOAD/STORE) =====

    /// Set the clear color for a color target (RGBA)
    pub fn set_clear_color(&mut self, target_id: usize, color: [f32; 4]) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::Color { clear_color, .. } => {
                *clear_color = color;
                Ok(())
            }
            TargetOps::DepthStencil { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a depth/stencil target, not a color target", target_id);
            }
        }
    }

    /// Set the load operation for a color target
    pub fn set_color_load_op(&mut self, target_id: usize, op: graphics_device::LoadOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::Color { load_op, .. } => {
                *load_op = op;
                Ok(())
            }
            TargetOps::DepthStencil { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a depth/stencil target, not a color target", target_id);
            }
        }
    }

    /// Set the store operation for a color target
    pub fn set_color_store_op(&mut self, target_id: usize, op: graphics_device::StoreOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::Color { store_op, .. } => {
                *store_op = op;
                Ok(())
            }
            TargetOps::DepthStencil { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a depth/stencil target, not a color target", target_id);
            }
        }
    }

    /// Set the depth clear value for a depth/stencil target
    pub fn set_depth_clear(&mut self, target_id: usize, depth: f32) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { depth_clear, .. } => {
                *depth_clear = depth;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Set the stencil clear value for a depth/stencil target
    pub fn set_stencil_clear(&mut self, target_id: usize, stencil: u32) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { stencil_clear, .. } => {
                *stencil_clear = stencil;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Set the depth load operation for a depth/stencil target
    pub fn set_depth_load_op(&mut self, target_id: usize, op: graphics_device::LoadOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { depth_load_op, .. } => {
                *depth_load_op = op;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Set the depth store operation for a depth/stencil target
    pub fn set_depth_store_op(&mut self, target_id: usize, op: graphics_device::StoreOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { depth_store_op, .. } => {
                *depth_store_op = op;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Set the stencil load operation for a depth/stencil target
    pub fn set_stencil_load_op(&mut self, target_id: usize, op: graphics_device::LoadOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { stencil_load_op, .. } => {
                *stencil_load_op = op;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Set the stencil store operation for a depth/stencil target
    pub fn set_stencil_store_op(&mut self, target_id: usize, op: graphics_device::StoreOp) -> Result<()> {
        let target = self.target_mut(target_id)?;
        match target.ops_mut() {
            TargetOps::DepthStencil { stencil_store_op, .. } => {
                *stencil_store_op = op;
                Ok(())
            }
            TargetOps::Color { .. } => {
                engine_bail!("galaxy3d::RenderGraph",
                    "Target {} is a color target, not a depth/stencil target", target_id);
            }
        }
    }

    /// Get a mutable reference to a target by index (internal helper)
    fn target_mut(&mut self, target_id: usize) -> Result<&mut RenderTarget> {
        let target_count = self.targets.len();
        self.targets.get_mut(target_id)
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderTarget index {} out of bounds (count: {})", target_id, target_count))
    }

    // ===== MSAA RESOLVE =====

    /// Associate a resolve target with an MSAA color target.
    ///
    /// At the end of the render pass, the GPU automatically resolves the MSAA
    /// color attachment into the resolve texture (single-sample). This is
    /// required to sample the result in subsequent passes (e.g. tonemap).
    ///
    /// The resolve texture must have the same format and dimensions as the
    /// MSAA target, with `sample_count: S1`.
    pub fn set_resolve_target(
        &mut self,
        msaa_target_id: usize,
        resolve_texture: Arc<resource::Texture>,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<()> {
        let target = self.target_mut(msaa_target_id)?;

        // Validate: target must be a color attachment
        if !matches!(target.ops(), TargetOps::Color { .. }) {
            engine_bail!("galaxy3d::RenderGraph",
                "set_resolve_target: target {} is not a color target", msaa_target_id);
        }

        let resolve_rt = graphics_device.create_render_target_texture(
            resolve_texture.graphics_device_texture().as_ref(),
            target.layer(),
            target.mip_level(),
        )?;

        target.resolve = Some(render_target::ResolveTarget {
            texture: resolve_texture,
            graphics_device_render_target: resolve_rt,
        });

        Ok(())
    }

    // ===== COMPILE =====

    /// Resolve the graph: topological sort, GPU render passes,
    /// framebuffers, and command lists.
    ///
    /// This method:
    /// 1. Computes the execution order via topological sort (Kahn's algorithm)
    /// 2. For each pass with attachment outputs, creates a `graphics_device::RenderPass`
    ///    and a `graphics_device::Framebuffer` from its attachment targets
    /// 3. Creates `frames_in_flight` command lists for double/triple buffering
    ///
    /// Synchronization barriers are handled internally by the backend via
    /// `ImageAccess` declarations passed to `begin_render_pass()`.
    ///
    /// Call once after building the graph. Call `execute()` each frame.
    pub fn compile(
        &mut self,
        graphics_device: &dyn graphics_device::GraphicsDevice,
        frames_in_flight: usize,
    ) -> Result<()> {
        if frames_in_flight == 0 {
            engine_bail!("galaxy3d::RenderGraph",
                "frames_in_flight must be at least 1");
        }

        // Topological sort
        self.execution_order = self.topological_sort()?;

        // Resolve previous access types for barrier emission
        self.resolve_previous_accesses();

        // Create GPU render passes and framebuffers
        for pass_idx in 0..self.passes.len() {
            // Refresh per-pass cached vectors (clear_values, image_accesses).
            // These reuse their internal Vec capacity → zero allocation in steady
            // state once the pass has been compiled at least once.
            // Done for ALL passes, including those without attachments, so that
            // a pass that loses its attachments between two compiles ends up with
            // empty cached vectors instead of stale data.
            self.passes[pass_idx].refresh_clear_values(&self.targets);
            self.passes[pass_idx].refresh_image_accesses(&self.targets);

            let attachments: Vec<(usize, AccessType)> = self.passes[pass_idx]
                .accesses()
                .iter()
                .filter(|a| a.access_type.is_attachment())
                .map(|a| (a.target_id, a.access_type))
                .collect();

            if attachments.is_empty() {
                continue;
            }

            let mut color_attachment_descs = Vec::new();
            let mut color_targets = Vec::new();
            let mut depth_attachment_desc = None;
            let mut depth_target = None;
            let mut fb_width = 0u32;
            let mut fb_height = 0u32;
            let mut pass_sample_count: Option<graphics_device::SampleCount> = None;

            for &(target_id, _access_type) in &attachments {
                let target = &self.targets[target_id];
                let rt = target.graphics_device_render_target().clone();
                let texture_sample_count = target.texture().graphics_device_texture().info().sample_count;

                // Validate all attachments have the same sample count (Vulkan requirement)
                match pass_sample_count {
                    None => pass_sample_count = Some(texture_sample_count),
                    Some(existing) if existing != texture_sample_count => {
                        crate::engine_bail!("galaxy3d::render_graph",
                            "compile: mismatched sample counts in pass — \
                             expected {:?}, found {:?}",
                            existing, texture_sample_count);
                    }
                    _ => {}
                }

                if fb_width == 0 {
                    fb_width = rt.width();
                    fb_height = rt.height();
                }

                match target.ops() {
                    TargetOps::Color { load_op, store_op, .. } => {
                        color_attachment_descs.push(graphics_device::AttachmentDesc {
                            format: rt.format(),
                            samples: texture_sample_count,
                            load_op: *load_op,
                            store_op: *store_op,
                            stencil_load_op: graphics_device::LoadOp::DontCare,
                            stencil_store_op: graphics_device::StoreOp::DontCare,
                        });
                        color_targets.push(rt);
                    }
                    TargetOps::DepthStencil {
                        depth_load_op, depth_store_op,
                        stencil_load_op, stencil_store_op, ..
                    } => {
                        depth_attachment_desc = Some(graphics_device::AttachmentDesc {
                            format: rt.format(),
                            samples: texture_sample_count,
                            load_op: *depth_load_op,
                            store_op: *depth_store_op,
                            stencil_load_op: *stencil_load_op,
                            stencil_store_op: *stencil_store_op,
                        });
                        depth_target = Some(rt);
                    }
                }
            }

            // Collect resolve attachments for MSAA color targets
            let mut resolve_attachment_descs = Vec::new();
            let mut resolve_targets = Vec::new();
            for &(target_id, _) in &attachments {
                let target = &self.targets[target_id];
                if let Some(resolve) = &target.resolve {
                    let resolve_rt = &resolve.graphics_device_render_target;
                    resolve_attachment_descs.push(graphics_device::AttachmentDesc {
                        format: resolve_rt.format(),
                        samples: graphics_device::SampleCount::S1,
                        load_op: graphics_device::LoadOp::DontCare,
                        store_op: graphics_device::StoreOp::Store,
                        stencil_load_op: graphics_device::LoadOp::DontCare,
                        stencil_store_op: graphics_device::StoreOp::DontCare,
                    });
                    resolve_targets.push(resolve_rt.clone());
                }
            }

            // Create GPU render pass
            let render_pass_desc = graphics_device::RenderPassDesc {
                color_attachments: color_attachment_descs,
                depth_stencil_attachment: depth_attachment_desc,
                color_resolve_attachments: resolve_attachment_descs,
            };
            let render_pass = graphics_device.create_render_pass(&render_pass_desc)?;

            // Create GPU framebuffer
            let fb_desc = graphics_device::FramebufferDesc {
                render_pass: &render_pass,
                color_attachments: color_targets,
                depth_stencil_attachment: depth_target,
                color_resolve_attachments: resolve_targets,
                width: fb_width,
                height: fb_height,
            };
            let framebuffer = graphics_device.create_framebuffer(&fb_desc)?;

            self.passes[pass_idx].set_graphics_device_render_pass(render_pass);
            self.passes[pass_idx].set_graphics_device_framebuffer(framebuffer);

            // Derive PassInfo from resolved attachment targets.
            // Zero allocation if formats haven't changed since last compile.
            let color_target_refs: Vec<&RenderTarget> = attachments.iter()
                .filter(|(tid, _)| matches!(self.targets[*tid].ops(), TargetOps::Color { .. }))
                .map(|(tid, _)| &self.targets[*tid])
                .collect();
            let depth_target_ref = attachments.iter()
                .find(|(tid, _)| matches!(self.targets[*tid].ops(), TargetOps::DepthStencil { .. }))
                .map(|(tid, _)| &self.targets[*tid]);
            let sample_count = pass_sample_count.unwrap_or(graphics_device::SampleCount::S1);
            self.passes[pass_idx].update_pass_info_from_targets(
                &color_target_refs,
                depth_target_ref,
                sample_count,
            );
        }

        // Create command lists for double/triple buffering
        self.command_lists.clear();
        for _ in 0..frames_in_flight {
            self.command_lists.push(graphics_device.create_command_list()?);
        }
        self.current_frame = frames_in_flight - 1;

        Ok(())
    }

    /// Topological sort using Kahn's algorithm
    ///
    /// Build dependencies from access declarations: if pass A writes a target
    /// that pass B reads (non-attachment), then A must execute before B.
    fn topological_sort(&self) -> Result<Vec<usize>> {
        let pass_count = self.passes.len();
        let mut in_degree = vec![0u32; pass_count];
        let mut successors = vec![Vec::new(); pass_count];

        // Build a map: target_id → writer pass index
        let mut target_writers: FxHashMap<usize, usize> = FxHashMap::default();
        for (pass_idx, pass) in self.passes.iter().enumerate() {
            for access in pass.accesses() {
                if access.access_type.is_write() {
                    target_writers.insert(access.target_id, pass_idx);
                }
            }
        }

        // For each pass that reads a target, add dependency on the writer
        for (pass_idx, pass) in self.passes.iter().enumerate() {
            for access in pass.accesses() {
                if !access.access_type.is_write() {
                    if let Some(&writer) = target_writers.get(&access.target_id) {
                        if writer != pass_idx {
                            in_degree[pass_idx] += 1;
                            successors[writer].push(pass_idx);
                        }
                    }
                }
            }
        }

        // Start with all passes that have no dependencies
        let mut queue: VecDeque<usize> = (0..pass_count)
            .filter(|&i| in_degree[i] == 0)
            .collect();

        let mut order = Vec::with_capacity(pass_count);

        while let Some(pass_idx) = queue.pop_front() {
            order.push(pass_idx);
            for &succ in &successors[pass_idx] {
                in_degree[succ] -= 1;
                if in_degree[succ] == 0 {
                    queue.push_back(succ);
                }
            }
        }

        if order.len() != pass_count {
            engine_bail!("galaxy3d::RenderGraph",
                "Cycle detected: {} passes could not be ordered (total: {})",
                pass_count - order.len(), pass_count);
        }

        Ok(order)
    }

    // ===== EXECUTE =====

    /// Execute the compiled graph: begin, passes, post-passes callback, end.
    ///
    /// Records a complete frame into the current command list:
    /// 1. Advances to the next command list (double buffering)
    /// 2. Calls `cmd.begin()`
    /// 3. For each pass (topological order):
    ///    begin_render_pass (with accesses for backend barrier tracking) → action → end_render_pass
    /// 4. Calls `post_passes(cmd)` for extra commands (e.g. swapchain blit)
    /// 5. Calls `cmd.end()`
    ///
    /// After execute(), call `command_list()` to get the recorded command list
    /// for submission.
    pub fn execute<F>(&mut self, post_passes: F) -> Result<()>
    where
        F: FnOnce(&mut dyn graphics_device::CommandList) -> Result<()>,
    {
        if self.command_lists.is_empty() {
            engine_bail!("galaxy3d::RenderGraph",
                "No command lists — call compile() before execute()");
        }
        if self.execution_order.is_empty() && !self.passes.is_empty() {
            engine_bail!("galaxy3d::RenderGraph",
                "Graph not compiled — call compile() before execute()");
        }

        // Advance to next command list
        let frame = (self.current_frame + 1) % self.command_lists.len();
        self.current_frame = frame;

        self.command_lists[frame].begin()?;

        // Execute all passes in topological order.
        // If any error occurs, we must still call end() to leave the command list
        // in a clean state (not recording), otherwise the next frame's begin() will fail.
        let result = (|| -> Result<()> {
            // Iterate by index to avoid holding a borrow on `self.execution_order`
            // during the mut accesses on `self.command_lists` and `self.passes` below.
            // pass_idx is a usize (Copy), so reading it releases the borrow immediately.
            let pass_count = self.execution_order.len();
            for i in 0..pass_count {
                let pass_idx = self.execution_order[i];
                // Skip passes with no attachments (pure compute passes would go here later)
                let rp = match self.passes[pass_idx].graphics_device_render_pass() {
                    Some(rp) => rp.clone(),
                    None => continue,
                };
                let fb = match self.passes[pass_idx].graphics_device_framebuffer() {
                    Some(fb) => fb.clone(),
                    None => continue,
                };

                // Read pre-computed clear values and image accesses (refreshed at compile()).
                // Zero allocation: these are slices into the RenderPass-owned Vecs.
                let clear_values = self.passes[pass_idx].clear_values();
                let accesses = self.passes[pass_idx].image_accesses();

                self.command_lists[frame].begin_render_pass(&rp, &fb, clear_values, accesses)?;

                let (action, pass_info) = self.passes[pass_idx].action_and_pass_info_mut();
                if let Some(action) = action {
                    let pass_info = pass_info.expect(
                        "PassInfo should be available after compile for passes with attachments"
                    );
                    action.execute(&mut *self.command_lists[frame], pass_info)?;
                }

                self.command_lists[frame].end_render_pass()?;
            }

            // Post-passes commands (e.g. swapchain blit)
            post_passes(&mut *self.command_lists[frame])?;

            Ok(())
        })();

        // Always end the command list, even if an error occurred above
        self.command_lists[frame].end()?;
        result?;
        Ok(())
    }

    /// Get the current command list (the one recorded by the last execute() call)
    pub fn command_list(&self) -> Result<&dyn graphics_device::CommandList> {
        if self.command_lists.is_empty() {
            engine_bail!("galaxy3d::RenderGraph",
                "No command lists — call compile() first");
        }
        Ok(&*self.command_lists[self.current_frame])
    }

    /// Resolve previous access types for each resource access.
    ///
    /// Walks the execution order and for each access, records the last
    /// AccessType that touched the same target. This is precalculated
    /// once during `compile()` so `execute()` has zero overhead.
    fn resolve_previous_accesses(&mut self) {
        let mut last_access: Vec<Option<AccessType>> = vec![None; self.targets.len()];
        for &pass_idx in &self.execution_order {
            for access in self.passes[pass_idx].accesses_mut() {
                access.previous_access_type = last_access[access.target_id];
                last_access[access.target_id] = Some(access.access_type);
            }
        }
    }

    // ===== QUERY =====

    /// Get the execution order (available after compile)
    pub fn execution_order(&self) -> &[usize] {
        &self.execution_order
    }

    // ===== COUNTS =====

    /// Get the number of passes in the graph
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get the number of targets in the graph
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

}
