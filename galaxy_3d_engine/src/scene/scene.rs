/// Scene — a collection of RenderInstances for rendering.
///
/// Uses a SlotMap for O(1) insert/remove with stable keys.
/// Instances are stored contiguously for cache-friendly iteration.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use slotmap::SlotMap;
use glam::Mat4;
use crate::error::Result;
use crate::engine_err;
use crate::graphics_device::{self, BindingGroup, BindingResource};
use crate::resource::buffer::Buffer;
use crate::resource::mesh::Mesh;
use crate::utils::SlotAllocator;
use super::render_instance::{
    RenderInstance, RenderInstanceKey, AABB,
};

/// A renderable scene containing RenderInstances.
///
/// Instances are managed via stable keys (RenderInstanceKey).
/// Keys remain valid even after other instances are removed.
/// The scene holds a reference to the GraphicsDevice for creating GPU binding groups.
pub struct Scene {
    /// Graphics device for creating GPU resources (binding groups)
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    /// Render instances stored in a slot map for O(1) insert/remove
    render_instances: SlotMap<RenderInstanceKey, RenderInstance>,
    /// Allocator for unique draw slot indices (one per submesh in the GPU scene SSBO)
    draw_slot_allocator: SlotAllocator,
    /// Per-frame uniform buffer (camera, lighting, time, post-process)
    frame_buffer: Arc<Buffer>,
    /// Per-instance storage buffer (world matrices, material slot, flags)
    instance_buffer: Arc<Buffer>,
    /// Material storage buffer (shared material parameters)
    material_buffer: Arc<Buffer>,
    /// Instances whose world matrix changed since last take_dirty_transforms()
    dirty_transforms: HashSet<RenderInstanceKey>,
    /// Newly created instances pending full GPU buffer initialization
    new_instances: HashSet<RenderInstanceKey>,
    /// Set 0 binding group (frame UBO + instance SSBO + material SSBO), shared by all instances
    global_binding_group: Option<Arc<dyn BindingGroup>>,
}

impl Scene {
    /// Create a new empty scene (internal: only via SceneManager)
    ///
    /// # Arguments
    ///
    /// * `graphics_device` - GraphicsDevice for creating GPU resources
    /// * `frame_buffer` - Per-frame uniform buffer (camera, lighting, time)
    /// * `instance_buffer` - Per-instance storage buffer (world matrices, flags)
    /// * `material_buffer` - Material storage buffer (shared material parameters)
    pub(crate) fn new(
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        frame_buffer: Arc<Buffer>,
        instance_buffer: Arc<Buffer>,
        material_buffer: Arc<Buffer>,
    ) -> Self {
        Self {
            graphics_device,
            render_instances: SlotMap::with_key(),
            draw_slot_allocator: SlotAllocator::new(),
            frame_buffer,
            instance_buffer,
            material_buffer,
            dirty_transforms: HashSet::new(),
            new_instances: HashSet::new(),
            global_binding_group: None,
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
        self.ensure_global_binding_group(mesh, variant_index)?;

        let instance = RenderInstance::from_mesh(
            mesh, world_matrix, bounding_box, variant_index,
            &mut self.draw_slot_allocator,
        )?;
        let key = self.render_instances.insert(instance);
        self.new_instances.insert(key);
        Ok(key)
    }

    /// Remove a RenderInstance by key
    ///
    /// Returns the removed instance, or None if the key was invalid.
    pub fn remove_render_instance(
        &mut self,
        key: RenderInstanceKey,
    ) -> Option<RenderInstance> {
        self.dirty_transforms.remove(&key);
        self.new_instances.remove(&key);
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

    /// Set the world matrix of a render instance. Returns false if key is invalid.
    pub fn set_world_matrix(&mut self, key: RenderInstanceKey, matrix: Mat4) -> bool {
        if let Some(instance) = self.render_instances.get_mut(key) {
            instance.set_world_matrix(matrix);
            self.dirty_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Get the set of instances with pending transform changes.
    pub fn dirty_transforms(&self) -> &HashSet<RenderInstanceKey> {
        &self.dirty_transforms
    }

    /// Take and clear the dirty transform set.
    pub fn take_dirty_transforms(&mut self) -> HashSet<RenderInstanceKey> {
        std::mem::take(&mut self.dirty_transforms)
    }

    /// Get the set of newly created instances pending GPU initialization.
    pub fn new_instances(&self) -> &HashSet<RenderInstanceKey> {
        &self.new_instances
    }

    /// Take and clear the new instances set.
    pub fn take_new_instances(&mut self) -> HashSet<RenderInstanceKey> {
        std::mem::take(&mut self.new_instances)
    }

    /// Iterate over all render instances (key, instance)
    pub fn render_instances(
        &self,
    ) -> impl Iterator<Item = (RenderInstanceKey, &RenderInstance)> {
        self.render_instances.iter()
    }

    /// Get the number of render instances
    pub fn render_instance_count(&self) -> usize {
        self.render_instances.len()
    }

    /// Get the per-frame uniform buffer
    pub fn frame_buffer(&self) -> &Arc<Buffer> {
        &self.frame_buffer
    }

    /// Get the per-instance storage buffer
    pub fn instance_buffer(&self) -> &Arc<Buffer> {
        &self.instance_buffer
    }

    /// Get the material storage buffer
    pub fn material_buffer(&self) -> &Arc<Buffer> {
        &self.material_buffer
    }

    /// Get the global binding group (Set 0: frame UBO + instance SSBO + material SSBO).
    ///
    /// Returns None if no instance has been created yet.
    pub fn global_binding_group(&self) -> Option<&Arc<dyn BindingGroup>> {
        self.global_binding_group.as_ref()
    }

    /// Lazily create the global binding group (Set 0) from the first pipeline encountered.
    ///
    /// All pipelines must declare the same Set 0 layout (frame UBO + instance SSBO +
    /// material SSBO). We use the first pipeline from the mesh to create the descriptor set.
    fn ensure_global_binding_group(&mut self, mesh: &Mesh, variant_index: usize) -> Result<()> {
        if self.global_binding_group.is_some() {
            return Ok(());
        }

        let mesh_lod = mesh.lod(0)
            .ok_or_else(|| engine_err!("galaxy3d::Scene", "Mesh has no LODs"))?;
        let submesh = mesh_lod.submesh(0)
            .ok_or_else(|| engine_err!("galaxy3d::Scene", "MeshLOD has no submeshes"))?;
        let variant = submesh.material().pipeline().variant(variant_index as u32)
            .ok_or_else(|| engine_err!("galaxy3d::Scene",
                "Pipeline variant {} not found", variant_index))?;
        let pass = variant.pass(0)
            .ok_or_else(|| engine_err!("galaxy3d::Scene",
                "Pipeline variant has no passes"))?;

        let graphics_device_lock = self.graphics_device.lock().unwrap();
        let bg = graphics_device_lock.create_binding_group(
            pass.graphics_device_pipeline(),
            0,
            &[
                BindingResource::UniformBuffer(self.frame_buffer.graphics_device_buffer().as_ref()),
                BindingResource::StorageBuffer(self.instance_buffer.graphics_device_buffer().as_ref()),
                BindingResource::StorageBuffer(self.material_buffer.graphics_device_buffer().as_ref()),
            ],
        )?;

        self.global_binding_group = Some(bg);
        Ok(())
    }

    /// Remove all render instances and reset the draw slot allocator
    pub fn clear(&mut self) {
        self.render_instances.clear();
        self.draw_slot_allocator = SlotAllocator::new();
        self.dirty_transforms.clear();
        self.new_instances.clear();
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
