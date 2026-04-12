/// VisibleInstances — output of frustum culling.
///
/// Created by the caller (typically once at scene init) and passed by mutable
/// reference to `CameraCuller::cull_into()` each frame. The culler updates the
/// camera snapshot and refills the visible instances buffer in place — no
/// allocation in steady state once the high-water mark of visible instances
/// has been reached.
///
/// Not consumed directly by the Drawer. A `ViewDispatcher` converts this into
/// one or more `RenderView`s (one per pass type), each containing a flat list
/// of `VisibleSubMesh` entries ready for drawing.

use glam::{Mat4, Vec4};
use crate::graphics_device::command_list::Viewport;
use crate::scene::RenderInstanceKey;
use super::camera::Camera;
use super::frustum::Frustum;

/// A single visible instance with its view-space depth.
///
/// `distance` is the raw view-space depth in world units (positive in front
/// of the camera, negative behind). It is intentionally kept as a `f32` so
/// that multiple consumers — sort key encoding, LOD selection, distance
/// fade, etc. — can interpret it as needed without precision loss.
#[derive(Debug, Clone, Copy)]
pub struct VisibleInstance {
    pub key: RenderInstanceKey,
    pub distance: f32,
}

/// The list of instances that survived frustum culling, together with the
/// camera snapshot at the time of culling.
///
/// Created once by the caller (e.g. via `VisibleInstances::new_empty()`), then
/// repeatedly refilled by `CameraCuller::cull_into()`. The internal buffer is
/// reused across frames via `clear()` + repush — zero allocation in steady
/// state.
#[derive(Debug, Clone)]
pub struct VisibleInstances {
    camera: Camera,
    instances: Vec<VisibleInstance>,
}

impl VisibleInstances {
    /// Create an empty VisibleInstances with a placeholder camera.
    ///
    /// The placeholder camera will be overwritten on the first call to
    /// `CameraCuller::cull_into()`. The internal buffer starts with zero
    /// capacity and grows on first cull; subsequent culls reuse the existing
    /// capacity.
    pub fn new_empty() -> Self {
        let dummy_camera = Camera::new(
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Frustum { planes: [Vec4::ZERO; 6] },
            Viewport {
                x: 0.0, y: 0.0,
                width: 0.0, height: 0.0,
                min_depth: 0.0, max_depth: 1.0,
            },
        );
        Self {
            camera: dummy_camera,
            instances: Vec::new(),
        }
    }

    // ===== CAMERA =====

    /// Camera snapshot at the time of culling.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Replace the camera snapshot. Called by `CameraCuller::cull_into()`.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }

    // ===== INSTANCES =====

    /// Read-only slice of all visible instances.
    pub fn instances(&self) -> &[VisibleInstance] {
        &self.instances
    }

    /// Mutable access to the instances buffer.
    ///
    /// Used by `CameraCuller::cull_into()` and `SceneIndex::query_frustum()`
    /// to push visible instances.
    pub fn instances_mut(&mut self) -> &mut Vec<VisibleInstance> {
        &mut self.instances
    }

    /// Number of visible instances.
    pub fn visible_count(&self) -> usize {
        self.instances.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Clear all instances (preserves capacity for reuse).
    pub fn clear_instances(&mut self) {
        self.instances.clear();
    }
}

#[cfg(test)]
#[path = "visible_instances_tests.rs"]
mod tests;
