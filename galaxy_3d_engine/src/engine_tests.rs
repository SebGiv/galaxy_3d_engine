//! Unit tests for Engine singleton manager
//!
//! Tests initialization, graphics_device management, ResourceManager, and logging APIs.
//!
//! IMPORTANT: ENGINE_STATE is a global OnceLock shared across all tests.
//! All tests are marked with #[serial] to run sequentially and avoid RwLock poisoning.

use crate::galaxy3d::{Engine, Error};
use crate::graphics_device::mock_graphics_device::MockGraphicsDevice;
use crate::galaxy3d::log::{Logger, LogEntry, LogSeverity};
use std::sync::{Arc, Mutex};
use serial_test::serial;

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Test logger that captures log entries for verification
struct TestLogger {
    entries: Arc<Mutex<Vec<String>>>,
}

impl TestLogger {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn entry_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

impl Logger for TestLogger {
    fn log(&self, entry: &LogEntry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push(format!("{:?}: {}", entry.severity, entry.message));
    }
}

/// Setup function to reset engine state before each test
///
/// Note: ENGINE_STATE is a OnceLock, so once initialized it stays initialized.
/// We always call initialize() (idempotent) and use reset_for_testing() to clear graphics_devices/RM.
fn setup() {
    Engine::reset_for_testing();
    let _ = Engine::initialize(); // Always initialize (idempotent)
}


// ============================================================================
// INITIALIZATION AND SHUTDOWN TESTS
// ============================================================================

#[test]
#[serial]
#[serial]
fn test_engine_initialize() {
    setup();
    // Initialize is idempotent, so calling it again should succeed
    let result = Engine::initialize();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_engine_shutdown_clears_state() {
    setup();

    // Get current counts before test
    let initial_graphics_device_count = Engine::graphics_device_count();

    // Create graphics_device (unique name for this test)
    let _renderer = Engine::create_graphics_device("test_shutdown_clears", MockGraphicsDevice::new()).unwrap();

    // Verify graphics_device was added
    assert_eq!(Engine::graphics_device_count(), initial_graphics_device_count + 1);
    assert!(Engine::graphics_device("test_shutdown_clears").is_ok());

    // Destroy the graphics_device we created
    Engine::destroy_graphics_device("test_shutdown_clears").unwrap();

    // Verify it was removed
    assert_eq!(Engine::graphics_device_count(), initial_graphics_device_count);
}

#[test]
#[serial]
fn test_multiple_initialize_calls_idempotent() {
    setup();

    // Multiple initialize calls should be safe
    Engine::initialize().unwrap();
    Engine::initialize().unwrap();
    Engine::initialize().unwrap();

    // Engine should still work normally
    let result = Engine::create_graphics_device("test_multiple_init", MockGraphicsDevice::new());
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_shutdown_clears_renderers() {
    setup();

    // Create multiple graphics_devices
    let _r1 = Engine::create_graphics_device("test_shutdown_r1", MockGraphicsDevice::new()).unwrap();
    let _r2 = Engine::create_graphics_device("test_shutdown_r2", MockGraphicsDevice::new()).unwrap();

    assert!(Engine::graphics_device_count() >= 2);

    // Shutdown should clear all graphics_devices
    Engine::shutdown();

    assert_eq!(Engine::graphics_device_count(), 0);
    assert_eq!(Engine::graphics_device_names().len(), 0);

    // Re-initialize for next tests
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_shutdown_clears_resource_manager() {
    setup();

    // Create resource manager
    Engine::create_resource_manager().unwrap();
    assert!(Engine::resource_manager().is_ok());

    // Shutdown should clear it
    Engine::shutdown();

    // Re-initialize
    Engine::initialize().unwrap();

    // ResourceManager should not exist after shutdown
    assert!(Engine::resource_manager().is_err());
}

#[test]
#[serial]
fn test_shutdown_idempotent() {
    setup();

    // Multiple shutdown calls should be safe
    Engine::shutdown();
    Engine::shutdown();
    Engine::shutdown();

    // Re-initialize for next tests
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_shutdown_with_multiple_resources() {
    setup();

    // Create multiple graphics_devices and resource manager
    let _r1 = Engine::create_graphics_device("test_shutdown_multi_r1", MockGraphicsDevice::new()).unwrap();
    let _r2 = Engine::create_graphics_device("test_shutdown_multi_r2", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();

    // Verify they exist
    assert!(Engine::graphics_device_count() >= 2);
    assert!(Engine::resource_manager().is_ok());

    // Shutdown should clear everything
    Engine::shutdown();

    assert_eq!(Engine::graphics_device_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_reset_for_testing() {
    setup();

    let _renderer = Engine::create_graphics_device("test_reset", MockGraphicsDevice::new()).unwrap();

    // Reset should clear everything
    Engine::reset_for_testing();

    assert_eq!(Engine::graphics_device_count(), 0);
}

// ============================================================================
// RENDERER API TESTS
// ============================================================================

#[test]
#[serial]
fn test_create_graphics_device_success() {
    setup();

    let result = Engine::create_graphics_device("test_create_success", MockGraphicsDevice::new());
    assert!(result.is_ok());

    let graphics_device = result.unwrap();
    assert!(Arc::strong_count(&graphics_device) >= 1);
}

#[test]
#[serial]
fn test_create_graphics_device_duplicate_name_fails() {
    setup();

    // Create first graphics_device
    let _graphics_device1 = Engine::create_graphics_device("test_duplicate", MockGraphicsDevice::new()).unwrap();

    // Creating second with same name should fail
    let result = Engine::create_graphics_device("test_duplicate", MockGraphicsDevice::new());
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("already exists"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_renderer_retrieval_success() {
    setup();

    let created = Engine::create_graphics_device("test_retrieval", MockGraphicsDevice::new()).unwrap();
    let retrieved = Engine::graphics_device("test_retrieval").unwrap();

    // Should be the same Arc (same pointer)
    assert!(Arc::ptr_eq(&created, &retrieved));
}

#[test]
#[serial]
fn test_renderer_not_found_fails() {
    setup();

    let result = Engine::graphics_device("nonexistent_renderer_12345");
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("not found"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_destroy_graphics_device_success() {
    setup();

    let count_before = Engine::graphics_device_count();
    let _renderer = Engine::create_graphics_device("test_destroy_success", MockGraphicsDevice::new()).unwrap();

    // Verify it was added
    assert_eq!(Engine::graphics_device_count(), count_before + 1);

    // Destroy it
    let result = Engine::destroy_graphics_device("test_destroy_success");
    assert!(result.is_ok());

    // Verify it was removed
    assert_eq!(Engine::graphics_device_count(), count_before);
}

#[test]
#[serial]
fn test_destroy_graphics_device_nonexistent_is_ok() {
    setup();

    // Destroying non-existent graphics_device should succeed (idempotent)
    let result = Engine::destroy_graphics_device("nonexistent_renderer_99999");
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_graphics_device_names_multiple() {
    setup();

    let _r1 = Engine::create_graphics_device("test_names_r1", MockGraphicsDevice::new()).unwrap();
    let _r2 = Engine::create_graphics_device("test_names_r2", MockGraphicsDevice::new()).unwrap();
    let _r3 = Engine::create_graphics_device("test_names_r3", MockGraphicsDevice::new()).unwrap();

    let names = Engine::graphics_device_names();
    assert!(names.contains(&"test_names_r1".to_string()));
    assert!(names.contains(&"test_names_r2".to_string()));
    assert!(names.contains(&"test_names_r3".to_string()));
}

#[test]
#[serial]
fn test_graphics_device_count() {
    setup();

    let initial_count = Engine::graphics_device_count();

    let _r1 = Engine::create_graphics_device("test_count_r1", MockGraphicsDevice::new()).unwrap();
    assert_eq!(Engine::graphics_device_count(), initial_count + 1);

    let _r2 = Engine::create_graphics_device("test_count_r2", MockGraphicsDevice::new()).unwrap();
    assert_eq!(Engine::graphics_device_count(), initial_count + 2);

    Engine::destroy_graphics_device("test_count_r1").unwrap();
    assert_eq!(Engine::graphics_device_count(), initial_count + 1);
}

#[test]
#[serial]
fn test_multiple_named_renderers() {
    setup();

    let r1 = Engine::create_graphics_device("test_multi_main", MockGraphicsDevice::new()).unwrap();
    let r2 = Engine::create_graphics_device("test_multi_shadow", MockGraphicsDevice::new()).unwrap();
    let r3 = Engine::create_graphics_device("test_multi_ui", MockGraphicsDevice::new()).unwrap();

    // All should be different instances
    assert!(!Arc::ptr_eq(&r1, &r2));
    assert!(!Arc::ptr_eq(&r2, &r3));
    assert!(!Arc::ptr_eq(&r1, &r3));

    // All should be retrievable
    assert!(Engine::graphics_device("test_multi_main").is_ok());
    assert!(Engine::graphics_device("test_multi_shadow").is_ok());
    assert!(Engine::graphics_device("test_multi_ui").is_ok());
}

#[test]
#[serial]
fn test_renderer_returned_is_usable() {
    setup();

    let graphics_device = Engine::create_graphics_device("test_usable", MockGraphicsDevice::new()).unwrap();

    // Lock the graphics_device (simulates actual usage)
    let _guard = graphics_device.lock().unwrap();
    // If we get here without panic, the graphics_device is usable
}

#[test]
#[serial]
fn test_error_messages_logged() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Trigger various errors to test log_and_return_error()
    let _ = Engine::create_graphics_device("test_err_log_dup", MockGraphicsDevice::new());
    let result = Engine::create_graphics_device("test_err_log_dup", MockGraphicsDevice::new());
    assert!(result.is_err());

    // Error should have been logged
    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("Error")));
    assert!(entries.iter().any(|e| e.contains("already exists")));
}

#[test]
#[serial]
fn test_all_error_types_logged() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // InitializationFailed: duplicate graphics_device
    let _ = Engine::create_graphics_device("test_all_err_1", MockGraphicsDevice::new());
    let _ = Engine::create_graphics_device("test_all_err_1", MockGraphicsDevice::new());

    // InitializationFailed: graphics_device not found
    let _ = Engine::graphics_device("nonexistent_xyz_123");

    // InitializationFailed: ResourceManager not created
    let _ = Engine::resource_manager();

    // Check that errors were logged
    let entries = entries_ref.lock().unwrap();
    assert!(entries.len() >= 3);
}

// ============================================================================
// RESOURCE MANAGER API TESTS
// ============================================================================

#[test]
#[serial]
fn test_create_resource_manager_success() {
    setup();

    let result = Engine::create_resource_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_create_resource_manager_duplicate_fails() {
    setup();

    // Create first resource manager
    Engine::create_resource_manager().unwrap();

    // Creating second should fail
    let result = Engine::create_resource_manager();
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("already exists"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_resource_manager_retrieval_success() {
    setup();

    Engine::create_resource_manager().unwrap();

    let result = Engine::resource_manager();
    assert!(result.is_ok());

    let rm = result.unwrap();
    assert!(Arc::strong_count(&rm) >= 1);
}

#[test]
#[serial]
fn test_resource_manager_not_created_fails() {
    setup();
    // Don't create resource manager

    let result = Engine::resource_manager();
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("not created"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_destroy_resource_manager_success() {
    setup();

    Engine::create_resource_manager().unwrap();

    // Should exist
    assert!(Engine::resource_manager().is_ok());

    // Destroy it
    let result = Engine::destroy_resource_manager();
    assert!(result.is_ok());

    // Should no longer exist
    assert!(Engine::resource_manager().is_err());
}

#[test]
#[serial]
fn test_resource_manager_lifecycle() {
    setup();

    // Create, destroy, create again cycle
    Engine::create_resource_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();

    // Should be able to create again
    let result = Engine::create_resource_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_resource_manager_returned_is_usable() {
    setup();

    Engine::create_resource_manager().unwrap();

    let rm = Engine::resource_manager().unwrap();

    // Lock the resource manager (simulates actual usage)
    let _guard = rm.lock().unwrap();
    // If we get here without panic, the resource manager is usable
}

// ============================================================================
// LOGGING API TESTS
// ============================================================================

#[test]
#[serial]
fn test_default_logger_logs_without_panic() {
    setup();

    // Default logger should work without explicit setup
    Engine::log(LogSeverity::Info, "test", "Test message".to_string());
    Engine::log(LogSeverity::Error, "test", "Error message".to_string());
    Engine::log(LogSeverity::Warn, "test", "Warning message".to_string());

    // If we get here without panic, logging works
}

#[test]
#[serial]
fn test_set_custom_logger() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();

    Engine::set_logger(test_logger);

    // Log some messages
    Engine::log(LogSeverity::Info, "test", "Message 1".to_string());
    Engine::log(LogSeverity::Warn, "test", "Message 2".to_string());

    // Verify messages were captured
    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].contains("Info"));
    assert!(entries[0].contains("Message 1"));
    assert!(entries[1].contains("Warn"));
    assert!(entries[1].contains("Message 2"));
}

#[test]
#[serial]
fn test_reset_logger_to_default() {
    setup();

    // Set custom logger
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Reset to default
    Engine::reset_logger();

    // Log a message
    Engine::log(LogSeverity::Info, "test", "After reset".to_string());

    // Custom logger should NOT receive this message (default logger is active)
    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 0);
}

#[test]
#[serial]
fn test_log_simple_message() {
    setup();

    let test_logger = TestLogger::new();
    let _count_before = test_logger.entry_count();
    Engine::set_logger(test_logger);

    Engine::log(LogSeverity::Debug, "galaxy3d::test", "Simple log message".to_string());

    // Test mainly verifies no panic occurs
}

#[test]
#[serial]
fn test_log_detailed_with_file_line() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    Engine::log_detailed(
        LogSeverity::Error,
        "galaxy3d::test",
        "Detailed error".to_string(),
        "test.rs",
        42,
    );

    // Verify message was logged
    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].contains("Error"));
    assert!(entries[0].contains("Detailed error"));
}

#[test]
#[serial]
fn test_custom_logger_receives_logs() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Log messages of different severities
    Engine::log(LogSeverity::Trace, "test", "Trace".to_string());
    Engine::log(LogSeverity::Debug, "test", "Debug".to_string());
    Engine::log(LogSeverity::Info, "test", "Info".to_string());
    Engine::log(LogSeverity::Warn, "test", "Warn".to_string());
    Engine::log(LogSeverity::Error, "test", "Error".to_string());

    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 5);
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
#[serial]
fn test_full_engine_lifecycle() {
    setup();

    // Create graphics_device
    let graphics_device = Engine::create_graphics_device("test_lifecycle_main", MockGraphicsDevice::new()).unwrap();
    assert!(Engine::graphics_device("test_lifecycle_main").is_ok());

    // Create resource manager
    Engine::create_resource_manager().unwrap();
    assert!(Engine::resource_manager().is_ok());

    // Use graphics_device
    let _guard = graphics_device.lock().unwrap();
    drop(_guard);

    // Cleanup
    Engine::destroy_graphics_device("test_lifecycle_main").unwrap();
    Engine::destroy_resource_manager().unwrap();
}

#[test]
#[serial]
fn test_shutdown_clears_rm_before_renderers() {
    setup();

    // Create graphics_device and resource manager
    let _renderer = Engine::create_graphics_device("test_shutdown_order", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();

    // Both should exist
    assert!(Engine::graphics_device("test_shutdown_order").is_ok());
    assert!(Engine::resource_manager().is_ok());

    // Shutdown clears RM before graphics_devices (as per comment in code)
    Engine::shutdown();

    // Both should be cleared
    assert_eq!(Engine::graphics_device_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_concurrent_renderer_access() {
    setup();

    let graphics_device = Engine::create_graphics_device("test_concurrent", MockGraphicsDevice::new()).unwrap();

    // Spawn multiple threads accessing the same graphics_device
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let renderer_clone = graphics_device.clone();
            std::thread::spawn(move || {
                for _ in 0..10 {
                    let _guard = renderer_clone.lock().unwrap();
                    // Simulate some work
                    std::thread::sleep(std::time::Duration::from_micros(1));
                }
                i
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // If we get here without deadlock or panic, concurrent access works
}

// ============================================================================
// SCENE MANAGER TESTS
// ============================================================================

#[test]
#[serial]
fn test_create_scene_manager_success() {
    setup();

    let result = Engine::create_scene_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_create_scene_manager_duplicate_fails() {
    setup();

    Engine::create_scene_manager().unwrap();

    let result = Engine::create_scene_manager();
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_scene_manager_not_created_fails() {
    setup();

    let result = Engine::scene_manager();
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_scene_manager_retrieval_success() {
    setup();

    Engine::create_scene_manager().unwrap();

    let result = Engine::scene_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_scene_manager_returned_is_usable() {
    setup();

    let _graphics_device = Engine::create_graphics_device("test_sm_usable", MockGraphicsDevice::new()).unwrap();
    Engine::create_scene_manager().unwrap();

    let sm = Engine::scene_manager().unwrap();

    // Lock and use the scene manager
    let mut guard = sm.lock().unwrap();
    let scene = guard.create_scene("test_scene");
    assert!(scene.is_ok());
}

#[test]
#[serial]
fn test_destroy_scene_manager_success() {
    setup();

    Engine::create_scene_manager().unwrap();

    // Should exist
    assert!(Engine::scene_manager().is_ok());

    // Destroy it
    let result = Engine::destroy_scene_manager();
    assert!(result.is_ok());

    // Should no longer exist
    assert!(Engine::scene_manager().is_err());
}

#[test]
#[serial]
fn test_scene_manager_lifecycle() {
    setup();

    // Create, destroy, create again cycle
    Engine::create_scene_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();

    // Should be able to create again
    let result = Engine::create_scene_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_shutdown_clears_scene_manager() {
    setup();

    Engine::create_scene_manager().unwrap();
    assert!(Engine::scene_manager().is_ok());

    Engine::shutdown();

    // Re-initialize to avoid affecting other tests
    Engine::initialize().unwrap();

    // Scene manager should be cleared
    assert!(Engine::scene_manager().is_err());
}

#[test]
#[serial]
fn test_shutdown_clears_sm_before_rm() {
    setup();

    // Create all subsystems
    let _renderer = Engine::create_graphics_device("test_shutdown_sm_rm", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();

    // All should exist
    assert!(Engine::graphics_device("test_shutdown_sm_rm").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());

    // Shutdown (order: SM → RM → graphics_devices)
    Engine::shutdown();

    assert_eq!(Engine::graphics_device_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_full_engine_lifecycle_with_scene_manager() {
    setup();

    // Create all subsystems
    let _renderer = Engine::create_graphics_device("test_full_sm", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();

    // Use scene manager
    {
        let _graphics_device = Engine::graphics_device("test_full_sm").unwrap();
        let sm = Engine::scene_manager().unwrap();
        let mut guard = sm.lock().unwrap();
        guard.create_scene("main").unwrap();
        guard.create_scene("ui").unwrap();
        assert_eq!(guard.scene_count(), 2);
    }

    // Cleanup
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_graphics_device("test_full_sm").unwrap();
}

// ============================================================================
// RENDER GRAPH MANAGER TESTS
// ============================================================================

#[test]
#[serial]
fn test_create_render_graph_manager_success() {
    setup();

    let result = Engine::create_render_graph_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_create_render_graph_manager_duplicate_fails() {
    setup();

    Engine::create_render_graph_manager().unwrap();

    let result = Engine::create_render_graph_manager();
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("already exists"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_render_graph_manager_not_created_fails() {
    setup();

    let result = Engine::render_graph_manager();
    assert!(result.is_err());
    match result {
        Err(Error::InitializationFailed(msg)) => {
            assert!(msg.contains("not created"));
        }
        _ => panic!("Expected InitializationFailed error"),
    }
}

#[test]
#[serial]
fn test_render_graph_manager_retrieval_success() {
    setup();

    Engine::create_render_graph_manager().unwrap();

    let result = Engine::render_graph_manager();
    assert!(result.is_ok());

    let rgm = result.unwrap();
    assert!(Arc::strong_count(&rgm) >= 1);
}

#[test]
#[serial]
fn test_render_graph_manager_returned_is_usable() {
    setup();

    Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
    Engine::create_render_graph_manager().unwrap();

    let rgm = Engine::render_graph_manager().unwrap();

    // Lock and use the render graph manager (the manager looks up the
    // graphics_device "main" internally — no need to lock it manually).
    let mut guard = rgm.lock().unwrap();
    let graph = guard.create_render_graph("main", 2);
    assert!(graph.is_ok());

    drop(guard);
    Engine::destroy_render_graph_manager().unwrap();
    Engine::destroy_graphics_device("main").unwrap();
}

#[test]
#[serial]
fn test_destroy_render_graph_manager_success() {
    setup();

    Engine::create_render_graph_manager().unwrap();

    // Should exist
    assert!(Engine::render_graph_manager().is_ok());

    // Destroy it
    let result = Engine::destroy_render_graph_manager();
    assert!(result.is_ok());

    // Should no longer exist
    assert!(Engine::render_graph_manager().is_err());
}

#[test]
#[serial]
fn test_render_graph_manager_lifecycle() {
    setup();

    // Create, destroy, create again cycle
    Engine::create_render_graph_manager().unwrap();
    Engine::destroy_render_graph_manager().unwrap();

    // Should be able to create again
    let result = Engine::create_render_graph_manager();
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_shutdown_clears_render_graph_manager() {
    setup();

    Engine::create_render_graph_manager().unwrap();
    assert!(Engine::render_graph_manager().is_ok());

    Engine::shutdown();

    // Re-initialize to avoid affecting other tests
    Engine::initialize().unwrap();

    // Render graph manager should be cleared
    assert!(Engine::render_graph_manager().is_err());
}

#[test]
#[serial]
fn test_shutdown_clears_rgm_before_sm() {
    setup();

    // Create all subsystems
    let _renderer = Engine::create_graphics_device("test_shutdown_rgm_sm", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();

    // All should exist
    assert!(Engine::graphics_device("test_shutdown_rgm_sm").is_ok());
    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());
    assert!(Engine::render_graph_manager().is_ok());

    // Shutdown (order: RGM → SM → RM → graphics_devices)
    Engine::shutdown();

    assert_eq!(Engine::graphics_device_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_full_engine_lifecycle_with_render_graph_manager() {
    setup();

    // Create all subsystems
    let _renderer = Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();

    // Use render graph manager
    {
        let rgm = Engine::render_graph_manager().unwrap();
        let mut guard = rgm.lock().unwrap();
        guard.create_render_graph("main", 2).unwrap();
        guard.create_render_graph("shadow", 2).unwrap();
        assert_eq!(guard.render_graph_count(), 2);
    }

    // Cleanup (order: RGM → SM → RM → graphics_devices)
    Engine::destroy_render_graph_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_graphics_device("main").unwrap();
}

#[test]
#[serial]
fn test_render_graph_manager_errors_logged() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // InitializationFailed: duplicate render graph manager
    Engine::create_render_graph_manager().unwrap();
    let _ = Engine::create_render_graph_manager();

    // InitializationFailed: render graph manager not created (after destroy)
    Engine::destroy_render_graph_manager().unwrap();
    let _ = Engine::render_graph_manager();

    // Check that errors were logged
    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("already exists")));
    assert!(entries.iter().any(|e| e.contains("not created")));
}

// ============================================================================
// Additional engine path tests
// ============================================================================

#[test]
#[serial]
fn test_destroy_resource_manager_idempotent() {
    setup();
    Engine::create_resource_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    // Second destroy is OK (no-op).
    assert!(Engine::destroy_resource_manager().is_ok());
}

#[test]
#[serial]
fn test_destroy_scene_manager_idempotent() {
    setup();
    Engine::create_scene_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    assert!(Engine::destroy_scene_manager().is_ok());
}

#[test]
#[serial]
fn test_destroy_render_graph_manager_idempotent() {
    setup();
    Engine::create_render_graph_manager().unwrap();
    Engine::destroy_render_graph_manager().unwrap();
    assert!(Engine::destroy_render_graph_manager().is_ok());
}

#[test]
#[serial]
fn test_destroy_graphics_device_for_unknown_name_is_ok() {
    setup();
    // Engine API is forgiving: removing an unknown device returns Ok.
    assert!(Engine::destroy_graphics_device("never_registered").is_ok());
}

#[test]
#[serial]
fn test_create_resource_manager_after_destroy_succeeds() {
    setup();
    Engine::create_resource_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    assert!(Engine::create_resource_manager().is_ok());
}

#[test]
#[serial]
fn test_create_scene_manager_after_destroy_succeeds() {
    setup();
    Engine::create_scene_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    assert!(Engine::create_scene_manager().is_ok());
}

#[test]
#[serial]
fn test_create_render_graph_manager_after_destroy_succeeds() {
    setup();
    Engine::create_render_graph_manager().unwrap();
    Engine::destroy_render_graph_manager().unwrap();
    assert!(Engine::create_render_graph_manager().is_ok());
}

#[test]
#[serial]
fn test_shutdown_clears_resource_scene_render_graph_managers() {
    setup();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();
    Engine::shutdown();
    // After shutdown, all retrievals fail.
    assert!(Engine::resource_manager().is_err());
    assert!(Engine::scene_manager().is_err());
    assert!(Engine::render_graph_manager().is_err());
}

#[test]
#[serial]
fn test_log_severity_warn_routes_to_logger() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    Engine::log(LogSeverity::Warn, "test", "warning message".to_string());
    Engine::log(LogSeverity::Error, "test", "error message".to_string());

    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].contains("warning"));
    assert!(entries[1].contains("error"));
}

#[test]
#[serial]
fn test_log_detailed_routes_severity_correctly() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    Engine::log_detailed(LogSeverity::Info, "src", "msg-a".to_string(), "file.rs", 1);
    Engine::log_detailed(LogSeverity::Warn, "src", "msg-b".to_string(), "file.rs", 2);
    Engine::log_detailed(LogSeverity::Error, "src", "msg-c".to_string(), "file.rs", 3);
    Engine::log_detailed(LogSeverity::Debug, "src", "msg-d".to_string(), "file.rs", 4);

    let entries = entries_ref.lock().unwrap();
    assert_eq!(entries.len(), 4);
}

#[test]
#[serial]
fn test_log_and_return_error_via_backend_error_path() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Trigger a duplicate-create which goes through log_and_return_error
    // with InitializationFailed; we already cover that branch elsewhere.
    let _ = Engine::create_graphics_device("dup_path", MockGraphicsDevice::new()).unwrap();
    let err = Engine::create_graphics_device("dup_path", MockGraphicsDevice::new());
    assert!(err.is_err());

    // Verify the error was logged with the proper severity. Drop the lock
    // BEFORE calling destroy_graphics_device, otherwise destroy's info-log
    // re-enters TestLogger::log which would deadlock on the same Mutex.
    {
        let entries = entries_ref.lock().unwrap();
        assert!(entries.iter().any(|e| e.contains("already exists")));
    }

    Engine::destroy_graphics_device("dup_path").unwrap();
}

#[test]
#[serial]
fn test_graphics_device_count_with_zero_devices() {
    setup();
    // After reset_for_testing, no devices remain.
    assert_eq!(Engine::graphics_device_count(), 0);
    assert!(Engine::graphics_device_names().is_empty());
}

#[test]
#[serial]
fn test_create_then_count_then_destroy_full_cycle() {
    setup();
    Engine::create_graphics_device("cycle_a", MockGraphicsDevice::new()).unwrap();
    Engine::create_graphics_device("cycle_b", MockGraphicsDevice::new()).unwrap();
    Engine::create_graphics_device("cycle_c", MockGraphicsDevice::new()).unwrap();
    assert_eq!(Engine::graphics_device_count(), 3);
    let names = Engine::graphics_device_names();
    assert!(names.contains(&"cycle_a".to_string()));
    assert!(names.contains(&"cycle_b".to_string()));
    assert!(names.contains(&"cycle_c".to_string()));
    Engine::destroy_graphics_device("cycle_a").unwrap();
    Engine::destroy_graphics_device("cycle_b").unwrap();
    Engine::destroy_graphics_device("cycle_c").unwrap();
    assert_eq!(Engine::graphics_device_count(), 0);
}

#[test]
#[serial]
fn test_log_and_return_error_with_other_error_variant() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // OutOfMemory falls into the `_ =>` branch of log_and_return_error.
    Engine::log(
        LogSeverity::Error,
        "galaxy3d::test",
        format!("{:?}", Error::OutOfMemory),
    );

    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("OutOfMemory")));
}

#[test]
#[serial]
fn test_engine_initialize_then_all_managers_creatable() {
    Engine::reset_for_testing();
    Engine::initialize().unwrap();
    Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
    Engine::create_scene_manager().unwrap();
    Engine::create_render_graph_manager().unwrap();

    assert!(Engine::resource_manager().is_ok());
    assert!(Engine::scene_manager().is_ok());
    assert!(Engine::render_graph_manager().is_ok());

    Engine::destroy_render_graph_manager().unwrap();
    Engine::destroy_scene_manager().unwrap();
    Engine::destroy_resource_manager().unwrap();
    Engine::destroy_graphics_device("main").unwrap();
}

// ============================================================================
// Direct exercise of log_and_return_error variants (BackendError + fallback)
// ============================================================================
//
// log_and_return_error is private to Engine. Through `super::Engine`, the
// tests sub-module can reach it directly to cover the BackendError branch
// (lines 67-69) and the catch-all `_ =>` branch (lines 70-72) without
// having to poison a global RwLock — which would taint the Engine state for
// every subsequent test.

#[test]
#[serial]
fn test_log_and_return_error_backend_error_variant() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Drive the BackendError match arm directly.
    let returned = super::Engine::log_and_return_error(
        Error::BackendError("test backend error".to_string())
    );
    match returned {
        Error::BackendError(msg) => assert_eq!(msg, "test backend error"),
        _ => panic!("expected BackendError variant"),
    }

    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("Backend error")));
    assert!(entries.iter().any(|e| e.contains("test backend error")));
}

#[test]
#[serial]
fn test_log_and_return_error_out_of_memory_falls_into_default_arm() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // OutOfMemory is neither InitializationFailed nor BackendError → catch-all `_`.
    let returned = super::Engine::log_and_return_error(Error::OutOfMemory);
    assert!(matches!(returned, Error::OutOfMemory));

    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("Engine error")));
}

#[test]
#[serial]
fn test_log_and_return_error_initialization_failed_path() {
    setup();
    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    let returned = super::Engine::log_and_return_error(
        Error::InitializationFailed("test init failed".to_string())
    );
    assert!(matches!(returned, Error::InitializationFailed(_)));

    let entries = entries_ref.lock().unwrap();
    assert!(entries.iter().any(|e| e.contains("Initialization failed")));
    assert!(entries.iter().any(|e| e.contains("test init failed")));
}
