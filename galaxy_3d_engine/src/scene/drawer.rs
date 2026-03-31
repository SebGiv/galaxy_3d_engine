/// Drawing strategies.
///
/// A Drawer renders visible instances from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use crate::error::Result;
use crate::engine::Engine;
use crate::graphics_device::CommandList;
use crate::camera::RenderView;
use crate::resource::resource_manager::PassInfo;
use super::scene::Scene;

/// Strategy for drawing visible instances.
///
/// Called within an active render pass. The Drawer issues draw commands
/// into the command list, resolving pipelines lazily via the pipeline cache.
///
/// `&self` because drawing is stateless — the same Drawer can be
/// reused across multiple scenes and frames.
pub trait Drawer: Send + Sync {
    /// Draw visible instances from the RenderView into the command list.
    fn draw(
        &self,
        scene: &mut Scene,
        view: &RenderView,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        render_graph_gen: u64,
    ) -> Result<()>;
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
    fn draw(
        &self,
        scene: &mut Scene,
        view: &RenderView,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        render_graph_gen: u64,
    ) -> Result<()> {
        let camera = view.camera();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        // Collect visible instance keys (avoid borrowing scene during mutation)
        let visible_keys: Vec<_> = view.visible_instances().to_vec();

        for key in &visible_keys {
            let instance = match scene.render_instance(*key) {
                Some(inst) => inst,
                None => continue,
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

            let sub_mesh_count = lod.sub_mesh_count();

            for sm_idx in 0..sub_mesh_count {
                // Read instance and submesh data
                let (vertex_shader, vertex_layout, topology, material_key, draw_slot,
                     vertex_offset, vertex_count, index_offset, index_count,
                     is_pipeline_valid) = {
                    let inst = scene.render_instance(*key).unwrap();
                    let lod = inst.lod(0).unwrap();
                    let sm = lod.sub_mesh(sm_idx).unwrap();
                    let rm_arc = Engine::resource_manager().unwrap();
                    let rm = rm_arc.lock().unwrap();
                    let mat = rm.material(sm.material()).unwrap();
                    let mat_gen = mat.generation();
                    (
                        inst.vertex_shader(),
                        inst.vertex_layout().clone(),
                        sm.topology(),
                        sm.material(),
                        sm.draw_slot(),
                        sm.vertex_offset(),
                        sm.vertex_count(),
                        sm.index_offset(),
                        sm.index_count(),
                        inst.is_pipeline_valid(render_graph_gen, mat_gen),
                    )
                };

                // Resolve pipeline if stale or missing
                if !is_pipeline_valid {
                    let rm_arc = Engine::resource_manager()?;
                    let mut rm = rm_arc.lock().unwrap();
                    let mat = rm.material(material_key).unwrap();
                    let frag_shader = mat.fragment_shader();
                    let color_blend = *mat.color_blend();
                    let polygon_mode = mat.polygon_mode();
                    let mat_gen = mat.generation();
                    let gd_arc = Engine::graphics_device("main")?;
                    let mut gd = gd_arc.lock().unwrap();
                    let pipeline_key = rm.resolve_pipeline(
                        vertex_shader, frag_shader, &vertex_layout, topology,
                        &color_blend, polygon_mode, pass_info, &mut *gd,
                    )?;
                    drop(gd);
                    drop(rm);

                    // Cache on instance
                    let inst_mut = scene.render_instance_mut(*key).unwrap();
                    inst_mut.set_cached_pipeline(pipeline_key, render_graph_gen, mat_gen);
                }

                // Draw with cached pipeline
                let inst = scene.render_instance(*key).unwrap();
                let pipeline_key = inst.cached_pipeline_key().unwrap();

                let rm_arc = Engine::resource_manager()?;
                let rm = rm_arc.lock().unwrap();
                let pipeline = rm.pipeline(pipeline_key).unwrap();
                let gd_pipeline = pipeline.graphics_device_pipeline().clone();
                let render_state = {
                    let mat = rm.material(material_key).unwrap();
                    mat.render_state().clone()
                };
                drop(rm);

                // Ensure global binding group is created (lazy, on first draw)
                scene.ensure_global_binding_group_with_pipeline(&gd_pipeline)?;

                // Bind pipeline
                cmd.bind_pipeline(&gd_pipeline)?;

                // Set dynamic render state
                cmd.set_dynamic_state(&render_state)?;

                // Set 1: global buffers (from Scene, shared across all instances)
                if let Some(global_bg) = scene.global_binding_group() {
                    cmd.bind_binding_group(&gd_pipeline, global_bg.set_index(), global_bg)?;
                }

                // Push draw slot index (shader reads instance data from SSBO)
                let reflection = gd_pipeline.reflection();
                if let Some(pc) = reflection.push_constants().first() {
                    cmd.push_constants(
                        pc.stage_flags, 0, bytemuck::bytes_of(&draw_slot),
                    )?;
                }

                // Issue draw call
                if index_count > 0 {
                    cmd.draw_indexed(index_count, index_offset, vertex_offset as i32)?;
                } else {
                    cmd.draw(vertex_count, vertex_offset)?;
                }
            }
        }

        Ok(())
    }
}
