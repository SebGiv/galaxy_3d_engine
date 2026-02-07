//! Integration tests for CommandList workflow with real Vulkan backend
//!
//! These tests verify the complete command recording workflow with a real GPU.
//! All tests require a GPU and are marked with #[ignore].
//!
//! Run with: cargo test --test command_list_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Renderer;
use gpu_test_utils::get_test_renderer;
use serial_test::serial;

// ============================================================================
// COMMAND LIST WORKFLOW TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_command_list_basic_workflow() {
    // Get shared Vulkan renderer
    let renderer = get_test_renderer();
    let renderer_guard = renderer.lock().unwrap();

    // Create command list
    let mut cmd_list = renderer_guard.create_command_list().unwrap();

    // Test basic workflow: begin -> end
    let result = cmd_list.begin();
    assert!(result.is_ok(), "CommandList::begin() should succeed");

    let result = cmd_list.end();
    assert!(result.is_ok(), "CommandList::end() should succeed");
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_command_list_multiple_begin_end_cycles() {
    // Get shared Vulkan renderer
    let renderer = get_test_renderer();
    let renderer_guard = renderer.lock().unwrap();

    // Create command list
    let mut cmd_list = renderer_guard.create_command_list().unwrap();

    // Test multiple begin/end cycles
    for i in 0..5 {
        let result = cmd_list.begin();
        assert!(result.is_ok(), "Cycle {}: begin() failed", i);

        let result = cmd_list.end();
        assert!(result.is_ok(), "Cycle {}: end() failed", i);
    }
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_multiple_command_lists() {
    // Get shared Vulkan renderer
    let renderer = get_test_renderer();
    let renderer_guard = renderer.lock().unwrap();

    // Create multiple command lists
    let mut cmd_lists: Vec<_> = (0..3)
        .map(|_| renderer_guard.create_command_list().unwrap())
        .collect();

    // All command lists should work independently
    for (i, cmd_list) in cmd_lists.iter_mut().enumerate() {
        let result = cmd_list.begin();
        assert!(result.is_ok(), "CommandList {} begin() failed", i);

        let result = cmd_list.end();
        assert!(result.is_ok(), "CommandList {} end() failed", i);
    }
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_command_list_reuse() {
    // Get shared Vulkan renderer
    let renderer = get_test_renderer();
    let renderer_guard = renderer.lock().unwrap();

    // Create command list
    let mut cmd_list = renderer_guard.create_command_list().unwrap();

    // Record commands multiple times (reuse)
    for i in 0..10 {
        cmd_list.begin().unwrap();
        // In a real scenario, we'd record actual draw commands here
        cmd_list.end().unwrap();

        // Command list should be reusable
        assert!(true, "Iteration {} completed successfully", i);
    }
}
