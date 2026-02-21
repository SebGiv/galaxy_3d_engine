/// Scene — a collection of RenderInstances for rendering.
///
/// Uses a SlotMap for O(1) insert/remove with stable keys.
/// Instances are stored contiguously for cache-friendly iteration.

use std::sync::{Arc, Mutex};
use slotmap::SlotMap;
use glam::Mat4;
use crate::error::Result;
use crate::renderer;
use crate::resource::mesh::Mesh;
use crate::utils::SlotAllocator;
use super::render_instance::{
    RenderInstance, RenderInstanceKey, AABB,
};

/// A renderable scene containing RenderInstances.
///
/// Instances are managed via stable keys (RenderInstanceKey).
/// Keys remain valid even after other instances are removed.
/// The scene holds a reference to the Renderer for creating GPU binding groups.
pub struct Scene {
    /// Renderer for creating GPU resources (binding groups)
    renderer: Arc<Mutex<dyn renderer::Renderer>>,
    /// Render instances stored in a slot map for O(1) insert/remove
    render_instances: SlotMap<RenderInstanceKey, RenderInstance>,
    /// Allocator for unique draw slot indices (one per submesh in the GPU scene SSBO)
    draw_slot_allocator: SlotAllocator,
}

impl Scene {
    /// Create a new empty scene (internal: only via SceneManager)
    pub(crate) fn new(renderer: Arc<Mutex<dyn renderer::Renderer>>) -> Self {
        Self {
            renderer,
            render_instances: SlotMap::with_key(),
            draw_slot_allocator: SlotAllocator::new(),
        }
    }

    /// Iterate over all render instance keys.
    pub fn render_instance_keys(&self) -> impl Iterator<Item = RenderInstanceKey> + '_ {
        self.render_instances.keys()
    }

    /// Create a RenderInstance from a Mesh and add it to the scene
    ///
    /// Returns a stable key that remains valid until the instance is removed.
    /// Resolves binding groups and push constants against pipeline reflection.
    ///
    /// # Arguments
    ///
    /// * `mesh` - Source mesh resource
    /// * `world_matrix` - World transform matrix
    /// * `bounding_box` - AABB in local space
    /// * `variant_index` - Pipeline variant to use (0 = default)
    pub fn create_render_instance(
        &mut self,
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        variant_index: usize,
    ) -> Result<RenderInstanceKey> {
        let instance = RenderInstance::from_mesh(
            mesh, world_matrix, bounding_box, variant_index,
            &self.renderer, &mut self.draw_slot_allocator,
        )?;
        let key = self.render_instances.insert(instance);
        Ok(key)
    }

    /// Remove a RenderInstance by key
    ///
    /// Returns the removed instance, or None if the key was invalid.
    pub fn remove_render_instance(
        &mut self,
        key: RenderInstanceKey,
    ) -> Option<RenderInstance> {
        if let Some(instance) = self.render_instances.get(key) {
            instance.free_draw_slots(&mut self.draw_slot_allocator);
        }
        self.render_instances.remove(key)
    }

    /// Get a RenderInstance by key
    pub fn render_instance(
        &self,
        key: RenderInstanceKey,
    ) -> Option<&RenderInstance> {
        self.render_instances.get(key)
    }

    /// Get a mutable RenderInstance by key
    pub fn render_instance_mut(
        &mut self,
        key: RenderInstanceKey,
    ) -> Option<&mut RenderInstance> {
        self.render_instances.get_mut(key)
    }

    /// Iterate over all render instances (key, instance)
    pub fn render_instances(
        &self,
    ) -> impl Iterator<Item = (RenderInstanceKey, &RenderInstance)> {
        self.render_instances.iter()
    }

    /// Iterate over all render instances mutably
    pub fn render_instances_mut(
        &mut self,
    ) -> impl Iterator<Item = (RenderInstanceKey, &mut RenderInstance)> {
        self.render_instances.iter_mut()
    }

    /// Get the number of render instances
    pub fn render_instance_count(&self) -> usize {
        self.render_instances.len()
    }

    /// Remove all render instances and reset the draw slot allocator
    pub fn clear(&mut self) {
        self.render_instances.clear();
        self.draw_slot_allocator = SlotAllocator::new();
    }

    /// Minimum SSBO capacity needed (in number of slots)
    pub fn draw_slot_high_water_mark(&self) -> u32 {
        self.draw_slot_allocator.high_water_mark()
    }

    /// Number of currently allocated draw slots
    pub fn draw_slot_count(&self) -> u32 {
        self.draw_slot_allocator.len()
    }
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
