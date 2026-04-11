/// Render pass node in a render graph.
///
/// High-level description of a rendering step (e.g. shadow pass,
/// geometry pass, post-process pass). This is a DAG node — not to
/// be confused with `graphics_device::RenderPass` which is the low-level
/// GPU render pass configuration.
///
/// Each pass declares which resources it accesses and how, via
/// `ResourceAccess` entries. The render graph compiler uses these
/// to determine execution order.
///
/// After `RenderGraph::compile()`, each pass with attachment outputs
/// holds a resolved `graphics_device::RenderPass`, `graphics_device::Framebuffer`,
/// and a `PassInfo` derived from the resolved attachment formats.

use std::sync::Arc;
use crate::graphics_device;
use crate::resource::resource_manager::PassInfo;
use super::access_type::ResourceAccess;
use super::pass_action::PassAction;
use super::render_target::{RenderTarget, TargetOps};

pub struct RenderPass {
    /// Resource accesses declared for this pass
    accesses: Vec<ResourceAccess>,
    /// Resolved GPU render pass (created by compile())
    graphics_device_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
    /// Resolved GPU framebuffer (created by compile())
    graphics_device_framebuffer: Option<Arc<dyn graphics_device::Framebuffer>>,
    /// Action to execute during this pass
    action: Option<Box<dyn PassAction>>,
    /// Attachment format info, derived at compile() from resolved targets.
    /// Generation is incremented only when formats actually change.
    pass_info: Option<PassInfo>,
    /// Pre-computed clear values for begin_render_pass.
    /// Refreshed at compile() via refresh_clear_values(). Reused via clear() +
    /// repush — no allocation in steady state.
    clear_values: Vec<graphics_device::ClearValue>,
    /// Pre-computed image accesses for backend barrier tracking.
    /// Refreshed at compile() via refresh_image_accesses(). Reused via clear() +
    /// repush — no allocation in steady state.
    image_accesses: Vec<graphics_device::ImageAccess>,
}

impl RenderPass {
    pub(crate) fn new() -> Self {
        Self {
            accesses: Vec::new(),
            graphics_device_render_pass: None,
            graphics_device_framebuffer: None,
            action: None,
            pass_info: None,
            clear_values: Vec::new(),
            image_accesses: Vec::new(),
        }
    }

    /// Get all resource accesses declared for this pass
    pub fn accesses(&self) -> &[ResourceAccess] {
        &self.accesses
    }

    /// Get the target indices this pass writes to (attachment outputs).
    ///
    /// Derived from accesses — returns targets with write-type access.
    pub fn output_targets(&self) -> Vec<usize> {
        self.accesses
            .iter()
            .filter(|a| a.access_type.is_write() && a.access_type.is_attachment())
            .map(|a| a.target_id)
            .collect()
    }

    /// Get all target indices this pass uses as attachments (color + depth, read or write).
    ///
    /// Used by compile() to create the framebuffer.
    pub fn attachment_targets(&self) -> Vec<usize> {
        self.accesses
            .iter()
            .filter(|a| a.access_type.is_attachment())
            .map(|a| a.target_id)
            .collect()
    }

    /// Get target indices this pass reads from (non-attachment reads).
    ///
    /// These are shader reads, compute reads, etc. — not render pass attachments.
    pub fn input_targets(&self) -> Vec<usize> {
        self.accesses
            .iter()
            .filter(|a| !a.access_type.is_attachment() && !a.access_type.is_write())
            .map(|a| a.target_id)
            .collect()
    }

    /// Get the resolved GPU render pass (available after compile)
    pub fn graphics_device_render_pass(&self) -> Option<&Arc<dyn graphics_device::RenderPass>> {
        self.graphics_device_render_pass.as_ref()
    }

    /// Get the resolved GPU framebuffer (available after compile)
    pub fn graphics_device_framebuffer(&self) -> Option<&Arc<dyn graphics_device::Framebuffer>> {
        self.graphics_device_framebuffer.as_ref()
    }

    /// Add a resource access declaration
    pub(crate) fn add_access(&mut self, access: ResourceAccess) {
        self.accesses.push(access);
    }

    /// Get mutable access to resource access declarations (for compile-time resolution)
    pub(crate) fn accesses_mut(&mut self) -> &mut [ResourceAccess] {
        &mut self.accesses
    }

    /// Set the resolved GPU render pass
    pub(crate) fn set_graphics_device_render_pass(&mut self, rp: Arc<dyn graphics_device::RenderPass>) {
        self.graphics_device_render_pass = Some(rp);
    }

    /// Set the resolved GPU framebuffer
    pub(crate) fn set_graphics_device_framebuffer(&mut self, fb: Arc<dyn graphics_device::Framebuffer>) {
        self.graphics_device_framebuffer = Some(fb);
    }

    /// Get the action (immutable)
    pub fn action(&self) -> Option<&dyn PassAction> {
        self.action.as_deref()
    }

    /// Get the action (mutable, needed for execute)
    pub fn action_mut(&mut self) -> Option<&mut (dyn PassAction + 'static)> {
        self.action.as_mut().map(|a| a.as_mut())
    }

    /// Set the action for this pass
    pub(crate) fn set_action(&mut self, action: Box<dyn PassAction>) {
        self.action = Some(action);
    }

    // ===== PASS INFO =====

    /// Get the PassInfo (available after compile).
    pub fn pass_info(&self) -> Option<&PassInfo> {
        self.pass_info.as_ref()
    }

    /// Update the PassInfo from resolved attachment targets.
    ///
    /// Compares element by element against the existing PassInfo.
    /// Zero allocation if nothing changed. Increments generation only
    /// when formats or sample count actually change.
    pub(crate) fn update_pass_info_from_targets(
        &mut self,
        color_targets: &[&RenderTarget],
        depth_target: Option<&RenderTarget>,
        sample_count: graphics_device::SampleCount,
    ) {
        let depth_format = depth_target
            .map(|t| t.texture().graphics_device_texture().info().format);

        if let Some(ref mut existing) = self.pass_info {
            // Compare element by element — zero allocation
            let same_colors = existing.color_formats.len() == color_targets.len()
                && existing.color_formats.iter().zip(color_targets.iter())
                    .all(|(fmt, target)| *fmt == target.texture().graphics_device_texture().info().format);

            if same_colors
                && existing.depth_format == depth_format
                && existing.sample_count == sample_count
            {
                return; // nothing changed — no allocation, no generation bump
            }

            // Something changed — update in place, reuse Vec capacity
            existing.color_formats.clear();
            for t in color_targets {
                existing.color_formats.push(t.texture().graphics_device_texture().info().format);
            }
            existing.depth_format = depth_format;
            existing.sample_count = sample_count;
            existing.increment_generation();
        } else {
            // First time — one allocation for the Vec
            let color_formats = color_targets.iter()
                .map(|t| t.texture().graphics_device_texture().info().format)
                .collect();
            self.pass_info = Some(PassInfo::new(color_formats, depth_format, sample_count));
        }
    }

    // ===== CLEAR VALUES =====

    /// Get the pre-computed clear values (refreshed at compile()).
    pub fn clear_values(&self) -> &[graphics_device::ClearValue] {
        &self.clear_values
    }

    /// Refresh the clear values from the targets the pass writes/reads as
    /// attachments. Reuses the existing Vec capacity (zero allocation in
    /// steady state once the pass has been compiled at least once).
    ///
    /// Color attachments first (matching compile() order), then depth/stencil.
    pub(crate) fn refresh_clear_values(&mut self, targets: &[RenderTarget]) {
        self.clear_values.clear();

        // Color attachments first
        for access in &self.accesses {
            if access.access_type.is_attachment() {
                if let TargetOps::Color { clear_color, .. } = targets[access.target_id].ops() {
                    self.clear_values.push(graphics_device::ClearValue::Color(*clear_color));
                }
            }
        }

        // Depth/stencil attachment last
        for access in &self.accesses {
            if access.access_type.is_attachment() {
                if let TargetOps::DepthStencil { depth_clear, stencil_clear, .. } = targets[access.target_id].ops() {
                    self.clear_values.push(graphics_device::ClearValue::DepthStencil {
                        depth: *depth_clear,
                        stencil: *stencil_clear,
                    });
                }
            }
        }
    }

    // ===== IMAGE ACCESSES =====

    /// Get the pre-computed image accesses (refreshed at compile()).
    pub fn image_accesses(&self) -> &[graphics_device::ImageAccess] {
        &self.image_accesses
    }

    /// Refresh the image accesses from the resource access declarations.
    /// Reuses the existing Vec capacity (zero allocation in steady state).
    ///
    /// Note: `previous_access_type` on each ResourceAccess must already be
    /// resolved by the caller (via RenderGraph::resolve_previous_accesses()).
    pub(crate) fn refresh_image_accesses(&mut self, targets: &[RenderTarget]) {
        self.image_accesses.clear();
        for access in &self.accesses {
            let target = &targets[access.target_id];
            self.image_accesses.push(graphics_device::ImageAccess {
                texture: target.texture().graphics_device_texture().clone(),
                access_type: access.access_type,
                previous_access_type: access.previous_access_type,
            });
        }
    }

    /// Split borrow: get action (mut) and pass_info (ref) simultaneously.
    ///
    /// This avoids the borrow conflict between `action_mut()` and `pass_info()`,
    /// which both borrow `&mut self` / `&self` on the same `RenderPass`.
    /// Inside this method, Rust sees that `self.action` and `self.pass_info`
    /// are disjoint fields and allows simultaneous borrows.
    pub fn action_and_pass_info_mut(
        &mut self,
    ) -> (Option<&mut (dyn PassAction + 'static)>, Option<&PassInfo>) {
        (
            self.action.as_mut().map(|a| a.as_mut()),
            self.pass_info.as_ref(),
        )
    }
}
