/// DescriptorSet trait - represents a descriptor set for binding resources

/// Descriptor set abstraction for binding resources (textures, uniform buffers, etc.)
///
/// Descriptor sets group together resources that shaders can access.
/// This is a marker trait - the actual binding is done via RendererCommandList::bind_descriptor_sets()
pub trait DescriptorSet: Send + Sync {
    // Marker trait - no public methods
    // Backend implementations (VulkanDescriptorSet) contain the actual descriptor set handle
}
