use glam::Mat4;
use crate::graphics_device::command_list::Viewport;
use crate::camera::Frustum;
use crate::scene::RenderInstanceKey;
use super::*;

fn create_test_camera() -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

// ============================================================================
// Construction (via pub(crate) new)
// ============================================================================

#[test]
fn test_render_view_new() {
    let camera = create_test_camera();
    let keys: Vec<RenderInstanceKey> = Vec::new();

    let view = RenderView::new(camera, keys);

    assert_eq!(view.visible_count(), 0);
    assert!(view.visible_instances().is_empty());
}

// ============================================================================
// Accessors
// ============================================================================

#[test]
fn test_render_view_camera_snapshot() {
    let camera = create_test_camera();
    let view = RenderView::new(camera.clone(), Vec::new());

    assert_eq!(*view.camera().view_matrix(), Mat4::IDENTITY);
    assert_eq!(view.camera().viewport().width, 1920.0);
}

#[test]
fn test_render_view_visible_count() {
    // We can't create real RenderInstanceKeys without a SlotMap,
    // but we can test with an empty list
    let camera = create_test_camera();
    let view = RenderView::new(camera, Vec::new());

    assert_eq!(view.visible_count(), 0);
}

// ============================================================================
// Clone
// ============================================================================

#[test]
fn test_render_view_clone() {
    let camera = create_test_camera();
    let view = RenderView::new(camera, Vec::new());
    let cloned = view.clone();

    assert_eq!(cloned.visible_count(), view.visible_count());
    assert_eq!(*cloned.camera().view_matrix(), *view.camera().view_matrix());
}
