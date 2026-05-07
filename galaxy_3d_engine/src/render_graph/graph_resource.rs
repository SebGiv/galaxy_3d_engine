/// Render graph resource — a typed reference to a resource that the graph
/// can read or write.
///
/// Holds a `TextureKey` (with the mip / layer subset to view) or a
/// `BufferKey` (with the byte subrange) into the central `ResourceManager`,
/// not an `Arc<...>`. The actual resource is resolved at execute time. This
/// keeps the graph independent from resource lifetimes and lets a pass
/// swap its target at runtime by changing only the key.

use crate::resource::resource_manager::{TextureKey, BufferKey};

slotmap::new_key_type! {
    /// Stable key for a `GraphResource` in the `RenderGraphManager`.
    pub struct GraphResourceKey;
}

/// Sentinel value for `mip_count` meaning "all remaining mips from
/// `base_mip_level`". Maps to `vk::REMAINING_MIP_LEVELS` in the Vulkan
/// backend.
pub const REMAINING_MIP_LEVELS: u32 = u32::MAX;

/// Sentinel value for `layer_count` meaning "all remaining layers from
/// `base_array_layer`". Maps to `vk::REMAINING_ARRAY_LAYERS` in the Vulkan
/// backend.
pub const REMAINING_ARRAY_LAYERS: u32 = u32::MAX;

/// Sentinel value for buffer `size` meaning "from `offset` to end of
/// buffer". Maps to `vk::WHOLE_SIZE` in the Vulkan backend.
pub const WHOLE_SIZE: u64 = u64::MAX;

/// View into a texture's mip / layer subresources, without the
/// `TextureKey`. Used internally by the render graph to track per-frame
/// access history at a finer grain than the `GraphResource` itself.
///
/// `mip_count == REMAINING_MIP_LEVELS` and
/// `layer_count == REMAINING_ARRAY_LAYERS` are accepted as sentinels
/// meaning "all remaining mips / layers from the base index".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSubRange {
    pub base_mip_level: u32,
    pub mip_count: u32,
    pub base_array_layer: u32,
    pub layer_count: u32,
}

impl ImageSubRange {
    /// Test whether two ranges overlap on both mip AND layer axes.
    /// Sentinel `REMAINING_*` values (= u32::MAX) extend the upper
    /// bound to infinity (saturating arithmetic prevents wraparound).
    pub fn overlaps(&self, other: &ImageSubRange) -> bool {
        let a_mip_end = if self.mip_count == REMAINING_MIP_LEVELS {
            u32::MAX
        } else {
            self.base_mip_level.saturating_add(self.mip_count)
        };
        let b_mip_end = if other.mip_count == REMAINING_MIP_LEVELS {
            u32::MAX
        } else {
            other.base_mip_level.saturating_add(other.mip_count)
        };
        let mips_overlap =
            self.base_mip_level < b_mip_end && other.base_mip_level < a_mip_end;

        let a_layer_end = if self.layer_count == REMAINING_ARRAY_LAYERS {
            u32::MAX
        } else {
            self.base_array_layer.saturating_add(self.layer_count)
        };
        let b_layer_end = if other.layer_count == REMAINING_ARRAY_LAYERS {
            u32::MAX
        } else {
            other.base_array_layer.saturating_add(other.layer_count)
        };
        let layers_overlap =
            self.base_array_layer < b_layer_end && other.base_array_layer < a_layer_end;

        mips_overlap && layers_overlap
    }
}

/// View into a buffer's byte subrange. Used internally by the render
/// graph to track per-frame access history.
///
/// `size == WHOLE_SIZE` is accepted as a sentinel meaning "from offset
/// to end of buffer".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferSubRange {
    pub offset: u64,
    pub size: u64,
}

impl BufferSubRange {
    /// Test whether two byte ranges overlap. Sentinel `WHOLE_SIZE`
    /// (= u64::MAX) extends the upper bound to infinity (saturating
    /// arithmetic prevents wraparound).
    pub fn overlaps(&self, other: &BufferSubRange) -> bool {
        let a_end = if self.size == WHOLE_SIZE {
            u64::MAX
        } else {
            self.offset.saturating_add(self.size)
        };
        let b_end = if other.size == WHOLE_SIZE {
            u64::MAX
        } else {
            other.offset.saturating_add(other.size)
        };
        self.offset < b_end && other.offset < a_end
    }
}

/// Typed reference to a resource used by the render graph.
///
/// `Hash`/`Eq` are derived so a `GraphResource` can sit inside a
/// `FramebufferLookupKey` or any other cache key — equality means the
/// referenced resource AND the same view (mip / layer / byte subrange).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphResource {
    /// References a `Texture` in the `ResourceManager`. The fields below
    /// describe the subresource range targeted by this resource. For
    /// framebuffer attachments, `mip_count` MUST be `1` (Vulkan does not
    /// allow rendering into multiple mip levels of an attachment
    /// simultaneously). For sampled / storage usages, `mip_count` may be
    /// any value `>= 1` or the sentinel `REMAINING_MIP_LEVELS`.
    Texture {
        texture_key: TextureKey,
        /// Base mip level (0 = largest mip).
        base_mip_level: u32,
        /// Number of consecutive mip levels covered by this view. Must be
        /// `>= 1` or `REMAINING_MIP_LEVELS`.
        mip_count: u32,
        /// First array layer (0 = base layer).
        base_array_layer: u32,
        /// Number of consecutive array layers. Must be `>= 1` or
        /// `REMAINING_ARRAY_LAYERS`.
        layer_count: u32,
    },
    /// References a `Buffer` in the `ResourceManager` over a byte
    /// subrange. `offset` is in bytes from the start of the buffer; `size`
    /// is the number of bytes covered or the sentinel `WHOLE_SIZE` to
    /// reach the end of the buffer.
    Buffer {
        buffer_key: BufferKey,
        /// Byte offset from the start of the buffer.
        offset: u64,
        /// Size in bytes covered by this view. Must be `>= 1` or
        /// `WHOLE_SIZE`.
        size: u64,
    },
}

impl GraphResource {
    /// Attachment-style texture: one mip, one or more layers.
    /// Sets `mip_count = 1` (the only value Vulkan allows for attachments).
    pub fn texture_attachment(
        texture_key: TextureKey,
        mip: u32,
        base_array_layer: u32,
        layer_count: u32,
    ) -> Self {
        Self::Texture {
            texture_key,
            base_mip_level: mip,
            mip_count: 1,
            base_array_layer,
            layer_count,
        }
    }

    /// Whole-texture access: all mips, all layers. Useful for sampled
    /// reads that span the full mip-chain.
    pub fn texture_full(texture_key: TextureKey) -> Self {
        Self::Texture {
            texture_key,
            base_mip_level: 0,
            mip_count: REMAINING_MIP_LEVELS,
            base_array_layer: 0,
            layer_count: REMAINING_ARRAY_LAYERS,
        }
    }

    /// Whole-buffer access: offset 0, size = `WHOLE_SIZE`.
    pub fn buffer_full(buffer_key: BufferKey) -> Self {
        Self::Buffer {
            buffer_key,
            offset: 0,
            size: WHOLE_SIZE,
        }
    }

    /// Buffer subrange.
    pub fn buffer_range(buffer_key: BufferKey, offset: u64, size: u64) -> Self {
        Self::Buffer { buffer_key, offset, size }
    }

    /// True if this resource is a texture.
    pub fn is_texture(self) -> bool {
        matches!(self, GraphResource::Texture { .. })
    }

    /// True if this resource is a buffer.
    pub fn is_buffer(self) -> bool {
        matches!(self, GraphResource::Buffer { .. })
    }
}

#[cfg(test)]
#[path = "graph_resource_tests.rs"]
mod tests;
