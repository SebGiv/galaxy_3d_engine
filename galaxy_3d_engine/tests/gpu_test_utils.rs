//! GPU test utilities - Shared Vulkan renderer for integration tests
//!
//! This module provides a global VulkanRenderer instance shared across all GPU tests.
//! This avoids the `RecreationAttempt` error from ash-window when creating multiple
//! Vulkan surfaces in the same process.
//!
//! # Why a global renderer?
//!
//! On Windows, `ash-window::create_surface()` detects multiple surface creation attempts
//! and returns `RecreationAttempt`. By sharing a single VulkanRenderer across all tests,
//! we avoid this issue and more closely simulate real-world usage (1 renderer per app).

use galaxy_3d_engine::galaxy3d::render::Config;
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;
use std::sync::{Arc, Mutex, OnceLock};
use winit::event_loop::{EventLoop, EventLoopBuilder};
use winit::window::Window;

// Platform-specific imports for EventLoop threading
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;

/// Global VulkanRenderer instance (initialized once)
static GPU_RENDERER: OnceLock<Arc<Mutex<VulkanRenderer>>> = OnceLock::new();

/// Global Window (kept alive for the renderer)
/// Note: EventLoop is intentionally leaked with mem::forget to keep Window valid
static GPU_WINDOW: OnceLock<Window> = OnceLock::new();

/// Get the shared VulkanRenderer for GPU tests
///
/// Lazily initializes the renderer on first call. All subsequent calls
/// return a clone of the same Arc<Mutex<VulkanRenderer>>.
///
/// Note: EventLoop is intentionally leaked with mem::forget to keep Window valid.
/// This is necessary because EventLoop cannot be stored in a static (not Sync).
///
/// # Returns
///
/// A shared reference to the VulkanRenderer wrapped in Arc<Mutex<_>>
///
/// # Example
///
/// ```no_run
/// let renderer = get_test_renderer();
/// let mut guard = renderer.lock().unwrap();
/// let cmd_list = guard.create_command_list().unwrap();
/// ```
pub fn get_test_renderer() -> Arc<Mutex<VulkanRenderer>> {
    GPU_RENDERER
        .get_or_init(|| {
            // Create window once
            let (window, event_loop) = create_test_window();

            // Create VulkanRenderer once
            let renderer = VulkanRenderer::new(&window, Config::default())
                .expect("Failed to create VulkanRenderer for tests");

            // Leak EventLoop intentionally to keep Window valid
            // This is a test-only workaround for EventLoop not being Sync
            std::mem::forget(event_loop);

            // Store window to keep it alive
            GPU_WINDOW.set(window).ok();

            Arc::new(Mutex::new(renderer))
        })
        .clone()
}

/// Create a test window for Vulkan
///
/// Creates a hidden window with EventLoop that supports any_thread on Windows.
/// Use this when you need to create a new VulkanRenderer instance (e.g., for Engine tests).
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
