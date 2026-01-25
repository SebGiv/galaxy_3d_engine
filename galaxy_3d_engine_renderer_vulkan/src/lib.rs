/*!
# Galaxy 3D Engine - Vulkan Renderer Backend

Vulkan implementation of the Galaxy 3D rendering engine.

This crate provides a Vulkan backend that implements the galaxy_3d_engine traits
using the Ash library for Vulkan bindings and gpu-allocator for memory management.

The backend is registered as a plugin and can be selected at runtime.
*/

// Vulkan implementation modules
// mod vulkan_renderer; // OLD - commented out, using VulkanRendererDevice now
mod vulkan_renderer_texture;
mod vulkan_renderer_buffer;
mod vulkan_renderer_shader;
mod vulkan_renderer_pipeline;

// New architecture modules
mod vulkan_renderer_device;
mod vulkan_renderer_command_list;
mod vulkan_renderer_render_target;
mod vulkan_renderer_render_pass;
mod vulkan_renderer_swapchain;

use galaxy_3d_engine::{RendererConfig, RenderResult};
use std::sync::{Arc, Mutex};
use winit::window::Window;

pub use vulkan_renderer_device::VulkanRendererDevice;
pub use vulkan_renderer_swapchain::VulkanRendererSwapchain;

/// Register the Vulkan backend with the plugin system
///
/// # Note
///
/// This function is temporarily disabled during refactoring.
/// The new architecture uses VulkanRendererDevice instead of the old Renderer trait.
///
/// # Example
///
/// ```no_run
/// // OLD: galaxy_3d_engine_renderer_vulkan::register();
/// // NEW: Use VulkanRendererDevice directly
/// use galaxy_3d_engine_renderer_vulkan::VulkanRendererDevice;
/// ```
pub fn register() {
    // TODO: Update plugin system to support RendererDevice
    // galaxy_3d_engine::register_renderer_plugin("vulkan", create_vulkan_renderer);
}
