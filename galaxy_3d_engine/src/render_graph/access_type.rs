/// Render graph resource access declarations.
///
/// Re-exports `AccessType` from `graphics_device` and adds
/// `ResourceAccess` for render graph target-level declarations.

pub use crate::graphics_device::AccessType;

/// Per-resource access declaration for a render graph pass.
///
/// `previous_access_type` is filled during `compile()` by
/// `resolve_previous_accesses()` — `None` until then.
#[derive(Debug, Clone, Copy)]
pub struct ResourceAccess {
    /// Target index within the render graph
    pub target_id: usize,
    /// How the pass uses this target
    pub access_type: AccessType,
    /// How this target was last accessed (filled by compile)
    pub previous_access_type: Option<AccessType>,
}
