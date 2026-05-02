/// Render-graph-level Framebuffer.
///
/// Wraps a backend `graphics_device::Framebuffer` and remembers the set of
/// `GraphResourceKey`s that compose it. Lives in the `RenderGraphManager`
/// — entries are indexed by a `FramebufferKey` (no name) and looked up
/// for reuse via a `FramebufferLookupKey`.

use std::sync::Arc;
use crate::graphics_device;
use super::graph_resource::GraphResourceKey;

slotmap::new_key_type! {
    /// Stable key for a `Framebuffer` in the `RenderGraphManager`.
    pub struct FramebufferKey;
}

/// One color attachment slot inside a framebuffer: a color
/// `GraphResource` plus its optional MSAA resolve target.
///
/// Pairing color and resolve in the same struct preserves their
/// association by construction (vs two parallel `Vec`s) and makes the
/// lookup key trivially hashable.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct ColorAttachmentSlot {
    /// The MSAA (or single-sample) color attachment.
    pub color: GraphResourceKey,
    /// Optional single-sample resolve target. `Some` for every slot of a
    /// MSAA-resolving framebuffer, `None` for every slot of a regular
    /// framebuffer (the manager rejects mixed cases).
    pub resolve: Option<GraphResourceKey>,
}

/// Render-graph framebuffer: backend handle + the resource set it was
/// built from.
pub struct Framebuffer {
    gd_framebuffer: Arc<dyn graphics_device::Framebuffer>,
    color_attachments: Vec<ColorAttachmentSlot>,
    depth_stencil_attachment: Option<GraphResourceKey>,
}

impl Framebuffer {
    pub(crate) fn new(
        gd_framebuffer: Arc<dyn graphics_device::Framebuffer>,
        color_attachments: Vec<ColorAttachmentSlot>,
        depth_stencil_attachment: Option<GraphResourceKey>,
    ) -> Self {
        Self {
            gd_framebuffer,
            color_attachments,
            depth_stencil_attachment,
        }
    }

    pub fn gd_framebuffer(&self) -> &Arc<dyn graphics_device::Framebuffer> {
        &self.gd_framebuffer
    }

    pub fn color_attachments(&self) -> &[ColorAttachmentSlot] {
        &self.color_attachments
    }

    pub fn depth_stencil_attachment(&self) -> Option<GraphResourceKey> {
        self.depth_stencil_attachment
    }
}

/// Hash key used by `RenderGraphManager` to deduplicate `Framebuffer`s.
///
/// Two passes that target the exact same set of `(color, resolve)` slots
/// in the same order plus the same depth/stencil end up with the same
/// `FramebufferKey`.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) struct FramebufferLookupKey {
    pub color_attachments: Vec<ColorAttachmentSlot>,
    pub depth_stencil_attachment: Option<GraphResourceKey>,
}

#[cfg(test)]
#[path = "frame_buffer_tests.rs"]
mod tests;
