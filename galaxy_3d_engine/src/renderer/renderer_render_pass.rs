/// RendererRenderPass trait - describes how to configure a render pass

use crate::renderer::TextureFormat;

/// Render pass trait
///
/// Describes how attachments are loaded, stored, and transitioned during rendering.
pub trait RendererRenderPass: Send + Sync {
    // No methods for now - just a marker trait for type safety
}

/// Descriptor for creating a render pass
#[derive(Debug, Clone)]
pub struct RendererRenderPassDesc {
    /// Color attachments
    pub color_attachments: Vec<AttachmentDesc>,
    /// Optional depth attachment
    pub depth_attachment: Option<AttachmentDesc>,
}

/// Descriptor for a single attachment in a render pass
#[derive(Debug, Clone)]
pub struct AttachmentDesc {
    /// Pixel format
    pub format: TextureFormat,
    /// Number of samples (1 = no MSAA)
    pub samples: u32,
    /// Load operation (what to do with existing content)
    pub load_op: LoadOp,
    /// Store operation (what to do with rendered content)
    pub store_op: StoreOp,
    /// Initial layout (how the attachment starts)
    pub initial_layout: ImageLayout,
    /// Final layout (how the attachment ends)
    pub final_layout: ImageLayout,
}

/// Load operation for an attachment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadOp {
    /// Load existing content
    Load,
    /// Clear the content
    Clear,
    /// Don't care about existing content
    DontCare,
}

/// Store operation for an attachment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOp {
    /// Store the rendered content
    Store,
    /// Don't care about storing the content
    DontCare,
}

/// Image layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageLayout {
    /// Undefined layout (initial state)
    Undefined,
    /// Layout for color attachment
    ColorAttachment,
    /// Layout for depth/stencil attachment
    DepthStencilAttachment,
    /// Layout for shader read-only access
    ShaderReadOnly,
    /// Layout for transfer source
    TransferSrc,
    /// Layout for transfer destination
    TransferDst,
    /// Layout for presenting to swapchain
    PresentSrc,
}
