/// Framebuffer trait - groups texture attachments for a render pass
///
/// A framebuffer binds together color and depth/stencil attachments
/// that a render pass will render into.
///
/// Each attachment is a `FramebufferAttachment` describing which texture,
/// mip level, and layer range to render into.
///
/// Created once and reused each frame. Must be recreated only when
/// attachments change (e.g., window resize).

use std::sync::Arc;
use crate::graphics_device::{RenderPass, Texture};

/// Framebuffer — groups color and depth/stencil attachments together
///
/// Represents the set of texture attachments that a render pass renders into.
/// Created via `GraphicsDevice::create_framebuffer()`.
pub trait Framebuffer: Send + Sync {
    /// Get the width in pixels
    fn width(&self) -> u32;

    /// Get the height in pixels
    fn height(&self) -> u32;
}

/// A single framebuffer attachment: a texture + a mip level + a layer range.
///
/// Maps directly to a `VkImageView` in the backend, with
/// `level_count = 1` (Vulkan does not allow rendering into multiple mip
/// levels of an attachment simultaneously) and a configurable
/// `layer_count` for layered rendering (cubemap one-pass, cascaded shadow
/// maps, multiview/VR, voxelization).
pub struct FramebufferAttachment {
    /// Source texture providing the underlying image.
    pub texture: Arc<dyn Texture>,
    /// Mip level to render into (0 = base level).
    pub base_mip_level: u32,
    /// First array layer to render into (0 = base layer).
    pub base_array_layer: u32,
    /// Number of consecutive layers to render into. Must be >= 1.
    /// Use > 1 only for layered rendering (e.g. cubemap one-pass with 6).
    pub layer_count: u32,
}

impl FramebufferAttachment {
    /// Helper for the common case: mip 0, layer 0, single layer.
    pub fn whole(texture: Arc<dyn Texture>) -> Self {
        Self {
            texture,
            base_mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        }
    }

    /// Helper for a single mip + a single layer.
    pub fn mip_layer(texture: Arc<dyn Texture>, mip_level: u32, layer: u32) -> Self {
        Self {
            texture,
            base_mip_level: mip_level,
            base_array_layer: layer,
            layer_count: 1,
        }
    }
}

/// Descriptor for creating a framebuffer
pub struct FramebufferDesc<'a> {
    /// The render pass this framebuffer is compatible with
    pub render_pass: &'a Arc<dyn RenderPass>,
    /// Color attachments (one or more texture attachment views)
    pub color_attachments: Vec<FramebufferAttachment>,
    /// Optional depth/stencil attachment
    pub depth_stencil_attachment: Option<FramebufferAttachment>,
    /// Resolve attachments for MSAA color targets (empty if no MSAA).
    /// Must match `color_attachments` in length when present.
    pub color_resolve_attachments: Vec<FramebufferAttachment>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

#[cfg(test)]
#[path = "frame_buffer_tests.rs"]
mod tests;
