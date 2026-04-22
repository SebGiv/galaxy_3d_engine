/// ViewDispatcher — converts a VisibleInstances into per-pass RenderViews.
///
/// For each visible instance from the culling result, iterates its submeshes
/// and checks the `pass_mask` against each RenderView's `pass_type`. When a
/// submesh has an active pass matching the view, a `VisibleSubMesh` entry is
/// pushed into that view.
///
/// LOD selection: the dispatcher projects each submesh's bounding sphere
/// (derived from its world-space AABB) onto the view's camera to obtain a
/// screen-space diameter. Hysteresis thresholds stored on the GeometrySubMesh
/// are then used to pick the new LOD, based on the LOD last emitted on that
/// (instance, submesh, pass) tuple. Submeshes whose selected LOD has zero
/// vertices and indices are silently dropped — this is the intended way to
/// hide a submesh past a given distance.

use super::scene::Scene;
use super::render_view::{RenderView, VisibleSubMesh};
use super::lod::apply_hysteresis;
use crate::camera::{VisibleInstances, project_sphere_diameter};
use crate::resource::ResourceManager;

/// Dispatches culled instances into per-pass RenderViews.
///
/// Stateless — can be reused across frames.
pub struct ViewDispatcher;

impl ViewDispatcher {
    pub fn new() -> Self {
        Self
    }

    /// Dispatch all visible instances from `visible` into the provided
    /// `render_views`. Each RenderView is cleared then refilled.
    ///
    /// `render_views` must be pre-created with the desired `pass_type` values.
    /// The camera snapshot is copied from `visible` into each RenderView.
    pub fn dispatch(
        visible: &VisibleInstances,
        scene: &mut Scene,
        rm: &ResourceManager,
        render_views: &mut [RenderView],
    ) {
        // Copy camera snapshot + clear all views
        let camera = visible.camera();
        for view in render_views.iter_mut() {
            view.set_camera(camera.clone());
            view.clear();
        }

        for vi in visible.instances().iter() {
            let instance = match scene.render_instance_mut(vi.key) {
                Some(inst) => inst,
                None => continue,
            };

            // Per-instance data: world-space bounding sphere (from AABB) and
            // the GeometryMesh this instance points to.
            let world_aabb = instance.bounding_box().transformed(instance.world_matrix());
            let center = (world_aabb.min + world_aabb.max) * 0.5;
            let radius = (world_aabb.max - world_aabb.min).length() * 0.5;

            let geom_key = instance.geometry();
            let mesh_id = instance.geometry_mesh_id();
            let geo = match rm.geometry(geom_key) { Some(g) => g, None => continue };
            let geo_mesh = match geo.mesh(mesh_id) { Some(m) => m, None => continue };

            let sm_count = instance.sub_mesh_count();
            for sm_idx in 0..sm_count {
                // SAFETY: `sm_idx` ranges over `0..sub_mesh_count()` which
                // is the length of the underlying `sub_meshes` vec, so
                // `sub_mesh_mut(sm_idx)` is always `Some`.
                let sub_mesh = unsafe { instance.sub_mesh_mut(sm_idx).unwrap_unchecked() };

                let mask = sub_mesh.pass_mask();
                if mask == 0 { continue; }

                let geo_sm_id = sub_mesh.geometry_submesh_id();
                let geo_sm = match geo_mesh.submesh(geo_sm_id) { Some(s) => s, None => continue };
                let thresholds = geo_sm.lod_thresholds();

                // All RenderViews currently share the culling camera (set above),
                // so the projected screen size is the same for every view of a
                // given submesh — compute it once here and reuse inside the
                // view loop. When views gain per-view cameras, move this back
                // inside the loop and cache by `camera_id` instead.
                let screen_size = project_sphere_diameter(center, radius, camera);

                for view in render_views.iter_mut() {
                    let pt = view.pass_type();

                    // Fast bitmask test
                    if mask & (1u64 << pt) == 0 { continue; }
                    if !sub_mesh.has_pass(pt) { continue; }
                    let pass_index = sub_mesh.pass_index_for_type(pt);

                    let pass = match sub_mesh.pass_by_index_mut(pass_index as usize) {
                        Some(p) => p,
                        None => continue,
                    };
                    let new_lod = apply_hysteresis(pass.current_lod(), screen_size, thresholds);
                    pass.set_current_lod(new_lod);

                    // Hide the submesh when its selected LOD has no geometry.
                    let lod_data = match geo_sm.lod(new_lod as usize) {
                        Some(l) => l,
                        None => continue,
                    };
                    if lod_data.vertex_count() == 0 && lod_data.index_count() == 0 {
                        continue;
                    }

                    view.push(VisibleSubMesh {
                        key: vi.key,
                        distance: vi.distance,
                        submesh_index: sm_idx as u8,
                        pass_index,
                        lod_index: new_lod,
                    });
                }
            }
        }
    }
}
