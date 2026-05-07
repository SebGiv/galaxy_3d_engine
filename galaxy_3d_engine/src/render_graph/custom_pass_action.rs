//! Custom pass action: closure-based escape hatch.
//!
//! Wraps a user closure into a `PassAction`. Useful for one-off passes that
//! do not fit the data-driven `FullscreenPassAction` / `ScenePassAction`
//! shapes (e.g. swapchain blits, debug overlays, or inline test recordings).

use crate::error::Result;
use crate::graphics_device::CommandList;
use crate::resource::resource_manager::PassInfo;

use super::pass_action::PassAction;

/// Custom pass action (closure-based).
pub struct CustomPassAction {
    callback: Box<dyn FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync>,
}

impl CustomPassAction {
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync + 'static,
    {
        Self { callback: Box::new(callback) }
    }
}

impl PassAction for CustomPassAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()> {
        (self.callback)(cmd, pass_info)
    }
}

#[cfg(test)]
#[path = "custom_pass_action_tests.rs"]
mod tests;
