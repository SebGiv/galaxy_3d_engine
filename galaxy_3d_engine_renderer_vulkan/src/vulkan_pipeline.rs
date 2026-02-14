/// Pipeline - Vulkan implementation of RendererPipeline trait

use galaxy_3d_engine::galaxy3d::render::Pipeline as RendererPipeline;
use ash::vk;

/// Vulkan pipeline implementation
///
/// Stores the descriptor set layouts internally (Option B design).
/// The layouts are created from BindingGroupLayoutDesc at pipeline creation time
/// and used by VulkanRenderer::create_binding_group() to allocate descriptor sets.
pub struct Pipeline {
    /// Vulkan graphics pipeline
    pub(crate) pipeline: vk::Pipeline,
    /// Pipeline layout (crate-private, accessed internally for binding group binding)
    pub(crate) pipeline_layout: vk::PipelineLayout,
    /// Descriptor set layouts created for this pipeline (one per set index)
    pub(crate) descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererPipeline for Pipeline {
    fn binding_group_layout_count(&self) -> u32 {
        self.descriptor_set_layouts.len() as u32
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            // Destroy pipeline
            self.device.destroy_pipeline(self.pipeline, None);
            // Destroy pipeline layout
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            // Destroy descriptor set layouts owned by this pipeline
            for &layout in &self.descriptor_set_layouts {
                self.device.destroy_descriptor_set_layout(layout, None);
            }
        }
    }
}
