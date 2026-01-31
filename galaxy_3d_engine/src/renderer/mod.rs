/// Renderer module - all rendering-related types and traits

// Module declarations
pub mod renderer;
pub mod texture;
pub mod buffer;
pub mod shader;
pub mod pipeline;

// New architecture modules
pub mod command_list;
pub mod render_target;
pub mod render_pass;
pub mod swapchain;
pub mod descriptor_set;

// Re-export everything from renderer.rs
pub use renderer::*;

// Re-export from other modules
pub use texture::*;
pub use buffer::*;
pub use shader::*;
pub use pipeline::*;

// Re-export new architecture types
pub use command_list::*;
pub use render_target::*;
pub use render_pass::*;
pub use swapchain::*;
pub use descriptor_set::*;
