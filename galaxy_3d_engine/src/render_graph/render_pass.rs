/// Render pass node in a render graph.
///
/// High-level description of a rendering step (e.g. shadow pass,
/// geometry pass, post-process pass). This is a DAG node — not to
/// be confused with `renderer::RenderPass` which is the low-level
/// GPU render pass configuration.
///
/// Each pass declares which targets it reads from (inputs) and
/// writes to (outputs), using target indices within the parent
/// `RenderGraph`.
///
/// After `RenderGraph::compile()`, each pass with outputs holds
/// a resolved `renderer::RenderPass` and `renderer::Framebuffer`.

use std::sync::Arc;
use crate::renderer;
use super::pass_action::PassAction;

pub struct RenderPass {
    /// Target indices this pass reads from
    inputs: Vec<usize>,
    /// Target indices this pass writes to
    outputs: Vec<usize>,
    /// Resolved GPU render pass (created by compile())
    renderer_render_pass: Option<Arc<dyn renderer::RenderPass>>,
    /// Resolved GPU framebuffer (created by compile())
    renderer_framebuffer: Option<Arc<dyn renderer::Framebuffer>>,
    /// Action to execute during this pass
    action: Option<Box<dyn PassAction>>,
}

impl RenderPass {
    pub(crate) fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            renderer_render_pass: None,
            renderer_framebuffer: None,
            action: None,
        }
    }

    /// Get the target indices this pass reads from
    pub fn inputs(&self) -> &[usize] {
        &self.inputs
    }

    /// Get the target indices this pass writes to
    pub fn outputs(&self) -> &[usize] {
        &self.outputs
    }

    /// Get the resolved GPU render pass (available after compile)
    pub fn renderer_render_pass(&self) -> Option<&Arc<dyn renderer::RenderPass>> {
        self.renderer_render_pass.as_ref()
    }

    /// Get the resolved GPU framebuffer (available after compile)
    pub fn renderer_framebuffer(&self) -> Option<&Arc<dyn renderer::Framebuffer>> {
        self.renderer_framebuffer.as_ref()
    }

    /// Add an input target index
    pub(crate) fn add_input(&mut self, target_id: usize) {
        self.inputs.push(target_id);
    }

    /// Add an output target index
    pub(crate) fn add_output(&mut self, target_id: usize) {
        self.outputs.push(target_id);
    }

    /// Set the resolved GPU render pass
    pub(crate) fn set_renderer_render_pass(&mut self, rp: Arc<dyn renderer::RenderPass>) {
        self.renderer_render_pass = Some(rp);
    }

    /// Set the resolved GPU framebuffer
    pub(crate) fn set_renderer_framebuffer(&mut self, fb: Arc<dyn renderer::Framebuffer>) {
        self.renderer_framebuffer = Some(fb);
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
