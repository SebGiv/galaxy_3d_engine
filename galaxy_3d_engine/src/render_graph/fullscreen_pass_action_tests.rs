use super::*;
use crate::graphics_device::mock_graphics_device::{
    MockBindingGroup, MockCommandList, MockGraphicsDevice, MockPipeline,
};
use crate::render_graph::test_helpers::make_pass_info;
use std::sync::Arc;

#[test]
fn test_fullscreen_action_execute_emits_three_commands() {
    let pipeline: Arc<dyn crate::graphics_device::Pipeline> =
        Arc::new(MockPipeline::new("fs_pipeline".to_string()));
    let binding_group: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("fs_bg".to_string(), 0));
    let mut action = FullscreenPassAction::new(pipeline, binding_group);
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let mut gd = MockGraphicsDevice::new();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    assert_eq!(cmd.commands, vec!["bind_pipeline", "bind_binding_group", "draw"]);
}

#[test]
fn test_fullscreen_action_execute_called_twice_emits_six_commands() {
    let pipeline: Arc<dyn crate::graphics_device::Pipeline> =
        Arc::new(MockPipeline::new("fs_pipeline".to_string()));
    let binding_group: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("fs_bg".to_string(), 0));
    let mut action = FullscreenPassAction::new(pipeline, binding_group);
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let mut gd = MockGraphicsDevice::new();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    assert_eq!(cmd.commands.len(), 6);
}
