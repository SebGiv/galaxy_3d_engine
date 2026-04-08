/// RenderView — result of frustum culling.
///
/// Created by `Scene::frustum_cull()`. Contains a snapshot of the camera
/// at culling time and the list of visible instances (with pre-computed
/// view-space distances encoded as u16 sort keys).
///
/// Ephemeral: lives for one frame. No Arc, no Mutex.
/// Shareable: the caller can pass the same RenderView to multiple passes.

use crate::scene::VisibleInstanceList;
use super::camera::Camera;

/// Result of frustum culling. Ephemeral — lives for one frame.
///
/// Created exclusively by `Scene::frustum_cull()`.
/// Contains a camera snapshot and the list of visible instances with their
/// pre-computed sort distances.
#[derive(Debug, Clone)]
pub struct RenderView {
    camera: Camera,
    visible_instances: VisibleInstanceList,
}

impl RenderView {
    /// Create a new RenderView (crate-internal: only Scene::frustum_cull creates these).
    pub(crate) fn new(camera: Camera, visible_instances: VisibleInstanceList) -> Self {
        Self {
            camera,
            visible_instances,
        }
    }

    /// Camera snapshot at the time of culling.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Visible instances (with pre-computed u16 sort distances).
    pub fn visible_instances(&self) -> &VisibleInstanceList {
        &self.visible_instances
    }

    /// Number of visible instances.
    pub fn visible_count(&self) -> usize {
        self.visible_instances.len()
    }
}

#[cfg(test)]
#[path = "render_view_tests.rs"]
mod tests;
