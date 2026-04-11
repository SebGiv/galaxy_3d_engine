/// RenderView — result of frustum culling.
///
/// Created by the caller (typically once at scene init) and passed by mutable
/// reference to `CameraCuller::cull_into()` each frame. The culler updates the
/// camera snapshot and refills the visible instances buffer in place — no
/// allocation in steady state once the high-water mark of visible instances
/// has been reached.
///
/// Shareable: the caller can pass the same RenderView to multiple passes.

use glam::{Mat4, Vec4};
use crate::graphics_device::command_list::Viewport;
use crate::scene::VisibleInstanceList;
use super::camera::Camera;
use super::frustum::Frustum;

/// Result of frustum culling.
///
/// Created once by the caller (e.g. via `RenderView::new_empty()`), then
/// repeatedly refilled by `CameraCuller::cull_into()`. The internal
/// `VisibleInstanceList` buffer is reused across frames via `clear()` + repush
/// — zero allocation in steady state.
#[derive(Debug, Clone)]
pub struct RenderView {
    camera: Camera,
    visible_instances: VisibleInstanceList,
}

impl RenderView {
    /// Create an empty RenderView with a placeholder camera and an empty
    /// visible instances buffer.
    ///
    /// The placeholder camera will be overwritten on the first call to
    /// `CameraCuller::cull_into()`. The internal `VisibleInstanceList` starts
    /// with zero capacity and grows on first cull; subsequent culls reuse the
    /// existing capacity.
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
            visible_instances: VisibleInstanceList::new(),
        }
    }

    /// Camera snapshot at the time of culling.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Replace the camera snapshot. Called by `CameraCuller::cull_into()`.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }

    /// Visible instances (with pre-computed u16 sort distances).
    pub fn visible_instances(&self) -> &VisibleInstanceList {
        &self.visible_instances
    }

    /// Mutable access to the visible instances buffer.
    ///
    /// Used by `CameraCuller::cull_into()` to `clear()` and refill the buffer
    /// without reallocating.
    pub fn visible_instances_mut(&mut self) -> &mut VisibleInstanceList {
        &mut self.visible_instances
    }

    /// Number of visible instances.
    pub fn visible_count(&self) -> usize {
        self.visible_instances.len()
    }
}

#[cfg(test)]
#[path = "render_view_tests.rs"]
mod tests;
