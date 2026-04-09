/// Pass action trait and implementations.
///
/// Defines how a render pass records its draw commands between
/// begin_render_pass() and end_render_pass().

use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::graphics_device::{self, CommandList};
use crate::resource::resource_manager::PassInfo;
use crate::camera::RenderView;
use crate::scene::{Scene, Drawer};

/// Action executed by a render pass.
///
/// Determines what draw commands are recorded between
/// begin_render_pass() and end_render_pass().
///
/// `pass_info` provides the attachment formats and generation counter
/// of the current render pass (derived automatically at compile time).
pub trait PassAction: Send + Sync {
    /// Record draw commands into the command list.
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()>;
}

/// Fullscreen pass action (data-driven, no closure)
///
/// Binds a pipeline and a binding group, then draws a fullscreen
/// triangle (3 vertices). Used for post-processing passes
/// (bloom, blur, tone mapping, etc.).
pub struct FullscreenAction {
    pipeline: Arc<dyn graphics_device::Pipeline>,
    binding_group: Arc<dyn graphics_device::BindingGroup>,
}

impl FullscreenAction {
    pub fn new(
        pipeline: Arc<dyn graphics_device::Pipeline>,
        binding_group: Arc<dyn graphics_device::BindingGroup>,
    ) -> Self {
        Self { pipeline, binding_group }
    }
}

impl PassAction for FullscreenAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, _pass_info: &PassInfo) -> Result<()> {
        cmd.bind_pipeline(&self.pipeline)?;
        cmd.bind_binding_group(&self.pipeline, 0, &self.binding_group)?;
        cmd.draw(3, 0)
    }
}

/// Custom pass action (closure-based)
///
/// Executes a user-provided closure for full control over
/// draw command recording.
pub struct CustomAction {
    callback: Box<dyn FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync>,
}

impl CustomAction {
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync + 'static,
    {
        Self { callback: Box::new(callback) }
    }
}

impl PassAction for CustomAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()> {
        (self.callback)(cmd, pass_info)
    }
}

/// Scene pass action — draws visible instances from a RenderView.
///
/// Replaces the common CustomAction boilerplate for scene rendering passes.
/// Encapsulates a Scene, a Drawer, and a RenderView. The PassInfo is received
/// at execute() time from the RenderPass (not stored here).
pub struct ScenePassAction {
    scene: Arc<Mutex<Scene>>,
    drawer: Arc<Mutex<dyn Drawer>>,
    render_view: Arc<Mutex<Option<RenderView>>>,
}

impl ScenePassAction {
    pub fn new(
        scene: Arc<Mutex<Scene>>,
        drawer: Arc<Mutex<dyn Drawer>>,
        render_view: Arc<Mutex<Option<RenderView>>>,
    ) -> Self {
        Self { scene, drawer, render_view }
    }
}

impl PassAction for ScenePassAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()> {
        let mut scene = self.scene.lock().unwrap();
        let drawer = self.drawer.lock().unwrap();
        let view = self.render_view.lock().unwrap();
        if let Some(ref view) = *view {
            drawer.draw(&mut scene, view, cmd, pass_info)?;
        }
        Ok(())
    }
}
