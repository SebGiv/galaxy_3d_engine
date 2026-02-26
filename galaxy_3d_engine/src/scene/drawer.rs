/// Drawing strategies.
///
/// A Drawer renders visible instances from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use crate::error::Result;
use crate::renderer::{CommandList, ShaderStage};
use crate::camera::RenderView;
use super::scene::Scene;

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
/// V1 implementation: LOD 0 only, pushes draw slot index as push constant
/// (shader reads instance data from the per-instance SSBO).
pub struct ForwardDrawer;

impl ForwardDrawer {
    pub fn new() -> Self {
        Self
    }
}

impl Drawer for ForwardDrawer {
    fn draw(&self, scene: &Scene, view: &RenderView, cmd: &mut dyn CommandList) -> Result<()> {
        let camera = view.camera();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        for &key in view.visible_instances() {
            let instance = match scene.render_instance(key) {
                Some(inst) => inst,
                None => continue, // removed between cull and draw
            };

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

                    // Set 0: global buffers (from Scene, shared across all instances)
                    if let Some(global_bg) = scene.global_binding_group() {
                        cmd.bind_binding_group(pass.pipeline(), global_bg.set_index(), global_bg)?;
                    }

                    // Sets 1+: texture bindings (shared from Material)
                    for bg in pass.texture_binding_groups() {
                        cmd.bind_binding_group(pass.pipeline(), bg.set_index(), bg)?;
                    }

                    // Push draw slot index (shader reads instance data from SSBO)
                    let draw_slot = sub_mesh.draw_slot();
                    cmd.push_constants(
                        &[ShaderStage::Vertex], 0, bytemuck::bytes_of(&draw_slot),
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
