/// Render target edge in a render graph.
///
/// High-level description of a rendering surface that connects
/// passes in the DAG. References a specific view (layer + mip)
/// of a resource::Texture, and holds the resolved GPU render target.
///
/// A render target can be written by at most one pass (single writer)
/// and read by multiple passes (multiple readers).

use std::sync::Arc;
use crate::error::Result;
use crate::renderer;
use crate::resource;

pub struct RenderTarget {
    /// The resource texture this target references
    texture: Arc<resource::Texture>,
    /// Array layer index (0 for simple textures)
    layer: u32,
    /// Mip level (0 for full resolution)
    mip_level: u32,
    /// Resolved GPU render target (image view targeting this layer/mip)
    renderer_render_target: Arc<dyn renderer::RenderTarget>,
    /// Pass index that writes to this target (at most one)
    written_by: Option<usize>,
}

impl RenderTarget {
    pub(crate) fn new(
        texture: Arc<resource::Texture>,
        layer: u32,
        mip_level: u32,
        renderer: &dyn renderer::Renderer,
    ) -> Result<Self> {
        let renderer_render_target = renderer.create_render_target_texture(
            texture.renderer_texture().as_ref(),
            layer,
            mip_level,
        )?;
        Ok(Self {
            texture,
            layer,
            mip_level,
            renderer_render_target,
            written_by: None,
        })
    }

    /// Get the resource texture this target references
    pub fn texture(&self) -> &Arc<resource::Texture> {
        &self.texture
    }

    /// Get the array layer index
    pub fn layer(&self) -> u32 {
        self.layer
    }

    /// Get the mip level
    pub fn mip_level(&self) -> u32 {
        self.mip_level
    }

    /// Get the resolved GPU render target
    pub fn renderer_render_target(&self) -> &Arc<dyn renderer::RenderTarget> {
        &self.renderer_render_target
    }

    /// Get the pass index that writes to this target
    pub fn written_by(&self) -> Option<usize> {
        self.written_by
    }

    /// Set the writer pass index
    pub(crate) fn set_written_by(&mut self, pass_id: usize) {
        self.written_by = Some(pass_id);
    }
}
