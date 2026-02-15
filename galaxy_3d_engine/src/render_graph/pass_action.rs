/// Pass action trait and implementations.
///
/// Defines how a render pass records its draw commands between
/// begin_render_pass() and end_render_pass().

use std::sync::Arc;
use crate::error::Result;
use crate::renderer::{self, CommandList};

/// Action executed by a render pass
///
/// Determines what draw commands are recorded between
/// begin_render_pass() and end_render_pass().
pub trait PassAction: Send + Sync {
    /// Record draw commands into the command list
    fn execute(&mut self, cmd: &mut dyn CommandList) -> Result<()>;
}

/// Fullscreen pass action (data-driven, no closure)
///
/// Binds a pipeline and a binding group, then draws a fullscreen
/// triangle (3 vertices). Used for post-processing passes
/// (bloom, blur, tone mapping, etc.).
pub struct FullscreenAction {
    pipeline: Arc<dyn renderer::Pipeline>,
    binding_group: Arc<dyn renderer::BindingGroup>,
}

impl FullscreenAction {
    pub fn new(
        pipeline: Arc<dyn renderer::Pipeline>,
        binding_group: Arc<dyn renderer::BindingGroup>,
    ) -> Self {
        Self { pipeline, binding_group }
    }
}

impl PassAction for FullscreenAction {
    fn execute(&mut self, cmd: &mut dyn CommandList) -> Result<()> {
        cmd.bind_pipeline(&self.pipeline)?;
        cmd.bind_binding_group(&self.pipeline, 0, &self.binding_group)?;
        cmd.draw(3, 0)
    }
}

/// Custom pass action (closure-based)
///
/// Executes a user-provided closure for full control over
/// draw command recording. Used for scene rendering passes
/// (geometry, shadows, etc.).
pub struct CustomAction {
    callback: Box<dyn FnMut(&mut dyn CommandList) -> Result<()> + Send + Sync>,
}

impl CustomAction {
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(&mut dyn CommandList) -> Result<()> + Send + Sync + 'static,
    {
        Self { callback: Box::new(callback) }
    }
}

impl PassAction for CustomAction {
    fn execute(&mut self, cmd: &mut dyn CommandList) -> Result<()> {
        (self.callback)(cmd)
    }
}
