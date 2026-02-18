use glam::{Mat4, Vec3};
use crate::renderer::command_list::{Viewport, Rect2D};
use super::*;

fn create_test_viewport() -> Viewport {
    Viewport {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        min_depth: 0.0,
        max_depth: 1.0,
    }
}

fn create_test_frustum() -> Frustum {
    let vp = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_4,
        16.0 / 9.0,
        0.1,
        100.0,
    );
    Frustum::from_view_projection(&vp)
}

// ============================================================================
// Construction
// ============================================================================

#[test]
fn test_camera_new() {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();

    let camera = Camera::new(view, proj, frustum, viewport);

    assert_eq!(*camera.view_matrix(), view);
    assert_eq!(*camera.projection_matrix(), proj);
    assert_eq!(camera.viewport().width, 1920.0);
    assert!(camera.scissor().is_none());
}

// ============================================================================
// view_projection_matrix
// ============================================================================

#[test]
fn test_view_projection_matrix() {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();

    let camera = Camera::new(view, proj, frustum, viewport);

    let expected = proj * view;
    assert_eq!(camera.view_projection_matrix(), expected);
}

// ============================================================================
// Setters
// ============================================================================

#[test]
fn test_set_view() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    let new_view = Mat4::look_at_rh(Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, Vec3::Y);
    camera.set_view(new_view);

    assert_eq!(*camera.view_matrix(), new_view);
}

#[test]
fn test_set_projection() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    let new_proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 1.0, 0.5, 500.0);
    camera.set_projection(new_proj);

    assert_eq!(*camera.projection_matrix(), new_proj);
}

#[test]
fn test_set_frustum() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    let new_frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    camera.set_frustum(new_frustum);

    // Just verify it was set (compare a plane value)
    assert_eq!(camera.frustum().planes[0], new_frustum.planes[0]);
}

#[test]
fn test_set_viewport() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    let new_viewport = Viewport {
        x: 100.0,
        y: 50.0,
        width: 800.0,
        height: 600.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    camera.set_viewport(new_viewport);

    assert_eq!(camera.viewport().x, 100.0);
    assert_eq!(camera.viewport().width, 800.0);
}

#[test]
fn test_set_scissor() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    assert!(camera.scissor().is_none());

    let scissor = Rect2D { x: 10, y: 20, width: 400, height: 300 };
    camera.set_scissor(Some(scissor));

    assert!(camera.scissor().is_some());
    assert_eq!(camera.scissor().unwrap().x, 10);
    assert_eq!(camera.scissor().unwrap().width, 400);

    camera.set_scissor(None);
    assert!(camera.scissor().is_none());
}

// ============================================================================
// effective_scissor
// ============================================================================

#[test]
fn test_effective_scissor_when_none() {
    let frustum = create_test_frustum();
    let viewport = Viewport {
        x: 100.0,
        y: 50.0,
        width: 800.0,
        height: 600.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    let camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    let scissor = camera.effective_scissor();
    assert_eq!(scissor.x, 100);
    assert_eq!(scissor.y, 50);
    assert_eq!(scissor.width, 800);
    assert_eq!(scissor.height, 600);
}

#[test]
fn test_effective_scissor_when_set() {
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();
    let mut camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);

    camera.set_scissor(Some(Rect2D { x: 10, y: 20, width: 100, height: 50 }));

    let scissor = camera.effective_scissor();
    assert_eq!(scissor.x, 10);
    assert_eq!(scissor.y, 20);
    assert_eq!(scissor.width, 100);
    assert_eq!(scissor.height, 50);
}

// ============================================================================
// Clone
// ============================================================================

#[test]
fn test_camera_clone() {
    let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, 16.0 / 9.0, 0.1, 100.0);
    let frustum = create_test_frustum();
    let viewport = create_test_viewport();

    let camera = Camera::new(view, proj, frustum, viewport);
    let cloned = camera.clone();

    assert_eq!(*cloned.view_matrix(), view);
    assert_eq!(*cloned.projection_matrix(), proj);
    assert_eq!(cloned.viewport().width, 1920.0);
}
