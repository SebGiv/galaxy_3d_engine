//! Fullscreen pass action: data-driven full-screen triangle.
//!
//! Records the canonical 3-vertex draw used by tonemapping and other
//! full-screen post-effects: bind pipeline, bind a single set-0 binding
//! group, draw 3 vertices.

use std::sync::Arc;

use crate::error::Result;
use crate::graphics_device::{self, CommandList};
use crate::resource::resource_manager::PassInfo;

use super::pass_action::PassAction;

/// Fullscreen pass action (data-driven, no closure).
pub struct FullscreenPassAction {
    pipeline: Arc<dyn graphics_device::Pipeline>,
    binding_group: Arc<dyn graphics_device::BindingGroup>,
}

impl FullscreenPassAction {
    pub fn new(
        pipeline: Arc<dyn graphics_device::Pipeline>,
        binding_group: Arc<dyn graphics_device::BindingGroup>,
    ) -> Self {
        Self { pipeline, binding_group }
    }
}

impl PassAction for FullscreenPassAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, _pass_info: &PassInfo) -> Result<()> {
        cmd.bind_pipeline(&self.pipeline)?;
        cmd.bind_binding_group(&self.pipeline, 0, &self.binding_group)?;
        cmd.draw(3, 0)
    }
}

#[cfg(test)]
#[path = "fullscreen_pass_action_tests.rs"]
mod tests;
