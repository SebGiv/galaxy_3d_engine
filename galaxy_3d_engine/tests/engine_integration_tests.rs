//! Integration tests for Engine lifecycle and management
//!
//! These tests verify the complete Engine workflow with real components.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test engine_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanGraphicsDevice;
use gpu_test_utils::create_test_window;
use serial_test::serial;

// ============================================================================
// ENGINE LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_engine_full_lifecycle() {
    // Step 1: Initialize engine
    let result = Engine::initialize();
    assert!(result.is_ok(), "Engine initialization should succeed");

    // Step 2: Create graphics device
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();
    let result = Engine::create_graphics_device("main", graphics_device);
    assert!(result.is_ok(), "GraphicsDevice creation should succeed");

    // Step 3: Verify graphics device is registered
    assert_eq!(Engine::graphics_device_count(), 1);
    assert_eq!(Engine::graphics_device_names(), vec!["main".to_string()]);

    // Step 4: Get graphics device
    let result = Engine::graphics_device("main");
    assert!(result.is_ok(), "Getting graphics device should succeed");

    // Step 5: Create resource manager
    let result = Engine::create_resource_manager();
    assert!(result.is_ok(), "ResourceManager creation should succeed");

    // Step 6: Get resource manager
    let result = Engine::resource_manager();
    assert!(result.is_ok(), "Getting ResourceManager should succeed");

    // Step 7: Cleanup - destroy resource manager
    let result = Engine::destroy_resource_manager();
    assert!(result.is_ok(), "ResourceManager destruction should succeed");

    // Step 8: Cleanup - destroy graphics_device
    let result = Engine::destroy_graphics_device("main");
    assert!(result.is_ok(), "GraphicsDevice destruction should succeed");
    assert_eq!(Engine::graphics_device_count(), 0);

    // Step 9: Shutdown engine
    Engine::shutdown();
}

// Note: This test creates multiple Vulkan graphics_devices, which is not supported on Windows
// due to ash-window's RecreationAttempt error when creating multiple surfaces.
#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_multiple_renderers() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create multiple graphics devices with different names
    let (window1, _el1) = create_test_window();
    let graphics_device1 = VulkanGraphicsDevice::new(&window1, Config::default()).unwrap();
    Engine::create_graphics_device("graphics_device1", graphics_device1).unwrap();

    let (window2, _el2) = create_test_window();
    let graphics_device2 = VulkanGraphicsDevice::new(&window2, Config::default()).unwrap();
    Engine::create_graphics_device("graphics_device2", graphics_device2).unwrap();

    let (window3, _el3) = create_test_window();
    let graphics_device3 = VulkanGraphicsDevice::new(&window3, Config::default()).unwrap();
    Engine::create_graphics_device("graphics_device3", graphics_device3).unwrap();

    // Verify all graphics devices are registered
    assert_eq!(Engine::graphics_device_count(), 3);

    let names = Engine::graphics_device_names();
    assert!(names.contains(&"graphics_device1".to_string()));
    assert!(names.contains(&"graphics_device2".to_string()));
    assert!(names.contains(&"graphics_device3".to_string()));

    // Get each graphics device individually
    assert!(Engine::graphics_device("graphics_device1").is_ok());
    assert!(Engine::graphics_device("graphics_device2").is_ok());
    assert!(Engine::graphics_device("graphics_device3").is_ok());

    // Destroy graphics devices one by one
    Engine::destroy_graphics_device("graphics_device1").unwrap();
    assert_eq!(Engine::graphics_device_count(), 2);

    Engine::destroy_graphics_device("graphics_device2").unwrap();
    assert_eq!(Engine::graphics_device_count(), 1);

    Engine::destroy_graphics_device("graphics_device3").unwrap();
    assert_eq!(Engine::graphics_device_count(), 0);

    // Cleanup
    Engine::shutdown();
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_engine_reinitialize_after_shutdown() {
    // First lifecycle
    Engine::initialize().unwrap();

    let (window1, _el1) = create_test_window();
    let graphics_device1 = VulkanGraphicsDevice::new(&window1, Config::default()).unwrap();
    Engine::create_graphics_device("main", graphics_device1).unwrap();

    Engine::create_resource_manager().unwrap();

    // Shutdown
    Engine::shutdown();

    // Second lifecycle - reinitialize
    Engine::initialize().unwrap();

    let (window2, _el2) = create_test_window();
    let graphics_device2 = VulkanGraphicsDevice::new(&window2, Config::default()).unwrap();
    let result = Engine::create_graphics_device("main", graphics_device2);
    assert!(result.is_ok(), "Should be able to create graphics device after shutdown");

    let result = Engine::create_resource_manager();
    assert!(result.is_ok(), "Should be able to create ResourceManager after shutdown");

    // Verify everything works
    assert_eq!(Engine::graphics_device_count(), 1);
    assert!(Engine::graphics_device("main").is_ok());
    assert!(Engine::resource_manager().is_ok());

    // Cleanup
    Engine::shutdown();
}

// Note: Error handling tests removed - they require Engine::reset_for_testing()
// which is only available in internal tests, not integration tests.
