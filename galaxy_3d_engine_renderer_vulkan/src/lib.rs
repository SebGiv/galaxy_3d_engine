/*!
# Galaxy 3D Engine - Vulkan Renderer Backend

Vulkan implementation of the Galaxy 3D rendering engine.

This crate provides a Vulkan backend that implements the galaxy_3d_engine traits
using the Ash library for Vulkan bindings and gpu-allocator for memory management.

The backend is registered as a plugin and can be selected at runtime.
*/

// Vulkan implementation modules
mod vulkan_renderer;
mod vulkan_renderer_texture;
mod vulkan_renderer_buffer;
mod vulkan_renderer_shader;
mod vulkan_renderer_pipeline;
mod vulkan_renderer_frame;

use galaxy_3d_engine::{Renderer, RendererConfig, RenderResult};
use std::sync::{Arc, Mutex};
use winit::window::Window;

pub use vulkan_renderer::VulkanRenderer;

/// Register the Vulkan backend with the plugin system
///
/// This function should be called during application initialization to make
/// the Vulkan backend available for use.
///
/// # Example
///
/// ```no_run
/// galaxy_3d_engine_renderer_vulkan::register();
/// ```
pub fn register() {
    galaxy_3d_engine::register_renderer_plugin("vulkan", create_vulkan_renderer);
}

/// Factory function to create a Vulkan renderer instance
///
/// # Arguments
///
/// * `window` - Window to render to
/// * `config` - Renderer configuration
///
/// # Returns
///
/// A shared, thread-safe Vulkan renderer instance
fn create_vulkan_renderer(
    window: &Window,
    config: RendererConfig,
) -> RenderResult<Arc<Mutex<dyn Renderer>>> {
    let renderer = VulkanRenderer::new(window, config)?;
    Ok(Arc::new(Mutex::new(renderer)))
}
