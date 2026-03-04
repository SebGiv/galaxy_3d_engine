/// Scene — a collection of RenderInstances and Lights for rendering.
///
/// Uses SlotMaps for O(1) insert/remove with stable keys.
/// Instances and lights are stored contiguously for cache-friendly iteration.

use rustc_hash::FxHashSet;
use std::sync::{Arc, Mutex};
use slotmap::SlotMap;
use glam::{Mat4, Vec3};
use crate::error::Result;
use crate::engine_err;
use crate::graphics_device::{self, BindingGroup, BindingResource};
use crate::resource::buffer::Buffer;
use crate::resource::mesh::Mesh;
use crate::utils::{SlotAllocator, SwapSet};
use super::render_instance::{
    RenderInstance, RenderInstanceKey, AABB,
};
use super::light::{Light, LightKey, LightType, LightDesc};

/// A renderable scene containing RenderInstances and Lights.
///
/// Instances and lights are managed via stable keys (RenderInstanceKey, LightKey).
/// Keys remain valid even after other entries are removed.
/// The scene holds a reference to the GraphicsDevice for creating GPU binding groups.
pub struct Scene {
    /// Graphics device for creating GPU resources (binding groups)
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,

    // ----- Render Instances -----

    /// Render instances stored in a slot map for O(1) insert/remove
    render_instances: SlotMap<RenderInstanceKey, RenderInstance>,
    /// Allocator for unique draw slot indices (one per submesh in the GPU scene SSBO)
    draw_slot_allocator: SlotAllocator,
    /// Instances whose world matrix changed since last frame
    dirty_instance_transforms: SwapSet<RenderInstanceKey>,
    /// Newly created instances pending full GPU buffer initialization
    new_instances: SwapSet<RenderInstanceKey>,
    /// Instances marked for deferred removal (processed by Updater)
    removed_instances: SwapSet<RenderInstanceKey>,

    // ----- Lights -----

    /// Lights stored in a slot map for O(1) insert/remove
    lights: SlotMap<LightKey, Light>,
    /// Allocator for unique light slot indices (one per light in the GPU light SSBO)
    light_slot_allocator: SlotAllocator,
    /// Newly created lights pending full GPU buffer initialization
    new_lights: SwapSet<LightKey>,
    /// Lights whose spatial data changed (position, direction, range, type)
    dirty_light_transforms: SwapSet<LightKey>,
    /// Lights whose visual data changed (color, intensity, attenuation, spot angles, enabled)
    dirty_light_data: SwapSet<LightKey>,
    /// Lights marked for deferred removal
    removed_lights: SwapSet<LightKey>,

    // ----- GPU Buffers -----

    /// Per-frame uniform buffer (camera, lighting, time, post-process)
    frame_buffer: Arc<Buffer>,
    /// Per-instance storage buffer (world matrices, material slot, flags)
    instance_buffer: Arc<Buffer>,
    /// Material storage buffer (shared material parameters)
    material_buffer: Arc<Buffer>,
    /// Light storage buffer (shared light parameters)
    light_buffer: Arc<Buffer>,
    /// Set 0 binding group (frame UBO + instance SSBO + material SSBO + light SSBO), shared by all instances
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
    /// * `light_buffer` - Light storage buffer (shared light parameters)
    pub(crate) fn new(
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        frame_buffer: Arc<Buffer>,
        instance_buffer: Arc<Buffer>,
        material_buffer: Arc<Buffer>,
        light_buffer: Arc<Buffer>,
    ) -> Self {
        Self {
            graphics_device,
            render_instances: SlotMap::with_key(),
            draw_slot_allocator: SlotAllocator::new(),
            dirty_instance_transforms: SwapSet::new(),
            new_instances: SwapSet::new(),
            removed_instances: SwapSet::new(),
            lights: SlotMap::with_key(),
            light_slot_allocator: SlotAllocator::new(),
            new_lights: SwapSet::new(),
            dirty_light_transforms: SwapSet::new(),
            dirty_light_data: SwapSet::new(),
            removed_lights: SwapSet::new(),
            frame_buffer,
            instance_buffer,
            material_buffer,
            light_buffer,
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

    /// Mark a RenderInstance for deferred removal.
    ///
    /// The instance stays in the scene until `removed_instances()` is called,
    /// which drains the removal set and deletes the instances from the SlotMap.
    /// Returns false if the key is invalid.
    pub fn remove_render_instance(&mut self, key: RenderInstanceKey) -> bool {
        if self.render_instances.contains_key(key) {
            self.removed_instances.insert(key);
            self.dirty_instance_transforms.remove(&key);
            self.new_instances.remove(&key);
            true
        } else {
            false
        }
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
            self.dirty_instance_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Flip and return the set of instances with pending transform changes.
    ///
    /// Returns the dirty keys accumulated since the previous call.
    /// Zero allocation — uses double-buffered SwapSet internally.
    pub fn dirty_instance_transforms(&self) -> &FxHashSet<RenderInstanceKey> {
        self.dirty_instance_transforms.flip()
    }

    /// Flip and return the set of newly created instances pending GPU initialization.
    ///
    /// Returns the new keys accumulated since the previous call.
    pub fn new_instances(&self) -> &FxHashSet<RenderInstanceKey> {
        self.new_instances.flip()
    }

    /// Flip the removal set, free draw slots, remove from SlotMap,
    /// and return the removed keys (for SceneIndex cleanup).
    pub fn removed_instances(&mut self) -> &FxHashSet<RenderInstanceKey> {
        let keys = self.removed_instances.flip();
        for &key in keys.iter() {
            if let Some(instance) = self.render_instances.get(key) {
                instance.free_draw_slots(&mut self.draw_slot_allocator);
            }
            self.render_instances.remove(key);
        }
        keys
    }

    /// Check if an instance has a pending dirty transform (front buffer).
    pub fn has_dirty_instance_transform(&self, key: RenderInstanceKey) -> bool {
        self.dirty_instance_transforms.contains(&key)
    }

    /// Number of instances with pending dirty transforms (front buffer).
    pub fn dirty_instance_transform_count(&self) -> usize {
        self.dirty_instance_transforms.len()
    }

    /// Check if an instance is in the new instances set (front buffer).
    pub fn has_new_instance(&self, key: RenderInstanceKey) -> bool {
        self.new_instances.contains(&key)
    }

    /// Number of newly created instances pending GPU init (front buffer).
    pub fn new_instance_count(&self) -> usize {
        self.new_instances.len()
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

    /// Get the light storage buffer
    pub fn light_buffer(&self) -> &Arc<Buffer> {
        &self.light_buffer
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
    /// material SSBO + light SSBO). We use the first pipeline from the mesh to create
    /// the descriptor set.
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
                BindingResource::StorageBuffer(self.light_buffer.graphics_device_buffer().as_ref()),
            ],
        )?;

        self.global_binding_group = Some(bg);
        Ok(())
    }

    // ===== LIGHTS =====

    /// Create a Light from a descriptor and add it to the scene.
    ///
    /// Returns a stable key that remains valid until the light is removed.
    pub fn create_light(&mut self, desc: LightDesc) -> LightKey {
        let mut light = Light::from_desc(desc);
        light.light_slot = self.light_slot_allocator.alloc();
        let key = self.lights.insert(light);
        self.new_lights.insert(key);
        key
    }

    /// Mark a Light for deferred removal.
    ///
    /// The light stays in the scene until `removed_lights()` is called.
    /// Returns false if the key is invalid.
    pub fn remove_light(&mut self, key: LightKey) -> bool {
        if self.lights.contains_key(key) {
            self.removed_lights.insert(key);
            self.dirty_light_transforms.remove(&key);
            self.dirty_light_data.remove(&key);
            self.new_lights.remove(&key);
            true
        } else {
            false
        }
    }

    /// Get a Light by key.
    pub fn light(&self, key: LightKey) -> Option<&Light> {
        self.lights.get(key)
    }

    /// Replace a Light entirely. Marks both dirty sets (spatial + data).
    ///
    /// Preserves the existing GPU light_slot assignment.
    /// Returns false if the key is invalid.
    pub fn set_light(&mut self, key: LightKey, desc: LightDesc) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            let slot = light.light_slot;
            *light = Light::from_desc(desc);
            light.light_slot = slot;
            self.dirty_light_transforms.insert(key);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's position. Marks dirty_light_transforms.
    pub fn set_light_position(&mut self, key: LightKey, position: Vec3) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_position(position);
            self.dirty_light_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's direction (will be normalized). Marks dirty_light_transforms.
    pub fn set_light_direction(&mut self, key: LightKey, direction: Vec3) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_direction(direction);
            self.dirty_light_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's range. Marks dirty_light_transforms.
    pub fn set_light_range(&mut self, key: LightKey, range: f32) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_range(range);
            self.dirty_light_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's type. Marks dirty_light_transforms.
    pub fn set_light_type(&mut self, key: LightKey, light_type: LightType) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_light_type(light_type);
            self.dirty_light_transforms.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's color. Marks dirty_light_data.
    pub fn set_light_color(&mut self, key: LightKey, color: Vec3) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_color(color);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's intensity. Marks dirty_light_data.
    pub fn set_light_intensity(&mut self, key: LightKey, intensity: f32) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_intensity(intensity);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's attenuation factors. Marks dirty_light_data.
    pub fn set_light_attenuation(
        &mut self,
        key: LightKey,
        constant: f32,
        linear: f32,
        quadratic: f32,
    ) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_attenuation(constant, linear, quadratic);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's spot cone angles (in radians). Marks dirty_light_data.
    pub fn set_light_spot_angles(
        &mut self,
        key: LightKey,
        inner_angle: f32,
        outer_angle: f32,
    ) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_spot_angles(inner_angle, outer_angle);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Set a light's enabled state. Marks dirty_light_data.
    pub fn set_light_enabled(&mut self, key: LightKey, enabled: bool) -> bool {
        if let Some(light) = self.lights.get_mut(key) {
            light.set_enabled(enabled);
            self.dirty_light_data.insert(key);
            true
        } else {
            false
        }
    }

    /// Flip and return the set of newly created lights pending GPU initialization.
    pub fn new_lights(&self) -> &FxHashSet<LightKey> {
        self.new_lights.flip()
    }

    /// Flip and return the set of lights with pending spatial changes.
    pub fn dirty_light_transforms(&self) -> &FxHashSet<LightKey> {
        self.dirty_light_transforms.flip()
    }

    /// Flip and return the set of lights with pending visual data changes.
    pub fn dirty_light_data(&self) -> &FxHashSet<LightKey> {
        self.dirty_light_data.flip()
    }

    /// Flip the removal set, remove from SlotMap, and return the removed keys.
    pub fn removed_lights(&mut self) -> &FxHashSet<LightKey> {
        let keys = self.removed_lights.flip();
        for &key in keys.iter() {
            if let Some(light) = self.lights.get(key) {
                self.light_slot_allocator.free(light.light_slot());
            }
            self.lights.remove(key);
        }
        keys
    }

    /// Check if a light is in the new lights set (front buffer).
    pub fn has_new_light(&self, key: LightKey) -> bool {
        self.new_lights.contains(&key)
    }

    /// Number of newly created lights pending GPU init (front buffer).
    pub fn new_light_count(&self) -> usize {
        self.new_lights.len()
    }

    /// Iterate over all lights (key, light).
    pub fn lights(&self) -> impl Iterator<Item = (LightKey, &Light)> {
        self.lights.iter()
    }

    /// Get the number of lights.
    pub fn light_count(&self) -> usize {
        self.lights.len()
    }

    // ===== CLEAR =====

    /// Remove all render instances, lights, and reset allocators.
    pub fn clear(&mut self) {
        self.render_instances.clear();
        self.draw_slot_allocator = SlotAllocator::new();
        self.dirty_instance_transforms.clear();
        self.new_instances.clear();
        self.removed_instances.clear();
        self.lights.clear();
        self.light_slot_allocator = SlotAllocator::new();
        self.new_lights.clear();
        self.dirty_light_transforms.clear();
        self.dirty_light_data.clear();
        self.removed_lights.clear();
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
