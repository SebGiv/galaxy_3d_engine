/// Central render graph manager for the engine.
///
/// Owns four independent collections — render graphs, render passes,
/// graph resources, and framebuffers — each with the (key, name) pattern
/// used everywhere else (`SlotMap` + `FxHashMap<String, Key>`), except
/// `Framebuffer`s which have no name and are deduplicated through a
/// content-addressed lookup map.
///
/// Render passes are eager: their cache (`framebuffer_key`, `pass_info`,
/// `gd_render_pass`, `clear_values`) is rebuilt synchronously by every
/// setter, so the pass is always ready to execute and validation errors
/// surface at call time rather than at the next frame.

use std::sync::Arc;
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use crate::error::Result;
use crate::engine_bail;
use crate::engine::Engine;
use crate::graphics_device;
use crate::resource::resource_manager::{PassInfo, ResourceManager};
use super::access_type::{AccessType, ResourceAccess, TargetOps};
use super::frame_buffer::{ColorAttachmentSlot, Framebuffer, FramebufferKey, FramebufferLookupKey};
use super::graph_resource::{GraphResource, GraphResourceKey};
use super::pass_action::PassAction;
use super::render_graph::{RenderGraph, RenderGraphKey};
use super::render_pass::{RenderPass, RenderPassKey};

pub struct RenderGraphManager {
    graphs: SlotMap<RenderGraphKey, RenderGraph>,
    graph_names: FxHashMap<String, RenderGraphKey>,

    passes: SlotMap<RenderPassKey, RenderPass>,
    pass_names: FxHashMap<String, RenderPassKey>,

    graph_resources: SlotMap<GraphResourceKey, GraphResource>,
    graph_resource_names: FxHashMap<String, GraphResourceKey>,

    framebuffers: SlotMap<FramebufferKey, Framebuffer>,
    framebuffer_lookup: FxHashMap<FramebufferLookupKey, FramebufferKey>,
}

/// Result bundle of `build_pass_cache` — everything a `RenderPass` needs
/// to be ready for `execute()`.
struct PassCache {
    framebuffer_key: Option<FramebufferKey>,
    pass_info: Option<PassInfo>,
    gd_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
    clear_values: Vec<graphics_device::ClearValue>,
}

impl RenderGraphManager {
    pub(crate) fn new() -> Self {
        Self {
            graphs: SlotMap::with_key(),
            graph_names: FxHashMap::default(),
            passes: SlotMap::with_key(),
            pass_names: FxHashMap::default(),
            graph_resources: SlotMap::with_key(),
            graph_resource_names: FxHashMap::default(),
            framebuffers: SlotMap::with_key(),
            framebuffer_lookup: FxHashMap::default(),
        }
    }

    // ===== RENDER GRAPH =====

    pub fn create_render_graph(
        &mut self,
        name: &str,
        frames_in_flight: usize,
    ) -> Result<RenderGraphKey> {
        if self.graph_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraphManager",
                "RenderGraph '{}' already exists", name);
        }
        let gd_arc = Engine::graphics_device("main")?;
        let gd = gd_arc.lock().unwrap();
        let graph = RenderGraph::new(name.to_string(), &*gd, frames_in_flight)?;
        drop(gd);
        let key = self.graphs.insert(graph);
        self.graph_names.insert(name.to_string(), key);
        Ok(key)
    }

    pub fn render_graph(&self, key: RenderGraphKey) -> Option<&RenderGraph> {
        self.graphs.get(key)
    }

    pub fn render_graph_mut(&mut self, key: RenderGraphKey) -> Option<&mut RenderGraph> {
        self.graphs.get_mut(key)
    }

    pub fn render_graph_by_name(&self, name: &str) -> Option<&RenderGraph> {
        self.graph_names.get(name).and_then(|k| self.graphs.get(*k))
    }

    pub fn render_graph_id(&self, name: &str) -> Option<RenderGraphKey> {
        self.graph_names.get(name).copied()
    }

    pub fn render_graph_count(&self) -> usize {
        self.graphs.len()
    }

    // ===== RENDER PASS =====

    /// Create a render pass and immediately compute its full cache
    /// (framebuffer, render-pass descriptor, pass info, clear values).
    ///
    /// Validation errors (mismatched sample counts, > 1 depth/stencil
    /// write, mixed resolves, GraphResource is a buffer, …) surface
    /// here — not at the next frame.
    pub fn create_render_pass(
        &mut self,
        name: &str,
        accesses: Vec<ResourceAccess>,
        action: Box<dyn PassAction>,
    ) -> Result<RenderPassKey> {
        if self.pass_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraphManager",
                "RenderPass '{}' already exists", name);
        }
        let rm_arc = Engine::resource_manager()?;
        let gd_arc = Engine::graphics_device("main")?;
        let rm = rm_arc.lock().unwrap();
        let gd = gd_arc.lock().unwrap();
        let cache = Self::build_pass_cache(
            &mut self.framebuffers,
            &mut self.framebuffer_lookup,
            &self.graph_resources,
            &accesses,
            name,
            None,
            &*rm,
            &*gd,
        )?;
        drop(gd);
        drop(rm);
        let pass = RenderPass::new(
            name.to_string(),
            accesses,
            action,
            cache.framebuffer_key,
            cache.pass_info,
            cache.gd_render_pass,
            cache.clear_values,
        );
        let key = self.passes.insert(pass);
        self.pass_names.insert(name.to_string(), key);
        Ok(key)
    }

    pub fn render_pass(&self, key: RenderPassKey) -> Option<&RenderPass> {
        self.passes.get(key)
    }

    pub fn render_pass_by_name(&self, name: &str) -> Option<&RenderPass> {
        self.pass_names.get(name).and_then(|k| self.passes.get(*k))
    }

    pub fn render_pass_id(&self, name: &str) -> Option<RenderPassKey> {
        self.pass_names.get(name).copied()
    }

    pub fn render_pass_count(&self) -> usize {
        self.passes.len()
    }

    // ===== RENDER PASS — EAGER SETTERS =====

    /// Replace the `GraphResource` of access #`access_idx`. Rebuilds the
    /// pass cache atomically.
    pub fn set_pass_access_resource(
        &mut self,
        pass_key: RenderPassKey,
        access_idx: usize,
        gr_key: GraphResourceKey,
    ) -> Result<()> {
        let (name, accesses, prev_info) = {
            let pass = self.passes.get_mut(pass_key).ok_or_else(|| {
                crate::engine_err!("galaxy3d::RenderGraphManager",
                    "set_pass_access_resource: RenderPassKey not found")
            })?;
            let access_count = pass.accesses().len();
            let access = pass.accesses_mut().get_mut(access_idx).ok_or_else(|| {
                crate::engine_err!("galaxy3d::RenderGraphManager",
                    "set_pass_access_resource: access {} out of bounds (count: {})",
                    access_idx, access_count)
            })?;
            access.graph_resource_key = gr_key;
            (
                pass.name().to_string(),
                pass.accesses().to_vec(),
                pass.pass_info().cloned(),
            )
        };

        let rm_arc = Engine::resource_manager()?;
        let gd_arc = Engine::graphics_device("main")?;
        let rm = rm_arc.lock().unwrap();
        let gd = gd_arc.lock().unwrap();
        let cache = Self::build_pass_cache(
            &mut self.framebuffers,
            &mut self.framebuffer_lookup,
            &self.graph_resources,
            &accesses,
            &name,
            prev_info.as_ref(),
            &*rm,
            &*gd,
        )?;
        drop(gd);
        drop(rm);
        let pass = self.passes.get_mut(pass_key).unwrap();
        pass.set_cache(cache.framebuffer_key, cache.pass_info, cache.gd_render_pass, cache.clear_values);
        Ok(())
    }

    /// Replace the `TargetOps` of access #`access_idx`. Rebuilds the pass
    /// cache atomically (the framebuffer cache hits when the resolve
    /// target is unchanged).
    pub fn set_pass_access_target_ops(
        &mut self,
        pass_key: RenderPassKey,
        access_idx: usize,
        ops: TargetOps,
    ) -> Result<()> {
        let (name, accesses, prev_info) = {
            let pass = self.passes.get_mut(pass_key).ok_or_else(|| {
                crate::engine_err!("galaxy3d::RenderGraphManager",
                    "set_pass_access_target_ops: RenderPassKey not found")
            })?;
            let access_count = pass.accesses().len();
            let access = pass.accesses_mut().get_mut(access_idx).ok_or_else(|| {
                crate::engine_err!("galaxy3d::RenderGraphManager",
                    "set_pass_access_target_ops: access {} out of bounds (count: {})",
                    access_idx, access_count)
            })?;
            access.target_ops = Some(ops);
            (
                pass.name().to_string(),
                pass.accesses().to_vec(),
                pass.pass_info().cloned(),
            )
        };

        let rm_arc = Engine::resource_manager()?;
        let gd_arc = Engine::graphics_device("main")?;
        let rm = rm_arc.lock().unwrap();
        let gd = gd_arc.lock().unwrap();
        let cache = Self::build_pass_cache(
            &mut self.framebuffers,
            &mut self.framebuffer_lookup,
            &self.graph_resources,
            &accesses,
            &name,
            prev_info.as_ref(),
            &*rm,
            &*gd,
        )?;
        drop(gd);
        drop(rm);
        let pass = self.passes.get_mut(pass_key).unwrap();
        pass.set_cache(cache.framebuffer_key, cache.pass_info, cache.gd_render_pass, cache.clear_values);
        Ok(())
    }

    /// Replace every `ResourceAccess` of the pass at once. Useful when
    /// the access set changes structurally (e.g. toggling MSAA) — using
    /// the per-access setters one by one would walk through invalid
    /// intermediate states (mixed resolves, etc.) and create transient
    /// framebuffers we don't actually want.
    pub fn replace_pass_accesses(
        &mut self,
        pass_key: RenderPassKey,
        new_accesses: Vec<ResourceAccess>,
    ) -> Result<()> {
        let (name, prev_info) = {
            let pass = self.passes.get(pass_key).ok_or_else(|| {
                crate::engine_err!("galaxy3d::RenderGraphManager",
                    "replace_pass_accesses: RenderPassKey not found")
            })?;
            (pass.name().to_string(), pass.pass_info().cloned())
        };

        let rm_arc = Engine::resource_manager()?;
        let gd_arc = Engine::graphics_device("main")?;
        let rm = rm_arc.lock().unwrap();
        let gd = gd_arc.lock().unwrap();
        let cache = Self::build_pass_cache(
            &mut self.framebuffers,
            &mut self.framebuffer_lookup,
            &self.graph_resources,
            &new_accesses,
            &name,
            prev_info.as_ref(),
            &*rm,
            &*gd,
        )?;
        drop(gd);
        drop(rm);
        let pass = self.passes.get_mut(pass_key).unwrap();
        pass.replace_accesses(new_accesses);
        pass.set_cache(cache.framebuffer_key, cache.pass_info, cache.gd_render_pass, cache.clear_values);
        Ok(())
    }

    // ===== GRAPH RESOURCE =====

    pub fn create_graph_resource(
        &mut self,
        name: &str,
        resource: GraphResource,
    ) -> Result<GraphResourceKey> {
        if self.graph_resource_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraphManager",
                "GraphResource '{}' already exists", name);
        }
        let key = self.graph_resources.insert(resource);
        self.graph_resource_names.insert(name.to_string(), key);
        Ok(key)
    }

    pub fn graph_resource(&self, key: GraphResourceKey) -> Option<GraphResource> {
        self.graph_resources.get(key).copied()
    }

    pub fn graph_resource_by_name(&self, name: &str) -> Option<GraphResource> {
        self.graph_resource_names
            .get(name)
            .and_then(|k| self.graph_resources.get(*k))
            .copied()
    }

    pub fn graph_resource_id(&self, name: &str) -> Option<GraphResourceKey> {
        self.graph_resource_names.get(name).copied()
    }

    pub fn graph_resource_count(&self) -> usize {
        self.graph_resources.len()
    }

    pub fn remove_graph_resource(&mut self, key: GraphResourceKey) -> bool {
        let removed = self.graph_resources.remove(key).is_some();
        if removed {
            self.graph_resource_names.retain(|_, v| *v != key);
        }
        removed
    }

    pub fn remove_graph_resource_by_name(&mut self, name: &str) -> bool {
        match self.graph_resource_names.remove(name) {
            Some(key) => {
                self.graph_resources.remove(key);
                true
            }
            None => false,
        }
    }

    // ===== FRAMEBUFFER =====

    /// Get or create a `Framebuffer` matching the given attachment set.
    ///
    /// Idempotent: identical inputs return the same `FramebufferKey`
    /// across calls. Validates dimensions, layer counts, sample counts,
    /// resolve consistency (all-or-none), and that every
    /// `GraphResourceKey` is a `Texture` (not a `Buffer`).
    pub fn get_or_create_framebuffer(
        &mut self,
        color_attachments: &[ColorAttachmentSlot],
        depth_stencil_attachment: Option<GraphResourceKey>,
    ) -> Result<FramebufferKey> {
        let rm_arc = Engine::resource_manager()?;
        let gd_arc = Engine::graphics_device("main")?;
        let rm = rm_arc.lock().unwrap();
        let gd = gd_arc.lock().unwrap();
        let key = Self::get_or_create_framebuffer_internal(
            &mut self.framebuffers,
            &mut self.framebuffer_lookup,
            &self.graph_resources,
            color_attachments,
            depth_stencil_attachment,
            &*rm,
            &*gd,
        )?;
        Ok(key)
    }

    pub fn framebuffer(&self, key: FramebufferKey) -> Option<&Framebuffer> {
        self.framebuffers.get(key)
    }

    pub fn framebuffer_count(&self) -> usize {
        self.framebuffers.len()
    }

    /// Remove a framebuffer by key. The lookup entry that pointed at it
    /// is also dropped so a future `get_or_create_framebuffer` call with
    /// the same attachment set will rebuild from scratch.
    pub fn remove_framebuffer(&mut self, key: FramebufferKey) -> bool {
        let removed = self.framebuffers.remove(key).is_some();
        if removed {
            self.framebuffer_lookup.retain(|_, v| *v != key);
        }
        removed
    }

    // ===== EXECUTION =====

    /// Execute one render graph for the current frame.
    ///
    /// `post_passes` is a closure invoked after the last pass and before
    /// the command list is closed (typical use: a `vkCmdBlit` to copy
    /// the offscreen color into the swapchain image).
    ///
    /// # Locking
    ///
    /// The manager locks `Engine::resource_manager()` only briefly while
    /// it builds the per-pass barriers; it then releases the lock
    /// **before** calling each `PassAction::execute` or `post_passes`
    /// callback. That way user code (e.g. drawers) can re-lock the
    /// `ResourceManager` / `GraphicsDevice` freely without deadlocking.
    pub fn execute_render_graph<F>(
        &mut self,
        graph_key: RenderGraphKey,
        passes: &[RenderPassKey],
        post_passes: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut dyn graphics_device::CommandList) -> Result<()>,
    {
        let graph = self.graphs.get_mut(graph_key).ok_or_else(|| {
            crate::engine_err!("galaxy3d::RenderGraphManager",
                "execute_render_graph: RenderGraphKey not found")
        })?;
        graph.execute(
            &mut self.passes,
            &self.graph_resources,
            &self.framebuffers,
            passes,
            post_passes,
        )
    }

    pub fn clear(&mut self) {
        self.graphs.clear();
        self.graph_names.clear();
        self.passes.clear();
        self.pass_names.clear();
        self.graph_resources.clear();
        self.graph_resource_names.clear();
        self.framebuffers.clear();
        self.framebuffer_lookup.clear();
    }

    // ===== PRIVATE HELPERS =====

    /// Walk a slice of `ResourceAccess` and build everything a `RenderPass`
    /// needs to execute: framebuffer (via the manager's cache), render-pass
    /// descriptor (formats / load-store ops), `PassInfo` (with generation
    /// bumping when formats actually change), and clear values.
    ///
    /// Pure / order-preserving: colors are taken in their declared order
    /// (color_attachments + clear_values), depth/stencil last.
    fn build_pass_cache(
        framebuffers: &mut SlotMap<FramebufferKey, Framebuffer>,
        framebuffer_lookup: &mut FxHashMap<FramebufferLookupKey, FramebufferKey>,
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        accesses: &[ResourceAccess],
        pass_name: &str,
        previous_pass_info: Option<&PassInfo>,
        resource_manager: &ResourceManager,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<PassCache> {
        let mut color_slots: Vec<ColorAttachmentSlot> = Vec::new();
        let mut color_attachment_descs: Vec<graphics_device::AttachmentDesc> = Vec::new();
        let mut color_resolve_descs: Vec<graphics_device::AttachmentDesc> = Vec::new();
        let mut depth_attachment_desc: Option<graphics_device::AttachmentDesc> = None;
        let mut depth_attachment_key: Option<GraphResourceKey> = None;
        let mut clear_values: Vec<graphics_device::ClearValue> = Vec::new();
        let mut sample_count: Option<graphics_device::SampleCount> = None;

        // Walk accesses once, splitting attachment inputs across color /
        // resolve / depth-write buckets.
        for access in accesses {
            match access.access_type {
                AccessType::ColorAttachmentWrite | AccessType::ColorAttachmentRead => {
                    let (color_format, color_samples) = Self::resolve_texture_info(
                        graph_resources, resource_manager,
                        access.graph_resource_key, pass_name,
                        "color attachment",
                    )?;
                    Self::check_sample_count(&mut sample_count, color_samples, pass_name)?;

                    let target_ops = access.target_ops.ok_or_else(|| {
                        crate::engine_err!("galaxy3d::RenderGraphManager",
                            "Pass '{}': color attachment access has no TargetOps", pass_name)
                    })?;
                    let TargetOps::Color { clear_color, load_op, store_op, resolve_target }
                        = target_ops
                    else {
                        engine_bail!("galaxy3d::RenderGraphManager",
                            "Pass '{}': color access carries DepthStencil ops", pass_name);
                    };

                    color_attachment_descs.push(graphics_device::AttachmentDesc {
                        format: color_format,
                        samples: color_samples,
                        load_op,
                        store_op,
                        stencil_load_op: graphics_device::LoadOp::DontCare,
                        stencil_store_op: graphics_device::StoreOp::DontCare,
                    });
                    clear_values.push(graphics_device::ClearValue::Color(clear_color));

                    // Optional MSAA resolve: validate format match, single-sample.
                    if let Some(resolve_key) = resolve_target {
                        let (resolve_format, resolve_samples) = Self::resolve_texture_info(
                            graph_resources, resource_manager,
                            resolve_key, pass_name, "resolve attachment",
                        )?;
                        if resolve_samples != graphics_device::SampleCount::S1 {
                            engine_bail!("galaxy3d::RenderGraphManager",
                                "Pass '{}': resolve_target must be single-sampled (got {:?})",
                                pass_name, resolve_samples);
                        }
                        if resolve_format != color_format {
                            engine_bail!("galaxy3d::RenderGraphManager",
                                "Pass '{}': resolve_target format {:?} doesn't match color format {:?}",
                                pass_name, resolve_format, color_format);
                        }
                        color_resolve_descs.push(graphics_device::AttachmentDesc {
                            format: resolve_format,
                            samples: graphics_device::SampleCount::S1,
                            load_op: graphics_device::LoadOp::DontCare,
                            store_op: graphics_device::StoreOp::Store,
                            stencil_load_op: graphics_device::LoadOp::DontCare,
                            stencil_store_op: graphics_device::StoreOp::DontCare,
                        });
                    }

                    color_slots.push(ColorAttachmentSlot {
                        color: access.graph_resource_key,
                        resolve: resolve_target,
                    });
                }

                AccessType::DepthStencilWrite => {
                    if depth_attachment_desc.is_some() {
                        engine_bail!("galaxy3d::RenderGraphManager",
                            "Pass '{}': more than one DepthStencilWrite attachment \
                             (only one depth/stencil OUTPUT is allowed per pass)",
                            pass_name);
                    }
                    let (depth_format, depth_samples) = Self::resolve_texture_info(
                        graph_resources, resource_manager,
                        access.graph_resource_key, pass_name,
                        "depth/stencil attachment",
                    )?;
                    Self::check_sample_count(&mut sample_count, depth_samples, pass_name)?;

                    let target_ops = access.target_ops.ok_or_else(|| {
                        crate::engine_err!("galaxy3d::RenderGraphManager",
                            "Pass '{}': depth/stencil access has no TargetOps", pass_name)
                    })?;
                    let TargetOps::DepthStencil {
                        depth_clear, stencil_clear,
                        depth_load_op, depth_store_op,
                        stencil_load_op, stencil_store_op,
                    } = target_ops
                    else {
                        engine_bail!("galaxy3d::RenderGraphManager",
                            "Pass '{}': depth/stencil access carries Color ops", pass_name);
                    };

                    depth_attachment_desc = Some(graphics_device::AttachmentDesc {
                        format: depth_format,
                        samples: depth_samples,
                        load_op: depth_load_op,
                        store_op: depth_store_op,
                        stencil_load_op,
                        stencil_store_op,
                    });
                    depth_attachment_key = Some(access.graph_resource_key);
                    clear_values.push(graphics_device::ClearValue::DepthStencil {
                        depth: depth_clear,
                        stencil: stencil_clear,
                    });
                }

                // DepthStencilReadOnly = sampling the depth, NOT a
                // framebuffer attachment for our engine semantics.
                // Likewise for *ShaderRead, Compute*, Transfer*,
                // RayTracingRead, and buffer accesses — none of them
                // contribute to the framebuffer.
                _ => {}
            }
        }

        // All-or-none resolve: Vulkan rejects mixed cases, surface it here.
        if !color_resolve_descs.is_empty()
            && color_resolve_descs.len() != color_attachment_descs.len()
        {
            engine_bail!("galaxy3d::RenderGraphManager",
                "Pass '{}': mixed resolve targets — {} of {} color attachments \
                 have a resolve. Either all colors have a resolve_target or none.",
                pass_name, color_resolve_descs.len(), color_attachment_descs.len());
        }

        // Compute-only / read-only pass: nothing to attach.
        if color_attachment_descs.is_empty() && depth_attachment_desc.is_none() {
            return Ok(PassCache {
                framebuffer_key: None,
                pass_info: None,
                gd_render_pass: None,
                clear_values: Vec::new(),
            });
        }

        // Get or create the framebuffer for this exact attachment set.
        let framebuffer_key = Self::get_or_create_framebuffer_internal(
            framebuffers,
            framebuffer_lookup,
            graph_resources,
            &color_slots,
            depth_attachment_key,
            resource_manager,
            graphics_device,
        )?;

        // Build the gd_render_pass descriptor.
        let rp_desc = graphics_device::RenderPassDesc {
            color_attachments: color_attachment_descs.clone(),
            depth_stencil_attachment: depth_attachment_desc.clone(),
            color_resolve_attachments: color_resolve_descs,
        };
        let gd_render_pass = graphics_device.create_render_pass(&rp_desc)?;

        // Build / refresh PassInfo, bumping generation only when formats
        // or sample count actually changed.
        let new_color_formats: Vec<graphics_device::TextureFormat> =
            color_attachment_descs.iter().map(|a| a.format).collect();
        let new_depth_format = depth_attachment_desc.as_ref().map(|a| a.format);
        let resolved_sample_count = sample_count.unwrap_or(graphics_device::SampleCount::S1);
        let pass_info = match previous_pass_info {
            Some(prev)
                if prev.color_formats == new_color_formats
                    && prev.depth_format == new_depth_format
                    && prev.sample_count == resolved_sample_count =>
            {
                prev.clone()
            }
            Some(prev) => {
                let mut next = PassInfo::new(
                    new_color_formats,
                    new_depth_format,
                    resolved_sample_count,
                );
                // PassInfo::new() starts at generation 1; bump again so
                // generations stay strictly monotonic across rebuilds and
                // exceed `prev.generation()` even when prev was already > 1.
                while next.generation() <= prev.generation() {
                    next.increment_generation();
                }
                next
            }
            None => PassInfo::new(
                new_color_formats,
                new_depth_format,
                resolved_sample_count,
            ),
        };

        Ok(PassCache {
            framebuffer_key: Some(framebuffer_key),
            pass_info: Some(pass_info),
            gd_render_pass: Some(gd_render_pass),
            clear_values,
        })
    }

    /// Resolve a `GraphResourceKey` to `(format, sample_count)` of the
    /// underlying texture. Bails with a clear message if the key is
    /// absent, points to a buffer, or the texture is missing from the RM.
    fn resolve_texture_info(
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        resource_manager: &ResourceManager,
        key: GraphResourceKey,
        pass_name: &str,
        role: &str,
    ) -> Result<(graphics_device::TextureFormat, graphics_device::SampleCount)> {
        let (texture_key, _, _, _) = match graph_resources.get(key).copied() {
            Some(GraphResource::Texture { texture_key, base_mip_level, base_array_layer, layer_count }) =>
                (texture_key, base_mip_level, base_array_layer, layer_count),
            Some(GraphResource::Buffer(_)) => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "Pass '{}': {} expected a Texture GraphResource, got Buffer",
                    pass_name, role);
            }
            None => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "Pass '{}': {} GraphResourceKey not found",
                    pass_name, role);
            }
        };
        let texture = resource_manager.texture(texture_key).ok_or_else(|| {
            crate::engine_err!("galaxy3d::RenderGraphManager",
                "Pass '{}': {} TextureKey not found in ResourceManager",
                pass_name, role)
        })?;
        let info = texture.graphics_device_texture().info();
        Ok((info.format, info.sample_count))
    }

    /// Enforce that every attachment of a single pass shares the same
    /// `SampleCount` (Vulkan requirement for color + depth in one render pass).
    fn check_sample_count(
        sample_count: &mut Option<graphics_device::SampleCount>,
        candidate: graphics_device::SampleCount,
        pass_name: &str,
    ) -> Result<()> {
        match *sample_count {
            None => *sample_count = Some(candidate),
            Some(existing) if existing != candidate => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "Pass '{}': mismatched sample counts in attachments \
                     (expected {:?}, found {:?})",
                    pass_name, existing, candidate);
            }
            _ => {}
        }
        Ok(())
    }

    /// `get_or_create_framebuffer` body, free-function form so the
    /// public method and `build_pass_cache` can share it under a split
    /// borrow on `self`.
    fn get_or_create_framebuffer_internal(
        framebuffers: &mut SlotMap<FramebufferKey, Framebuffer>,
        framebuffer_lookup: &mut FxHashMap<FramebufferLookupKey, FramebufferKey>,
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        color_attachments: &[ColorAttachmentSlot],
        depth_stencil_attachment: Option<GraphResourceKey>,
        resource_manager: &ResourceManager,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<FramebufferKey> {
        let lookup_key = FramebufferLookupKey {
            color_attachments: color_attachments.to_vec(),
            depth_stencil_attachment,
        };
        if let Some(&existing) = framebuffer_lookup.get(&lookup_key) {
            return Ok(existing);
        }

        // All-or-none resolve, again — `build_pass_cache` already enforces
        // it but a direct caller of `get_or_create_framebuffer` could miss it.
        let resolves_count = color_attachments.iter().filter(|s| s.resolve.is_some()).count();
        if resolves_count != 0 && resolves_count != color_attachments.len() {
            engine_bail!("galaxy3d::RenderGraphManager",
                "get_or_create_framebuffer: mixed resolve targets — {} of {} \
                 color attachments have a resolve. Either all or none.",
                resolves_count, color_attachments.len());
        }

        // Resolve every GraphResourceKey to a `FramebufferAttachment`,
        // collecting (width, height, layer_count, sample_count) for
        // dimension / sample-count consistency checks.
        let mut color_atts: Vec<graphics_device::FramebufferAttachment> = Vec::new();
        let mut resolve_atts: Vec<graphics_device::FramebufferAttachment> = Vec::new();
        let mut depth_att: Option<graphics_device::FramebufferAttachment> = None;
        let mut fb_width: Option<u32> = None;
        let mut fb_height: Option<u32> = None;
        let mut fb_layer_count: Option<u32> = None;
        let mut fb_sample_count: Option<graphics_device::SampleCount> = None;
        let mut fb_resolve_sample_count: Option<graphics_device::SampleCount> = None;

        for slot in color_attachments {
            let att = Self::build_attachment_and_check_dims(
                graph_resources, resource_manager,
                slot.color, false,
                &mut fb_width, &mut fb_height, &mut fb_layer_count,
                &mut fb_sample_count, &mut fb_resolve_sample_count,
            )?;
            color_atts.push(att);
            if let Some(resolve_key) = slot.resolve {
                let att = Self::build_attachment_and_check_dims(
                    graph_resources, resource_manager,
                    resolve_key, true,
                    &mut fb_width, &mut fb_height, &mut fb_layer_count,
                    &mut fb_sample_count, &mut fb_resolve_sample_count,
                )?;
                resolve_atts.push(att);
            }
        }
        if let Some(depth_key) = depth_stencil_attachment {
            let att = Self::build_attachment_and_check_dims(
                graph_resources, resource_manager,
                depth_key, false,
                &mut fb_width, &mut fb_height, &mut fb_layer_count,
                &mut fb_sample_count, &mut fb_resolve_sample_count,
            )?;
            depth_att = Some(att);
        }

        let width = fb_width.ok_or_else(|| {
            crate::engine_err!("galaxy3d::RenderGraphManager",
                "get_or_create_framebuffer: empty attachment set")
        })?;
        let height = fb_height.unwrap();

        // Need a backend `RenderPass` value for the FramebufferDesc.
        // The descriptor itself doesn't matter for dimensions / view
        // creation — it's a placeholder under dynamic-rendering backends
        // and a compatible-pass spec under classic backends. Build a
        // minimal one matching the attachments.
        let placeholder_desc = graphics_device::RenderPassDesc {
            color_attachments: color_atts.iter().map(|a| graphics_device::AttachmentDesc {
                format: a.texture.info().format,
                samples: a.texture.info().sample_count,
                load_op: graphics_device::LoadOp::DontCare,
                store_op: graphics_device::StoreOp::Store,
                stencil_load_op: graphics_device::LoadOp::DontCare,
                stencil_store_op: graphics_device::StoreOp::DontCare,
            }).collect(),
            depth_stencil_attachment: depth_att.as_ref().map(|a| graphics_device::AttachmentDesc {
                format: a.texture.info().format,
                samples: a.texture.info().sample_count,
                load_op: graphics_device::LoadOp::DontCare,
                store_op: graphics_device::StoreOp::Store,
                stencil_load_op: graphics_device::LoadOp::DontCare,
                stencil_store_op: graphics_device::StoreOp::DontCare,
            }),
            color_resolve_attachments: resolve_atts.iter().map(|a| graphics_device::AttachmentDesc {
                format: a.texture.info().format,
                samples: graphics_device::SampleCount::S1,
                load_op: graphics_device::LoadOp::DontCare,
                store_op: graphics_device::StoreOp::Store,
                stencil_load_op: graphics_device::LoadOp::DontCare,
                stencil_store_op: graphics_device::StoreOp::DontCare,
            }).collect(),
        };
        let placeholder_rp = graphics_device.create_render_pass(&placeholder_desc)?;

        let fb_desc = graphics_device::FramebufferDesc {
            render_pass: &placeholder_rp,
            color_attachments: color_atts,
            depth_stencil_attachment: depth_att,
            color_resolve_attachments: resolve_atts,
            width,
            height,
        };
        let gd_fb = graphics_device.create_framebuffer(&fb_desc)?;

        let fb = Framebuffer::new(gd_fb, color_attachments.to_vec(), depth_stencil_attachment);
        let key = framebuffers.insert(fb);
        framebuffer_lookup.insert(lookup_key, key);
        Ok(key)
    }

    /// Resolve a `GraphResourceKey` to a `FramebufferAttachment` and
    /// fold its dimensions / layer count / sample count into the
    /// running framebuffer-wide trackers, bailing on any mismatch.
    fn build_attachment_and_check_dims(
        graph_resources: &SlotMap<GraphResourceKey, GraphResource>,
        resource_manager: &ResourceManager,
        key: GraphResourceKey,
        is_resolve: bool,
        fb_width: &mut Option<u32>,
        fb_height: &mut Option<u32>,
        fb_layer_count: &mut Option<u32>,
        fb_sample_count: &mut Option<graphics_device::SampleCount>,
        fb_resolve_sample_count: &mut Option<graphics_device::SampleCount>,
    ) -> Result<graphics_device::FramebufferAttachment> {
        let (texture_key, base_mip_level, base_array_layer, layer_count)
            = match graph_resources.get(key).copied() {
                Some(GraphResource::Texture { texture_key, base_mip_level, base_array_layer, layer_count }) =>
                    (texture_key, base_mip_level, base_array_layer, layer_count),
                Some(GraphResource::Buffer(_)) => {
                    engine_bail!("galaxy3d::RenderGraphManager",
                        "framebuffer attachment expected a Texture, got Buffer");
                }
                None => {
                    engine_bail!("galaxy3d::RenderGraphManager",
                        "framebuffer attachment GraphResourceKey not found");
                }
            };
        let texture = resource_manager.texture(texture_key).ok_or_else(|| {
            crate::engine_err!("galaxy3d::RenderGraphManager",
                "framebuffer attachment TextureKey not found in ResourceManager")
        })?;
        let info = texture.graphics_device_texture().info();

        let mip_w = (info.width >> base_mip_level).max(1);
        let mip_h = (info.height >> base_mip_level).max(1);

        match *fb_width {
            None => *fb_width = Some(mip_w),
            Some(w) if w != mip_w => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "framebuffer attachments have mismatched widths (expected {}, got {})",
                    w, mip_w);
            }
            _ => {}
        }
        match *fb_height {
            None => *fb_height = Some(mip_h),
            Some(h) if h != mip_h => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "framebuffer attachments have mismatched heights (expected {}, got {})",
                    h, mip_h);
            }
            _ => {}
        }
        match *fb_layer_count {
            None => *fb_layer_count = Some(layer_count),
            Some(lc) if lc != layer_count => {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "framebuffer attachments have mismatched layer_count \
                     (expected {}, got {})", lc, layer_count);
            }
            _ => {}
        }

        // Resolves must be single-sampled and consistent with each other.
        // Color + depth (non-resolve) must share a single sample count.
        if is_resolve {
            if info.sample_count != graphics_device::SampleCount::S1 {
                engine_bail!("galaxy3d::RenderGraphManager",
                    "framebuffer resolve attachment must be single-sampled (got {:?})",
                    info.sample_count);
            }
            match *fb_resolve_sample_count {
                None => *fb_resolve_sample_count = Some(info.sample_count),
                Some(existing) if existing != info.sample_count => {
                    engine_bail!("galaxy3d::RenderGraphManager",
                        "framebuffer resolve attachments have mismatched sample counts");
                }
                _ => {}
            }
        } else {
            match *fb_sample_count {
                None => *fb_sample_count = Some(info.sample_count),
                Some(existing) if existing != info.sample_count => {
                    engine_bail!("galaxy3d::RenderGraphManager",
                        "framebuffer attachments have mismatched sample counts \
                         (expected {:?}, got {:?})", existing, info.sample_count);
                }
                _ => {}
            }
        }

        Ok(graphics_device::FramebufferAttachment {
            texture: texture.graphics_device_texture().clone(),
            base_mip_level,
            base_array_layer,
            layer_count,
        })
    }
}

#[cfg(test)]
#[path = "render_graph_manager_tests.rs"]
mod tests;
