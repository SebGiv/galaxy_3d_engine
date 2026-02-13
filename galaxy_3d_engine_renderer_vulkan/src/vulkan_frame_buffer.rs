/// Framebuffer - Vulkan implementation of RendererFramebuffer trait
///
/// Wraps a VkFramebuffer that groups color and depth/stencil attachments.
/// Created once via Renderer::create_framebuffer(), reused each frame.

use galaxy_3d_engine::galaxy3d::render::Framebuffer as RendererFramebuffer;
use ash::vk;

/// Vulkan framebuffer implementation
///
/// Wraps a VkFramebuffer. Destroyed when dropped.
pub struct Framebuffer {
    /// Vulkan framebuffer handle
    pub(crate) framebuffer: vk::Framebuffer,
    /// Width in pixels
    width: u32,
    /// Height in pixels
    height: u32,
    /// Vulkan device (for cleanup)
    device: ash::Device,
}

impl Framebuffer {
    pub(crate) fn new(
        framebuffer: vk::Framebuffer,
        width: u32,
        height: u32,
        device: ash::Device,
    ) -> Self {
        Self { framebuffer, width, height, device }
    }
}

impl RendererFramebuffer for Framebuffer {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_framebuffer(self.framebuffer, None);
        }
    }
}
