/// Camera culling strategies.
///
/// A CameraCuller determines which RenderInstances are visible
/// from a given camera. Implementations range from brute-force
/// (return all) to spatial structures (Octree, BVH).

use crate::camera::{Camera, Frustum, RenderView};
use super::scene::Scene;
use super::scene_index::SceneIndex;
use super::render_instance::RenderInstanceKey;

/// Strategy for determining visible instances from a camera.
///
/// Called once per frame before drawing. The returned RenderView
/// is ephemeral and consumed by a Drawer.
///
/// `&mut self` allows stateful implementations (e.g. caching)
/// to maintain state across frames.
pub trait CameraCuller: Send + Sync {
    /// Cull the scene against the camera and return visible instances.
    fn cull(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        scene_index: Option<&dyn SceneIndex>,
    ) -> RenderView;
}

/// Brute-force culler — returns ALL instances (no actual culling).
///
/// Suitable for small scenes or as a baseline for comparison.
/// Ignores the SceneIndex entirely.
pub struct BruteForceCuller;

impl BruteForceCuller {
    pub fn new() -> Self {
        Self
    }
}

impl CameraCuller for BruteForceCuller {
    fn cull(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        _scene_index: Option<&dyn SceneIndex>,
    ) -> RenderView {
        let visible: Vec<RenderInstanceKey> = scene.render_instance_keys().collect();
        RenderView::new(camera.clone(), visible)
    }
}

/// Frustum culler — tests instance AABBs against the camera frustum.
///
/// With a SceneIndex: spatial query (O(log n) for Octree/BVH).
/// Without: brute-force frustum test on all instances (O(n), still
/// culls invisible objects unlike BruteForceCuller).
pub struct FrustumCuller;

impl FrustumCuller {
    pub fn new() -> Self {
        Self
    }
}

impl CameraCuller for FrustumCuller {
    fn cull(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        scene_index: Option<&dyn SceneIndex>,
    ) -> RenderView {
        let frustum = Frustum::from_view_projection(
            &camera.view_projection_matrix(),
        );

        let visible = match scene_index {
            Some(idx) => {
                let mut results = Vec::new();
                idx.query_frustum(&frustum, &mut results);
                results
            }
            None => {
                scene.render_instances()
                    .filter_map(|(key, instance)| {
                        let world_aabb = instance.bounding_box()
                            .transformed(instance.world_matrix());
                        if frustum.intersects_aabb(&world_aabb) {
                            Some(key)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        };

        RenderView::new(camera.clone(), visible)
    }
}
