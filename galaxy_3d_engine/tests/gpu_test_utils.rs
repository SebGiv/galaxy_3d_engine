#![allow(dead_code)]
//! GPU test utilities - Shared Vulkan graphics_device for integration tests
//!
//! This module provides a global VulkanGraphicsDevice instance shared across all GPU tests.
//! This avoids the `RecreationAttempt` error from ash-window when creating multiple
//! Vulkan surfaces in the same process.
//!
//! # Why a global graphics_device?
//!
//! On Windows, `ash-window::create_surface()` detects multiple surface creation attempts
//! and returns `RecreationAttempt`. By sharing a single VulkanGraphicsDevice across all tests,
//! we avoid this issue and more closely simulate real-world usage (1 graphics_device per app).

use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanGraphicsDevice;
use std::sync::{Arc, Mutex, OnceLock};
use winit::event_loop::{EventLoop, EventLoopBuilder};
use winit::window::Window;

// Platform-specific imports for EventLoop threading
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;

/// Global VulkanGraphicsDevice instance (initialized once)
static GPU_GRAPHICS_DEVICE: OnceLock<Arc<Mutex<VulkanGraphicsDevice>>> = OnceLock::new();

/// Global Window (kept alive for the graphics_device)
/// Note: EventLoop is intentionally leaked with mem::forget to keep Window valid
static GPU_WINDOW: OnceLock<Window> = OnceLock::new();

/// Get the shared VulkanGraphicsDevice for GPU tests
///
/// Lazily initializes the graphics_device on first call. All subsequent calls
/// return a clone of the same Arc<Mutex<VulkanGraphicsDevice>>.
///
/// Note: EventLoop is intentionally leaked with mem::forget to keep Window valid.
/// This is necessary because EventLoop cannot be stored in a static (not Sync).
///
/// # Returns
///
/// A shared reference to the VulkanGraphicsDevice wrapped in Arc<Mutex<_>>
///
/// # Example
///
/// ```no_run
/// let graphics_device = get_test_graphics_device();
/// let mut guard = graphics_device.lock().unwrap();
/// let cmd_list = guard.create_command_list().unwrap();
/// ```
pub fn get_test_graphics_device() -> Arc<Mutex<VulkanGraphicsDevice>> {
    GPU_GRAPHICS_DEVICE
        .get_or_init(|| {
            // Create window once
            let (window, event_loop) = create_test_window();

            // Create VulkanGraphicsDevice once
            let graphics_device = VulkanGraphicsDevice::new(&window, Config::default())
                .expect("Failed to create VulkanGraphicsDevice for tests");

            // Leak EventLoop intentionally to keep Window valid
            // This is a test-only workaround for EventLoop not being Sync
            std::mem::forget(event_loop);

            // Store window to keep it alive
            GPU_WINDOW.set(window).ok();

            Arc::new(Mutex::new(graphics_device))
        })
        .clone()
}

/// Create a test window for Vulkan
///
/// Creates a hidden window with EventLoop that supports any_thread on Windows.
/// Use this when you need to create a new VulkanGraphicsDevice instance (e.g., for Engine tests).
#[allow(deprecated)]
pub fn create_test_window() -> (Window, EventLoop<()>) {
    // Create EventLoop with any_thread support for Windows
    // This allows EventLoop creation outside the main thread (required for cargo test)
    let event_loop = {
        #[cfg(target_os = "windows")]
        {
            EventLoopBuilder::new()
                .with_any_thread(true)
                .build()
                .unwrap()
        }
        #[cfg(not(target_os = "windows"))]
        {
            EventLoopBuilder::new().build().unwrap()
        }
    };

    let window_attrs = Window::default_attributes()
        .with_title("GPU Test Window")
        .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
        .with_visible(false); // Hidden window for tests

    let window = event_loop.create_window(window_attrs).unwrap();
    (window, event_loop)
}
