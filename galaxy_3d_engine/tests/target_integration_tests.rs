//! Integration tests for Target system with GPU
//!
//! These tests verify the TargetManager lifecycle through Engine with VulkanRenderer.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test target_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;
use gpu_test_utils::create_test_window;
use serial_test::serial;

// ============================================================================
// TARGET MANAGER LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_target_manager_lifecycle() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create renderer
    let (window, _event_loop) = create_test_window();
    let renderer = VulkanRenderer::new(&window, Config::default()).unwrap();
    Engine::create_renderer("main", renderer).unwrap();

    // Create all managers
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_target_manager().unwrap();

    // Get target manager
    let tm_arc = Engine::target_manager().unwrap();

    // Use target manager: create render targets
    {
        let mut tm = tm_arc.lock().unwrap();
        tm.create_render_target("screen").unwrap();
        tm.create_render_target("shadow_map").unwrap();
        assert_eq!(tm.render_target_count(), 2);

        // Get a render target
        let screen = tm.render_target("screen");
        assert!(screen.is_some());

        // Remove a render target
        tm.remove_render_target("shadow_map");
        assert_eq!(tm.render_target_count(), 1);
    }

    // Cleanup (order: TM → SM → RM → renderers)
    Engine::destroy_target_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_renderer("main").unwrap();
    Engine::shutdown();
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_target_manager_with_full_engine() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create all subsystems
    let (window, _event_loop) = create_test_window();
    let renderer = VulkanRenderer::new(&window, Config::default()).unwrap();
    Engine::create_renderer("main", renderer).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_target_manager().unwrap();

    // Verify all subsystems are accessible
    assert!(Engine::renderer("main").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());
    assert!(Engine::target_manager().is_ok());

    // Shutdown clears everything
    Engine::shutdown();

    // Re-initialize
    Engine::initialize().unwrap();

    // All should be cleared
    assert!(Engine::target_manager().is_err());
    assert!(Engine::scene_manager().is_err());
    assert!(Engine::resource_manager().is_err());
    assert_eq!(Engine::renderer_count(), 0);

    // Cleanup
    Engine::shutdown();
}
