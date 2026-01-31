/// VulkanRendererShader - Vulkan implementation of RendererShader trait

use galaxy_3d_engine::RendererShader;
use ash::vk;

/// Vulkan shader implementation
pub struct VulkanRendererShader {
    /// Vulkan shader module
    pub(crate) module: vk::ShaderModule,
    /// Shader stage flags
    pub(crate) stage: vk::ShaderStageFlags,
    /// Entry point name
    pub(crate) entry_point: String,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererShader for VulkanRendererShader {
    // No public methods
}

impl Drop for VulkanRendererShader {
    fn drop(&mut self) {
        unsafe {
            // Destroy shader module
            self.device.destroy_shader_module(self.module, None);
        }
    }
}
