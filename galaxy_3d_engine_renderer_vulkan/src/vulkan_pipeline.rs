/// Pipeline - Vulkan implementation of RendererPipeline trait

use galaxy_3d_engine::galaxy3d::render::Pipeline as RendererPipeline;
use ash::vk;

/// Vulkan pipeline implementation
pub struct Pipeline {
    /// Vulkan graphics pipeline
    pub(crate) pipeline: vk::Pipeline,
    /// Pipeline layout (crate-private, accessed internally for descriptor set binding)
    pub(crate) pipeline_layout: vk::PipelineLayout,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererPipeline for Pipeline {
    // No public methods
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            // Destroy pipeline
            self.device.destroy_pipeline(self.pipeline, None);
            // Destroy pipeline layout
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}
