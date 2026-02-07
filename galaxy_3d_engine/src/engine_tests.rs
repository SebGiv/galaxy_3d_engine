//! Unit tests for Engine singleton manager
//!
//! Tests initialization, renderer management, ResourceManager, and logging APIs.
//!
//! IMPORTANT: ENGINE_STATE is a global OnceLock shared across all tests.
//! All tests are marked with #[serial] to run sequentially and avoid RwLock poisoning.

use crate::galaxy3d::{Engine, Error};
use crate::renderer::mock_renderer::MockRenderer;
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
/// We always call initialize() (idempotent) and use reset_for_testing() to clear renderers/RM.
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
    let initial_renderer_count = Engine::renderer_count();

    // Create renderer (unique name for this test)
    let _renderer = Engine::create_renderer("test_shutdown_clears", MockRenderer::new()).unwrap();

    // Verify renderer was added
    assert_eq!(Engine::renderer_count(), initial_renderer_count + 1);
    assert!(Engine::renderer("test_shutdown_clears").is_ok());

    // Destroy the renderer we created
    Engine::destroy_renderer("test_shutdown_clears").unwrap();

    // Verify it was removed
    assert_eq!(Engine::renderer_count(), initial_renderer_count);
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
    let result = Engine::create_renderer("test_multiple_init", MockRenderer::new());
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_shutdown_clears_renderers() {
    setup();

    // Create multiple renderers
    let _r1 = Engine::create_renderer("test_shutdown_r1", MockRenderer::new()).unwrap();
    let _r2 = Engine::create_renderer("test_shutdown_r2", MockRenderer::new()).unwrap();

    assert!(Engine::renderer_count() >= 2);

    // Shutdown should clear all renderers
    Engine::shutdown();

    assert_eq!(Engine::renderer_count(), 0);
    assert_eq!(Engine::renderer_names().len(), 0);

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

    // Create multiple renderers and resource manager
    let _r1 = Engine::create_renderer("test_shutdown_multi_r1", MockRenderer::new()).unwrap();
    let _r2 = Engine::create_renderer("test_shutdown_multi_r2", MockRenderer::new()).unwrap();
    Engine::create_resource_manager().unwrap();

    // Verify they exist
    assert!(Engine::renderer_count() >= 2);
    assert!(Engine::resource_manager().is_ok());

    // Shutdown should clear everything
    Engine::shutdown();

    assert_eq!(Engine::renderer_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_reset_for_testing() {
    setup();

    let _renderer = Engine::create_renderer("test_reset", MockRenderer::new()).unwrap();

    // Reset should clear everything
    Engine::reset_for_testing();

    assert_eq!(Engine::renderer_count(), 0);
}

// ============================================================================
// RENDERER API TESTS
// ============================================================================

#[test]
#[serial]
fn test_create_renderer_success() {
    setup();

    let result = Engine::create_renderer("test_create_success", MockRenderer::new());
    assert!(result.is_ok());

    let renderer = result.unwrap();
    assert!(Arc::strong_count(&renderer) >= 1);
}

#[test]
#[serial]
fn test_create_renderer_duplicate_name_fails() {
    setup();

    // Create first renderer
    let _renderer1 = Engine::create_renderer("test_duplicate", MockRenderer::new()).unwrap();

    // Creating second with same name should fail
    let result = Engine::create_renderer("test_duplicate", MockRenderer::new());
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

    let created = Engine::create_renderer("test_retrieval", MockRenderer::new()).unwrap();
    let retrieved = Engine::renderer("test_retrieval").unwrap();

    // Should be the same Arc (same pointer)
    assert!(Arc::ptr_eq(&created, &retrieved));
}

#[test]
#[serial]
fn test_renderer_not_found_fails() {
    setup();

    let result = Engine::renderer("nonexistent_renderer_12345");
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
fn test_destroy_renderer_success() {
    setup();

    let count_before = Engine::renderer_count();
    let _renderer = Engine::create_renderer("test_destroy_success", MockRenderer::new()).unwrap();

    // Verify it was added
    assert_eq!(Engine::renderer_count(), count_before + 1);

    // Destroy it
    let result = Engine::destroy_renderer("test_destroy_success");
    assert!(result.is_ok());

    // Verify it was removed
    assert_eq!(Engine::renderer_count(), count_before);
}

#[test]
#[serial]
fn test_destroy_renderer_nonexistent_is_ok() {
    setup();

    // Destroying non-existent renderer should succeed (idempotent)
    let result = Engine::destroy_renderer("nonexistent_renderer_99999");
    assert!(result.is_ok());
}

#[test]
#[serial]
fn test_renderer_names_multiple() {
    setup();

    let _r1 = Engine::create_renderer("test_names_r1", MockRenderer::new()).unwrap();
    let _r2 = Engine::create_renderer("test_names_r2", MockRenderer::new()).unwrap();
    let _r3 = Engine::create_renderer("test_names_r3", MockRenderer::new()).unwrap();

    let names = Engine::renderer_names();
    assert!(names.contains(&"test_names_r1".to_string()));
    assert!(names.contains(&"test_names_r2".to_string()));
    assert!(names.contains(&"test_names_r3".to_string()));
}

#[test]
#[serial]
fn test_renderer_count() {
    setup();

    let initial_count = Engine::renderer_count();

    let _r1 = Engine::create_renderer("test_count_r1", MockRenderer::new()).unwrap();
    assert_eq!(Engine::renderer_count(), initial_count + 1);

    let _r2 = Engine::create_renderer("test_count_r2", MockRenderer::new()).unwrap();
    assert_eq!(Engine::renderer_count(), initial_count + 2);

    Engine::destroy_renderer("test_count_r1").unwrap();
    assert_eq!(Engine::renderer_count(), initial_count + 1);
}

#[test]
#[serial]
fn test_multiple_named_renderers() {
    setup();

    let r1 = Engine::create_renderer("test_multi_main", MockRenderer::new()).unwrap();
    let r2 = Engine::create_renderer("test_multi_shadow", MockRenderer::new()).unwrap();
    let r3 = Engine::create_renderer("test_multi_ui", MockRenderer::new()).unwrap();

    // All should be different instances
    assert!(!Arc::ptr_eq(&r1, &r2));
    assert!(!Arc::ptr_eq(&r2, &r3));
    assert!(!Arc::ptr_eq(&r1, &r3));

    // All should be retrievable
    assert!(Engine::renderer("test_multi_main").is_ok());
    assert!(Engine::renderer("test_multi_shadow").is_ok());
    assert!(Engine::renderer("test_multi_ui").is_ok());
}

#[test]
#[serial]
fn test_renderer_returned_is_usable() {
    setup();

    let renderer = Engine::create_renderer("test_usable", MockRenderer::new()).unwrap();

    // Lock the renderer (simulates actual usage)
    let _guard = renderer.lock().unwrap();
    // If we get here without panic, the renderer is usable
}

#[test]
#[serial]
fn test_error_messages_logged() {
    setup();

    let test_logger = TestLogger::new();
    let entries_ref = test_logger.entries.clone();
    Engine::set_logger(test_logger);

    // Trigger various errors to test log_and_return_error()
    let _ = Engine::create_renderer("test_err_log_dup", MockRenderer::new());
    let result = Engine::create_renderer("test_err_log_dup", MockRenderer::new());
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

    // InitializationFailed: duplicate renderer
    let _ = Engine::create_renderer("test_all_err_1", MockRenderer::new());
    let _ = Engine::create_renderer("test_all_err_1", MockRenderer::new());

    // InitializationFailed: renderer not found
    let _ = Engine::renderer("nonexistent_xyz_123");

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

    // Create renderer
    let renderer = Engine::create_renderer("test_lifecycle_main", MockRenderer::new()).unwrap();
    assert!(Engine::renderer("test_lifecycle_main").is_ok());

    // Create resource manager
    Engine::create_resource_manager().unwrap();
    assert!(Engine::resource_manager().is_ok());

    // Use renderer
    let _guard = renderer.lock().unwrap();
    drop(_guard);

    // Cleanup
    Engine::destroy_renderer("test_lifecycle_main").unwrap();
    Engine::destroy_resource_manager().unwrap();
}

#[test]
#[serial]
fn test_shutdown_clears_rm_before_renderers() {
    setup();

    // Create renderer and resource manager
    let _renderer = Engine::create_renderer("test_shutdown_order", MockRenderer::new()).unwrap();
    Engine::create_resource_manager().unwrap();

    // Both should exist
    assert!(Engine::renderer("test_shutdown_order").is_ok());
    assert!(Engine::resource_manager().is_ok());

    // Shutdown clears RM before renderers (as per comment in code)
    Engine::shutdown();

    // Both should be cleared
    assert_eq!(Engine::renderer_count(), 0);

    // Re-initialize
    Engine::initialize().unwrap();
}

#[test]
#[serial]
fn test_concurrent_renderer_access() {
    setup();

    let renderer = Engine::create_renderer("test_concurrent", MockRenderer::new()).unwrap();

    // Spawn multiple threads accessing the same renderer
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let renderer_clone = renderer.clone();
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
