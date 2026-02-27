/// Graphics device module - all rendering-related types and traits

// Module declarations
pub mod graphics_device;
pub mod texture;
pub mod buffer;
pub mod shader;
pub mod pipeline;

// New architecture modules
pub mod command_list;
pub mod render_target;
pub mod render_pass;
pub mod swapchain;
pub mod binding_group;
pub mod frame_buffer;

// Re-export everything from graphics_device.rs
pub use graphics_device::*;

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
pub use binding_group::*;
pub use frame_buffer::*;

// Mock graphics device for tests (no GPU required)
#[cfg(test)]
pub mod mock_graphics_device;
