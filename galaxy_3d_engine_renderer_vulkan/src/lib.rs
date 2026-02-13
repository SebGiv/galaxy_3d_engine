/*!
# Galaxy 3D Engine - Vulkan Renderer Backend

Vulkan implementation of the Galaxy 3D rendering engine.

This crate provides a Vulkan backend that implements the galaxy_3d_engine traits
using the Ash library for Vulkan bindings and gpu-allocator for memory management.

The backend is registered as a plugin and can be selected at runtime.
*/

// Internal modules
mod vulkan;
mod vulkan_context;
mod vulkan_texture;
mod vulkan_buffer;
mod vulkan_shader;
mod vulkan_pipeline;
mod debug;
mod vulkan_command_list;
mod vulkan_render_target;
mod vulkan_render_pass;
mod vulkan_swapchain;
mod vulkan_descriptor_set;
mod vulkan_frame_buffer;

// Main galaxy3d namespace module
pub mod galaxy3d {
    // VulkanRenderer at root of galaxy3d
    pub use crate::vulkan::VulkanRenderer;

    // Vulkan sub-module with all implementations
    pub mod vulkan {
        pub use crate::vulkan_texture::Texture;
        pub use crate::vulkan_buffer::Buffer;
        pub use crate::vulkan_shader::Shader;
        pub use crate::vulkan_pipeline::Pipeline;
        pub use crate::vulkan_command_list::CommandList;
        pub use crate::vulkan_render_target::RenderTarget;
        pub use crate::vulkan_render_pass::RenderPass;
        pub use crate::vulkan_swapchain::Swapchain;
        pub use crate::vulkan_descriptor_set::DescriptorSet;
        pub use crate::vulkan_frame_buffer::Framebuffer;
    }

    // Debug sub-module
    pub mod debug {
        pub use crate::debug::*;
    }
}

/// Register the Vulkan backend with the plugin system
pub fn register() {
    // TODO: Implement plugin registration
    // galaxy_3d_engine::register_renderer_plugin("vulkan", create_vulkan_renderer);
}
