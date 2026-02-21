/// Drawing strategies.
///
/// A Drawer renders visible instances from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use glam::Mat4;
use crate::error::Result;
use crate::renderer::{CommandList, ShaderStage};
use crate::camera::RenderView;
use super::scene::Scene;

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

/// Strategy for drawing visible instances.
///
/// Called within an active render pass. The Drawer reads from the Scene
/// (immutably) and issues draw commands into the command list.
///
/// `&self` because drawing is stateless — the same Drawer can be
/// reused across multiple scenes and frames.
pub trait Drawer: Send + Sync {
    /// Draw visible instances from the RenderView into the command list.
    fn draw(&self, scene: &Scene, view: &RenderView, cmd: &mut dyn CommandList) -> Result<()>;
}

/// Forward drawer — draws each instance sequentially (no sorting, no instancing).
///
/// V1 implementation: LOD 0 only, pushes MVP + Model matrices as push constants.
pub struct ForwardDrawer;

impl ForwardDrawer {
    pub fn new() -> Self {
        Self
    }
}

impl Drawer for ForwardDrawer {
    fn draw(&self, scene: &Scene, view: &RenderView, cmd: &mut dyn CommandList) -> Result<()> {
        let camera = view.camera();
        let view_proj = camera.view_projection_matrix();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        for &key in view.visible_instances() {
            let instance = match scene.render_instance(key) {
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
