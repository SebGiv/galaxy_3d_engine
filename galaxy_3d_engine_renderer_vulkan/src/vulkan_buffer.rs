/// Buffer - Vulkan implementation of RendererBuffer trait

use galaxy_3d_engine::galaxy3d::{
    Result,
    render::Buffer as RendererBuffer,
};
use galaxy_3d_engine::{engine_bail, engine_err};
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
    #[allow(dead_code)]
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
                    .ok_or_else(|| engine_err!("galaxy3d::vulkan", "Buffer update failed: buffer is not CPU-accessible"))?
                    .as_ptr() as *mut u8;

                // Copy data
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.offset(offset as isize),
                    data.len(),
                );

                Ok(())
            } else {
                engine_bail!("galaxy3d::vulkan", "Buffer update failed: no GPU allocation");
            }
        }
    }

    fn mapped_ptr(&self) -> Option<*mut u8> {
        self.allocation.as_ref()
            .and_then(|alloc| alloc.mapped_ptr())
            .map(|ptr| ptr.as_ptr() as *mut u8)
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