/// RenderPass - Vulkan implementation of RendererRenderPass trait
///
/// With `VK_KHR_dynamic_rendering`, there is no `VkRenderPass` object. The
/// `RenderPass` becomes a pure Rust descriptor that records attachment
/// formats and load/store ops. The command list reads this descriptor at
/// `begin_render_pass` time and builds a `VkRenderingInfo` inline.

use galaxy_3d_engine::galaxy3d::render::{
    AttachmentDesc, RenderPass as RendererRenderPass,
};

/// Vulkan render pass descriptor (data-only, no GPU object).
pub struct RenderPass {
    /// Color attachment descriptors (format, samples, load/store ops).
    pub(crate) color_attachments: Vec<AttachmentDesc>,
    /// Optional depth/stencil attachment descriptor.
    pub(crate) depth_stencil_attachment: Option<AttachmentDesc>,
    /// Resolve attachment descriptors (same length as `color_attachments`
    /// when MSAA resolve is active, empty otherwise).
    pub(crate) color_resolve_attachments: Vec<AttachmentDesc>,
}

impl RendererRenderPass for RenderPass {
    // No methods needed for now - just a type-safe wrapper
}
