/// Spatial acceleration structures for scene queries.
///
/// A SceneIndex indexes RenderInstances by their world-space AABB
/// for efficient frustum culling and spatial queries.
/// Implementations include Octree, BVH, grid, etc.
///
/// Ownership: the caller creates and owns the SceneIndex.
/// It is passed by reference to Updater and CameraCuller.

use crate::camera::Frustum;
use super::render_instance::{RenderInstanceKey, AABB};

/// Trait for spatial indexing of scene instances.
///
/// Used by CameraCuller (frustum queries) and Updater (instance placement).
/// The caller owns the SceneIndex and passes it as a parameter.
pub trait SceneIndex: Send + Sync {
    /// Insert an instance with its world-space AABB.
    fn insert(&mut self, key: RenderInstanceKey, world_aabb: &AABB);

    /// Remove an instance from the index.
    fn remove(&mut self, key: RenderInstanceKey);

    /// Update an instance's world-space AABB (e.g. after transform change).
    fn update(&mut self, key: RenderInstanceKey, world_aabb: &AABB);

    /// Query all instances whose world AABB intersects the frustum.
    /// Results are appended to `results`.
    fn query_frustum(&self, frustum: &Frustum, results: &mut Vec<RenderInstanceKey>);

    /// Remove all instances from the index.
    fn clear(&mut self);
}
