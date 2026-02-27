/// Framebuffer trait - groups render target attachments for a render pass
///
/// A framebuffer binds together color and depth/stencil attachments
/// that a render pass will render into.
///
/// Created once and reused each frame. Must be recreated only when
/// attachments change (e.g., window resize).

use std::sync::Arc;
use crate::graphics_device::{RenderPass, RenderTarget};

/// Framebuffer — groups color and depth/stencil attachments together
///
/// Represents the set of render target views that a render pass renders into.
/// Created via `GraphicsDevice::create_framebuffer()`.
pub trait Framebuffer: Send + Sync {
    /// Get the width in pixels
    fn width(&self) -> u32;

    /// Get the height in pixels
    fn height(&self) -> u32;
}

/// Descriptor for creating a framebuffer
pub struct FramebufferDesc<'a> {
    /// The render pass this framebuffer is compatible with
    pub render_pass: &'a Arc<dyn RenderPass>,
    /// Color attachments (one or more render target views)
    pub color_attachments: Vec<Arc<dyn RenderTarget>>,
    /// Optional depth/stencil attachment
    pub depth_stencil_attachment: Option<Arc<dyn RenderTarget>>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}
