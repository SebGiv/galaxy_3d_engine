/// Drawing strategies.
///
/// A Drawer renders visible instances from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use std::sync::Arc;
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

        // Acquire ResourceManager lock ONCE for the whole draw pass.
        // All draw calls share this lock — eliminates per-submesh lock/unlock
        // overhead and allows direct references to material data without cloning.
        let rm_arc = Engine::resource_manager()?;
        let mut rm = rm_arc.lock().unwrap();

        // Iterate visible instances directly from the RenderView — no intermediate
        // Vec clone. `view` is an independent parameter (not a field of `scene`),
        // so holding an immutable iterator on it does not conflict with the mutable
        // accesses on `scene` / `cmd` / `rm` inside the loop body.
        for vi in view.visible_instances().iter() {
            let key = vi.key;  // Copy — no further borrow on view after this point

            // Bind shared geometry buffers (per-instance) — read from the Geometry
            {
                let instance = match scene.render_instance(key) {
                    Some(inst) => inst,
                    None => continue,
                };
                let geo = match rm.geometry(instance.geometry()) {
                    Some(g) => g,
                    None => continue,
                };
                cmd.bind_vertex_buffer(geo.vertex_buffer(), 0)?;
                if let Some(ib) = geo.index_buffer() {
                    cmd.bind_index_buffer(ib, 0, geo.index_type())?;
                }
            }

            // Read sub_mesh count from the RenderInstance directly
            // (no LOD level on the instance — LOD selection is per-submesh on the Geometry)
            let sub_mesh_count = match scene.render_instance(key) {
                Some(inst) => inst.sub_mesh_count(),
                None => continue,
            };

            for sm_idx in 0..sub_mesh_count {
                // ===== Phase 1: read submesh + cache validity =====
                // All borrows on rm/scene are scoped here so they release before
                // the mut calls in Phase 2.
                // LOD 0 hardcoded for all submeshes (V1) — to be replaced later
                // by per-submesh dynamic LOD selection.
                let (vertex_shader, vertex_layout_arc, topology, material_key, draw_slot,
                     vertex_offset, vertex_count, index_offset, index_count,
                     is_pipeline_valid, mat_gen) = {
                    let inst = scene.render_instance(key).unwrap();
                    let render_sm = inst.sub_mesh(sm_idx).unwrap();
                    let geo = rm.geometry(inst.geometry()).unwrap();
                    let geo_mesh = geo.mesh(inst.geometry_mesh_id()).unwrap();
                    let geo_sm = geo_mesh.submesh(render_sm.geometry_submesh_id()).unwrap();
                    let geo_sm_lod = geo_sm.lod(0).unwrap();
                    let mat = rm.material(render_sm.material()).unwrap();
                    let mat_gen = mat.generation();
                    (
                        render_sm.vertex_shader(),
                        // Arc::clone — single atomic increment, ZERO allocation.
                        Arc::clone(geo.vertex_layout()),
                        geo_sm_lod.topology(),
                        render_sm.material(),
                        render_sm.draw_slot(),
                        geo_sm_lod.vertex_offset(),
                        geo_sm_lod.vertex_count(),
                        geo_sm_lod.index_offset(),
                        geo_sm_lod.index_count(),
                        render_sm.is_pipeline_valid(pass_info_gen, mat_gen),
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
                        vertex_shader, frag_shader, vertex_layout_arc, topology,
                        &color_blend, polygon_mode, pass_info, &mut *gd,
                    )?;
                    drop(gd);

                    // Cache on the SUB_MESH (not the instance) — different
                    // submeshes of the same instance can have different pipelines.
                    scene.render_instance_mut(key).unwrap()
                        .sub_mesh_mut(sm_idx).unwrap()
                        .set_cached_pipeline(pipeline_key, pass_info_gen, mat_gen);
                }

                // ===== Phase 3: draw with cached pipeline =====
                let pipeline_key = scene.render_instance(key).unwrap()
                    .sub_mesh(sm_idx).unwrap()
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
