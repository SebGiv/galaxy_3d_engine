//! Integration tests for Scene system with GPU
//!
//! These tests verify the SceneManager lifecycle through Engine with VulkanRenderer.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test scene_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;
use gpu_test_utils::create_test_window;
use serial_test::serial;

// ============================================================================
// SCENE MANAGER LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_scene_manager_lifecycle() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create renderer
    let (window, _event_loop) = create_test_window();
    let renderer = VulkanRenderer::new(&window, Config::default()).unwrap();
    Engine::create_renderer("main", renderer).unwrap();

    // Create resource manager
    Engine::create_resource_manager().unwrap();

    // Create scene manager
    let result = Engine::create_scene_manager();
    assert!(result.is_ok(), "SceneManager creation should succeed");

    // Get scene manager
    let sm_arc = Engine::scene_manager().unwrap();

    // Use scene manager: create scenes
    {
        let mut sm = sm_arc.lock().unwrap();
        sm.create_scene("game").unwrap();
        sm.create_scene("ui").unwrap();
        assert_eq!(sm.scene_count(), 2);

        // Get a scene
        let game_scene = sm.scene("game");
        assert!(game_scene.is_some());

        // Remove a scene
        sm.remove_scene("ui");
        assert_eq!(sm.scene_count(), 1);
    }

    // Cleanup (order: SM → RM → renderers)
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_renderer("main").unwrap();
    Engine::shutdown();
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_scene_manager_with_full_engine() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create all subsystems
    let (window, _event_loop) = create_test_window();
    let renderer = VulkanRenderer::new(&window, Config::default()).unwrap();
    Engine::create_renderer("main", renderer).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();

    // Verify all subsystems are accessible
    assert!(Engine::renderer("main").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());

    // Shutdown clears everything
    Engine::shutdown();

    // Re-initialize
    Engine::initialize().unwrap();

    // All should be cleared
    assert!(Engine::scene_manager().is_err());
    assert!(Engine::resource_manager().is_err());
    assert_eq!(Engine::renderer_count(), 0);

    // Cleanup
    Engine::shutdown();
}
