//! Unit tests for log.rs
//!
//! Tests Logger trait, LogEntry, LogSeverity, and DefaultLogger.

use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};
use std::time::SystemTime;

// ============================================================================
// LOG SEVERITY TESTS
// ============================================================================

#[test]
fn test_log_severity_ordering() {
    // Test PartialOrd implementation
    assert!(LogSeverity::Trace < LogSeverity::Debug);
    assert!(LogSeverity::Debug < LogSeverity::Info);
    assert!(LogSeverity::Info < LogSeverity::Warn);
    assert!(LogSeverity::Warn < LogSeverity::Error);
}

#[test]
fn test_log_severity_equality() {
    // Test PartialEq implementation
    assert_eq!(LogSeverity::Trace, LogSeverity::Trace);
    assert_eq!(LogSeverity::Debug, LogSeverity::Debug);
    assert_eq!(LogSeverity::Info, LogSeverity::Info);
    assert_eq!(LogSeverity::Warn, LogSeverity::Warn);
    assert_eq!(LogSeverity::Error, LogSeverity::Error);

    assert_ne!(LogSeverity::Trace, LogSeverity::Debug);
    assert_ne!(LogSeverity::Info, LogSeverity::Error);
}

#[test]
fn test_log_severity_clone() {
    let sev1 = LogSeverity::Error;
    let sev2 = sev1.clone();
    assert_eq!(sev1, sev2);
}

#[test]
fn test_log_severity_debug() {
    assert_eq!(format!("{:?}", LogSeverity::Trace), "Trace");
    assert_eq!(format!("{:?}", LogSeverity::Debug), "Debug");
    assert_eq!(format!("{:?}", LogSeverity::Info), "Info");
    assert_eq!(format!("{:?}", LogSeverity::Warn), "Warn");
    assert_eq!(format!("{:?}", LogSeverity::Error), "Error");
}

#[test]
fn test_log_severity_copy() {
    let sev1 = LogSeverity::Info;
    let sev2 = sev1; // Copy, not move
    assert_eq!(sev1, sev2);
    // Can still use sev1
    assert_eq!(sev1, LogSeverity::Info);
}

// ============================================================================
// LOG ENTRY TESTS
// ============================================================================

#[test]
fn test_log_entry_creation_without_file_line() {
    let entry = LogEntry {
        severity: LogSeverity::Info,
        timestamp: SystemTime::now(),
        source: "galaxy3d::Engine".to_string(),
        message: "Engine initialized".to_string(),
        file: None,
        line: None,
    };

    assert_eq!(entry.severity, LogSeverity::Info);
    assert_eq!(entry.source, "galaxy3d::Engine");
    assert_eq!(entry.message, "Engine initialized");
    assert!(entry.file.is_none());
    assert!(entry.line.is_none());
}

#[test]
fn test_log_entry_creation_with_file_line() {
    let entry = LogEntry {
        severity: LogSeverity::Error,
        timestamp: SystemTime::now(),
        source: "galaxy3d::vulkan".to_string(),
        message: "Vulkan error".to_string(),
        file: Some("vulkan.rs"),
        line: Some(42),
    };

    assert_eq!(entry.severity, LogSeverity::Error);
    assert_eq!(entry.source, "galaxy3d::vulkan");
    assert_eq!(entry.message, "Vulkan error");
    assert_eq!(entry.file, Some("vulkan.rs"));
    assert_eq!(entry.line, Some(42));
}

#[test]
fn test_log_entry_clone() {
    let entry1 = LogEntry {
        severity: LogSeverity::Warn,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "warning".to_string(),
        file: Some("test.rs"),
        line: Some(10),
    };

    let entry2 = entry1.clone();

    assert_eq!(entry1.severity, entry2.severity);
    assert_eq!(entry1.source, entry2.source);
    assert_eq!(entry1.message, entry2.message);
    assert_eq!(entry1.file, entry2.file);
    assert_eq!(entry1.line, entry2.line);
}

#[test]
fn test_log_entry_debug() {
    let entry = LogEntry {
        severity: LogSeverity::Debug,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "debug message".to_string(),
        file: None,
        line: None,
    };

    let debug_str = format!("{:?}", entry);
    assert!(debug_str.contains("Debug"));
    assert!(debug_str.contains("test"));
    assert!(debug_str.contains("debug message"));
}

// ============================================================================
// DEFAULT LOGGER TESTS
// ============================================================================

#[test]
fn test_default_logger_trace() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Trace,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "trace message".to_string(),
        file: None,
        line: None,
    };

    // Just verify it doesn't panic
    logger.log(&entry);
}

#[test]
fn test_default_logger_debug() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Debug,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "debug message".to_string(),
        file: None,
        line: None,
    };

    logger.log(&entry);
}

#[test]
fn test_default_logger_info() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Info,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "info message".to_string(),
        file: None,
        line: None,
    };

    logger.log(&entry);
}

#[test]
fn test_default_logger_warn() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Warn,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "warning message".to_string(),
        file: None,
        line: None,
    };

    logger.log(&entry);
}

#[test]
fn test_default_logger_error() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Error,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "error message".to_string(),
        file: None,
        line: None,
    };

    logger.log(&entry);
}

#[test]
fn test_default_logger_error_with_file_line() {
    let logger = DefaultLogger;
    let entry = LogEntry {
        severity: LogSeverity::Error,
        timestamp: SystemTime::now(),
        source: "galaxy3d::vulkan".to_string(),
        message: "Critical Vulkan error".to_string(),
        file: Some("vulkan.rs"),
        line: Some(123),
    };

    // Test the file:line branch
    logger.log(&entry);
}

#[test]
fn test_default_logger_all_severities_without_file_line() {
    let logger = DefaultLogger;
    let timestamp = SystemTime::now();

    for severity in [
        LogSeverity::Trace,
        LogSeverity::Debug,
        LogSeverity::Info,
        LogSeverity::Warn,
        LogSeverity::Error,
    ] {
        let entry = LogEntry {
            severity,
            timestamp,
            source: "test".to_string(),
            message: format!("{:?} message", severity),
            file: None,
            line: None,
        };
        logger.log(&entry);
    }
}

#[test]
fn test_default_logger_all_severities_with_file_line() {
    let logger = DefaultLogger;
    let timestamp = SystemTime::now();

    for severity in [
        LogSeverity::Trace,
        LogSeverity::Debug,
        LogSeverity::Info,
        LogSeverity::Warn,
        LogSeverity::Error,
    ] {
        let entry = LogEntry {
            severity,
            timestamp,
            source: "test".to_string(),
            message: format!("{:?} message with location", severity),
            file: Some("test.rs"),
            line: Some(42),
        };
        logger.log(&entry);
    }
}

// ============================================================================
// LOGGER TRAIT TESTS
// ============================================================================

struct TestLogger {
    logged_count: std::sync::Mutex<usize>,
}

impl TestLogger {
    fn new() -> Self {
        Self {
            logged_count: std::sync::Mutex::new(0),
        }
    }

    fn get_count(&self) -> usize {
        *self.logged_count.lock().unwrap()
    }
}

impl Logger for TestLogger {
    fn log(&self, _entry: &LogEntry) {
        let mut count = self.logged_count.lock().unwrap();
        *count += 1;
    }
}

#[test]
fn test_custom_logger_implementation() {
    let logger = TestLogger::new();
    assert_eq!(logger.get_count(), 0);

    let entry = LogEntry {
        severity: LogSeverity::Info,
        timestamp: SystemTime::now(),
        source: "test".to_string(),
        message: "test".to_string(),
        file: None,
        line: None,
    };

    logger.log(&entry);
    assert_eq!(logger.get_count(), 1);

    logger.log(&entry);
    assert_eq!(logger.get_count(), 2);
}

#[test]
fn test_logger_trait_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DefaultLogger>();
}

// ============================================================================
// TIMESTAMP TESTS
// ============================================================================

#[test]
fn test_log_entry_with_different_timestamps() {
    let time1 = SystemTime::now();
    let entry1 = LogEntry {
        severity: LogSeverity::Info,
        timestamp: time1,
        source: "test".to_string(),
        message: "first".to_string(),
        file: None,
        line: None,
    };

    std::thread::sleep(std::time::Duration::from_millis(10));

    let time2 = SystemTime::now();
    let entry2 = LogEntry {
        severity: LogSeverity::Info,
        timestamp: time2,
        source: "test".to_string(),
        message: "second".to_string(),
        file: None,
        line: None,
    };

    // time2 should be after time1
    assert!(entry2.timestamp > entry1.timestamp);
}
