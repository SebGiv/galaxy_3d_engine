/// Render graph resource — a typed reference to a resource that the graph
/// can read or write.
///
/// Holds a `TextureKey` (with the mip / layer subset to view) or a
/// `BufferKey` into the central `ResourceManager`, not an `Arc<...>`. The
/// actual resource is resolved at execute time. This keeps the graph
/// independent from resource lifetimes and lets a pass swap its target at
/// runtime by changing only the key.

use crate::resource::resource_manager::{TextureKey, BufferKey};

slotmap::new_key_type! {
    /// Stable key for a `GraphResource` in the `RenderGraphManager`.
    pub struct GraphResourceKey;
}

/// Typed reference to a resource used by the render graph.
///
/// `Hash`/`Eq` are derived so a `GraphResource` can sit inside a
/// `FramebufferLookupKey` or any other cache key — equality means the
/// referenced resource AND the same view (mip / layer subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphResource {
    /// References a `Texture` in the `ResourceManager`. The fields below
    /// describe the subresource range targeted when this resource is used
    /// as a framebuffer attachment (mirrors `FramebufferAttachment`).
    Texture {
        texture_key: TextureKey,
        /// Mip level to render into / sample from (0 = base level).
        base_mip_level: u32,
        /// First array layer (0 = base layer).
        base_array_layer: u32,
        /// Number of consecutive layers. Must be >= 1.
        layer_count: u32,
    },
    /// References a `Buffer` in the `ResourceManager`.
    Buffer(BufferKey),
}

impl GraphResource {
    /// True if this resource is a texture.
    pub fn is_texture(self) -> bool {
        matches!(self, GraphResource::Texture { .. })
    }

    /// True if this resource is a buffer.
    pub fn is_buffer(self) -> bool {
        matches!(self, GraphResource::Buffer(_))
    }
}
