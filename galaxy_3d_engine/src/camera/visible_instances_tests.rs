use glam::Mat4;
use crate::graphics_device::command_list::Viewport;
use crate::camera::Frustum;
use crate::scene::RenderInstanceKey;
use slotmap::SlotMap;
use super::*;

fn create_test_camera() -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport {
        x: 0.0, y: 0.0,
        width: 1920.0, height: 1080.0,
        min_depth: 0.0, max_depth: 1.0,
    };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

fn make_key(idx: u32) -> RenderInstanceKey {
    let mut sm = SlotMap::<RenderInstanceKey, ()>::with_key();
    let mut key = sm.insert(());
    for _ in 1..idx {
        key = sm.insert(());
    }
    key
}

// ============================================================================
// Construction
// ============================================================================

#[test]
fn test_new_empty() {
    let vi = VisibleInstances::new_empty();
    assert!(vi.is_empty());
    assert_eq!(vi.visible_count(), 0);
    assert_eq!(vi.instances().len(), 0);
}

// ============================================================================
// Camera
// ============================================================================

#[test]
fn test_set_camera() {
    let mut vi = VisibleInstances::new_empty();
    let camera = create_test_camera();
    vi.set_camera(camera);
    assert_eq!(*vi.camera().view_matrix(), Mat4::IDENTITY);
    assert_eq!(vi.camera().viewport().width, 1920.0);
}

// ============================================================================
// Instances
// ============================================================================

#[test]
fn test_push_and_read() {
    let mut vi = VisibleInstances::new_empty();
    let key = make_key(1);
    vi.instances_mut().push(VisibleInstance { key, distance: 42.0 });
    assert_eq!(vi.visible_count(), 1);
    assert_eq!(vi.instances()[0].key, key);
    assert_eq!(vi.instances()[0].distance, 42.0);
}

#[test]
fn test_clear_preserves_capacity() {
    let mut vi = VisibleInstances::new_empty();
    for i in 1..=10 {
        vi.instances_mut().push(VisibleInstance { key: make_key(i), distance: i as f32 });
    }
    assert_eq!(vi.visible_count(), 10);
    vi.clear_instances();
    assert!(vi.is_empty());
}

#[test]
fn test_clone() {
    let mut vi = VisibleInstances::new_empty();
    vi.set_camera(create_test_camera());
    vi.instances_mut().push(VisibleInstance { key: make_key(1), distance: 5.0 });
    let cloned = vi.clone();
    assert_eq!(cloned.visible_count(), vi.visible_count());
    assert_eq!(*cloned.camera().view_matrix(), *vi.camera().view_matrix());
}
