/// Camera culling strategies.
///
/// A CameraCuller determines which RenderInstances are visible
/// from a given camera. Implementations range from brute-force
/// (return all) to spatial structures (Octree, BVH).

use glam::Vec3;
use crate::camera::{Camera, Frustum, RenderView};
use super::scene::Scene;
use super::scene_index::SceneIndex;

/// Strategy for determining visible instances from a camera.
///
/// Called once per frame before drawing. The caller owns the `RenderView` and
/// passes it by mutable reference; the culler updates the camera snapshot and
/// refills the visible instances buffer in place — no allocation in steady
/// state.
///
/// `&mut self` allows stateful implementations (e.g. caching)
/// to maintain state across frames.
pub trait CameraCuller: Send + Sync {
    /// Cull the scene against the camera and write the result into `view`.
    ///
    /// The culler:
    /// - calls `view.set_camera(camera.clone())` to snapshot the camera
    /// - calls `view.visible_instances_mut().clear()` to reset the buffer
    ///   (preserving capacity)
    /// - pushes visible instances into the buffer
    fn cull_into(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        scene_index: Option<&dyn SceneIndex>,
        view: &mut RenderView,
    );
}

/// Extract the camera world-space position and forward direction from its
/// view matrix.
///
/// `view_matrix` is the inverse of the camera's world transform. We invert it
/// once to obtain the world transform, then read:
/// - position = world.w_axis (translation column)
/// - forward = -world.z_axis (right-handed convention: camera looks down -Z)
fn camera_pos_and_forward(camera: &Camera) -> (Vec3, Vec3) {
    let world = camera.view_matrix().inverse();
    let pos = world.w_axis.truncate();
    let forward = -world.z_axis.truncate();
    (pos, forward)
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
    fn cull_into(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        _scene_index: Option<&dyn SceneIndex>,
        view: &mut RenderView,
    ) {
        view.set_camera(camera.clone());

        let (camera_pos, camera_forward) = camera_pos_and_forward(camera);
        let visible = view.visible_instances_mut();
        visible.clear();
        for (key, instance) in scene.render_instances() {
            let inst_pos = instance.world_matrix().w_axis.truncate();
            let depth = (inst_pos - camera_pos).dot(camera_forward);
            visible.push(key, depth);
        }
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
    fn cull_into(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        scene_index: Option<&dyn SceneIndex>,
        view: &mut RenderView,
    ) {
        view.set_camera(camera.clone());

        let frustum = Frustum::from_view_projection(
            &camera.view_projection_matrix(),
        );
        let (camera_pos, camera_forward) = camera_pos_and_forward(camera);

        let visible = view.visible_instances_mut();
        visible.clear();

        match scene_index {
            Some(idx) => {
                // Octree (or other spatial index) walks itself, computing
                // distances during traversal.
                idx.query_frustum(&frustum, camera_pos, camera_forward, visible);
            }
            None => {
                // Brute frustum loop over all instances
                for (key, instance) in scene.render_instances() {
                    let world_aabb = instance.bounding_box()
                        .transformed(instance.world_matrix());
                    if frustum.intersects_aabb(&world_aabb) {
                        let inst_pos = instance.world_matrix().w_axis.truncate();
                        let depth = (inst_pos - camera_pos).dot(camera_forward);
                        visible.push(key, depth);
                    }
                }
            }
        }
    }
}
