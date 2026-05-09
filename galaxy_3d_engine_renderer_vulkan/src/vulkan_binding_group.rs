/// BindingGroup - Vulkan implementation of graphics_device::BindingGroup trait

use galaxy_3d_engine::galaxy3d::render::{
    BindingGroup as RendererBindingGroup,
    Buffer as RendererBuffer,
};
use ash::vk;
use std::sync::Arc;

/// Vulkan binding group implementation
///
/// Wraps a VkDescriptorSet handle. The descriptor set itself is managed
/// by the descriptor pool and will be freed when the pool is destroyed.
/// Immutable after creation — create a new BindingGroup to change resources.
///
/// `dynamic_slot_sizes` carries the per-slot stride (in bytes) of every
/// `*_BUFFER_DYNAMIC` binding the descriptor set declares, in binding-index
/// order. The command list multiplies each entry by the current
/// frame-in-flight index to produce the `pDynamicOffsets` array passed to
/// `vkCmdBindDescriptorSets`. Empty for sets without any dynamic binding.
///
/// `dynamic_buffers` holds strong references to the same Dynamic buffers,
/// in the same order. The command list walks this list at bind time and
/// calls `ensure_slot_synced()` on each, which performs a CPU→GPU memcpy
/// from the buffer's master to the current frame's slot if not already done
/// for this frame. This guarantees the GPU sees a complete, fresh snapshot
/// in the slot it's about to read.
pub struct BindingGroup {
    /// Vulkan descriptor set handle
    pub(crate) descriptor_set: vk::DescriptorSet,
    /// Set index this binding group was created for
    pub(crate) set_index: u32,
    /// Per-slot stride (bytes) of every dynamic binding, in binding order.
    pub(crate) dynamic_slot_sizes: Vec<u64>,
    /// Strong references to every Dynamic buffer in this set, in the same
    /// order as `dynamic_slot_sizes`. Used to call `ensure_slot_synced()`
    /// at bind time. Empty for sets without any dynamic binding.
    pub(crate) dynamic_buffers: Vec<Arc<dyn RendererBuffer>>,
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
