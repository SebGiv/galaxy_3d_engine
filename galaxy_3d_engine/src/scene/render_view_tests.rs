use super::*;
use crate::camera::{Camera, Frustum};
use crate::graphics_device::Viewport;
use crate::scene::RenderInstanceKey;
use glam::Mat4;
use slotmap::Key;

fn make_camera(width: f32) -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport { x: 0.0, y: 0.0, width, height: 1080.0, min_depth: 0.0, max_depth: 1.0 };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

fn make_item() -> VisibleSubMesh {
    VisibleSubMesh {
        key: RenderInstanceKey::null(),
        distance: 1.0,
        submesh_index: 0,
        pass_index: 0,
        lod_index: 0,
    }
}

#[test]
fn test_new_view_is_empty() {
    let view = RenderView::new(make_camera(1920.0), 0);
    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
    assert_eq!(view.items().len(), 0);
    assert_eq!(view.iter().count(), 0);
}

#[test]
fn test_with_capacity_is_empty_but_preallocated() {
    let view = RenderView::with_capacity(make_camera(1280.0), 5, 256);
    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
    assert_eq!(view.pass_type(), 5);
}

#[test]
fn test_pass_type_accessor() {
    let view = RenderView::new(make_camera(800.0), 42);
    assert_eq!(view.pass_type(), 42);
}

#[test]
fn test_camera_accessor_matches_input() {
    let view = RenderView::new(make_camera(640.0), 0);
    assert_eq!(view.camera().viewport().width, 640.0);
}

#[test]
fn test_push_increments_len_and_items() {
    let mut view = RenderView::new(make_camera(1920.0), 0);
    view.push(make_item());
    assert_eq!(view.len(), 1);
    assert!(!view.is_empty());
    assert_eq!(view.items().len(), 1);
}

#[test]
fn test_push_multiple_items() {
    let mut view = RenderView::new(make_camera(1920.0), 0);
    for _ in 0..10 { view.push(make_item()); }
    assert_eq!(view.len(), 10);
    assert_eq!(view.iter().count(), 10);
}

#[test]
fn test_clear_resets_length() {
    let mut view = RenderView::new(make_camera(1920.0), 0);
    for _ in 0..5 { view.push(make_item()); }
    view.clear();
    assert_eq!(view.len(), 0);
    assert!(view.is_empty());
}

#[test]
fn test_set_camera_replaces_camera() {
    let mut view = RenderView::new(make_camera(1920.0), 0);
    view.set_camera(make_camera(640.0));
    assert_eq!(view.camera().viewport().width, 640.0);
}

#[test]
fn test_visible_sub_mesh_clone_copy() {
    let item = VisibleSubMesh {
        key: RenderInstanceKey::null(),
        distance: 3.5,
        submesh_index: 1,
        pass_index: 2,
        lod_index: 4,
    };
    let copy = item;
    let cloned = item.clone();
    assert_eq!(copy.distance, item.distance);
    assert_eq!(cloned.submesh_index, 1);
    assert_eq!(cloned.pass_index, 2);
    assert_eq!(cloned.lod_index, 4);
}

#[test]
fn test_iter_yields_pushed_items_in_order() {
    let mut view = RenderView::new(make_camera(1920.0), 0);
    let mut a = make_item(); a.distance = 1.0;
    let mut b = make_item(); b.distance = 2.0;
    let mut c = make_item(); c.distance = 3.0;
    view.push(a); view.push(b); view.push(c);
    let distances: Vec<f32> = view.iter().map(|i| i.distance).collect();
    assert_eq!(distances, vec![1.0, 2.0, 3.0]);
}

#[test]
fn test_clone_preserves_items_and_pass_type() {
    let mut view = RenderView::new(make_camera(1920.0), 7);
    view.push(make_item());
    view.push(make_item());
    let cloned = view.clone();
    assert_eq!(cloned.len(), 2);
    assert_eq!(cloned.pass_type(), 7);
}
