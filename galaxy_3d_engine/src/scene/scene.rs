/// Scene — a collection of RenderInstances for rendering.
///
/// Uses a SlotMap for O(1) insert/remove with stable keys.
/// Instances are stored contiguously for cache-friendly iteration.

use std::sync::{Arc, Mutex};
use slotmap::SlotMap;
use glam::Mat4;
use crate::error::Result;
use crate::renderer::{self, CommandList, ShaderStage};
use crate::resource::mesh::Mesh;
use crate::camera::{Camera, RenderView};
use super::render_instance::{
    RenderInstance, RenderInstanceKey, AABB,
};

/// Convert a Mat4 to a byte slice (64 bytes, column-major).
///
/// Safe for glam::Mat4 which is `#[repr(C)]` containing only f32 values.
fn mat4_as_bytes(m: &Mat4) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(
            m as *const Mat4 as *const u8,
            std::mem::size_of::<Mat4>(),
        )
    }
}

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
}

impl Scene {
    /// Create a new empty scene (internal: only via SceneManager)
    pub(crate) fn new(renderer: Arc<Mutex<dyn renderer::Renderer>>) -> Self {
        Self {
            renderer,
            render_instances: SlotMap::with_key(),
        }
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
            mesh, world_matrix, bounding_box, variant_index, &self.renderer,
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

    /// Remove all render instances
    pub fn clear(&mut self) {
        self.render_instances.clear();
    }

    /// Cull visible instances against the camera's frustum.
    ///
    /// Returns a RenderView containing a camera snapshot and the keys
    /// of all visible instances. The RenderView is ephemeral (one frame)
    /// and can be shared across multiple render passes.
    ///
    /// V1: returns ALL instances (no actual frustum culling).
    pub fn frustum_cull(&self, camera: &Camera) -> RenderView {
        let visible_instances: Vec<RenderInstanceKey> =
            self.render_instances.keys().collect();
        RenderView::new(camera.clone(), visible_instances)
    }

    /// Draw visible instances from a RenderView into a command list.
    ///
    /// Must be called within an active render pass (between begin_render_pass
    /// and end_render_pass). Sets viewport and scissor from the camera.
    ///
    /// V1: Uses LOD 0, pushes MVP (offset 0) and Model (offset 64) matrices
    /// as vertex shader push constants (128 bytes total).
    pub fn draw(
        &self,
        view: &RenderView,
        cmd: &mut dyn CommandList,
    ) -> Result<()> {
        let camera = view.camera();
        let view_proj = camera.view_projection_matrix();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        for &key in view.visible_instances() {
            let instance = match self.render_instances.get(key) {
                Some(inst) => inst,
                None => continue, // removed between cull and draw
            };

            let model = *instance.world_matrix();
            let mvp = view_proj * model;

            // Bind shared buffers
            cmd.bind_vertex_buffer(instance.vertex_buffer(), 0)?;
            if let Some(ib) = instance.index_buffer() {
                cmd.bind_index_buffer(ib, 0, instance.index_type())?;
            }

            // LOD 0 only (V1)
            let lod = match instance.lod(0) {
                Some(lod) => lod,
                None => continue,
            };

            for sm_idx in 0..lod.sub_mesh_count() {
                let sub_mesh = lod.sub_mesh(sm_idx).unwrap();

                for pass in sub_mesh.passes() {
                    cmd.bind_pipeline(pass.pipeline())?;

                    // Material binding groups
                    for (set_idx, bg) in pass.binding_groups().iter().enumerate() {
                        cmd.bind_binding_group(pass.pipeline(), set_idx as u32, bg)?;
                    }

                    // Engine push constants: MVP (offset 0) + Model (offset 64)
                    cmd.push_constants(
                        &[ShaderStage::Vertex], 0, mat4_as_bytes(&mvp),
                    )?;
                    cmd.push_constants(
                        &[ShaderStage::Vertex], 64, mat4_as_bytes(&model),
                    )?;

                    // Material push constants (at their reflected offsets)
                    for pc in pass.push_constants() {
                        let bytes = pc.value().as_bytes();
                        cmd.push_constants(
                            &[ShaderStage::Vertex], pc.offset(), &bytes,
                        )?;
                    }

                    // Issue draw call
                    if sub_mesh.index_count() > 0 {
                        cmd.draw_indexed(
                            sub_mesh.index_count(),
                            sub_mesh.index_offset(),
                            sub_mesh.vertex_offset() as i32,
                        )?;
                    } else {
                        cmd.draw(sub_mesh.vertex_count(), sub_mesh.vertex_offset())?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
