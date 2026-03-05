/// Resource access types for the render graph.
///
/// Each pass declares how it uses each resource via an `AccessType`.
/// The compiler deduces image layouts, pipeline stages, access masks,
/// and generates barriers automatically.

use crate::graphics_device::{PipelineStageFlags, AccessFlags, ImageLayout};

/// How a pass accesses a resource.
///
/// Determines layout, pipeline stage, and access mask automatically.
/// Inspired by Frostbite Frame Graph, Unreal Engine 5 RDG, and Granite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// Color attachment write (render pass output)
    ColorAttachmentWrite,
    /// Color attachment read (e.g. blending with existing content)
    ColorAttachmentRead,
    /// Depth/stencil write
    DepthStencilWrite,
    /// Depth/stencil read-only (e.g. depth testing without writing)
    DepthStencilReadOnly,
    /// Fragment shader sampling (texture read)
    FragmentShaderRead,
    /// Vertex shader sampling (e.g. displacement maps)
    VertexShaderRead,
    /// Compute shader read (storage buffer / image)
    ComputeRead,
    /// Compute shader write (storage buffer / image)
    ComputeWrite,
    /// Transfer source (copy, blit)
    TransferRead,
    /// Transfer destination (copy, blit)
    TransferWrite,
    /// Ray tracing acceleration structure read
    RayTracingRead,
}

/// Resolved pipeline state for an access type.
pub struct AccessInfo {
    pub stage: PipelineStageFlags,
    pub access: AccessFlags,
    pub layout: ImageLayout,
}

impl AccessType {
    /// Map an access type to its pipeline stage, access mask, and image layout.
    ///
    /// This is a static, deterministic mapping — each AccessType resolves to
    /// exactly one (stage, access, layout) triplet.
    pub fn info(self) -> AccessInfo {
        match self {
            Self::ColorAttachmentWrite => AccessInfo {
                stage: PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                access: AccessFlags::COLOR_ATTACHMENT_WRITE,
                layout: ImageLayout::ColorAttachment,
            },
            Self::ColorAttachmentRead => AccessInfo {
                stage: PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                access: AccessFlags::COLOR_ATTACHMENT_READ,
                layout: ImageLayout::ColorAttachment,
            },
            Self::DepthStencilWrite => AccessInfo {
                stage: PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    .union(PipelineStageFlags::LATE_FRAGMENT_TESTS),
                access: AccessFlags::DEPTH_STENCIL_WRITE,
                layout: ImageLayout::DepthStencilAttachment,
            },
            Self::DepthStencilReadOnly => AccessInfo {
                stage: PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    .union(PipelineStageFlags::LATE_FRAGMENT_TESTS),
                access: AccessFlags::DEPTH_STENCIL_READ,
                layout: ImageLayout::DepthStencilReadOnly,
            },
            Self::FragmentShaderRead => AccessInfo {
                stage: PipelineStageFlags::FRAGMENT_SHADER,
                access: AccessFlags::SHADER_READ,
                layout: ImageLayout::ShaderReadOnly,
            },
            Self::VertexShaderRead => AccessInfo {
                stage: PipelineStageFlags::VERTEX_SHADER,
                access: AccessFlags::SHADER_READ,
                layout: ImageLayout::ShaderReadOnly,
            },
            Self::ComputeRead => AccessInfo {
                stage: PipelineStageFlags::COMPUTE_SHADER,
                access: AccessFlags::SHADER_READ,
                layout: ImageLayout::ShaderReadOnly,
            },
            Self::ComputeWrite => AccessInfo {
                stage: PipelineStageFlags::COMPUTE_SHADER,
                access: AccessFlags::SHADER_WRITE,
                layout: ImageLayout::General,
            },
            Self::TransferRead => AccessInfo {
                stage: PipelineStageFlags::TRANSFER,
                access: AccessFlags::TRANSFER_READ,
                layout: ImageLayout::TransferSrc,
            },
            Self::TransferWrite => AccessInfo {
                stage: PipelineStageFlags::TRANSFER,
                access: AccessFlags::TRANSFER_WRITE,
                layout: ImageLayout::TransferDst,
            },
            Self::RayTracingRead => AccessInfo {
                stage: PipelineStageFlags::RAY_TRACING_SHADER,
                access: AccessFlags::SHADER_READ,
                layout: ImageLayout::ShaderReadOnly,
            },
        }
    }

    /// Returns true if this access type writes to the resource.
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::ColorAttachmentWrite
                | Self::DepthStencilWrite
                | Self::ComputeWrite
                | Self::TransferWrite
        )
    }

    /// Returns true if this access type is a color or depth/stencil attachment
    /// (i.e. participates in a render pass as an attachment).
    pub fn is_attachment(self) -> bool {
        matches!(
            self,
            Self::ColorAttachmentWrite
                | Self::ColorAttachmentRead
                | Self::DepthStencilWrite
                | Self::DepthStencilReadOnly
        )
    }
}

/// Per-resource access declaration for a pass.
#[derive(Debug, Clone, Copy)]
pub struct ResourceAccess {
    /// Target index within the render graph
    pub target_id: usize,
    /// How the pass uses this target
    pub access_type: AccessType,
}
