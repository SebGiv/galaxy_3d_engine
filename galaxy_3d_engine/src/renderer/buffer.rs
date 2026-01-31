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

/// Descriptor for creating a buffer
#[derive(Debug, Clone)]
pub struct BufferDesc {
    /// Size in bytes
    pub size: u64,
    /// Buffer usage
    pub usage: BufferUsage,
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
}
