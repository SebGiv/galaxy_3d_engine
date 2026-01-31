/// RenderPass - Vulkan implementation of RendererRenderPass trait

use galaxy_3d_engine::galaxy3d::render::RenderPass as RendererRenderPass;
use ash::vk;

/// Vulkan render pass implementation
///
/// Simple wrapper around vk::RenderPass
pub struct RenderPass {
    /// Vulkan render pass handle
    pub(crate) render_pass: vk::RenderPass,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
}

impl RendererRenderPass for RenderPass {
    // No methods needed for now - just a type-safe wrapper
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}
