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
mod vulkan_debug;

// New architecture modules
mod vulkan_renderer_command_list;
mod vulkan_renderer_render_target;
mod vulkan_renderer_render_pass;
mod vulkan_renderer_swapchain;
mod vulkan_renderer_descriptor_set;

use galaxy_3d_engine::{RendererConfig, RenderResult};
use std::sync::{Arc, Mutex};
use winit::window::Window;

pub use vulkan_renderer::VulkanRenderer;
pub use vulkan_renderer_swapchain::VulkanRendererSwapchain;
pub use vulkan_renderer_pipeline::VulkanRendererPipeline;
pub use vulkan_renderer_command_list::VulkanRendererCommandList;
pub use vulkan_renderer_texture::VulkanRendererTexture;

// Re-export debug utilities
pub use vulkan_debug::{get_validation_stats, print_validation_stats_report};

/// Register the Vulkan backend with the plugin system
///
/// # Example
///
/// ```no_run
/// use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;
/// use galaxy_3d_engine::RendererConfig;
/// use winit::window::Window;
///
/// // Create renderer directly
/// let renderer = VulkanRenderer::new(&window, RendererConfig::default())?;
/// ```
pub fn register() {
    // TODO: Implement plugin registration
    // galaxy_3d_engine::register_renderer_plugin("vulkan", create_vulkan_renderer);
}
