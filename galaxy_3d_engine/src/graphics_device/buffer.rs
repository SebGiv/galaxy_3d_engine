/// Buffer trait and buffer descriptor

use crate::error::Result;

/// Buffer usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    /// Vertex buffer
    Vertex,
    /// Index buffer
    Index,
    /// Uniform/constant buffer
    Uniform,
    /// Storage buffer
    Storage,
}

/// How a buffer's data is updated over its lifetime.
///
/// Orthogonal to `BufferUsage`: any `BufferUsage` can be either `Static`
/// (loaded once) or `Dynamic` (rewritten by the CPU each frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BufferUpdateMode {
    /// Written once at creation/loading, not modified after.
    /// Backed by a single GPU buffer.
    #[default]
    Static,
    /// Rewritten by the CPU each frame.
    /// Backed by a single VkBuffer of size `FRAMES_IN_FLIGHT × size`,
    /// accessed via dynamic offsets at descriptor binding time (UBO/SSBO)
    /// or via direct offset (VB/IB). The backend handles slot rotation
    /// transparently — the user always sees a buffer of `size` bytes.
    Dynamic,
}

/// Descriptor for creating a buffer
#[derive(Debug, Clone)]
pub struct BufferDesc {
    /// Size in bytes
    pub size: u64,
    /// Buffer usage
    pub usage: BufferUsage,
    /// Update mode (defaults to `Static` for backwards compatibility)
    pub update_mode: BufferUpdateMode,
}

/// Buffer data format for vertex attributes and indices
///
/// Defines the data type and component count for buffer elements.
/// Used for vertex attributes (position, normal, UV, etc.) and index types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum BufferFormat {
    // Float formats (vertex attributes)
    R32_SFLOAT,         // float (4 bytes)
    R32G32_SFLOAT,      // vec2 (8 bytes)
    R32G32B32_SFLOAT,   // vec3 (12 bytes)
    R32G32B32A32_SFLOAT, // vec4 (16 bytes)

    // Integer formats (signed)
    R32_SINT,
    R32G32_SINT,
    R32G32B32_SINT,
    R32G32B32A32_SINT,

    // Integer formats (unsigned)
    R32_UINT,
    R32G32_UINT,
    R32G32B32_UINT,
    R32G32B32A32_UINT,

    // Short formats (signed)
    R16_SINT,
    R16G16_SINT,
    R16G16B16A16_SINT,

    // Short formats (unsigned)
    R16_UINT,
    R16G16_UINT,
    R16G16B16A16_UINT,

    // Byte formats (signed)
    R8_SINT,
    R8G8_SINT,
    R8G8B8A8_SINT,

    // Byte formats (unsigned)
    R8_UINT,
    R8G8_UINT,
    R8G8B8A8_UINT,
}

impl BufferFormat {
    /// Returns size in bytes for this format
    pub fn size_bytes(&self) -> u32 {
        match self {
            // Float formats
            BufferFormat::R32_SFLOAT | BufferFormat::R32_SINT | BufferFormat::R32_UINT => 4,
            BufferFormat::R32G32_SFLOAT | BufferFormat::R32G32_SINT | BufferFormat::R32G32_UINT => 8,
            BufferFormat::R32G32B32_SFLOAT | BufferFormat::R32G32B32_SINT | BufferFormat::R32G32B32_UINT => 12,
            BufferFormat::R32G32B32A32_SFLOAT | BufferFormat::R32G32B32A32_SINT | BufferFormat::R32G32B32A32_UINT => 16,

            // Short formats
            BufferFormat::R16_SINT | BufferFormat::R16_UINT => 2,
            BufferFormat::R16G16_SINT | BufferFormat::R16G16_UINT => 4,
            BufferFormat::R16G16B16A16_SINT | BufferFormat::R16G16B16A16_UINT => 8,

            // Byte formats
            BufferFormat::R8_SINT | BufferFormat::R8_UINT => 1,
            BufferFormat::R8G8_SINT | BufferFormat::R8G8_UINT => 2,
            BufferFormat::R8G8B8A8_SINT | BufferFormat::R8G8B8A8_UINT => 4,
        }
    }
}

/// Buffer resource trait
///
/// Implemented by backend-specific buffer types (e.g., VulkanBuffer).
/// The buffer is automatically destroyed when dropped.
pub trait Buffer: Send + Sync {
    /// Update buffer data
    ///
    /// # Arguments
    ///
    /// * `offset` - Offset into the buffer in bytes
    /// * `data` - Data to write
    fn update(&self, offset: u64, data: &[u8]) -> Result<()>;

    /// Raw pointer to persistently mapped memory
    ///
    /// Returns None if the buffer is not CPU-accessible (device-local only).
    /// The pointer remains valid for the lifetime of the buffer.
    fn mapped_ptr(&self) -> Option<*mut u8>;

    /// How this buffer is updated over its lifetime.
    ///
    /// Used by the render graph to derive the matching `BindingType`
    /// (`*Buffer` vs `*BufferDynamic`) automatically when wiring a
    /// binding group. Backends return whatever `BufferUpdateMode` was
    /// requested at creation time.
    fn update_mode(&self) -> BufferUpdateMode;
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
