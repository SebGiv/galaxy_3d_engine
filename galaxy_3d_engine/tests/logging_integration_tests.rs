//! Integration tests for Engine logging system
//!
//! These tests verify the logging system functionality.
//! No GPU required.
//!
//! Run with: cargo test --test logging_integration_tests

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::log::{Logger, LogEntry, LogSeverity};
use std::sync::{Arc, Mutex};
use serial_test::serial;

// ============================================================================
// TEST LOGGER IMPLEMENTATION
// ============================================================================

/// Test logger that captures log entries for verification
struct TestLogger {
    entries: Arc<Mutex<Vec<LogEntry>>>,
}

impl TestLogger {
    fn new() -> (Self, Arc<Mutex<Vec<LogEntry>>>) {
        let entries = Arc::new(Mutex::new(Vec::new()));
        (Self { entries: entries.clone() }, entries)
    }
}

impl Logger for TestLogger {
    fn log(&self, entry: &LogEntry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push(LogEntry {
            severity: entry.severity,
            timestamp: entry.timestamp,
            source: entry.source.clone(),
            message: entry.message.clone(),
            file: entry.file,
            line: entry.line,
        });
    }
}

// ============================================================================
// LOGGING TESTS
// ============================================================================

#[test]
#[serial]
fn test_integration_custom_logger() {
    // Create test logger
    let (test_logger, entries) = TestLogger::new();

    // Set custom logger
    Engine::set_logger(test_logger);

    // Log some messages
    Engine::log(LogSeverity::Info, "test::module", "Test info message".to_string());
    Engine::log(LogSeverity::Warn, "test::module", "Test warning message".to_string());
    Engine::log(LogSeverity::Error, "test::module", "Test error message".to_string());

    // Verify logs were captured
    let captured_entries = entries.lock().unwrap();
    assert_eq!(captured_entries.len(), 3);

    // Verify first log (Info)
    assert_eq!(captured_entries[0].severity, LogSeverity::Info);
    assert_eq!(captured_entries[0].source, "test::module");
    assert_eq!(captured_entries[0].message, "Test info message");

    // Verify second log (Warn)
    assert_eq!(captured_entries[1].severity, LogSeverity::Warn);
    assert_eq!(captured_entries[1].source, "test::module");
    assert_eq!(captured_entries[1].message, "Test warning message");

    // Verify third log (Error)
    assert_eq!(captured_entries[2].severity, LogSeverity::Error);
    assert_eq!(captured_entries[2].source, "test::module");
    assert_eq!(captured_entries[2].message, "Test error message");

    // Reset to default logger
    Engine::reset_logger();
}

#[test]
#[serial]
fn test_integration_error_logging_with_location() {
    // Create test logger
    let (test_logger, entries) = TestLogger::new();

    // Set custom logger
    Engine::set_logger(test_logger);

    // Log error with file and line information
    Engine::log_detailed(
        LogSeverity::Error,
        "test::error",
        "Critical error occurred".to_string(),
        "test_file.rs",
        42,
    );

    // Verify log was captured with location
    let captured_entries = entries.lock().unwrap();
    assert_eq!(captured_entries.len(), 1);

    let entry = &captured_entries[0];
    assert_eq!(entry.severity, LogSeverity::Error);
    assert_eq!(entry.source, "test::error");
    assert_eq!(entry.message, "Critical error occurred");
    assert_eq!(entry.file, Some("test_file.rs"));
    assert_eq!(entry.line, Some(42));

    // Reset to default logger
    Engine::reset_logger();
}

#[test]
#[serial]
fn test_integration_logger_reset() {
    // Create test logger
    let (test_logger, entries) = TestLogger::new();

    // Set custom logger
    Engine::set_logger(test_logger);

    // Log a message
    Engine::log(LogSeverity::Info, "test", "Message 1".to_string());

    // Verify log was captured
    {
        let captured = entries.lock().unwrap();
        assert_eq!(captured.len(), 1);
    }

    // Reset to default logger
    Engine::reset_logger();

    // Log another message (will go to default logger, not captured)
    Engine::log(LogSeverity::Info, "test", "Message 2".to_string());

    // Verify no new logs in test logger
    let captured = entries.lock().unwrap();
    assert_eq!(captured.len(), 1); // Still only one message
}

#[test]
#[serial]
fn test_integration_logging_different_severities() {
    // Create test logger
    let (test_logger, entries) = TestLogger::new();

    // Set custom logger
    Engine::set_logger(test_logger);

    // Log messages with all severity levels
    Engine::log(LogSeverity::Trace, "test", "Trace message".to_string());
    Engine::log(LogSeverity::Debug, "test", "Debug message".to_string());
    Engine::log(LogSeverity::Info, "test", "Info message".to_string());
    Engine::log(LogSeverity::Warn, "test", "Warn message".to_string());
    Engine::log(LogSeverity::Error, "test", "Error message".to_string());

    // Verify all severities were captured
    let captured_entries = entries.lock().unwrap();
    assert_eq!(captured_entries.len(), 5);

    assert_eq!(captured_entries[0].severity, LogSeverity::Trace);
    assert_eq!(captured_entries[1].severity, LogSeverity::Debug);
    assert_eq!(captured_entries[2].severity, LogSeverity::Info);
    assert_eq!(captured_entries[3].severity, LogSeverity::Warn);
    assert_eq!(captured_entries[4].severity, LogSeverity::Error);

    // Reset to default logger
    Engine::reset_logger();
}
