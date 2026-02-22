/// Update strategies.
///
/// An Updater synchronizes scene data to GPU buffers each frame.
/// Two separate phases: per-frame camera data, then per-instance data.

use crate::error::Result;
use crate::camera::Camera;
use super::scene::Scene;

/// Strategy for synchronizing scene data to GPU buffers.
///
/// Called once per frame before culling. `&mut self` allows
/// stateful implementations to track dirty state and manage
/// GPU buffer allocations.
pub trait Updater: Send + Sync {
    /// Update the per-frame uniform buffer from the camera state.
    ///
    /// Writes camera matrices (view, projection, view-projection)
    /// and other per-frame data into the Scene's frame buffer.
    fn update_frame(&mut self, scene: &Scene, camera: &Camera) -> Result<()>;

    /// Update the per-instance storage buffer from dirty instances.
    ///
    /// Takes the dirty transform set from the Scene, then writes
    /// world matrices, material slot IDs, and flags into the
    /// Scene's instance buffer for each modified RenderInstance.
    fn update_instances(&mut self, scene: &mut Scene) -> Result<()>;
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
    fn update_frame(&mut self, _scene: &Scene, _camera: &Camera) -> Result<()> {
        Ok(())
    }

    fn update_instances(&mut self, _scene: &mut Scene) -> Result<()> {
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
///   3: materialSlotId (UInt), 4: flags (UInt), ...
pub struct DefaultUpdater;

impl DefaultUpdater {
    /// Field indices matching `create_default_frame_uniform_buffer()` layout
    const FRAME_FIELD_VIEW: usize            = 0;
    const FRAME_FIELD_PROJECTION: usize      = 1;
    const FRAME_FIELD_VIEW_PROJECTION: usize = 2;

    /// Field indices matching `create_default_instance_buffer()` layout
    const INSTANCE_FIELD_WORLD: usize            = 0;
    const INSTANCE_FIELD_PREVIOUS_WORLD: usize   = 1;
    const INSTANCE_FIELD_INVERSE_WORLD: usize    = 2;
    const INSTANCE_FIELD_MATERIAL_SLOT_ID: usize = 3;
    const INSTANCE_FIELD_FLAGS: usize            = 4;

    pub fn new() -> Self {
        Self
    }
}

impl Updater for DefaultUpdater {
    fn update_frame(&mut self, scene: &Scene, camera: &Camera) -> Result<()> {
        let buf = scene.frame_buffer();
        let view = camera.view_matrix();
        let proj = camera.projection_matrix();
        let view_proj = camera.view_projection_matrix();

        buf.update_field(0, Self::FRAME_FIELD_VIEW,            bytemuck::bytes_of(view))?;
        buf.update_field(0, Self::FRAME_FIELD_PROJECTION,      bytemuck::bytes_of(proj))?;
        buf.update_field(0, Self::FRAME_FIELD_VIEW_PROJECTION, bytemuck::bytes_of(&view_proj))?;

        Ok(())
    }

    fn update_instances(&mut self, scene: &mut Scene) -> Result<()> {
        // Phase 1: new instances — write ALL fields
        let new_keys = scene.take_new_instances();
        for key in &new_keys {
            let instance = match scene.render_instance(*key) {
                Some(inst) => inst,
                None => continue,
            };

            let world = *instance.world_matrix();
            let inverse_world = world.inverse();
            let flags = instance.flags() as u32;

            let lod = match instance.lod(0) {
                Some(lod) => lod,
                None => continue,
            };

            for sm_idx in 0..lod.sub_mesh_count() {
                let sub_mesh = lod.sub_mesh(sm_idx).unwrap();
                let slot = sub_mesh.draw_slot();
                let material_slot_id = sub_mesh.material_slot_id();

                let buf = scene.instance_buffer();
                buf.update_field(slot, Self::INSTANCE_FIELD_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_PREVIOUS_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_INVERSE_WORLD,
                    bytemuck::bytes_of(&inverse_world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_MATERIAL_SLOT_ID,
                    bytemuck::bytes_of(&material_slot_id))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_FLAGS,
                    bytemuck::bytes_of(&flags))?;
            }
        }

        // Phase 2: dirty transforms — write matrices only (skip already handled)
        let dirty_keys = scene.take_dirty_transforms();
        for key in &dirty_keys {
            if new_keys.contains(key) {
                continue;
            }

            let instance = match scene.render_instance(*key) {
                Some(inst) => inst,
                None => continue,
            };

            let world = *instance.world_matrix();
            let inverse_world = world.inverse();

            let lod = match instance.lod(0) {
                Some(lod) => lod,
                None => continue,
            };

            for sm_idx in 0..lod.sub_mesh_count() {
                let sub_mesh = lod.sub_mesh(sm_idx).unwrap();
                let slot = sub_mesh.draw_slot();

                let buf = scene.instance_buffer();
                buf.update_field(slot, Self::INSTANCE_FIELD_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_PREVIOUS_WORLD,
                    bytemuck::bytes_of(&world))?;
                buf.update_field(slot, Self::INSTANCE_FIELD_INVERSE_WORLD,
                    bytemuck::bytes_of(&inverse_world))?;
            }
        }

        Ok(())
    }
}
