/// Update strategies.
///
/// An Updater synchronizes scene data to GPU buffers each frame.
/// Four phases: per-frame camera data, per-instance data, per-light data,
/// and per-instance light assignment (post-culling).

use glam::Vec3;
use crate::error::Result;
use crate::camera::{Camera, VisibleInstances};
use crate::resource::buffer::Buffer;
use super::scene::Scene;
use super::scene_index::SceneIndex;
use super::render_instance::AABB;
use super::light::{LightType, LightKey};

/// Strategy for synchronizing scene data to GPU buffers.
///
/// Called once per frame before culling. GPU buffers are passed directly
/// to each method — the Scene does not hold any buffer references.
///
/// `&mut self` allows stateful implementations to track dirty state.
pub trait Updater: Send + Sync {
    /// Update the per-frame uniform buffer from the camera state.
    ///
    /// Writes camera matrices (view, projection, view-projection)
    /// and other per-frame data into `frame_buffer`.
    fn update_frame(&mut self, camera: &Camera, frame_buffer: &Buffer) -> Result<()>;

    /// Update the per-instance storage buffer from dirty instances.
    ///
    /// Processes removed, new, and dirty instances:
    /// - Removed: drains + deletes from Scene, then cleans up SceneIndex
    /// - New: writes all GPU fields + inserts into SceneIndex
    /// - Dirty: writes transform fields + updates SceneIndex
    fn update_instances(
        &mut self,
        scene: &mut Scene,
        scene_index: Option<&mut dyn SceneIndex>,
        instance_buffer: &Buffer,
    ) -> Result<()>;

    /// Update the per-light storage buffer from dirty lights.
    ///
    /// Processes removed, new, and dirty lights:
    /// - Removed: drains + frees light slots
    /// - New: writes all GPU fields to light buffer
    /// - Dirty transforms: writes position/type + direction/range
    /// - Dirty data: writes color/intensity + spot params + attenuation
    fn update_lights(&mut self, scene: &mut Scene, light_buffer: &Buffer) -> Result<()>;

    /// Assign lights to visible instances (post-culling).
    ///
    /// For each visible instance in the VisibleInstances, tests all enabled lights
    /// against the instance's world AABB (sphere-AABB for range, cone-AABB
    /// for spots), scores by intensity/distance², takes the top 8, and writes
    /// lightCount + lightIndices0/1 to the instance buffer.
    fn assign_lights(&mut self, scene: &Scene, visible: &VisibleInstances, instance_buffer: &Buffer) -> Result<()>;
}

/// No-op updater — does nothing.
///
/// Placeholder for scenes that don't need GPU buffer synchronization.
pub struct NoOpUpdater;

impl NoOpUpdater {
    pub fn new() -> Self {
        Self
    }
}

impl Updater for NoOpUpdater {
    fn update_frame(&mut self, _camera: &Camera, _frame_buffer: &Buffer) -> Result<()> {
        Ok(())
    }

    fn update_instances(
        &mut self,
        _scene: &mut Scene,
        _scene_index: Option<&mut dyn SceneIndex>,
        _instance_buffer: &Buffer,
    ) -> Result<()> {
        Ok(())
    }

    fn update_lights(&mut self, _scene: &mut Scene, _light_buffer: &Buffer) -> Result<()> {
        Ok(())
    }

    fn assign_lights(&mut self, _scene: &Scene, _visible: &VisibleInstances, _instance_buffer: &Buffer) -> Result<()> {
        Ok(())
    }
}

/// Default updater — synchronizes camera and instance data to GPU buffers.
///
/// Writes per-frame camera data into the Scene's frame buffer,
/// and per-instance world matrices into the Scene's instance buffer.
///
/// Assumes the Scene's frame buffer was created with
/// `ResourceManager::create_default_frame_uniform_buffer()` whose layout is:
///   0: view (Mat4), 1: projection (Mat4), 2: viewProjection (Mat4), ...
///
/// Assumes the Scene's instance buffer was created with
/// `ResourceManager::create_default_instance_buffer()` whose layout is:
///   0: world (Mat4), 1: previousWorld (Mat4), 2: inverseWorld (Mat4),
///   3: materialSlotId (UInt), 4: flags (UInt), 5: lightCount (UInt), ...
pub struct DefaultUpdater {
    /// Pre-allocated buffer for the keys of currently enabled lights.
    /// Reused across frames via clear() + repush — zero allocation in steady
    /// state once the high-water mark of enabled lights has been reached.
    enabled_light_keys: Vec<LightKey>,
    /// Pre-allocated buffer for the per-instance light scoring candidates
    /// `(light_slot, score)`. Reused across frames AND across visible
    /// instances within a frame via clear() + repush — zero allocation in
    /// steady state.
    candidates: Vec<(u32, f32)>,
}

impl DefaultUpdater {
    /// Field indices matching `create_default_frame_uniform_buffer()` layout
    const FRAME_FIELD_VIEW: usize              = 0;
    const FRAME_FIELD_PROJECTION: usize        = 1;
    const FRAME_FIELD_VIEW_PROJECTION: usize   = 2;
    const FRAME_FIELD_CAMERA_POSITION: usize   = 3;
    const FRAME_FIELD_CAMERA_DIRECTION: usize  = 4;

    /// Field indices matching `create_default_instance_buffer()` layout
    const INSTANCE_FIELD_WORLD: usize            = 0;
    const INSTANCE_FIELD_PREVIOUS_WORLD: usize   = 1;
    const INSTANCE_FIELD_INVERSE_WORLD: usize    = 2;
    const INSTANCE_FIELD_MATERIAL_SLOT_ID: usize = 3;
    const INSTANCE_FIELD_FLAGS: usize            = 4;
    const INSTANCE_FIELD_LIGHT_COUNT: usize      = 5;
    const INSTANCE_FIELD_LIGHT_INDICES_0: usize = 7;
    const INSTANCE_FIELD_LIGHT_INDICES_1: usize = 8;

    /// Maximum number of lights per instance (2 × UVec4 = 8 slots)
    const MAX_LIGHTS_PER_INSTANCE: usize = 8;

    /// Field indices matching `create_default_light_buffer()` layout
    const LIGHT_FIELD_POSITION_TYPE: usize    = 0;
    const LIGHT_FIELD_DIRECTION_RANGE: usize  = 1;
    const LIGHT_FIELD_COLOR_INTENSITY: usize  = 2;
    const LIGHT_FIELD_SPOT_PARAMS: usize      = 3;
    const LIGHT_FIELD_ATTENUATION: usize      = 4;

    pub fn new() -> Self {
        Self {
            enabled_light_keys: Vec::new(),
            candidates: Vec::new(),
        }
    }
}

impl Updater for DefaultUpdater {
    fn update_frame(&mut self, camera: &Camera, frame_buffer: &Buffer) -> Result<()> {
        let buf = frame_buffer;
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();
        let view_proj = camera.view_projection_matrix();

        buf.update_field(0, Self::FRAME_FIELD_VIEW,            bytemuck::bytes_of(view))?;
        buf.update_field(0, Self::FRAME_FIELD_PROJECTION,      bytemuck::bytes_of(proj))?;
        buf.update_field(0, Self::FRAME_FIELD_VIEW_PROJECTION, bytemuck::bytes_of(&view_proj))?;

        // Extract camera position & forward direction from view matrix inverse
        let inv_view = view.inverse();
        let pos = inv_view.col(3).truncate();
        let camera_pos: [f32; 4] = [pos.x, pos.y, pos.z, 1.0];
        buf.update_field(0, Self::FRAME_FIELD_CAMERA_POSITION, bytemuck::bytes_of(&camera_pos))?;

        let fwd = -inv_view.col(2).truncate();
        let camera_dir: [f32; 4] = [fwd.x, fwd.y, fwd.z, 0.0];
        buf.update_field(0, Self::FRAME_FIELD_CAMERA_DIRECTION, bytemuck::bytes_of(&camera_dir))?;

        Ok(())
    }

    fn update_instances(
        &mut self,
        scene: &mut Scene,
        mut scene_index: Option<&mut dyn SceneIndex>,
        instance_buffer: &Buffer,
    ) -> Result<()> {
        // Phase 0: removals — removed_instances() flips the SwapSet, frees draw
        // slots and removes from SlotMap, then we clean up the SceneIndex.
        {
            let removed_keys = scene.removed_instances();
            if let Some(ref mut idx) = scene_index {
                for key in removed_keys {
                    idx.remove(*key);
                }
            }
        }

        // Phase 1: new instances — write ALL GPU fields + insert into SceneIndex.
        // Lock the ResourceManager once for the whole new-instances loop, only
        // if there is anything to do (avoids paying the lock for an empty list).
        let new_keys = scene.new_instances();
        if !new_keys.is_empty() {
            let rm_arc = crate::engine::Engine::resource_manager()?;
            let rm = rm_arc.lock().unwrap();

            for key in new_keys {
                let instance = match scene.render_instance(*key) {
                    Some(inst) => inst,
                    None => continue,
                };

                let world = *instance.world_matrix();
                let inverse_world = world.inverse();
                let flags = instance.flags() as u32;

                for sm_idx in 0..instance.sub_mesh_count() {
                    let sub_mesh = instance.sub_mesh(sm_idx).unwrap();
                    let slot = sub_mesh.draw_slot();
                    // Look up the material slot id from the first pass of the
                    // submesh. When multiple passes reference different materials,
                    // the SSBO slot will need to change — for now V1 uses passes[0].
                    let material_slot_id = rm.material(
                            sub_mesh.pass_by_index(0).unwrap().material()
                        )
                        .ok_or_else(|| crate::engine_err!("galaxy3d::DefaultUpdater",
                            "Material key not found in ResourceManager"))?
                        .slot_id();

                    instance_buffer.update_field(slot, Self::INSTANCE_FIELD_WORLD,
                        bytemuck::bytes_of(&world))?;
                    instance_buffer.update_field(slot, Self::INSTANCE_FIELD_PREVIOUS_WORLD,
                        bytemuck::bytes_of(&world))?;
                    instance_buffer.update_field(slot, Self::INSTANCE_FIELD_INVERSE_WORLD,
                        bytemuck::bytes_of(&inverse_world))?;
                    instance_buffer.update_field(slot, Self::INSTANCE_FIELD_MATERIAL_SLOT_ID,
                        bytemuck::bytes_of(&material_slot_id))?;
                    instance_buffer.update_field(slot, Self::INSTANCE_FIELD_FLAGS,
                        bytemuck::bytes_of(&flags))?;
                }

                if let Some(ref mut idx) = scene_index {
                    let world_aabb = instance.bounding_box().transformed(&world);
                    let world_position = world.w_axis.truncate();
                    idx.insert(*key, world_position, &world_aabb);
                }
            }
        }

        // Phase 2: dirty instance transforms — write matrices + update SceneIndex
        let dirty_keys = scene.dirty_instance_transforms();
        for key in dirty_keys {
            let instance = match scene.render_instance(*key) {
                Some(inst) => inst,
                None => continue,
            };

            let world = *instance.world_matrix();
            let inverse_world = world.inverse();
            for sm_idx in 0..instance.sub_mesh_count() {
                let sub_mesh = instance.sub_mesh(sm_idx).unwrap();
                let slot = sub_mesh.draw_slot();

                let buf = instance_buffer;
                buf.update_field(slot, Self::INSTANCE_FIELD_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_PREVIOUS_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_INVERSE_WORLD,
                    bytemuck::bytes_of(&inverse_world))?;
            }

            if let Some(ref mut idx) = scene_index {
                let world_aabb = instance.bounding_box().transformed(&world);
                let world_position = world.w_axis.truncate();
                idx.update(*key, world_position, &world_aabb);
            }
        }

        Ok(())
    }

    fn update_lights(&mut self, scene: &mut Scene, light_buffer: &Buffer) -> Result<()> {
        // Phase 0: removals — free light slots + remove from SlotMap
        let _ = scene.removed_lights();

        // Phase 1: new lights — write ALL 5 fields to light buffer
        let new_keys = scene.new_lights();
        for key in new_keys {
            let light = match scene.light(*key) {
                Some(l) => l,
                None => continue,
            };
            let slot = light.light_slot();
            let type_id = match light.light_type() {
                LightType::Point => 0.0f32,
                LightType::Spot  => 1.0f32,
            };

            let buf = light_buffer;
            buf.update_field(slot, Self::LIGHT_FIELD_POSITION_TYPE,
                bytemuck::bytes_of(&[light.position().x, light.position().y, light.position().z, type_id]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_DIRECTION_RANGE,
                bytemuck::bytes_of(&[light.direction().x, light.direction().y, light.direction().z, light.range()]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_COLOR_INTENSITY,
                bytemuck::bytes_of(&[light.color().x, light.color().y, light.color().z, light.intensity()]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_SPOT_PARAMS,
                bytemuck::bytes_of(&[light.spot_inner_angle(), light.spot_outer_angle(), 0.0f32, 0.0f32]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_ATTENUATION,
                bytemuck::bytes_of(&[light.attenuation_constant(), light.attenuation_linear(), light.attenuation_quadratic(), 0.0f32]))?;
        }

        // Phase 2: dirty transforms — write positionType + directionRange
        let dirty_transforms = scene.dirty_light_transforms();
        for key in dirty_transforms {
            let light = match scene.light(*key) {
                Some(l) => l,
                None => continue,
            };
            let slot = light.light_slot();
            let type_id = match light.light_type() {
                LightType::Point => 0.0f32,
                LightType::Spot  => 1.0f32,
            };

            let buf = light_buffer;
            buf.update_field(slot, Self::LIGHT_FIELD_POSITION_TYPE,
                bytemuck::bytes_of(&[light.position().x, light.position().y, light.position().z, type_id]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_DIRECTION_RANGE,
                bytemuck::bytes_of(&[light.direction().x, light.direction().y, light.direction().z, light.range()]))?;
        }

        // Phase 3: dirty data — write colorIntensity + spotParams + attenuation
        let dirty_data = scene.dirty_light_data();
        for key in dirty_data {
            let light = match scene.light(*key) {
                Some(l) => l,
                None => continue,
            };
            let slot = light.light_slot();

            let buf = light_buffer;
            buf.update_field(slot, Self::LIGHT_FIELD_COLOR_INTENSITY,
                bytemuck::bytes_of(&[light.color().x, light.color().y, light.color().z, light.intensity()]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_SPOT_PARAMS,
                bytemuck::bytes_of(&[light.spot_inner_angle(), light.spot_outer_angle(), 0.0f32, 0.0f32]))?;
            buf.update_field(slot, Self::LIGHT_FIELD_ATTENUATION,
                bytemuck::bytes_of(&[light.attenuation_constant(), light.attenuation_linear(), light.attenuation_quadratic(), 0.0f32]))?;
        }

        Ok(())
    }

    fn assign_lights(&mut self, scene: &Scene, visible: &VisibleInstances, instance_buffer: &Buffer) -> Result<()> {
        // Refresh the persistent enabled-light-keys buffer.
        // clear() preserves capacity → no allocation in steady state.
        self.enabled_light_keys.clear();
        for (key, light) in scene.lights() {
            if light.enabled() {
                self.enabled_light_keys.push(key);
            }
        }

        if self.enabled_light_keys.is_empty() {
            // Write lightCount = 0 for all visible instances
            let zero_count = 0u32;
            let zero_indices = [0u32; 4];
            for vi in visible.instances().iter() {
                let inst_key = vi.key;
                let instance = match scene.render_instance(inst_key) {
                    Some(i) => i,
                    None => continue,
                };
                let buf = instance_buffer;
                for sm_idx in 0..instance.sub_mesh_count() {
                    let slot = instance.sub_mesh(sm_idx).unwrap().draw_slot();
                    buf.update_field(slot, Self::INSTANCE_FIELD_LIGHT_COUNT,
                        bytemuck::bytes_of(&zero_count))?;
                    buf.update_field(slot, Self::INSTANCE_FIELD_LIGHT_INDICES_0,
                        bytemuck::bytes_of(&zero_indices))?;
                    buf.update_field(slot, Self::INSTANCE_FIELD_LIGHT_INDICES_1,
                        bytemuck::bytes_of(&zero_indices))?;
                }
            }
            return Ok(());
        }

        // The light candidates buffer is persistent (field of DefaultUpdater),
        // reused across frames AND across visible instances within a frame.

        for vi in visible.instances().iter() {
            let inst_key = vi.key;
            let instance = match scene.render_instance(inst_key) {
                Some(i) => i,
                None => continue,
            };

            let world = *instance.world_matrix();
            let world_aabb = instance.bounding_box().transformed(&world);
            let aabb_center = world_aabb.center();

            self.candidates.clear();

            for &light_key in &self.enabled_light_keys {
                let light = match scene.light(light_key) {
                    Some(l) => l,
                    None => continue,
                };
                let light_pos = light.position();
                let range = light.range();
                let range_sq = range * range;

                // Sphere-AABB range test (shared by Point and Spot)
                let closest = world_aabb.closest_point(light_pos);
                let dist_sq = (light_pos - closest).length_squared();
                if dist_sq > range_sq {
                    continue;
                }

                // Spot cone-AABB test
                if light.light_type() == LightType::Spot {
                    let tan_outer = light.spot_outer_angle().tan();
                    if !cone_intersects_aabb(
                        light_pos, light.direction(), range, tan_outer, &world_aabb,
                    ) {
                        continue;
                    }
                }

                // Score: intensity / distance² (distance to AABB center)
                let center_dist_sq = (light_pos - aabb_center).length_squared().max(0.01);
                let score = light.intensity() / center_dist_sq;

                self.candidates.push((light.light_slot(), score));
            }

            // Sort by score descending, take top MAX_LIGHTS_PER_INSTANCE
            self.candidates.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let count = self.candidates.len().min(Self::MAX_LIGHTS_PER_INSTANCE);

            // Build GPU data
            let light_count = count as u32;
            let mut indices_0 = [0u32; 4];
            let mut indices_1 = [0u32; 4];
            for i in 0..count {
                if i < 4 {
                    indices_0[i] = self.candidates[i].0;
                } else {
                    indices_1[i - 4] = self.candidates[i].0;
                }
            }

            // Write to all submesh draw slots
            for sm_idx in 0..instance.sub_mesh_count() {
                let slot = instance.sub_mesh(sm_idx).unwrap().draw_slot();
                instance_buffer.update_field(slot, Self::INSTANCE_FIELD_LIGHT_COUNT,
                    bytemuck::bytes_of(&light_count))?;
                instance_buffer.update_field(slot, Self::INSTANCE_FIELD_LIGHT_INDICES_0,
                    bytemuck::bytes_of(&indices_0))?;
                instance_buffer.update_field(slot, Self::INSTANCE_FIELD_LIGHT_INDICES_1,
                    bytemuck::bytes_of(&indices_1))?;
            }
        }

        Ok(())
    }
}

// ===== CONE-AABB INTERSECTION =====

/// Test if a cone (defined by apex, direction, range, and tan of outer angle)
/// intersects an AABB.
///
/// Uses iterative closest-point refinement between the cone axis segment
/// and the AABB. The cone axis segment is [apex, apex + dir * range].
/// At parameter t ∈ [0,1], the cone radius is t * range * tan_outer.
///
/// The iteration alternates between finding the closest point on the segment
/// to the AABB and vice versa, converging to the true closest pair.
fn cone_intersects_aabb(
    apex: Vec3,
    dir: Vec3,
    range: f32,
    tan_outer: f32,
    aabb: &AABB,
) -> bool {
    let end = apex + dir * range;

    // Start from AABB center
    let mut aabb_point = aabb.center();

    // Iterative closest-point refinement (converges in 2-3 iterations)
    for _ in 0..3 {
        // Closest point on segment to current AABB point
        let (seg_point, t) = closest_point_on_segment(apex, end, aabb_point);
        // Closest point on AABB to that segment point
        aabb_point = aabb.closest_point(seg_point);

        // Check if the AABB point is within the cone radius at parameter t
        let radius = t * range * tan_outer;
        let dist_sq = (seg_point - aabb_point).length_squared();
        if dist_sq <= radius * radius {
            return true;
        }
    }

    false
}

/// Closest point on segment [a, b] to point p.
///
/// Returns (closest_point, parameter_t) where t ∈ [0, 1].
fn closest_point_on_segment(a: Vec3, b: Vec3, p: Vec3) -> (Vec3, f32) {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-12 {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (a + ab * t, t)
}
