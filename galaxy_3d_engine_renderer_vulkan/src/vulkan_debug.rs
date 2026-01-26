/// Vulkan Debug Messenger - Handles validation layer messages with colored output
///
/// This module provides a debug messenger callback for Vulkan validation layers
/// with support for colored console output, file logging, and break-on-error functionality.

use ash::vk;
use colored::*;
use galaxy_3d_engine::{DebugSeverity, DebugOutput};
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

/// Global debug configuration (shared across callbacks)
static DEBUG_CONFIG: Mutex<Option<DebugConfig>> = Mutex::new(None);

/// Debug configuration for the callback
#[derive(Clone)]
pub struct DebugConfig {
    pub severity: DebugSeverity,
    pub output: DebugOutput,
    pub break_on_error: bool,
}

/// Initialize debug configuration
pub fn init_debug_config(config: DebugConfig) {
    *DEBUG_CONFIG.lock().unwrap() = Some(config);
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
    let should_display = match config.severity {
        DebugSeverity::ErrorsOnly => {
            message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
        }
        DebugSeverity::ErrorsAndWarnings => {
            message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
                || message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING)
        }
        DebugSeverity::All => true,
    };

    if !should_display {
        return vk::FALSE;
    }

    // Determine severity level and color
    let (severity_str, severity_colored) =
        if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
            ("ERROR", "ERROR".red().bold())
        } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
            ("WARNING", "WARNING".yellow().bold())
        } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
            ("INFO", "INFO".cyan())
        } else {
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

    // Format output (console version with colors)
    let console_output = format!(
        "{} {} [{}]\n  ├─ {}: {}\n  └─ {}\n",
        "[VULKAN".bright_blue().bold(),
        format!("{}]", severity_colored).bright_blue().bold(),
        type_str.bright_black(),
        "Message ID".bright_black(),
        message_id_name.white(),
        message.white()
    );

    // Format output (file version without colors)
    let file_output = format!(
        "[VULKAN {}] [{}]\n  ├─ Message ID: {}\n  └─ {}\n",
        severity_str, type_str, message_id_name, message
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

    // Break on error if configured
    if config.break_on_error
        && message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
    {
        eprintln!(
            "\n{}\n",
            "⚠️  BREAK ON VALIDATION ERROR - Aborting execution"
                .red()
                .bold()
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
