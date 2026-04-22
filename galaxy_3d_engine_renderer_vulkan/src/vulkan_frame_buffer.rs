/// Framebuffer - Vulkan implementation of RendererFramebuffer trait
///
/// With `VK_KHR_dynamic_rendering`, there is no `VkFramebuffer` object. The
/// `Framebuffer` becomes a pure Rust descriptor that keeps the per-attachment
/// `VkImageView` handles the command list needs when building a
/// `VkRenderingInfo` at `begin_render_pass` time.

use galaxy_3d_engine::galaxy3d::render::Framebuffer as RendererFramebuffer;
use ash::vk;

/// Vulkan framebuffer descriptor (data-only, no GPU object).
///
/// Split into three buckets so the command list can wire MSAA resolve
/// attachments directly into each color `VkRenderingAttachmentInfo`.
pub struct Framebuffer {
    /// Image views for the color attachments, in render-pass order.
    pub(crate) color_image_views: Vec<vk::ImageView>,
    /// Optional depth/stencil image view.
    pub(crate) depth_image_view: Option<vk::ImageView>,
    /// Image views for the MSAA resolve targets (same length as
    /// `color_image_views` when resolving, empty otherwise).
    pub(crate) resolve_image_views: Vec<vk::ImageView>,
    /// Width in pixels
    width: u32,
    /// Height in pixels
    height: u32,
}

impl Framebuffer {
    pub(crate) fn new(
        color_image_views: Vec<vk::ImageView>,
        depth_image_view: Option<vk::ImageView>,
        resolve_image_views: Vec<vk::ImageView>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            color_image_views,
            depth_image_view,
            resolve_image_views,
            width,
            height,
        }
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
