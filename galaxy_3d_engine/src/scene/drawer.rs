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
    ) -> Result<()> {
        let pass_info_gen = pass_info.generation();
        let camera = view.camera();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        // Snapshot visible instance keys into a local Vec to detach from
        // `view` (which we no longer need to borrow during draw).
        let visible_keys: Vec<_> = view.visible_instances().iter().map(|vi| vi.key).collect();

        // Acquire ResourceManager lock ONCE for the whole draw pass.
        // All draw calls share this lock — eliminates per-submesh lock/unlock
        // overhead and allows direct references to material data without cloning.
        let rm_arc = Engine::resource_manager()?;
        let mut rm = rm_arc.lock().unwrap();

        for key in &visible_keys {
            // Bind shared geometry buffers (per-instance)
            {
                let instance = match scene.render_instance(*key) {
                    Some(inst) => inst,
                    None => continue,
                };
                cmd.bind_vertex_buffer(instance.vertex_buffer(), 0)?;
                if let Some(ib) = instance.index_buffer() {
                    cmd.bind_index_buffer(ib, 0, instance.index_type())?;
                }
            }

            // LOD 0 only (V1) — read sub_mesh count
            let sub_mesh_count = match scene.render_instance(*key).and_then(|inst| inst.lod(0)) {
                Some(lod) => lod.sub_mesh_count(),
                None => continue,
            };

            for sm_idx in 0..sub_mesh_count {
                // ===== Phase 1: read submesh + cache validity =====
                let (vertex_shader, vertex_layout, topology, material_key, draw_slot,
                     vertex_offset, vertex_count, index_offset, index_count,
                     is_pipeline_valid, mat_gen) = {
                    let inst = scene.render_instance(*key).unwrap();
                    let lod = inst.lod(0).unwrap();
                    let sm = lod.sub_mesh(sm_idx).unwrap();
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
                        inst.is_pipeline_valid(pass_info_gen, mat_gen),
                        mat_gen,
                    )
                };

                // ===== Phase 2: resolve pipeline if stale or missing =====
                if !is_pipeline_valid {
                    let (frag_shader, color_blend, polygon_mode) = {
                        let mat = rm.material(material_key).unwrap();
                        let pass = mat.pass(0).unwrap();
                        (pass.fragment_shader(), *pass.color_blend(), pass.polygon_mode())
                    };

                    let gd_arc = Engine::graphics_device("main")?;
                    let mut gd = gd_arc.lock().unwrap();
                    let pipeline_key = rm.resolve_pipeline(
                        vertex_shader, frag_shader, &vertex_layout, topology,
                        &color_blend, polygon_mode, pass_info, &mut *gd,
                    )?;
                    drop(gd);

                    // Cache on instance (mut on scene — no concurrent immut borrow at this point)
                    scene.render_instance_mut(*key).unwrap()
                        .set_cached_pipeline(pipeline_key, pass_info_gen, mat_gen);
                }

                // ===== Phase 3: draw with cached pipeline =====
                let pipeline_key = scene.render_instance(*key).unwrap()
                    .cached_pipeline_key().unwrap();
                let pipeline = rm.pipeline(pipeline_key).unwrap();
                let gd_pipeline = pipeline.graphics_device_pipeline().clone();

                // Direct reference to render_state — no clone.
                // Lifetime: tied to `rm`, valid until end of submesh iteration.
                let render_state = rm.material(material_key).unwrap()
                    .pass(0).unwrap()
                    .render_state();

                // Ensure global binding group is created (lazy, on first draw).
                // This mutates `scene` but does NOT touch `rm`, so the borrow on
                // `render_state` (derived from `rm`) remains valid.
                scene.ensure_global_binding_group_with_pipeline(&gd_pipeline)?;

                // Bind pipeline
                cmd.bind_pipeline(&gd_pipeline)?;

                // Set dynamic render state directly via reference (no clone)
                cmd.set_dynamic_state(render_state)?;

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

        // rm dropped here at end of scope
        Ok(())
    }
}
