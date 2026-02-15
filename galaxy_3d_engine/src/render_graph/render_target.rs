/// Render target edge in a render graph.
///
/// High-level description of a rendering surface that connects
/// passes in the DAG. References a specific view (layer + mip)
/// of a resource::Texture, and holds the resolved GPU render target.
///
/// A render target can be written by at most one pass (single writer)
/// and read by multiple passes (multiple readers).
///
/// Each target carries its own load/store/clear configuration via `TargetOps`,
/// auto-detected from the texture usage (color vs depth/stencil).

use std::sync::Arc;
use crate::error::Result;
use crate::renderer;
use crate::resource;

/// Per-target load/store/clear configuration.
///
/// Auto-detected from `TextureUsage` when the target is created.
/// Each variant provides sensible defaults that can be overridden
/// via `RenderGraph` setters.
#[derive(Debug, Clone, Copy)]
pub enum TargetOps {
    /// Configuration for color attachments
    Color {
        /// Clear color (RGBA), default: opaque black
        clear_color: [f32; 4],
        /// Load operation, default: Clear
        load_op: renderer::LoadOp,
        /// Store operation, default: Store
        store_op: renderer::StoreOp,
    },
    /// Configuration for depth/stencil attachments
    DepthStencil {
        /// Depth clear value, default: 1.0
        depth_clear: f32,
        /// Stencil clear value, default: 0
        stencil_clear: u32,
        /// Depth load operation, default: Clear
        depth_load_op: renderer::LoadOp,
        /// Depth store operation, default: DontCare
        depth_store_op: renderer::StoreOp,
        /// Stencil load operation, default: DontCare
        stencil_load_op: renderer::LoadOp,
        /// Stencil store operation, default: DontCare
        stencil_store_op: renderer::StoreOp,
    },
}

impl TargetOps {
    /// Default ops for a color target
    pub fn default_color() -> Self {
        Self::Color {
            clear_color: [0.0, 0.0, 0.0, 1.0],
            load_op: renderer::LoadOp::Clear,
            store_op: renderer::StoreOp::Store,
        }
    }

    /// Default ops for a depth/stencil target
    pub fn default_depth_stencil() -> Self {
        Self::DepthStencil {
            depth_clear: 1.0,
            stencil_clear: 0,
            depth_load_op: renderer::LoadOp::Clear,
            depth_store_op: renderer::StoreOp::DontCare,
            stencil_load_op: renderer::LoadOp::DontCare,
            stencil_store_op: renderer::StoreOp::DontCare,
        }
    }
}

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
    /// Per-target load/store/clear configuration
    ops: TargetOps,
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

        // Auto-detect ops from texture usage
        let usage = texture.renderer_texture().info().usage;
        let ops = match usage {
            renderer::TextureUsage::DepthStencil => TargetOps::default_depth_stencil(),
            _ => TargetOps::default_color(),
        };

        Ok(Self {
            texture,
            layer,
            mip_level,
            renderer_render_target,
            written_by: None,
            ops,
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

    /// Get the per-target ops configuration
    pub fn ops(&self) -> &TargetOps {
        &self.ops
    }

    /// Get a mutable reference to the per-target ops configuration
    pub(crate) fn ops_mut(&mut self) -> &mut TargetOps {
        &mut self.ops
    }

    /// Set the writer pass index
    pub(crate) fn set_written_by(&mut self, pass_id: usize) {
        self.written_by = Some(pass_id);
    }
}
