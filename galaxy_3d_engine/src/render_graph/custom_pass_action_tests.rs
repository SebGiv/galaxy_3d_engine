use super::*;
use crate::graphics_device::mock_graphics_device::{MockCommandList, MockGraphicsDevice};
use crate::render_graph::test_helpers::make_pass_info;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[test]
fn test_custom_action_execute_invokes_callback() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_inner = counter.clone();
    let mut action = CustomPassAction::new(move |_cmd, _pass_info| {
        counter_inner.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let mut gd = MockGraphicsDevice::new();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_custom_action_execute_propagates_error() {
    let mut action = CustomPassAction::new(|_cmd, _pass_info| {
        Err(crate::error::Error::InitializationFailed("bang".to_string()))
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let mut gd = MockGraphicsDevice::new();
    let result = action.execute(&mut cmd, &info, &mut gd);
    assert!(result.is_err());
}

#[test]
fn test_custom_action_callback_can_emit_commands() {
    let mut action = CustomPassAction::new(|cmd, _pass_info| {
        cmd.draw(6, 0)?;
        cmd.draw(3, 0)?;
        Ok(())
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let mut gd = MockGraphicsDevice::new();
    action.execute(&mut cmd, &info, &mut gd).unwrap();
    assert_eq!(cmd.commands, vec!["draw", "draw"]);
}
