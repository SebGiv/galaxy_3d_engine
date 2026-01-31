/// RenderTarget - Vulkan implementation of RendererRenderTarget trait

use galaxy_3d_engine::galaxy3d::render::{RenderTarget as RendererRenderTarget, TextureFormat};
use ash::vk;

/// Vulkan render target implementation
///
/// Can represent either a texture render target or a swapchain image render target
pub struct RenderTarget {
    /// Width in pixels
    width: u32,
    /// Height in pixels
    height: u32,
    /// Pixel format
    format: TextureFormat,
    /// Vulkan image view
    pub(crate) image_view: vk::ImageView,
    /// Vulkan device (for potential cleanup)
    pub(crate) device: Option<ash::Device>,
    /// Whether this target owns the image view (for cleanup)
    pub(crate) owns_image_view: bool,
}

impl RenderTarget {
    /// Create a new render target for a swapchain image
    ///
    /// # Arguments
    ///
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels
    /// * `format` - Pixel format
    /// * `image_view` - Vulkan image view (not owned)
    pub fn new_swapchain_target(
        width: u32,
        height: u32,
        format: TextureFormat,
        image_view: vk::ImageView,
    ) -> Self {
        Self {
            width,
            height,
            format,
            image_view,
            device: None,
            owns_image_view: false,
        }
    }

    /// Create a new render target for an offscreen texture
    ///
    /// # Arguments
    ///
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels
    /// * `format` - Pixel format
    /// * `image_view` - Vulkan image view (owned)
    /// * `device` - Vulkan device for cleanup
    pub fn new_texture_target(
        width: u32,
        height: u32,
        format: TextureFormat,
        image_view: vk::ImageView,
        device: ash::Device,
    ) -> Self {
        Self {
            width,
            height,
            format,
            image_view,
            device: Some(device),
            owns_image_view: true,
        }
    }
}

impl RendererRenderTarget for RenderTarget {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn format(&self) -> TextureFormat {
        self.format
    }
}

impl Drop for RenderTarget {
    fn drop(&mut self) {
        if self.owns_image_view {
            if let Some(device) = &self.device {
                unsafe {
                    device.destroy_image_view(self.image_view, None);
                }
            }
        }
    }
}
