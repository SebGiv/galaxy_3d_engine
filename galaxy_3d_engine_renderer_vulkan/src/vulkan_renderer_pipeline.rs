/// VulkanRendererPipeline - Vulkan implementation of RendererPipeline trait

use galaxy_3d_engine::RendererPipeline;
use ash::vk;

/// Vulkan pipeline implementation
pub struct VulkanRendererPipeline {
    /// Vulkan graphics pipeline
    pub(crate) pipeline: vk::Pipeline,
    /// Pipeline layout
    pub(crate) layout: vk::PipelineLayout,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererPipeline for VulkanRendererPipeline {
    // No public methods
}

impl Drop for VulkanRendererPipeline {
    fn drop(&mut self) {
        unsafe {
            // Destroy pipeline
            self.device.destroy_pipeline(self.pipeline, None);
            // Destroy pipeline layout
            self.device.destroy_pipeline_layout(self.layout, None);
        }
    }
}
