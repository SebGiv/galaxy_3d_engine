use glam::Mat4;
use crate::graphics_device::command_list::Viewport;
use crate::camera::Frustum;
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
// Construction (via new_empty)
// ============================================================================

#[test]
fn test_render_view_new_empty() {
    let view = RenderView::new_empty();

    assert_eq!(view.visible_count(), 0);
    assert!(view.visible_instances().is_empty());
}

// ============================================================================
// Mutators (set_camera, visible_instances_mut)
// ============================================================================

#[test]
fn test_render_view_set_camera() {
    let mut view = RenderView::new_empty();
    let camera = create_test_camera();
    view.set_camera(camera);

    assert_eq!(*view.camera().view_matrix(), Mat4::IDENTITY);
    assert_eq!(view.camera().viewport().width, 1920.0);
}

#[test]
fn test_render_view_visible_instances_mut() {
    let mut view = RenderView::new_empty();
    // visible_instances_mut allows the culler to refill the buffer in place
    view.visible_instances_mut().clear();
    assert_eq!(view.visible_count(), 0);
}

// ============================================================================
// Accessors
// ============================================================================

#[test]
fn test_render_view_visible_count() {
    let view = RenderView::new_empty();
    assert_eq!(view.visible_count(), 0);
}

// ============================================================================
// Clone
// ============================================================================

#[test]
fn test_render_view_clone() {
    let mut view = RenderView::new_empty();
    view.set_camera(create_test_camera());
    let cloned = view.clone();

    assert_eq!(cloned.visible_count(), view.visible_count());
    assert_eq!(*cloned.camera().view_matrix(), *view.camera().view_matrix());
}
