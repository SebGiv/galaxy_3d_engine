/// Camera culling strategies.
///
/// A CameraCuller determines which RenderInstances are visible
/// from a given camera. Implementations range from brute-force
/// (return all) to spatial structures (Octree, BVH).

use crate::camera::{Camera, RenderView};
use super::scene::Scene;
use super::render_instance::RenderInstanceKey;

/// Strategy for determining visible instances from a camera.
///
/// Called once per frame before drawing. The returned RenderView
/// is ephemeral and consumed by a Drawer.
///
/// `&mut self` allows stateful implementations (e.g. Octree, BVH)
/// to rebuild their spatial index when the scene changes.
pub trait CameraCuller: Send + Sync {
    /// Cull the scene against the camera and return visible instances.
    fn cull(&mut self, scene: &Scene, camera: &Camera) -> RenderView;
}

/// Brute-force culler — returns ALL instances (no actual culling).
///
/// Suitable for small scenes or as a baseline for comparison.
pub struct BruteForceCuller;

impl BruteForceCuller {
    pub fn new() -> Self {
        Self
    }
}

impl CameraCuller for BruteForceCuller {
    fn cull(&mut self, scene: &Scene, camera: &Camera) -> RenderView {
        let visible: Vec<RenderInstanceKey> = scene.render_instance_keys().collect();
        RenderView::new(camera.clone(), visible)
    }
}
