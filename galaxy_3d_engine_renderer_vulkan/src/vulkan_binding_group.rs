/// BindingGroup - Vulkan implementation of graphics_device::BindingGroup trait

use galaxy_3d_engine::galaxy3d::render::BindingGroup as RendererBindingGroup;
use ash::vk;

/// Vulkan binding group implementation
///
/// Wraps a VkDescriptorSet handle. The descriptor set itself is managed
/// by the descriptor pool and will be freed when the pool is destroyed.
/// Immutable after creation — create a new BindingGroup to change resources.
pub struct BindingGroup {
    /// Vulkan descriptor set handle
    pub(crate) descriptor_set: vk::DescriptorSet,
    /// Set index this binding group was created for
    pub(crate) set_index: u32,
}

impl RendererBindingGroup for BindingGroup {
    fn set_index(&self) -> u32 {
        self.set_index
    }
}

impl Drop for BindingGroup {
    fn drop(&mut self) {
        // Descriptor sets are automatically freed when the descriptor pool is destroyed.
        // No explicit cleanup needed here.
    }
}
