/// Drawing strategies.
///
/// A Drawer renders visible submeshes from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use std::sync::Arc;
use crate::error::Result;
use crate::engine::Engine;
use crate::graphics_device::{CommandList, BindingGroup};
use crate::resource::resource_manager::PassInfo;
use super::render_view::RenderView;
use super::scene::Scene;

/// Strategy for drawing visible submeshes.
///
/// Called within an active render pass. The Drawer issues draw commands
/// into the command list, resolving pipelines lazily via the pipeline cache.
///
/// `binding_group` is the per-pass descriptor set (set 1) containing the
/// global buffers. It is created by the ScenePassAction at construction
/// and passed here ready to use.
///
/// `&self` because drawing is stateless — the same Drawer can be
/// reused across multiple scenes and frames.
pub trait Drawer: Send + Sync {
    /// Draw visible submeshes from the RenderView into the command list.
    ///
    /// `bind_textures` controls whether the bindless texture descriptor set
    /// (set 0) is bound after each pipeline change. Shadow passes can set
    /// this to false to skip the texture bind.
    fn draw(
        &self,
        scene: &mut Scene,
        view: &RenderView,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        binding_group: &Arc<dyn BindingGroup>,
        bind_textures: bool,
    ) -> Result<()>;
}

/// Forward drawer — draws each submesh sequentially (no sorting, no instancing).
///
/// Iterates the flat list of VisibleSubMesh items from the RenderView.
/// Each item has pre-resolved submesh_index, pass_index, and lod_index.
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
        binding_group: &Arc<dyn BindingGroup>,
        bind_textures: bool,
    ) -> Result<()> {
        let pass_info_gen = pass_info.generation();
        let camera = view.camera();

        // Dynamic state from camera
        cmd.set_viewport(*camera.viewport())?;
        cmd.set_scissor(camera.effective_scissor())?;

        // Acquire ResourceManager lock ONCE for the whole draw pass.
        let rm_arc = Engine::resource_manager()?;
        let mut rm = rm_arc.lock().unwrap();

        // Track the last bound instance key to avoid re-binding geometry
        // buffers for consecutive submeshes of the same instance.
        let mut last_bound_key = None;

        // Iterate the flat draw list — each item is one submesh draw call.
        for item in view.items() {
            let key = item.key;
            let sm_idx = item.submesh_index as usize;
            let pass_idx = item.pass_index as usize;
            let lod_idx = item.lod_index as usize;

            // Bind shared geometry buffers (only when the instance changes)
            if last_bound_key != Some(key) {
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
                last_bound_key = Some(key);
            }

            // ===== Phase 1: read submesh + pass + cache validity =====
            let (vertex_shader, vertex_layout_arc, topology,
                 sm_pass_material, sm_pass_mat_pass_idx, draw_slot,
                 vertex_offset, vertex_count, index_offset, index_count,
                 is_pipeline_valid, mat_gen) = {
                let inst = scene.render_instance(key).unwrap();
                let render_sm = inst.sub_mesh(sm_idx).unwrap();
                let sm_pass = render_sm.pass_by_index(pass_idx).unwrap();
                let geo = rm.geometry(inst.geometry()).unwrap();
                let geo_mesh = geo.mesh(inst.geometry_mesh_id()).unwrap();
                let geo_sm = geo_mesh.submesh(render_sm.geometry_submesh_id()).unwrap();
                let geo_sm_lod = geo_sm.lod(lod_idx).unwrap();
                let mat = rm.material(sm_pass.material()).unwrap();
                let mat_gen = mat.generation();
                (
                    sm_pass.vertex_shader(),
                    Arc::clone(geo.vertex_layout()),
                    geo_sm_lod.topology(),
                    sm_pass.material(),
                    sm_pass.material_pass_index(),
                    render_sm.draw_slot(),
                    geo_sm_lod.vertex_offset(),
                    geo_sm_lod.vertex_count(),
                    geo_sm_lod.index_offset(),
                    geo_sm_lod.index_count(),
                    sm_pass.is_pipeline_valid(pass_info_gen, mat_gen),
                    mat_gen,
                )
            };

            // ===== Phase 2: resolve pipeline if stale or missing =====
            if !is_pipeline_valid {
                let (frag_shader, color_blend, polygon_mode) = {
                    let mat = rm.material(sm_pass_material).unwrap();
                    let pass = mat.pass(sm_pass_mat_pass_idx).unwrap();
                    (pass.fragment_shader(), *pass.color_blend(), pass.polygon_mode())
                };

                let gd_arc = Engine::graphics_device("main")?;
                let mut gd = gd_arc.lock().unwrap();
                let pipeline_key = rm.resolve_pipeline(
                    vertex_shader, frag_shader, vertex_layout_arc, topology,
                    &color_blend, polygon_mode, pass_info, &mut *gd,
                )?;
                drop(gd);

                scene.render_instance_mut(key).unwrap()
                    .sub_mesh_mut(sm_idx).unwrap()
                    .pass_by_index_mut(pass_idx).unwrap()
                    .set_cached_pipeline(pipeline_key, pass_info_gen, mat_gen);
            }

            // ===== Phase 3: draw with cached pipeline =====
            let pipeline_key = scene.render_instance(key).unwrap()
                .sub_mesh(sm_idx).unwrap()
                .pass_by_index(pass_idx).unwrap()
                .cached_pipeline_key().unwrap();
            let pipeline = rm.pipeline(pipeline_key).unwrap();
            let gd_pipeline = pipeline.graphics_device_pipeline().clone();

            let render_state = rm.material(sm_pass_material).unwrap()
                .pass(sm_pass_mat_pass_idx).unwrap()
                .render_state();

            cmd.bind_pipeline(&gd_pipeline)?;
            cmd.set_dynamic_state(render_state)?;
            cmd.bind_binding_group(&gd_pipeline, binding_group.set_index(), binding_group)?;
            if bind_textures {
                cmd.bind_textures()?;
            }

            let reflection = gd_pipeline.reflection();
            if let Some(pc) = reflection.push_constants().first() {
                cmd.push_constants(
                    pc.stage_flags, 0, bytemuck::bytes_of(&draw_slot),
                )?;
            }

            if index_count > 0 {
                cmd.draw_indexed(index_count, index_offset, vertex_offset as i32)?;
            } else {
                cmd.draw(vertex_count, vertex_offset)?;
            }
        }

        Ok(())
    }
}
