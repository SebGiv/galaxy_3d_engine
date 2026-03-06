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
/// holds a resolved `graphics_device::RenderPass` and `graphics_device::Framebuffer`.

use std::sync::Arc;
use crate::graphics_device;
use super::access_type::ResourceAccess;
use super::pass_action::PassAction;

pub struct RenderPass {
    /// Resource accesses declared for this pass
    accesses: Vec<ResourceAccess>,
    /// Resolved GPU render pass (created by compile())
    graphics_device_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
    /// Resolved GPU framebuffer (created by compile())
    graphics_device_framebuffer: Option<Arc<dyn graphics_device::Framebuffer>>,
    /// Action to execute during this pass
    action: Option<Box<dyn PassAction>>,
}

impl RenderPass {
    pub(crate) fn new() -> Self {
        Self {
            accesses: Vec::new(),
            graphics_device_render_pass: None,
            graphics_device_framebuffer: None,
            action: None,
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
}
