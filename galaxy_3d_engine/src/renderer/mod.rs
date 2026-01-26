/// Renderer module - all rendering-related types and traits

// Module declarations
pub mod renderer;
pub mod renderer_texture;
pub mod renderer_buffer;
pub mod renderer_shader;
pub mod renderer_pipeline;

// New architecture modules
pub mod renderer_command_list;
pub mod renderer_render_target;
pub mod renderer_render_pass;
pub mod renderer_swapchain;

// Re-export everything from renderer.rs
pub use renderer::*;

// Re-export from other modules
pub use renderer_texture::*;
pub use renderer_buffer::*;
pub use renderer_shader::*;
pub use renderer_pipeline::*;

// Re-export new architecture types
pub use renderer_command_list::*;
pub use renderer_render_target::*;
pub use renderer_render_pass::*;
pub use renderer_swapchain::*;
