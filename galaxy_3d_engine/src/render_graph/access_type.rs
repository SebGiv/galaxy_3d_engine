/// Render graph resource access declarations.
///
/// Re-exports `AccessType` from `graphics_device` and adds the per-pass
/// access record (`ResourceAccess`) plus the per-attachment ops (`TargetOps`).

use crate::graphics_device;

pub use crate::graphics_device::AccessType;

/// Per-attachment load/store/clear configuration.
///
/// Lives on `ResourceAccess` (per pass × resource) — not on the resource
/// itself — because the same texture is typically cleared on the first pass
/// that writes it and loaded by every subsequent pass.
///
/// Buffer accesses do not carry `TargetOps` (no LoadOp/StoreOp on SSBOs/UBOs).
#[derive(Debug, Clone, Copy)]
pub enum TargetOps {
    /// Configuration for a color attachment.
    Color {
        /// Clear color (RGBA).
        clear_color: [f32; 4],
        /// Load operation.
        load_op: graphics_device::LoadOp,
        /// Store operation.
        store_op: graphics_device::StoreOp,
        /// Optional MSAA resolve target. When `Some`, the GPU resolves
        /// this color attachment into the referenced texture at the end
        /// of the render pass. The referenced `GraphResource` must be a
        /// single-sampled texture matching this attachment's format and
        /// dimensions. Either every color attachment in the pass has a
        /// resolve target, or none — Vulkan rejects mixed cases.
        resolve_target: Option<super::graph_resource::GraphResourceKey>,
    },
    /// Configuration for a depth/stencil attachment.
    DepthStencil {
        /// Depth clear value.
        depth_clear: f32,
        /// Stencil clear value.
        stencil_clear: u32,
        /// Depth load operation.
        depth_load_op: graphics_device::LoadOp,
        /// Depth store operation.
        depth_store_op: graphics_device::StoreOp,
        /// Stencil load operation.
        stencil_load_op: graphics_device::LoadOp,
        /// Stencil store operation.
        stencil_store_op: graphics_device::StoreOp,
    },
}

/// Per-resource access declaration for a render graph pass.
///
/// `target_ops` is `Some` only for attachment accesses on textures.
/// Buffer accesses and sampled-read texture accesses leave it `None`.
///
/// `previous_access_type` is intentionally absent: the render graph
/// computes it in a per-frame scratch buffer during compile and feeds
/// it directly to the backend at barrier time.
#[derive(Debug, Clone, Copy)]
pub struct ResourceAccess {
    /// Key into `RenderGraphManager::graph_resources`.
    pub graph_resource_key: super::graph_resource::GraphResourceKey,
    /// How the pass uses the resource.
    pub access_type: AccessType,
    /// Attachment ops — `Some` only for `Color*` / `DepthStencil*` access types
    /// targeting a texture, `None` otherwise.
    pub target_ops: Option<TargetOps>,
}
