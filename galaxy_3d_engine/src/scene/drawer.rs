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
use super::render_queue::{RenderQueue, DrawCall, build_sort_key};

/// Default preallocated capacity for the internal RenderQueue.
/// Sized to cover typical scenes without any per-frame reallocation.
const DEFAULT_DRAW_CALL_CAPACITY: usize = 4096;

/// Strategy for drawing visible submeshes.
///
/// Called within an active render pass. The Drawer issues draw commands
/// into the command list, resolving pipelines lazily via the pipeline cache.
///
/// `binding_group` is the per-pass descriptor set (set 1) containing the
/// global buffers. It is created by the ScenePassAction at construction
/// and passed here ready to use.
///
/// Drawers are `&mut self` because they own per-frame working buffers
/// (e.g. the sort queue) that are reset and filled each draw.
pub trait Drawer: Send + Sync {
    /// Draw visible submeshes from the RenderView into the command list.
    ///
    /// `bind_textures` controls whether the bindless texture descriptor set
    /// (set 0) is bound after each pipeline-layout change. Shadow passes can
    /// set this to false to skip the texture bind.
    fn draw(
        &mut self,
        scene: &mut Scene,
        view: &RenderView,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        binding_group: &Arc<dyn BindingGroup>,
        bind_textures: bool,
    ) -> Result<()>;
}

/// Forward drawer — sorts visible submeshes by (signature, pipeline, geometry,
/// distance), then emits draw calls with state-tracked rebinds so identical
/// pipelines and geometries are not rebound back-to-back.
///
/// The internal `RenderQueue` is preallocated and reused frame-to-frame; no
/// allocation happens during a normal draw (unless the queue grows past its
/// initial capacity, in which case it grows once and stays at the new size).
pub struct ForwardDrawer {
    queue: RenderQueue,
}

impl ForwardDrawer {
    /// Create a ForwardDrawer with the default preallocated draw-call capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_DRAW_CALL_CAPACITY)
    }

    /// Create a ForwardDrawer with a specific preallocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { queue: RenderQueue::with_capacity(capacity) }
    }
}

impl Drawer for ForwardDrawer {
    fn draw(
        &mut self,
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

        // ===== PHASE 1: fill the queue =====
        // Resolve pipelines for stale/missing cache entries, collect per-submesh
        // draw data and its 64-bit sort key.
        self.queue.clear();

        for item in view.items() {
            let key = item.key;
            let sm_idx = item.submesh_index as usize;
            let pass_idx = item.pass_index as usize;
            let lod_idx = item.lod_index as usize;

            // Read submesh + pass + cache-validity info + geometry key + LOD data.
            let (
                vertex_shader, vertex_layout_arc, topology,
                sm_pass_material, sm_pass_mat_pass_idx, draw_slot,
                vertex_offset, vertex_count, index_offset, index_count,
                is_pipeline_valid, mat_gen,
                geometry_key,
            ) = {
                let inst = match scene.render_instance(key) {
                    Some(i) => i,
                    None => continue,
                };
                let geometry_key = inst.geometry();
                let render_sm = match inst.sub_mesh(sm_idx) { Some(s) => s, None => continue };
                let sm_pass = match render_sm.pass_by_index(pass_idx) { Some(p) => p, None => continue };
                let geo = match rm.geometry(geometry_key) { Some(g) => g, None => continue };
                let geo_mesh = match geo.mesh(inst.geometry_mesh_id()) { Some(m) => m, None => continue };
                let geo_sm = match geo_mesh.submesh(render_sm.geometry_submesh_id()) { Some(s) => s, None => continue };
                let geo_sm_lod = match geo_sm.lod(lod_idx) { Some(l) => l, None => continue };
                let mat = match rm.material(sm_pass.material()) { Some(m) => m, None => continue };
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
                    geometry_key,
                )
            };

            // Resolve pipeline if stale or missing.
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

            // Read final pipeline key + pipeline sort ids + material render state.
            let pipeline_key = scene.render_instance(key).unwrap()
                .sub_mesh(sm_idx).unwrap()
                .pass_by_index(pass_idx).unwrap()
                .cached_pipeline_key().unwrap();
            let pipeline = rm.pipeline(pipeline_key).unwrap();
            let signature_id = pipeline.signature_id();
            let pipeline_sort_id = pipeline.sort_id();
            let geometry_sort_id = rm.geometry(geometry_key).unwrap().sort_id();
            let render_state = *rm.material(sm_pass_material).unwrap()
                .pass(sm_pass_mat_pass_idx).unwrap()
                .render_state();

            let sort_key = build_sort_key(
                signature_id,
                pipeline_sort_id,
                geometry_sort_id,
                item.distance,
            );

            self.queue.push(
                DrawCall {
                    pipeline_key,
                    geometry_key,
                    vertex_offset,
                    vertex_count,
                    index_offset,
                    index_count,
                    draw_slot,
                    render_state,
                },
                sort_key,
            );
        }

        // ===== PHASE 2: sort =====
        self.queue.sort();

        // ===== PHASE 3: emit with state tracking =====
        // Track the last bound pipeline/geometry/signature so identical values
        // on consecutive draw calls don't re-issue Vulkan bind commands.
        let mut last_pipeline_key = None;
        let mut last_geometry_key = None;
        let mut last_signature_id: Option<u16> = None;

        for dc in self.queue.iter_sorted() {
            // Pipeline rebind if different from previous draw call.
            if last_pipeline_key != Some(dc.pipeline_key) {
                let pipeline = rm.pipeline(dc.pipeline_key).unwrap();
                let gd_pipeline = pipeline.graphics_device_pipeline();
                cmd.bind_pipeline(gd_pipeline)?;

                // When the pipeline layout signature changes, Vulkan invalidates
                // all previously bound descriptor sets. We must re-bind set 0
                // (bindless) and set 1 (per-pass binding group) here.
                let sig = pipeline.signature_id();
                if last_signature_id != Some(sig) {
                    if bind_textures {
                        cmd.bind_textures()?;
                    }
                    cmd.bind_binding_group(
                        gd_pipeline,
                        binding_group.set_index(),
                        binding_group,
                    )?;
                    last_signature_id = Some(sig);
                }

                last_pipeline_key = Some(dc.pipeline_key);
            }

            // Geometry rebind if different from previous draw call.
            if last_geometry_key != Some(dc.geometry_key) {
                let geo = rm.geometry(dc.geometry_key).unwrap();
                cmd.bind_vertex_buffer(geo.vertex_buffer(), 0)?;
                if let Some(ib) = geo.index_buffer() {
                    cmd.bind_index_buffer(ib, 0, geo.index_type())?;
                }
                last_geometry_key = Some(dc.geometry_key);
            }

            // Per-draw-call dynamic state (may differ between draw calls sharing
            // the same pipeline, e.g. blend / cull overrides from the material).
            cmd.set_dynamic_state(&dc.render_state)?;

            // Push constants (draw slot) and the draw command.
            let pipeline = rm.pipeline(dc.pipeline_key).unwrap();
            let gd_pipeline = pipeline.graphics_device_pipeline();
            let reflection = gd_pipeline.reflection();
            if let Some(pc) = reflection.push_constants().first() {
                cmd.push_constants(
                    pc.stage_flags, 0, bytemuck::bytes_of(&dc.draw_slot),
                )?;
            }

            if dc.index_count > 0 {
                cmd.draw_indexed(dc.index_count, dc.index_offset, dc.vertex_offset as i32)?;
            } else {
                cmd.draw(dc.vertex_count, dc.vertex_offset)?;
            }
        }

        Ok(())
    }
}
