/// DescriptorSet - Vulkan implementation of RendererDescriptorSet trait

use galaxy_3d_engine::galaxy3d::render::DescriptorSet as RendererDescriptorSet;
use ash::vk;

/// Vulkan descriptor set implementation
///
/// Wraps a Vulkan descriptor set handle (vk::DescriptorSet).
/// The descriptor set itself is managed by the descriptor pool and will be
/// freed when the pool is destroyed.
pub struct DescriptorSet {
    /// Vulkan descriptor set handle (private - not exposed to public API)
    pub(crate) descriptor_set: vk::DescriptorSet,

    /// Vulkan device (for potential cleanup operations)
    #[allow(dead_code)]
    pub(crate) device: ash::Device,
}

impl RendererDescriptorSet for DescriptorSet {
    // Marker trait - no methods to implement
}

impl Drop for DescriptorSet {
    fn drop(&mut self) {
        // Descriptor sets are automatically freed when the descriptor pool is destroyed
        // No explicit cleanup needed here
    }
}
