//! Internal logging system for Galaxy3D Engine
//!
//! This module provides a flexible logging system with:
//! - Customizable logger via Logger trait
//! - Severity levels (Trace, Debug, Info, Warn, Error)
//! - Colored console output by default
//! - Thread-safe logging with RwLock
//! - File and line information for detailed ERROR logs

use colored::*;
use std::time::SystemTime;
use chrono::{DateTime, Local};

/// Logger trait for custom logging implementations
///
/// Implement this trait to create custom loggers (file logging, network logging, etc.)
///
/// # Example
///
/// ```no_run
/// use galaxy_3d_engine::galaxy3d::log::{Logger, LogEntry};
///
/// struct FileLogger {
///     file: std::fs::File,
/// }
///
/// impl Logger for FileLogger {
///     fn log(&self, entry: &LogEntry) {
///         // Write to file...
///     }
/// }
/// ```
pub trait Logger: Send + Sync {
    /// Log an entry
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to process
    fn log(&self, entry: &LogEntry);
}

/// Log entry containing all information about a log message
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Severity level (Trace, Debug, Info, Warn, Error)
    pub severity: LogSeverity,

    /// Timestamp when the log was created
    pub timestamp: SystemTime,

    /// Source module (e.g., "galaxy3d::Engine", "galaxy3d::vulkan::Texture")
    pub source: String,

    /// Log message
    pub message: String,

    /// Source file (only for detailed ERROR logs)
    pub file: Option<&'static str>,

    /// Source line (only for detailed ERROR logs)
    pub line: Option<u32>,
}

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogSeverity {
    /// Very verbose debug information (typically disabled in release)
    Trace,

    /// Development/debugging information
    Debug,

    /// Important informational messages
    Info,

    /// Warning messages (potential issues)
    Warn,

    /// Error messages (critical issues with file:line details)
    Error,
}

/// Default logger implementation using colored console output
///
/// Colors:
/// - Trace: Gris (bright_black)
/// - Debug: Cyan
/// - Info: Vert (green)
/// - Warn: Jaune (yellow)
/// - Error: Rouge gras (red + bold)
///
/// Format:
/// - Normal: `[timestamp] [SEVERITY] [source] message`
/// - Error: `[timestamp] [ERROR] [source] message (file:line)`
pub struct DefaultLogger;

impl Logger for DefaultLogger {
    fn log(&self, entry: &LogEntry) {
        // Format timestamp as YYYY-MM-DD HH:MM:SS.mmm
        let datetime: DateTime<Local> = entry.timestamp.into();
        let timestamp = datetime.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        // Color severity string
        let severity_str = match entry.severity {
            LogSeverity::Trace => "TRACE".bright_black(),
            LogSeverity::Debug => "DEBUG".cyan(),
            LogSeverity::Info => "INFO ".green(),
            LogSeverity::Warn => "WARN ".yellow(),
            LogSeverity::Error => "ERROR".red().bold(),
        };

        // Color source
        let source = entry.source.bright_blue();

        // Print with or without file:line
        if let (Some(file), Some(line)) = (entry.file, entry.line) {
            println!(
                "[{}] [{}] [{}] {} ({}:{})",
                timestamp,
                severity_str,
                source,
                entry.message,
                file,
                line
            );
        } else {
            println!(
                "[{}] [{}] [{}] {}",
                timestamp,
                severity_str,
                source,
                entry.message
            );
        }
    }
}

// ===== LOGGING MACROS =====

/// Log a TRACE message (very verbose, typically disabled)
///
/// # Example
///
/// ```no_run
/// engine_trace!("galaxy3d::Engine", "Entering function foo()");
/// ```
#[macro_export]
macro_rules! engine_trace {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log(
            $crate::galaxy3d::log::LogSeverity::Trace,
            $source,
            format!($($arg)*)
        )
    };
}

/// Log a DEBUG message (development information)
///
/// # Example
///
/// ```no_run
/// engine_debug!("galaxy3d::Engine", "Initialized with {} subsystems", count);
/// ```
#[macro_export]
macro_rules! engine_debug {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log(
            $crate::galaxy3d::log::LogSeverity::Debug,
            $source,
            format!($($arg)*)
        )
    };
}

/// Log an INFO message (important events)
///
/// # Example
///
/// ```no_run
/// engine_info!("galaxy3d::Engine", "Renderer initialized successfully");
/// ```
#[macro_export]
macro_rules! engine_info {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log(
            $crate::galaxy3d::log::LogSeverity::Info,
            $source,
            format!($($arg)*)
        )
    };
}

/// Log a WARN message (potential issues)
///
/// # Example
///
/// ```no_run
/// engine_warn!("galaxy3d::Engine", "Performance warning: {} FPS", fps);
/// ```
#[macro_export]
macro_rules! engine_warn {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log(
            $crate::galaxy3d::log::LogSeverity::Warn,
            $source,
            format!($($arg)*)
        )
    };
}

/// Log an ERROR message with file:line information
///
/// # Example
///
/// ```no_run
/// engine_error!("galaxy3d::Engine", "Failed to initialize: {}", error);
/// ```
#[macro_export]
macro_rules! engine_error {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log_detailed(
            $crate::galaxy3d::log::LogSeverity::Error,
            $source,
            format!($($arg)*),
            file!(),
            line!()
        )
    };
}
