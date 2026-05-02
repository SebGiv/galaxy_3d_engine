use super::*;
use crate::camera::VisibleInstances;
use crate::scene::scene::Scene;
use crate::scene::scene_test_helpers::{setup_resources, create_test_aabb, create_test_camera};
use glam::{Mat4, Vec3};

fn build_scene_with_n_instances(n: usize) -> (Scene, crate::resource::resource_manager::ResourceManager) {
    let setup = setup_resources();
    let mut scene = Scene::new();
    for i in 0..n {
        let world = Mat4::from_translation(Vec3::new(i as f32 * 10.0, 0.0, 0.0));
        scene.create_render_instance(
            setup.mesh_key, world, create_test_aabb(),
            setup.vertex_shader_key, &[], &setup.rm,
        ).unwrap();
    }
    (scene, setup.rm)
}

// ============================================================================
// BruteForceCuller
// ============================================================================

#[test]
fn test_brute_force_culler_new() {
    let _culler = BruteForceCuller::new();
}

#[test]
fn test_brute_force_cull_into_empty_scene() {
    let mut culler = BruteForceCuller::new();
    let scene = Scene::new();
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.instances().len(), 0);
}

#[test]
fn test_brute_force_cull_into_returns_all_instances() {
    let mut culler = BruteForceCuller::new();
    let (scene, _rm) = build_scene_with_n_instances(5);
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.instances().len(), 5);
}

#[test]
fn test_brute_force_cull_clears_previous_visible_instances() {
    let mut culler = BruteForceCuller::new();
    let (scene, _rm) = build_scene_with_n_instances(2);
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    // First cull: should yield 2 entries.
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.instances().len(), 2);
    // Second cull on same VisibleInstances: still 2 (cleared and refilled).
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.instances().len(), 2);
}

#[test]
fn test_brute_force_cull_snapshots_camera() {
    let mut culler = BruteForceCuller::new();
    let scene = Scene::new();
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.camera().viewport().width, 1920.0);
}

// ============================================================================
// FrustumCuller
// ============================================================================

#[test]
fn test_frustum_culler_new() {
    let _culler = FrustumCuller::new();
}

#[test]
fn test_frustum_cull_into_empty_scene() {
    let mut culler = FrustumCuller::new();
    let scene = Scene::new();
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.instances().len(), 0);
}

#[test]
fn test_frustum_cull_into_no_scene_index() {
    let mut culler = FrustumCuller::new();
    // With identity view-projection, the frustum spans the whole NDC cube
    // and instances at the origin (within the AABB ±1) intersect it.
    let setup = setup_resources();
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &setup.rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    // Frustum on identity matrix is broad — at least the centered instance
    // should be considered visible.
    assert!(!visible.instances().is_empty());
}

#[test]
fn test_frustum_cull_clears_visible_instances() {
    let mut culler = FrustumCuller::new();
    let (scene, _rm) = build_scene_with_n_instances(3);
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    let count_first = visible.instances().len();
    culler.cull_into(&scene, &camera, None, &mut visible);
    let count_second = visible.instances().len();
    assert_eq!(count_first, count_second);
}

#[test]
fn test_frustum_cull_snapshots_camera() {
    let mut culler = FrustumCuller::new();
    let scene = Scene::new();
    let camera = create_test_camera();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);
    assert_eq!(visible.camera().viewport().width, 1920.0);
}
