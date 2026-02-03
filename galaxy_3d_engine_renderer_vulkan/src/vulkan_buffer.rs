/// Buffer - Vulkan implementation of RendererBuffer trait

use galaxy_3d_engine::galaxy3d::{
    Result,
    Error,
    render::Buffer as RendererBuffer,
};
use galaxy_3d_engine::engine_error;
use ash::vk;
use gpu_allocator::vulkan::Allocation;
use std::sync::Arc;

use crate::vulkan_context::GpuContext;

/// Vulkan buffer implementation
pub struct Buffer {
    /// Shared GPU context (device, allocator, queue, command pool)
    ctx: Arc<GpuContext>,
    /// Vulkan buffer
    pub(crate) buffer: vk::Buffer,
    /// GPU memory allocation
    pub(crate) allocation: Option<Allocation>,
    /// Buffer size
    pub(crate) size: u64,
}

impl Buffer {
    /// Create a new Vulkan buffer
    pub fn new(
        ctx: Arc<GpuContext>,
        buffer: vk::Buffer,
        allocation: Allocation,
        size: u64,
    ) -> Self {
        Self {
            ctx,
            buffer,
            allocation: Some(allocation),
            size,
        }
    }
}

impl RendererBuffer for Buffer {
    fn update(&self, offset: u64, data: &[u8]) -> Result<()> {
        unsafe {
            if let Some(allocation) = &self.allocation {
                // Map memory and copy data
                let mapped_ptr = allocation
                    .mapped_ptr()
                    .ok_or_else(|| Error::BackendError("Buffer is not CPU-accessible".to_string()))?
                    .as_ptr() as *mut u8;

                // Copy data
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.offset(offset as isize),
                    data.len(),
                );

                Ok(())
            } else {
                engine_error!("galaxy3d::vulkan", "Buffer update failed: no GPU allocation");
                Err(Error::BackendError("Buffer has no allocation".to_string()))
            }
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            // Free GPU memory
            if let Some(allocation) = self.allocation.take() {
                // Don't panic if lock fails - we still need to destroy the buffer
                if let Ok(mut allocator) = self.ctx.allocator.lock() {
                    allocator.free(allocation).ok();
                }
            }

            // Destroy buffer
            self.ctx.device.destroy_buffer(self.buffer, None);
        }
    }
}
