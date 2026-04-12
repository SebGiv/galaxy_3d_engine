/// Spatial acceleration structures for scene queries.
///
/// A SceneIndex indexes RenderInstances by their world-space AABB
/// for efficient frustum culling and spatial queries.
/// Implementations include Octree, BVH, grid, etc.
///
/// Ownership: the caller creates and owns the SceneIndex.
/// It is passed by reference to Updater and CameraCuller.

use glam::Vec3;
use crate::camera::{Frustum, VisibleInstance};
use super::render_instance::{RenderInstanceKey, AABB};

/// Trait for spatial indexing of scene instances.
///
/// Used by CameraCuller (frustum queries) and Updater (instance placement).
/// The caller owns the SceneIndex and passes it as a parameter.
pub trait SceneIndex: Send + Sync {
    /// Insert an instance with its world-space position and AABB.
    ///
    /// `world_position` is the instance's translation in world space (typically
    /// `world_matrix.w_axis.truncate()`). It is stored alongside the AABB so
    /// that `query_frustum` can compute camera-relative depth without a
    /// separate scene lookup.
    fn insert(&mut self, key: RenderInstanceKey, world_position: Vec3, world_aabb: &AABB);

    /// Remove an instance from the index.
    fn remove(&mut self, key: RenderInstanceKey);

    /// Update an instance's world-space position and AABB (e.g. after transform change).
    fn update(&mut self, key: RenderInstanceKey, world_position: Vec3, world_aabb: &AABB);

    /// Query all instances whose world AABB intersects the frustum.
    ///
    /// For each visible instance, computes the view-space depth
    /// `depth = dot(instance_pos - camera_pos, camera_forward)` and pushes
    /// a `VisibleInstance { key, distance }` into `results`.
    ///
    /// Results are appended to `results` (the vec is NOT cleared).
    fn query_frustum(
        &self,
        frustum: &Frustum,
        camera_pos: Vec3,
        camera_forward: Vec3,
        results: &mut Vec<VisibleInstance>,
    );

    /// Remove all instances from the index.
    fn clear(&mut self);
}
