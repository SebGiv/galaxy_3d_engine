/*!
# Galaxy 3D Engine - Vulkan Renderer Backend

Vulkan implementation of the Galaxy 3D rendering engine.

This crate provides a Vulkan backend that implements the galaxy_3d_engine traits
using the Ash library for Vulkan bindings and gpu-allocator for memory management.

The backend is registered as a plugin and can be selected at runtime.
*/

// Vulkan implementation modules
mod vulkan;
mod vulkan_texture;
mod vulkan_buffer;
mod vulkan_shader;
mod vulkan_pipeline;
mod debug;

// New architecture modules
mod vulkan_command_list;
mod vulkan_render_target;
mod vulkan_render_pass;
mod vulkan_swapchain;
mod vulkan_descriptor_set;

pub use vulkan::VulkanRenderer;
pub use vulkan_swapchain::VulkanRendererSwapchain;
pub use vulkan_pipeline::VulkanRendererPipeline;
pub use vulkan_command_list::VulkanRendererCommandList;
pub use vulkan_texture::VulkanRendererTexture;

// Re-export debug utilities
pub use debug::{get_validation_stats, print_validation_stats_report};

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
