//! Integration tests for RenderGraph system with GPU
//!
//! These tests verify the RenderGraphManager lifecycle through Engine with VulkanGraphicsDevice.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test render_graph_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanGraphicsDevice;
use gpu_test_utils::create_test_window;
use serial_test::serial;

// ============================================================================
// RENDER GRAPH MANAGER LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_render_graph_manager_lifecycle() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create graphics device
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();
    Engine::create_graphics_device("main", graphics_device).unwrap();

    // Create all managers
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();

    // Get render graph manager
    let rgm_arc = Engine::render_graph_manager().unwrap();

    // Use render graph manager: create render graphs
    {
        let mut rgm = rgm_arc.lock().unwrap();
        rgm.create_render_graph("main").unwrap();
        rgm.create_render_graph("shadow").unwrap();
        assert_eq!(rgm.render_graph_count(), 2);

        // Get a render graph
        let main_graph = rgm.render_graph("main");
        assert!(main_graph.is_some());

        // Remove a render graph
        rgm.remove_render_graph("shadow");
        assert_eq!(rgm.render_graph_count(), 1);
    }

    // Cleanup (order: RGM → SM → RM → graphics_devices)
    Engine::destroy_render_graph_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_graphics_device("main").unwrap();
    Engine::shutdown();
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_render_graph_manager_with_full_engine() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create all subsystems
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();
    Engine::create_graphics_device("main", graphics_device).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();

    // Verify all subsystems are accessible
    assert!(Engine::graphics_device("main").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());
    assert!(Engine::render_graph_manager().is_ok());

    // Shutdown clears everything
    Engine::shutdown();

    // Re-initialize
    Engine::initialize().unwrap();

    // All should be cleared
    assert!(Engine::render_graph_manager().is_err());
    assert!(Engine::scene_manager().is_err());
    assert!(Engine::resource_manager().is_err());
    assert_eq!(Engine::graphics_device_count(), 0);

    // Cleanup
    Engine::shutdown();
}
