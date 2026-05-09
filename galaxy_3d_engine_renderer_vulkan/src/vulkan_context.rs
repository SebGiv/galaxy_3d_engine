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
use std::sync::atomic::AtomicUsize;

/// Shared GPU context for all Vulkan resources.
///
/// This struct is shared (via `Arc`) by all GPU resources (textures, buffers, etc.)
/// to avoid duplicating device/allocator/queue references in each resource.
///
/// Note: Device and instance destruction is handled by VulkanGraphicsDevice::drop()
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

    /// Unified alignment for dynamic buffer slots, computed at device
    /// creation as the max of:
    /// - `minUniformBufferOffsetAlignment`
    /// - `minStorageBufferOffsetAlignment`
    /// - 16 bytes (vec4 attribute alignment for vertex buffers)
    /// - 4 bytes (UINT32 index alignment for index buffers)
    /// In practice the uniform buffer alignment dominates (64-256 bytes
    /// depending on GPU). Using a single unified value simplifies the
    /// backend at the cost of a few padding bytes per dynamic buffer slot.
    pub dynamic_buffer_alignment: u64,

    /// Index of the frame-in-flight slot the next submit will use.
    /// Advances modulo `FRAMES_IN_FLIGHT` after each successful submit
    /// (see `VulkanGraphicsDevice::submit_inner`).
    /// Read by `Buffer` (Dynamic mode), `BindingGroup` (dynamic offsets),
    /// and `CommandList` (VB/IB dynamic offsets) to resolve which slot
    /// the CPU is currently writing into.
    pub current_submit_fence: AtomicUsize,

    /// Vulkan instance (kept for reference, destroyed by VulkanGraphicsDevice)
    #[allow(dead_code)]
    instance: ash::Instance,

    /// Debug utils loader (for validation layers)
    #[cfg(feature = "vulkan-validation")]
    pub(crate) debug_utils_loader: Option<ash::ext::debug_utils::Instance>,

    /// Debug messenger handle
    #[cfg(feature = "vulkan-validation")]
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
    /// * `dynamic_buffer_alignment` - Unified alignment for dynamic buffer slots
    /// * `instance` - Vulkan instance
    /// * `debug_utils_loader` - Debug utils loader (if validation enabled)
    /// * `debug_messenger` - Debug messenger handle (if validation enabled)
    pub fn new(
        device: ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        graphics_queue: vk::Queue,
        graphics_queue_family: u32,
        upload_command_pool: vk::CommandPool,
        dynamic_buffer_alignment: u64,
        instance: ash::Instance,
        #[cfg(feature = "vulkan-validation")]
        debug_utils_loader: Option<ash::ext::debug_utils::Instance>,
        #[cfg(feature = "vulkan-validation")]
        debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    ) -> Self {
        Self {
            device,
            allocator: ManuallyDrop::new(allocator),
            graphics_queue,
            graphics_queue_family,
            upload_command_pool: Mutex::new(upload_command_pool),
            dynamic_buffer_alignment,
            current_submit_fence: AtomicUsize::new(0),
            instance,
            #[cfg(feature = "vulkan-validation")]
            debug_utils_loader,
            #[cfg(feature = "vulkan-validation")]
            debug_messenger,
        }
    }
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        // NOTE: Device and instance destruction is handled by VulkanGraphicsDevice::drop()
        // to avoid issues with drop ordering and callback exceptions on Windows.
        // This Drop impl intentionally does nothing.
    }
}
