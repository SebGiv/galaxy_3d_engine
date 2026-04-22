/// Drawing strategies.
///
/// A Drawer renders visible submeshes from a RenderView into a command list.
/// Implementations range from simple forward rendering to sorted/instanced approaches.

use std::sync::Arc;
use crate::error::Result;
use crate::engine::Engine;
use crate::graphics_device::{CommandList, BindingGroup, ShaderStageFlags};
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
            // `cached_pipeline_key` is Some only when the cached key is still
            // valid: we avoid re-fetching it from the scene after resolving.
            // `geo_sort_id` is captured here to avoid a second `rm.geometry()`
            // lookup later. `vertex_layout` is NOT cloned here — it is only
            // needed on the slow `resolve_pipeline` path below.
            let (
                vertex_shader, topology,
                sm_pass_material, sm_pass_mat_pass_idx, draw_slot,
                vertex_offset, vertex_count, index_offset, index_count,
                cached_pipeline_key, mat_gen,
                geometry_key, geo_sort_id,
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
                let cached = if sm_pass.is_pipeline_valid(pass_info_gen, mat_gen) {
                    sm_pass.cached_pipeline_key()
                } else {
                    None
                };
                (
                    sm_pass.vertex_shader(),
                    geo_sm_lod.topology(),
                    sm_pass.material(),
                    sm_pass.material_pass_index(),
                    render_sm.draw_slot(),
                    geo_sm_lod.vertex_offset(),
                    geo_sm_lod.vertex_count(),
                    geo_sm_lod.index_offset(),
                    geo_sm_lod.index_count(),
                    cached,
                    mat_gen,
                    geometry_key,
                    geo.sort_id(),
                )
            };

            // Resolve pipeline if stale or missing.
            let pipeline_key = match cached_pipeline_key {
                Some(k) => k,
                None => {
                    let (frag_shader, color_blend, polygon_mode, vertex_layout_arc) = {
                        let mat = rm.material(sm_pass_material).unwrap();
                        let pass = mat.pass(sm_pass_mat_pass_idx).unwrap();
                        let geo = rm.geometry(geometry_key).unwrap();
                        (pass.fragment_shader(), *pass.color_blend(), pass.polygon_mode(),
                         Arc::clone(geo.vertex_layout()))
                    };

                    let gd_arc = Engine::graphics_device("main")?;
                    let mut gd = gd_arc.lock().unwrap();
                    let resolved = rm.resolve_pipeline(
                        vertex_shader, frag_shader, vertex_layout_arc, topology,
                        &color_blend, polygon_mode, pass_info, &mut *gd,
                    )?;
                    drop(gd);

                    scene.render_instance_mut(key).unwrap()
                        .sub_mesh_mut(sm_idx).unwrap()
                        .pass_by_index_mut(pass_idx).unwrap()
                        .set_cached_pipeline(resolved, pass_info_gen, mat_gen);
                    resolved
                }
            };

            // Read final pipeline sort ids + material render state.
            // SAFETY: `pipeline_key` is either the cached key (produced by
            // a prior successful `resolve_pipeline`) or the one just returned
            // by the resolve above, which inserts into the cache before
            // returning. `sm_pass_material` was read from a `MaterialPass`
            // that we just dereferenced, and `sm_pass_mat_pass_idx` came from
            // the same `MaterialPass`. The `rm` lock is held continuously
            // from line ~89 to the end of PHASE 3, so no removal can occur.
            let pipeline = unsafe { rm.pipeline(pipeline_key).unwrap_unchecked() };
            let signature_id = pipeline.signature_id();
            let pipeline_sort_id = pipeline.sort_id();
            let mat_pass = unsafe {
                rm.material(sm_pass_material).unwrap_unchecked()
                    .pass(sm_pass_mat_pass_idx).unwrap_unchecked()
            };
            let render_state = *mat_pass.render_state();
            let render_state_sig = mat_pass.render_state_signature_id();

            let sort_key = build_sort_key(
                signature_id,
                pipeline_sort_id,
                geo_sort_id,
                render_state_sig,
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
                    render_state_sig,
                },
                sort_key,
            );
        }

        // ===== PHASE 2: sort =====
        self.queue.sort();

        // ===== PHASE 3: emit with state tracking =====
        // Track the last bound pipeline/geometry/signature so identical values
        // on consecutive draw calls don't re-issue Vulkan bind commands.
        // `bg_set_index` is invariant for the pass, hoist out of the loop.
        // `current_pc_flags` caches the push-constant stage flags of the
        // currently bound pipeline so we don't re-query reflection per draw.
        let bg_set_index = binding_group.set_index();
        let mut last_pipeline_key = None;
        let mut last_geometry_key = None;
        let mut last_signature_id: Option<u16> = None;
        let mut last_render_state_sig: Option<u16> = None;
        let mut current_pc_flags: Option<ShaderStageFlags> = None;

        for dc in self.queue.iter_sorted() {
            // Pipeline rebind if different from previous draw call.
            if last_pipeline_key != Some(dc.pipeline_key) {
                // SAFETY: `dc.pipeline_key` was pushed into the queue during
                // PHASE 1 after a successful lookup under the same `rm` lock
                // that we still hold. Nothing can have removed the pipeline
                // between PHASE 1 and PHASE 3.
                let pipeline = unsafe { rm.pipeline(dc.pipeline_key).unwrap_unchecked() };
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
                        bg_set_index,
                        binding_group,
                    )?;
                    last_signature_id = Some(sig);
                }

                // Cache push-constant stage flags for this pipeline; the
                // reflection is static per pipeline so we only query it on
                // rebind, not on every drawcall.
                current_pc_flags = gd_pipeline
                    .reflection()
                    .push_constants()
                    .first()
                    .map(|pc| pc.stage_flags);

                last_pipeline_key = Some(dc.pipeline_key);
            }

            // Geometry rebind if different from previous draw call.
            if last_geometry_key != Some(dc.geometry_key) {
                // SAFETY: same rationale as the pipeline lookup above —
                // `dc.geometry_key` was validated under the still-held `rm` lock.
                let geo = unsafe { rm.geometry(dc.geometry_key).unwrap_unchecked() };
                cmd.bind_vertex_buffer(geo.vertex_buffer(), 0)?;
                if let Some(ib) = geo.index_buffer() {
                    cmd.bind_index_buffer(ib, 0, geo.index_type())?;
                }
                last_geometry_key = Some(dc.geometry_key);
            }

            // Per-draw-call dynamic state (may differ between draw calls sharing
            // the same pipeline, e.g. blend / cull overrides from the material).
            // Skip re-emission when the render state signature is identical to
            // the previous draw call — sort key groups identical signatures.
            if last_render_state_sig != Some(dc.render_state_sig) {
                cmd.set_dynamic_state(&dc.render_state)?;
                last_render_state_sig = Some(dc.render_state_sig);
            }

            // Push constants (draw slot) and the draw command.
            if let Some(flags) = current_pc_flags {
                cmd.push_constants(
                    flags, 0, bytemuck::bytes_of(&dc.draw_slot),
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
