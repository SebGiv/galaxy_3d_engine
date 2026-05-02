/// RenderView — a per-pass draw list consumed by a Drawer.
///
/// Produced by the `ViewDispatcher` from a `CulledInstances` (the raw output
/// of frustum culling). Each RenderView is bound to a single `pass_type` and
/// contains a flat list of `VisibleSubMesh` entries — one per (instance,
/// submesh) pair that participates in this pass.
///
/// The `VisibleSubMesh` entries are fully resolved: submesh index, pass index,
/// LOD index, and distance are all pre-computed so the Drawer can iterate
/// without any lookup or decision logic.

use crate::camera::Camera;
use super::render_instance::RenderInstanceKey;

/// A single draw item — one submesh of one instance, ready for drawing.
///
/// All indices are pre-resolved by the `ViewDispatcher`. The `Drawer`
/// consumes these directly without further computation.
#[derive(Debug, Clone, Copy)]
pub struct VisibleSubMesh {
    /// Key of the RenderInstance in the Scene
    pub key: RenderInstanceKey,
    /// View-space depth (for future sort key construction + LOD override)
    pub distance: f32,
    /// Index into `RenderInstance::sub_meshes`
    pub submesh_index: u8,
    /// Index into `RenderSubMesh::passes` (compact vec)
    pub pass_index: u8,
    /// LOD index to use for the GeometrySubMesh (V1: always 0)
    pub lod_index: u8,
}

/// A per-pass draw list — produced by the ViewDispatcher, consumed by a Drawer.
///
/// Each RenderView is associated with a single `pass_type`. The items buffer
/// is reused across frames via `clear()` + repush — zero allocation in steady
/// state once the high-water mark has been reached.
#[derive(Debug, Clone)]
pub struct RenderView {
    camera: Camera,
    pass_type: u8,
    items: Vec<VisibleSubMesh>,
}

impl RenderView {
    /// Create an empty RenderView for a given pass type.
    pub fn new(camera: Camera, pass_type: u8) -> Self {
        Self {
            camera,
            pass_type,
            items: Vec::new(),
        }
    }

    /// Create an empty RenderView with a pre-allocated capacity.
    pub fn with_capacity(camera: Camera, pass_type: u8, capacity: usize) -> Self {
        Self {
            camera,
            pass_type,
            items: Vec::with_capacity(capacity),
        }
    }

    // ===== ACCESSORS =====

    /// Camera snapshot (copied from the CulledInstances at dispatch time).
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The pass type this view represents.
    pub fn pass_type(&self) -> u8 {
        self.pass_type
    }

    /// Read-only slice of all draw items.
    pub fn items(&self) -> &[VisibleSubMesh] {
        &self.items
    }

    /// Iterator over draw items.
    pub fn iter(&self) -> std::slice::Iter<'_, VisibleSubMesh> {
        self.items.iter()
    }

    /// Number of draw items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the view has no draw items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    // ===== MUTATION (used by ViewDispatcher) =====

    /// Replace the camera snapshot.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }

    /// Push a draw item.
    pub fn push(&mut self, item: VisibleSubMesh) {
        self.items.push(item);
    }

    /// Clear all items (preserves capacity for reuse).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
#[path = "render_view_tests.rs"]
mod tests;
