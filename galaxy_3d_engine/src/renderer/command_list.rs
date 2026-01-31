/// RendererCommandList trait - for recording rendering commands

use std::sync::Arc;
use crate::Galaxy3dResult;
use crate::renderer::{
    RendererRenderPass, RendererRenderTarget, RendererPipeline, RendererBuffer,
    RendererDescriptorSet,
};

/// Command list for recording rendering commands
///
/// Commands are recorded and later submitted to the GPU via RendererDevice::submit()
pub trait RendererCommandList: Send + Sync {
    /// Begin recording commands
    fn begin(&mut self) -> Galaxy3dResult<()>;

    /// End recording commands
    fn end(&mut self) -> Galaxy3dResult<()>;

    /// Begin a render pass
    ///
    /// # Arguments
    ///
    /// * `render_pass` - The render pass to begin
    /// * `render_target` - The target to render to
    /// * `clear_values` - Clear values for attachments
    fn begin_render_pass(
        &mut self,
        render_pass: &Arc<dyn RendererRenderPass>,
        render_target: &Arc<dyn RendererRenderTarget>,
        clear_values: &[ClearValue]
    ) -> Galaxy3dResult<()>;

    /// End the current render pass
    fn end_render_pass(&mut self) -> Galaxy3dResult<()>;

    /// Set the viewport
    ///
    /// # Arguments
    ///
    /// * `viewport` - Viewport dimensions and depth range
    fn set_viewport(&mut self, viewport: Viewport) -> Galaxy3dResult<()>;

    /// Set the scissor rectangle
    ///
    /// # Arguments
    ///
    /// * `scissor` - Scissor rectangle
    fn set_scissor(&mut self, scissor: Rect2D) -> Galaxy3dResult<()>;

    /// Bind a graphics pipeline
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline to bind
    fn bind_pipeline(&mut self, pipeline: &Arc<dyn RendererPipeline>) -> Galaxy3dResult<()>;

    /// Bind descriptor sets (for textures, uniform buffers, etc.)
    ///
    /// Descriptor sets group together resources that shaders can access.
    /// This method binds descriptor sets to the currently bound pipeline.
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline to bind descriptor sets to (needed to extract pipeline layout)
    /// * `descriptor_sets` - Slice of descriptor sets to bind
    fn bind_descriptor_sets(
        &mut self,
        pipeline: &Arc<dyn RendererPipeline>,
        descriptor_sets: &[&Arc<dyn RendererDescriptorSet>],
    ) -> Galaxy3dResult<()>;

    /// Push constants to the pipeline
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset in bytes into push constant range
    /// * `data` - Data to push
    fn push_constants(&mut self, offset: u32, data: &[u8]) -> Galaxy3dResult<()>;

    /// Bind a vertex buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to bind
    /// * `offset` - Offset into the buffer in bytes
    fn bind_vertex_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> Galaxy3dResult<()>;

    /// Bind an index buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to bind
    /// * `offset` - Offset into the buffer in bytes
    fn bind_index_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> Galaxy3dResult<()>;

    /// Draw vertices
    ///
    /// # Arguments
    ///
    /// * `vertex_count` - Number of vertices to draw
    /// * `first_vertex` - Index of first vertex
    fn draw(&mut self, vertex_count: u32, first_vertex: u32) -> Galaxy3dResult<()>;

    /// Draw indexed vertices
    ///
    /// # Arguments
    ///
    /// * `index_count` - Number of indices to draw
    /// * `first_index` - Index of first index
    /// * `vertex_offset` - Value added to vertex index before indexing into the vertex buffer
    fn draw_indexed(&mut self, index_count: u32, first_index: u32, vertex_offset: i32) -> Galaxy3dResult<()>;
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
