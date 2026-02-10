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

// ===== LOGGING MACROS (INTERNAL USE ONLY) =====

/// Log a TRACE message (very verbose, typically disabled)
///
/// **INTERNAL USE ONLY** - This macro is for engine internals.
/// Users should implement the `Logger` trait and use their own logging system.
#[doc(hidden)]
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
/// **INTERNAL USE ONLY** - This macro is for engine internals.
/// Users should implement the `Logger` trait and use their own logging system.
#[doc(hidden)]
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
/// **INTERNAL USE ONLY** - This macro is for engine internals.
/// Users should implement the `Logger` trait and use their own logging system.
#[doc(hidden)]
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
/// **INTERNAL USE ONLY** - This macro is for engine internals.
/// Users should implement the `Logger` trait and use their own logging system.
#[doc(hidden)]
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
/// **INTERNAL USE ONLY** - This macro is for engine internals.
/// Users should implement the `Logger` trait and use their own logging system.
#[doc(hidden)]
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

// ===== ERROR SHORTCUT MACROS =====
//
// These macros combine logging + error creation to prevent forgetting logs.
// - engine_bail!      → engine_error! + return Err(BackendError)
// - engine_bail_warn! → engine_warn!  + return Err(BackendError)
// - engine_err!       → engine_error! + Error::BackendError (value, for closures)
// - engine_warn_err!  → engine_warn!  + Error::BackendError (value, for closures)

/// Log an ERROR and immediately return Err(BackendError)
///
/// Combines `engine_error!` + `return Err(Error::BackendError(...))` in one call.
/// Eliminates message duplication and makes it impossible to forget the log.
#[doc(hidden)]
#[macro_export]
macro_rules! engine_bail {
    ($source:expr, $($arg:tt)*) => {{
        $crate::engine_error!($source, $($arg)*);
        return Err($crate::galaxy3d::Error::BackendError(format!($($arg)*)));
    }};
}

/// Log a WARN and immediately return Err(BackendError)
///
/// Combines `engine_warn!` + `return Err(Error::BackendError(...))` in one call.
#[doc(hidden)]
#[macro_export]
macro_rules! engine_bail_warn {
    ($source:expr, $($arg:tt)*) => {{
        $crate::engine_warn!($source, $($arg)*);
        return Err($crate::galaxy3d::Error::BackendError(format!($($arg)*)));
    }};
}

/// Log an ERROR and return Error::BackendError as a value (for closures)
///
/// Use in `.ok_or_else(|| engine_err!(...))` or `.map_err(|e| engine_err!(...))`.
#[doc(hidden)]
#[macro_export]
macro_rules! engine_err {
    ($source:expr, $($arg:tt)*) => {{
        $crate::engine_error!($source, $($arg)*);
        $crate::galaxy3d::Error::BackendError(format!($($arg)*))
    }};
}

/// Log a WARN and return Error::BackendError as a value (for closures)
///
/// Use in `.ok_or_else(|| engine_warn_err!(...))` or `.map_err(|e| engine_warn_err!(...))`.
#[doc(hidden)]
#[macro_export]
macro_rules! engine_warn_err {
    ($source:expr, $($arg:tt)*) => {{
        $crate::engine_warn!($source, $($arg)*);
        $crate::galaxy3d::Error::BackendError(format!($($arg)*))
    }};
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
