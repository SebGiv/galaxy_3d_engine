/// GpuContext - Shared GPU resources for all Vulkan objects
///
/// Contains everything needed for GPU operations:
/// - Device for Vulkan API calls
/// - Allocator for memory management
/// - Queue for command submission
/// - Command pool for one-shot upload operations

use ash::vk;
use gpu_allocator::vulkan::Allocator;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};

/// Shared GPU context for all Vulkan resources.
///
/// This struct is shared (via `Arc`) by all GPU resources (textures, buffers, etc.)
/// to avoid duplicating device/allocator/queue references in each resource.
///
/// Note: Device and instance destruction is handled by VulkanRenderer::drop()
/// to avoid issues with drop ordering and callback exceptions on Windows.
pub struct GpuContext {
    /// Vulkan logical device
    pub device: ash::Device,

    /// GPU memory allocator (shared, requires mutex for thread safety)
    /// Wrapped in ManuallyDrop to ensure it's dropped BEFORE the device is destroyed
    pub allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,

    /// Graphics queue for command submission
    pub graphics_queue: vk::Queue,

    /// Graphics queue family index
    pub graphics_queue_family: u32,

    /// Reusable command pool for one-shot upload operations
    /// (created with TRANSIENT + RESET_COMMAND_BUFFER flags)
    pub upload_command_pool: Mutex<vk::CommandPool>,

    /// Vulkan instance (kept for reference, destroyed by VulkanRenderer)
    #[allow(dead_code)]
    instance: ash::Instance,

    /// Debug utils loader (for validation layers)
    pub(crate) debug_utils_loader: Option<ash::ext::debug_utils::Instance>,

    /// Debug messenger handle
    pub(crate) debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
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
    /// * `instance` - Vulkan instance
    /// * `debug_utils_loader` - Debug utils loader (if validation enabled)
    /// * `debug_messenger` - Debug messenger handle (if validation enabled)
    pub fn new(
        device: ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        graphics_queue: vk::Queue,
        graphics_queue_family: u32,
        upload_command_pool: vk::CommandPool,
        instance: ash::Instance,
        debug_utils_loader: Option<ash::ext::debug_utils::Instance>,
        debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    ) -> Self {
        Self {
            device,
            allocator: ManuallyDrop::new(allocator),
            graphics_queue,
            graphics_queue_family,
            upload_command_pool: Mutex::new(upload_command_pool),
            instance,
            debug_utils_loader,
            debug_messenger,
        }
    }
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        // NOTE: Device and instance destruction is handled by VulkanRenderer::drop()
        // to avoid issues with drop ordering and callback exceptions on Windows.
        // This Drop impl intentionally does nothing.
    }
}
