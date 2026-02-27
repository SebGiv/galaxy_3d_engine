//! Integration tests for Scene system with GPU
//!
//! These tests verify the SceneManager lifecycle through Engine with VulkanGraphicsDevice.
//! Tests requiring GPU are marked with #[ignore].
//!
//! Run with: cargo test --test scene_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine::galaxy3d::resource::{Buffer, BufferDesc, BufferKind, FieldDesc, FieldType};
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanGraphicsDevice;
use gpu_test_utils::create_test_window;
use serial_test::serial;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_buffers_via_rm(
    graphics_device: Arc<Mutex<dyn galaxy_3d_engine::galaxy3d::render::GraphicsDevice>>,
    prefix: &str,
) -> (Arc<Buffer>, Arc<Buffer>, Arc<Buffer>) {
    let rm_arc = Engine::resource_manager().unwrap();
    let mut rm = rm_arc.lock().unwrap();

    let frame_buffer = rm.create_buffer(format!("{}_frame", prefix), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Uniform,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap();
    let instance_buffer = rm.create_buffer(format!("{}_instance", prefix), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap();
    let material_buffer = rm.create_buffer(format!("{}_material", prefix), BufferDesc {
        graphics_device,
        kind: BufferKind::Storage,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap();
    (frame_buffer, instance_buffer, material_buffer)
}

// ============================================================================
// SCENE MANAGER LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_scene_manager_lifecycle() {
    // Initialize engine
    Engine::initialize().unwrap();

    // Create graphics device
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();
    Engine::create_graphics_device("main", graphics_device).unwrap();

    // Create resource manager
    Engine::create_resource_manager().unwrap();

    // Create scene manager
    let result = Engine::create_scene_manager();
    assert!(result.is_ok(), "SceneManager creation should succeed");

    // Get scene manager
    let sm_arc = Engine::scene_manager().unwrap();

    // Use scene manager: create scenes
    {
        let graphics_device_arc = Engine::graphics_device("main").unwrap();
        let mut sm = sm_arc.lock().unwrap();
        let (fb, ib, mb) = create_test_buffers_via_rm(graphics_device_arc.clone(), "game");
        sm.create_scene("game", graphics_device_arc.clone(), fb, ib, mb).unwrap();
        let (fb2, ib2, mb2) = create_test_buffers_via_rm(graphics_device_arc.clone(), "ui");
        sm.create_scene("ui", graphics_device_arc.clone(), fb2, ib2, mb2).unwrap();
        assert_eq!(sm.scene_count(), 2);

        // Get a scene
        let game_scene = sm.scene("game");
        assert!(game_scene.is_some());

        // Remove a scene
        sm.remove_scene("ui");
        assert_eq!(sm.scene_count(), 1);
    }

    // Cleanup (order: SM → RM → graphics_devices)
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_graphics_device("main").unwrap();
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
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();
    Engine::create_graphics_device("main", graphics_device).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();

    // Verify all subsystems are accessible
    assert!(Engine::graphics_device("main").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());

    // Shutdown clears everything
    Engine::shutdown();

    // Re-initialize
    Engine::initialize().unwrap();

    // All should be cleared
    assert!(Engine::scene_manager().is_err());
    assert!(Engine::resource_manager().is_err());
    assert_eq!(Engine::graphics_device_count(), 0);

    // Cleanup
    Engine::shutdown();
}
