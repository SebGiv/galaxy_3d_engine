/// Vulkan Debug Messenger - Handles validation layer messages with colored output
///
/// This module provides a debug messenger callback for Vulkan validation layers
/// with support for colored console output, file logging, and break-on-error functionality.

use ash::vk;
use colored::*;
use galaxy_3d_engine::galaxy3d::render::{DebugSeverity, DebugOutput, DebugMessageFilter, ValidationStats};
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

/// Global debug configuration (shared across callbacks)
static DEBUG_CONFIG: Mutex<Option<Config>> = Mutex::new(None);

/// Global validation statistics (thread-safe atomic counters)
static VALIDATION_STATS: ValidationStatsTracker = ValidationStatsTracker::new();

/// Global message tracker for grouping identical messages
static MESSAGE_TRACKER: Mutex<Option<MessageTracker>> = Mutex::new(None);

/// Debug configuration for the callback
#[derive(Clone)]
pub struct Config {
    pub severity: DebugSeverity,
    pub output: DebugOutput,
    pub message_filter: DebugMessageFilter,
    pub break_on_error: bool,
    pub panic_on_error: bool,
    pub enable_stats: bool,
}

/// Thread-safe validation statistics tracker
struct ValidationStatsTracker {
    errors: AtomicU32,
    warnings: AtomicU32,
    info: AtomicU32,
    verbose: AtomicU32,
}

impl ValidationStatsTracker {
    const fn new() -> Self {
        Self {
            errors: AtomicU32::new(0),
            warnings: AtomicU32::new(0),
            info: AtomicU32::new(0),
            verbose: AtomicU32::new(0),
        }
    }

    fn increment_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_warning(&self) {
        self.warnings.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_info(&self) {
        self.info.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_verbose(&self) {
        self.verbose.fetch_add(1, Ordering::Relaxed);
    }

    fn get_stats(&self) -> ValidationStats {
        ValidationStats {
            errors: self.errors.load(Ordering::Relaxed),
            warnings: self.warnings.load(Ordering::Relaxed),
            info: self.info.load(Ordering::Relaxed),
            verbose: self.verbose.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.errors.store(0, Ordering::Relaxed);
        self.warnings.store(0, Ordering::Relaxed);
        self.info.store(0, Ordering::Relaxed);
        self.verbose.store(0, Ordering::Relaxed);
    }
}

/// Message tracker for grouping identical messages
struct MessageTracker {
    messages: HashMap<String, u32>,
}

impl MessageTracker {
    fn track_message(&mut self, message: &str) -> u32 {
        let count = self.messages.entry(message.to_string()).or_insert(0);
        *count += 1;
        *count
    }
}

/// Initialize debug configuration
pub fn init_debug_config(config: Config) {
    // Reset statistics when initializing
    VALIDATION_STATS.reset();

    // Reset message tracker
    *MESSAGE_TRACKER.lock().unwrap() = Some(MessageTracker {
        messages: HashMap::new(),
    });

    *DEBUG_CONFIG.lock().unwrap() = Some(config);
}

/// Get current validation statistics
pub fn get_validation_stats() -> ValidationStats {
    VALIDATION_STATS.get_stats()
}

/// Print validation statistics report
pub fn print_validation_stats_report() {
    let stats = get_validation_stats();

    if stats.total() == 0 {
        println!("\n{}", "✓ No validation messages".green().bold());
        return;
    }

    println!("\n{}", "=== Validation Statistics Report ===".bright_blue().bold());

    if stats.errors > 0 {
        println!("  {} {}", "Errors:".red().bold(), stats.errors);
    }
    if stats.warnings > 0 {
        println!("  {} {}", "Warnings:".yellow().bold(), stats.warnings);
    }
    if stats.info > 0 {
        println!("  {} {}", "Info:".cyan(), stats.info);
    }
    if stats.verbose > 0 {
        println!("  {} {}", "Verbose:".bright_black(), stats.verbose);
    }

    println!("  {} {}", "Total:".white().bold(), stats.total());

    // Print message grouping info
    let tracker_guard = MESSAGE_TRACKER.lock().unwrap();
    if let Some(tracker) = tracker_guard.as_ref() {
        let duplicate_count: u32 = tracker.messages.values().filter(|&&count| count > 1).count() as u32;

        if duplicate_count > 0 {
            println!("\n  {} {} message(s) appeared multiple times",
                "ℹ".cyan(),
                duplicate_count
            );
        }
    }

    println!("{}\n", "====================================".bright_blue().bold());
}

/// Vulkan debug messenger callback
///
/// This function is called by Vulkan validation layers when they detect issues.
/// It formats and outputs messages with colors and optional file logging.
pub unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    // Get callback data
    let callback_data = *p_callback_data;
    let message_id_name = if callback_data.p_message_id_name.is_null() {
        "Unknown"
    } else {
        CStr::from_ptr(callback_data.p_message_id_name)
            .to_str()
            .unwrap_or("Invalid UTF-8")
    };
    let message = if callback_data.p_message.is_null() {
        "No message"
    } else {
        CStr::from_ptr(callback_data.p_message)
            .to_str()
            .unwrap_or("Invalid UTF-8")
    };

    // Get config
    let config_guard = DEBUG_CONFIG.lock().unwrap();
    let config = match config_guard.as_ref() {
        Some(cfg) => cfg.clone(),
        None => return vk::FALSE, // No config, ignore
    };
    drop(config_guard);

    // Check severity filter
    let should_display_severity = match config.severity {
        DebugSeverity::ErrorsOnly => {
            message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
        }
        DebugSeverity::ErrorsAndWarnings => {
            message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
                || message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING)
        }
        DebugSeverity::All => true,
    };

    if !should_display_severity {
        return vk::FALSE;
    }

    // Check category filter
    let should_display_category = if message_type.contains(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION) {
        config.message_filter.show_validation
    } else if message_type.contains(vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE) {
        config.message_filter.show_performance
    } else {
        config.message_filter.show_general
    };

    if !should_display_category {
        return vk::FALSE;
    }

    // Determine severity level and color, increment statistics
    let (severity_str, severity_colored) =
        if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            if config.enable_stats {
                VALIDATION_STATS.increment_error();
            }
            ("ERROR", "ERROR".red().bold())
        } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
            if config.enable_stats {
                VALIDATION_STATS.increment_warning();
            }
            ("WARNING", "WARNING".yellow().bold())
        } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
            if config.enable_stats {
                VALIDATION_STATS.increment_info();
            }
            ("INFO", "INFO".cyan())
        } else {
            if config.enable_stats {
                VALIDATION_STATS.increment_verbose();
            }
            ("VERBOSE", "VERBOSE".bright_black())
        };

    // Determine message type
    let type_str = if message_type.contains(vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION) {
        "Validation"
    } else if message_type.contains(vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE) {
        "Performance"
    } else {
        "General"
    };

    // Track message for grouping
    let occurrence_count = if config.enable_stats {
        let mut tracker_guard = MESSAGE_TRACKER.lock().unwrap();
        if let Some(tracker) = tracker_guard.as_mut() {
            tracker.track_message(message)
        } else {
            // Initialize tracker if not done yet
            *tracker_guard = Some(MessageTracker {
                messages: HashMap::new(),
            });
            tracker_guard.as_mut().unwrap().track_message(message)
        }
    } else {
        1
    };

    // Add repetition indicator if message appeared before
    let repeat_indicator = if occurrence_count > 1 {
        format!(" [×{}]", occurrence_count)
    } else {
        String::new()
    };

    // Format output (console version with colors)
    let console_output = format!(
        "{} {} [{}]{}\n  ├─ {}: {}\n  └─ {}\n",
        "[VULKAN".bright_blue().bold(),
        format!("{}]", severity_colored).bright_blue().bold(),
        type_str.bright_black(),
        repeat_indicator.yellow(),
        "Message ID".bright_black(),
        message_id_name.white(),
        message.white()
    );

    // Format output (file version without colors)
    let file_output = format!(
        "[VULKAN {}] [{}]{}\n  ├─ Message ID: {}\n  └─ {}\n",
        severity_str, type_str, repeat_indicator, message_id_name, message
    );

    // Output to console and/or file
    match &config.output {
        DebugOutput::Console => {
            eprint!("{}", console_output);
        }
        DebugOutput::File(path) => {
            write_to_file(path, &file_output);
        }
        DebugOutput::Both(path) => {
            eprint!("{}", console_output);
            write_to_file(path, &file_output);
        }
    }

    // Panic on any error if strict mode enabled
    if config.panic_on_error
        && message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
    {
        panic!(
            "\n⚠️  PANIC ON ERROR (Strict Mode)\n\
            Message ID: {}\n\
            Type: {}\n\
            Message: {}\n",
            message_id_name, type_str, message
        );
    }

    // Break on error if configured (for debugger attachment)
    if config.break_on_error
        && message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
    {
        eprintln!(
            "\n{}\n  Context: {} [{}]\n  Message: {}\n",
            "⚠️  BREAK ON VALIDATION ERROR - Aborting execution"
                .red()
                .bold(),
            message_id_name.yellow(),
            type_str.cyan(),
            message.white()
        );
        std::process::abort();
    }

    vk::FALSE // Don't abort Vulkan execution
}

/// Write message to log file
fn write_to_file(path: &str, message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", message);
    }
}
