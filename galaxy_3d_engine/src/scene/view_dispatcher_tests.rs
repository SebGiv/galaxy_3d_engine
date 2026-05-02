use super::*;
use crate::camera::VisibleInstances;
use crate::scene::{Scene, BruteForceCuller, CameraCuller, RenderView, VisibleSubMesh, RenderInstanceKey};
use crate::scene::scene_test_helpers::{setup_resources, create_test_aabb, create_test_camera};
use glam::{Mat4, Vec3};
use slotmap::Key;

#[test]
fn test_view_dispatcher_new() {
    let _vd = ViewDispatcher::new();
}

#[test]
fn test_dispatch_with_no_views_does_not_panic() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();
    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    let mut views: [RenderView; 0] = [];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
}

#[test]
fn test_dispatch_clears_existing_view_items() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    {
        let mut culler = BruteForceCuller::new();
        culler.cull_into(&scene, &camera, None, &mut visible);
    }

    let mut view = RenderView::new(camera.clone(), 0);
    // Pre-populate the view with a fake item.
    view.push(VisibleSubMesh {
        key: visible.instances().first().map(|i| i.key)
            .unwrap_or_else(RenderInstanceKey::null),
        distance: 1.0,
        submesh_index: 0,
        pass_index: 0,
        lod_index: 0,
    });
    assert_eq!(view.len(), 1);

    let mut views = [view];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    // Empty scene → no instances → empty view after dispatch.
    assert_eq!(views[0].len(), 0);

    // Camera was copied even on empty dispatch.
    assert_eq!(views[0].camera().viewport().width, 1920.0);
}

#[test]
fn test_dispatch_populates_view_for_matching_pass_type() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    // pass_type = 0 matches the material's single pass.
    let mut views = [RenderView::new(camera.clone(), 0)];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    assert!(!views[0].is_empty(), "View for pass_type 0 should receive submesh entries");
}

#[test]
fn test_dispatch_drops_view_for_unmatched_pass_type() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    // pass_type = 7 does not match the material's pass_type = 0.
    let mut views = [RenderView::new(camera.clone(), 7)];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    assert!(views[0].is_empty(), "View for unmatched pass_type should be empty");
}

#[test]
fn test_dispatch_handles_multiple_views_in_one_call() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut views = [
        RenderView::new(camera.clone(), 0),  // matches
        RenderView::new(camera.clone(), 1),  // no match
        RenderView::new(camera.clone(), 2),  // no match
    ];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    assert!(!views[0].is_empty());
    assert!(views[1].is_empty());
    assert!(views[2].is_empty());
}

#[test]
fn test_dispatch_skips_invalid_render_instance_key() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    let key = scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    // Remove the instance after culling — dispatch must skip it.
    scene.remove_render_instance(key);
    scene.removed_instances();

    let mut views = [RenderView::new(camera.clone(), 0)];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    assert!(views[0].is_empty(), "Dispatch should skip removed instances");
}

#[test]
fn test_dispatch_with_translated_instance_emits_distance() {
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key,
        Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)),
        create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut views = [RenderView::new(camera.clone(), 0)];
    ViewDispatcher::dispatch(&visible, &mut scene, &setup.rm, &mut views);
    assert!(!views[0].is_empty());
    // Some non-trivial distance was recorded.
    let item = views[0].items()[0];
    assert!(item.distance.is_finite());
}
