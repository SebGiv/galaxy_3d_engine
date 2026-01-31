/// VulkanRendererBuffer - Vulkan implementation of RendererBuffer trait

use galaxy_3d_engine::{RendererBuffer, Galaxy3dResult, Galaxy3dError};
use ash::vk;
use gpu_allocator::vulkan::Allocation;

/// Vulkan buffer implementation
pub struct VulkanRendererBuffer {
    /// Vulkan buffer
    pub(crate) buffer: vk::Buffer,
    /// GPU memory allocation
    pub(crate) allocation: Option<Allocation>,
    /// Buffer size
    pub(crate) size: u64,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
    /// GPU allocator (for cleanup)
    pub(crate) allocator: std::sync::Arc<std::sync::Mutex<gpu_allocator::vulkan::Allocator>>,
}

impl RendererBuffer for VulkanRendererBuffer {
    fn update(&self, offset: u64, data: &[u8]) -> Galaxy3dResult<()> {
        unsafe {
            if let Some(allocation) = &self.allocation {
                // Map memory and copy data
                let mapped_ptr = allocation
                    .mapped_ptr()
                    .ok_or_else(|| Galaxy3dError::BackendError("Buffer is not CPU-accessible".to_string()))?
                    .as_ptr() as *mut u8;

                // Copy data
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.offset(offset as isize),
                    data.len(),
                );

                Ok(())
            } else {
                Err(Galaxy3dError::BackendError("Buffer has no allocation".to_string()))
            }
        }
    }
}

impl Drop for VulkanRendererBuffer {
    fn drop(&mut self) {
        unsafe {
            // Free GPU memory
            if let Some(allocation) = self.allocation.take() {
                // Don't panic if lock fails - we still need to destroy the buffer
                if let Ok(mut allocator) = self.allocator.lock() {
                    allocator.free(allocation).ok();
                }
            }

            // Destroy buffer
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
