/// ViewDispatcher — converts a VisibleInstances into per-pass RenderViews.
///
/// For each visible instance from the culling result, iterates its submeshes
/// and checks the `pass_mask` against each RenderView's `pass_type`. When a
/// submesh has an active pass matching the view, a `VisibleSubMesh` entry is
/// pushed into that view.
///
/// LOD selection: V1 hardcodes lod_index = 0 for all submeshes. Future
/// versions will compute the LOD from distance and per-submesh thresholds,
/// with an optional per-pass LOD bias.

use super::scene::Scene;
use super::render_view::{RenderView, VisibleSubMesh};
use crate::camera::VisibleInstances;

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
        scene: &Scene,
        render_views: &mut [RenderView],
    ) {
        // Copy camera snapshot + clear all views
        let camera = visible.camera();
        for view in render_views.iter_mut() {
            view.set_camera(camera.clone());
            view.clear();
        }

        // Dispatch each visible instance into matching views
        for vi in visible.instances().iter() {
            let instance = match scene.render_instance(vi.key) {
                Some(inst) => inst,
                None => continue,
            };

            for sm_idx in 0..instance.sub_mesh_count() {
                let sub_mesh = match instance.sub_mesh(sm_idx) {
                    Some(sm) => sm,
                    None => continue,
                };

                let mask = sub_mesh.pass_mask();
                if mask == 0 { continue; }

                for view in render_views.iter_mut() {
                    let pt = view.pass_type();

                    // Fast bitmask test
                    if mask & (1u64 << pt) == 0 { continue; }

                    // Lookup the local pass index
                    if !sub_mesh.has_pass(pt) { continue; }
                    let pass_index = sub_mesh.pass_index_for_type(pt);

                    view.push(VisibleSubMesh {
                        key: vi.key,
                        distance: vi.distance,
                        submesh_index: sm_idx as u8,
                        pass_index,
                        lod_index: 0, // V1: hardcoded
                    });
                }
            }
        }
    }
}
