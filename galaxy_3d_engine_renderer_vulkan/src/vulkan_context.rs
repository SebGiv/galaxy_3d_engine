/// GpuContext - Shared GPU resources for all Vulkan objects
///
/// Contains everything needed for GPU operations:
/// - Device for Vulkan API calls
/// - Allocator for memory management
/// - Queue for command submission
/// - Command pool for one-shot upload operations

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use std::sync::{Arc, Mutex};

/// Shared GPU context for all Vulkan resources.
///
/// This struct is shared (via `Arc`) by all GPU resources (textures, buffers, etc.)
/// to avoid duplicating device/allocator/queue references in each resource.
pub struct GpuContext {
    /// Vulkan logical device
    pub device: ash::Device,

    /// GPU memory allocator (shared, requires mutex for thread safety)
    pub allocator: Arc<Mutex<Allocator>>,

    /// Graphics queue for command submission
    pub graphics_queue: vk::Queue,

    /// Graphics queue family index
    pub graphics_queue_family: u32,

    /// Reusable command pool for one-shot upload operations
    /// (created with TRANSIENT + RESET_COMMAND_BUFFER flags)
    pub upload_command_pool: Mutex<vk::CommandPool>,
}

impl GpuContext {
    /// Create a new GPU context
    ///
    /// # Arguments
    ///
    /// * `device` - Vulkan logical device
    /// * `allocator` - GPU memory allocator
    /// * `graphics_queue` - Graphics queue for command submission
    /// * `graphics_queue_family` - Graphics queue family index
    /// * `upload_command_pool` - Command pool for upload operations
    pub fn new(
        device: ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        graphics_queue: vk::Queue,
        graphics_queue_family: u32,
        upload_command_pool: vk::CommandPool,
    ) -> Self {
        Self {
            device,
            allocator,
            graphics_queue,
            graphics_queue_family,
            upload_command_pool: Mutex::new(upload_command_pool),
        }
    }
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        unsafe {
            // Destroy the upload command pool
            let pool = *self.upload_command_pool.lock().unwrap();
            if pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(pool, None);
            }
        }
    }
}
