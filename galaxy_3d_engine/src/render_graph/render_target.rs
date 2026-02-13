/// Render target edge in a render graph.
///
/// High-level description of a rendering surface that connects
/// passes in the DAG. This is a graph edge — not to be confused
/// with `renderer::RenderTarget` which is the low-level GPU render
/// target (image view + dimensions).
///
/// A render target can be written by at most one pass (single writer)
/// and read by multiple passes (multiple readers).

use std::sync::{Arc, Mutex};
use crate::renderer;
use crate::resource;

/// Describes which part of a resource texture to render to
pub struct TextureTargetView {
    /// The resource texture to render to
    pub texture: Arc<resource::Texture>,
    /// Array layer index (0 for simple textures)
    pub layer: u32,
    /// Mip level to render to (0 for full resolution)
    pub mip_level: u32,
}

/// What a render target points to
pub enum RenderTargetKind {
    /// Render to the swapchain (acquires next image at execution time)
    Swapchain(Arc<Mutex<dyn renderer::Swapchain>>),
    /// Render to a specific view of a resource texture
    Texture(TextureTargetView),
}

pub struct RenderTarget {
    /// What this target points to
    kind: RenderTargetKind,
    /// Pass index that writes to this target (at most one)
    written_by: Option<usize>,
}

impl RenderTarget {
    pub(crate) fn new(kind: RenderTargetKind) -> Self {
        Self {
            kind,
            written_by: None,
        }
    }

    /// Get the kind of this render target
    pub fn kind(&self) -> &RenderTargetKind {
        &self.kind
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
