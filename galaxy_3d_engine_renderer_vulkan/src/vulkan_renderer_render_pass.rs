/// VulkanRendererRenderPass - Vulkan implementation of RendererRenderPass trait

use galaxy_3d_engine::RendererRenderPass;
use ash::vk;

/// Vulkan render pass implementation
///
/// Simple wrapper around vk::RenderPass
pub struct VulkanRendererRenderPass {
    /// Vulkan render pass handle
    pub(crate) render_pass: vk::RenderPass,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererRenderPass for VulkanRendererRenderPass {
    // No methods needed for now - just a type-safe wrapper
}

impl Drop for VulkanRendererRenderPass {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}
