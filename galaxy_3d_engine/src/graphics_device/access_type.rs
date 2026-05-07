/// GPU resource access types and image access descriptors.
///
/// AccessType is a semantic enum describing how a pass uses a resource.
/// The backend maps these to the appropriate pipeline stages, access masks,
/// and image layouts internally — no barrier concepts are exposed to the engine.

use std::sync::Arc;
use super::texture::Texture;
use super::buffer::Buffer;

/// How a pass accesses a resource.
///
/// Semantic descriptor — the backend maps this to the appropriate
/// pipeline stages, access masks, and image layouts internally.
/// Inspired by Frostbite Frame Graph, Unreal Engine 5 RDG, and Granite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// Color attachment write (render pass output)
    ColorAttachmentWrite,
    /// Color attachment read (e.g. blending with existing content)
    ColorAttachmentRead,
    /// Depth/stencil write
    DepthStencilWrite,
    /// Depth/stencil read-only (e.g. depth testing without writing)
    DepthStencilReadOnly,
    /// Fragment shader sampling (texture read)
    FragmentShaderRead,
    /// Vertex shader sampling (e.g. displacement maps)
    VertexShaderRead,
    /// Compute shader read (storage buffer / image)
    ComputeRead,
    /// Compute shader write (storage buffer / image)
    ComputeWrite,
    /// Transfer source (copy, blit)
    TransferRead,
    /// Transfer destination (copy, blit)
    TransferWrite,
    /// Ray tracing acceleration structure read
    RayTracingRead,
}

impl AccessType {
    /// Returns true if this access type writes to the resource.
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::ColorAttachmentWrite
                | Self::DepthStencilWrite
                | Self::ComputeWrite
                | Self::TransferWrite
        )
    }

    /// Returns true if this access type is a color or depth/stencil attachment
    /// (i.e. participates in a render pass as an attachment).
    pub fn is_attachment(self) -> bool {
        matches!(
            self,
            Self::ColorAttachmentWrite
                | Self::ColorAttachmentRead
                | Self::DepthStencilWrite
                | Self::DepthStencilReadOnly
        )
    }
}

/// Per-image access declaration for a render pass.
///
/// Passed to `begin_render_pass()` so the backend can emit
/// layout transitions and memory barriers internally.
///
/// `previous_access_type` is precalculated by the render graph during
/// `compile()` — the backend uses it to determine the source layout
/// and pipeline stage for barrier emission.
///
/// The `(base_mip_level, mip_count, base_array_layer, layer_count)`
/// fields describe the subresource view targeted by this access. They
/// are populated from the originating `GraphResource::Texture` and are
/// reserved for future barrier-granularity work; the current backend
/// emits barriers covering the whole image.
pub struct ImageAccess {
    /// The GPU texture being accessed
    pub texture: Arc<dyn Texture>,
    /// How this texture is accessed in the pass
    pub access_type: AccessType,
    /// How this texture was accessed previously (None = first use)
    pub previous_access_type: Option<AccessType>,
    /// Base mip level of the subresource view.
    pub base_mip_level: u32,
    /// Number of mip levels covered (`u32::MAX` = `REMAINING_MIP_LEVELS`).
    pub mip_count: u32,
    /// Base array layer of the subresource view.
    pub base_array_layer: u32,
    /// Number of array layers covered (`u32::MAX` = `REMAINING_ARRAY_LAYERS`).
    pub layer_count: u32,
}

/// Per-buffer access declaration for a render pass.
///
/// Counterpart to `ImageAccess` for SSBO/UBO/vertex/index buffers
/// that need an execution + memory barrier between passes (e.g. a
/// compute pass writing a buffer that a fragment pass reads).
///
/// Passed to `begin_render_pass()` alongside `ImageAccess`es —
/// the backend merges them into a single `vkCmdPipelineBarrier2`.
///
/// The `(offset, size)` fields describe the byte subrange targeted by
/// this access. They are populated from the originating
/// `GraphResource::Buffer` and are reserved for future
/// barrier-granularity work; the current backend emits barriers
/// covering the whole buffer.
pub struct BufferAccess {
    /// The GPU buffer being accessed
    pub buffer: Arc<dyn Buffer>,
    /// How this buffer is accessed in the pass
    pub access_type: AccessType,
    /// How this buffer was accessed previously (None = first use)
    pub previous_access_type: Option<AccessType>,
    /// Byte offset of the subrange.
    pub offset: u64,
    /// Number of bytes covered (`u64::MAX` = `WHOLE_SIZE`).
    pub size: u64,
}

#[cfg(test)]
#[path = "access_type_tests.rs"]
mod tests;
