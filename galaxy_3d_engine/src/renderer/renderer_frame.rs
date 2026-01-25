/// RendererFrame trait - represents a rendering frame

use std::sync::Arc;
use crate::renderer::{RenderResult, RendererPipeline, RendererBuffer};

/// Frame resource trait
///
/// Returned by Renderer::begin_frame(), used to record rendering commands.
/// Commands are executed when the frame is passed to Renderer::end_frame().
pub trait RendererFrame: Send + Sync {
    /// Bind a graphics pipeline for rendering
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline to bind
    fn bind_pipeline(&self, pipeline: &Arc<dyn RendererPipeline>) -> RenderResult<()>;

    /// Bind a vertex buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to bind
    /// * `offset` - Offset into the buffer in bytes
    fn bind_vertex_buffer(&self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> RenderResult<()>;

    /// Draw vertices
    ///
    /// # Arguments
    ///
    /// * `vertex_count` - Number of vertices to draw
    /// * `first_vertex` - Index of first vertex
    fn draw(&self, vertex_count: u32, first_vertex: u32) -> RenderResult<()>;
}
