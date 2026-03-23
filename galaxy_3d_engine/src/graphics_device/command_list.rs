/// CommandList trait - for recording rendering commands

use std::sync::Arc;
use crate::error::Result;
use crate::graphics_device::{
    RenderPass, Framebuffer, Pipeline, Buffer,
    BindingGroup, IndexType, ShaderStageFlags, ImageAccess,
    DynamicRenderState,
};

/// Command list for recording rendering commands
///
/// Commands are recorded and later submitted to the GPU via RendererDevice::submit()
pub trait CommandList: Send + Sync {
    /// Begin recording commands
    fn begin(&mut self) -> Result<()>;

    /// End recording commands
    fn end(&mut self) -> Result<()>;

    /// Begin a render pass
    ///
    /// The backend uses `accesses` to emit layout transitions and memory
    /// barriers internally before starting rendering (dynamic rendering).
    ///
    /// # Arguments
    ///
    /// * `render_pass` - The render pass to begin
    /// * `framebuffer` - The framebuffer containing color and depth/stencil attachments
    /// * `clear_values` - Clear values for attachments
    /// * `accesses` - Per-image access declarations for automatic barrier emission
    fn begin_render_pass(
        &mut self,
        render_pass: &Arc<dyn RenderPass>,
        framebuffer: &Arc<dyn Framebuffer>,
        clear_values: &[ClearValue],
        accesses: &[ImageAccess],
    ) -> Result<()>;

    /// End the current render pass
    fn end_render_pass(&mut self) -> Result<()>;

    /// Set the viewport
    ///
    /// # Arguments
    ///
    /// * `viewport` - Viewport dimensions and depth range
    fn set_viewport(&mut self, viewport: Viewport) -> Result<()>;

    /// Set the scissor rectangle
    ///
    /// # Arguments
    ///
    /// * `scissor` - Scissor rectangle
    fn set_scissor(&mut self, scissor: Rect2D) -> Result<()>;

    /// Bind a graphics pipeline
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline to bind
    fn bind_pipeline(&mut self, pipeline: &Arc<dyn Pipeline>) -> Result<()>;

    /// Bind a binding group to a pipeline slot
    ///
    /// Binding groups are immutable sets of GPU resource bindings (textures, buffers, samplers).
    /// This method binds a binding group at the given set index.
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline to bind the group to (needed to extract pipeline layout)
    /// * `set_index` - Set index (0 = per-frame, 1 = per-material, etc.)
    /// * `binding_group` - The binding group to bind
    fn bind_binding_group(
        &mut self,
        pipeline: &Arc<dyn Pipeline>,
        set_index: u32,
        binding_group: &Arc<dyn BindingGroup>,
    ) -> Result<()>;

    /// Push constants to the pipeline
    ///
    /// # Arguments
    ///
    /// * `stage_flags` - Shader stage flags that will access the push constants
    /// * `offset` - Offset in bytes into push constant range
    /// * `data` - Data to push
    fn push_constants(&mut self, stage_flags: ShaderStageFlags, offset: u32, data: &[u8]) -> Result<()>;

    /// Bind a vertex buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to bind
    /// * `offset` - Offset into the buffer in bytes
    fn bind_vertex_buffer(&mut self, buffer: &Arc<dyn Buffer>, offset: u64) -> Result<()>;

    /// Bind an index buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to bind
    /// * `offset` - Offset into the buffer in bytes
    /// * `index_type` - Type of indices (U16 or U32)
    fn bind_index_buffer(&mut self, buffer: &Arc<dyn Buffer>, offset: u64, index_type: IndexType) -> Result<()>;

    /// Draw vertices
    ///
    /// # Arguments
    ///
    /// * `vertex_count` - Number of vertices to draw
    /// * `first_vertex` - Index of first vertex
    fn draw(&mut self, vertex_count: u32, first_vertex: u32) -> Result<()>;

    /// Draw indexed vertices
    ///
    /// # Arguments
    ///
    /// * `index_count` - Number of indices to draw
    /// * `first_index` - Index of first index
    /// * `vertex_offset` - Value added to vertex index before indexing into the vertex buffer
    fn draw_indexed(&mut self, index_count: u32, first_index: u32, vertex_offset: i32) -> Result<()>;

    /// Set all dynamic pipeline states for the next draw call
    ///
    /// The backend translates this into the appropriate vkCmdSet* calls.
    /// All fields are set unconditionally — the driver handles redundancy filtering.
    ///
    /// # Arguments
    ///
    /// * `state` - The resolved dynamic render state for this draw call
    fn set_dynamic_state(&mut self, state: &DynamicRenderState) -> Result<()>;

}

/// Viewport dimensions and depth range
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
}

/// 2D rectangle
#[derive(Debug, Clone, Copy)]
pub struct Rect2D {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Clear value for an attachment
#[derive(Debug, Clone, Copy)]
pub enum ClearValue {
    /// Color clear value (RGBA)
    Color([f32; 4]),
    /// Depth/stencil clear value
    DepthStencil { depth: f32, stencil: u32 },
}
