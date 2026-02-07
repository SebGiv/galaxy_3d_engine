//! Integration tests for Engine lifecycle and management
//!
//! These tests verify the complete Engine workflow with real components.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test engine_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;
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

    // Step 2: Create renderer
    let (window, _event_loop) = create_test_window();
    let renderer = VulkanRenderer::new(&window, Config::default()).unwrap();
    let result = Engine::create_renderer("main", renderer);
    assert!(result.is_ok(), "Renderer creation should succeed");

    // Step 3: Verify renderer is registered
    assert_eq!(Engine::renderer_count(), 1);
    assert_eq!(Engine::renderer_names(), vec!["main".to_string()]);

    // Step 4: Get renderer
    let result = Engine::renderer("main");
    assert!(result.is_ok(), "Getting renderer should succeed");

    // Step 5: Create resource manager
    let result = Engine::create_resource_manager();
    assert!(result.is_ok(), "ResourceManager creation should succeed");

    // Step 6: Get resource manager
    let result = Engine::resource_manager();
    assert!(result.is_ok(), "Getting ResourceManager should succeed");

    // Step 7: Cleanup - destroy resource manager
    let result = Engine::destroy_resource_manager();
    assert!(result.is_ok(), "ResourceManager destruction should succeed");

    // Step 8: Cleanup - destroy renderer
    let result = Engine::destroy_renderer("main");
    assert!(result.is_ok(), "Renderer destruction should succeed");
    assert_eq!(Engine::renderer_count(), 0);

    // Step 9: Shutdown engine
    Engine::shutdown();
}

// Note: This test creates multiple Vulkan renderers, which is not supported on Windows
// due to ash-window's RecreationAttempt error when creating multiple surfaces.
#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_multiple_renderers() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create multiple renderers with different names
    let (window1, _el1) = create_test_window();
    let renderer1 = VulkanRenderer::new(&window1, Config::default()).unwrap();
    Engine::create_renderer("renderer1", renderer1).unwrap();

    let (window2, _el2) = create_test_window();
    let renderer2 = VulkanRenderer::new(&window2, Config::default()).unwrap();
    Engine::create_renderer("renderer2", renderer2).unwrap();

    let (window3, _el3) = create_test_window();
    let renderer3 = VulkanRenderer::new(&window3, Config::default()).unwrap();
    Engine::create_renderer("renderer3", renderer3).unwrap();

    // Verify all renderers are registered
    assert_eq!(Engine::renderer_count(), 3);

    let names = Engine::renderer_names();
    assert!(names.contains(&"renderer1".to_string()));
    assert!(names.contains(&"renderer2".to_string()));
    assert!(names.contains(&"renderer3".to_string()));

    // Get each renderer individually
    assert!(Engine::renderer("renderer1").is_ok());
    assert!(Engine::renderer("renderer2").is_ok());
    assert!(Engine::renderer("renderer3").is_ok());

    // Destroy renderers one by one
    Engine::destroy_renderer("renderer1").unwrap();
    assert_eq!(Engine::renderer_count(), 2);

    Engine::destroy_renderer("renderer2").unwrap();
    assert_eq!(Engine::renderer_count(), 1);

    Engine::destroy_renderer("renderer3").unwrap();
    assert_eq!(Engine::renderer_count(), 0);

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
    let renderer1 = VulkanRenderer::new(&window1, Config::default()).unwrap();
    Engine::create_renderer("main", renderer1).unwrap();

    Engine::create_resource_manager().unwrap();

    // Shutdown
    Engine::shutdown();

    // Second lifecycle - reinitialize
    Engine::initialize().unwrap();

    let (window2, _el2) = create_test_window();
    let renderer2 = VulkanRenderer::new(&window2, Config::default()).unwrap();
    let result = Engine::create_renderer("main", renderer2);
    assert!(result.is_ok(), "Should be able to create renderer after shutdown");

    let result = Engine::create_resource_manager();
    assert!(result.is_ok(), "Should be able to create ResourceManager after shutdown");

    // Verify everything works
    assert_eq!(Engine::renderer_count(), 1);
    assert!(Engine::renderer("main").is_ok());
    assert!(Engine::resource_manager().is_ok());

    // Cleanup
    Engine::shutdown();
}

// Note: Error handling tests removed - they require Engine::reset_for_testing()
// which is only available in internal tests, not integration tests.
