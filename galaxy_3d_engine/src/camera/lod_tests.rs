use super::*;
use glam::{Mat4, Vec3};
use crate::graphics_device::command_list::Viewport;
use crate::camera::Frustum;

/// Build a standard perspective camera looking down -Z from origin.
fn make_camera(fov_y: f32, width: f32, height: f32) -> Camera {
    let proj = Mat4::perspective_rh(fov_y, width / height, 0.1, 1000.0);
    let view = Mat4::look_at_rh(
        Vec3::ZERO,
        Vec3::new(0.0, 0.0, -1.0),
        Vec3::Y,
    );
    let frustum = Frustum::from_view_projection(&(proj * view));
    Camera::new(
        view,
        proj,
        frustum,
        Viewport {
            x: 0.0,
            y: 0.0,
            width,
            height,
            min_depth: 0.0,
            max_depth: 1.0,
        },
    )
}

#[test]
fn test_distance_inversely_proportional() {
    let camera = make_camera(std::f32::consts::FRAC_PI_2, 1000.0, 1000.0);

    let near = project_sphere_diameter(Vec3::new(0.0, 0.0, -10.0), 1.0, &camera);
    let far  = project_sphere_diameter(Vec3::new(0.0, 0.0, -100.0), 1.0, &camera);

    // 10x further away should be ~10x smaller on screen
    let ratio = near / far;
    assert!((ratio - 10.0).abs() < 0.01, "ratio was {}", ratio);
}

#[test]
fn test_radius_proportional() {
    let camera = make_camera(std::f32::consts::FRAC_PI_2, 1000.0, 1000.0);

    let small = project_sphere_diameter(Vec3::new(0.0, 0.0, -20.0), 1.0, &camera);
    let big   = project_sphere_diameter(Vec3::new(0.0, 0.0, -20.0), 5.0, &camera);

    // 5x radius → 5x screen size
    let ratio = big / small;
    assert!((ratio - 5.0).abs() < 0.01, "ratio was {}", ratio);
}

#[test]
fn test_viewport_height_proportional() {
    let small_vp = make_camera(std::f32::consts::FRAC_PI_2, 500.0, 500.0);
    let big_vp   = make_camera(std::f32::consts::FRAC_PI_2, 1000.0, 1000.0);

    let s = project_sphere_diameter(Vec3::new(0.0, 0.0, -10.0), 1.0, &small_vp);
    let b = project_sphere_diameter(Vec3::new(0.0, 0.0, -10.0), 1.0, &big_vp);

    // Double viewport height → double screen size
    let ratio = b / s;
    assert!((ratio - 2.0).abs() < 0.01, "ratio was {}", ratio);
}

#[test]
fn test_behind_camera_saturated() {
    let camera = make_camera(std::f32::consts::FRAC_PI_2, 1000.0, 1000.0);

    // Object behind the camera (positive Z in RH look-at -Z) — depth clamped,
    // function stays finite and non-negative.
    let size = project_sphere_diameter(Vec3::new(0.0, 0.0, 10.0), 1.0, &camera);
    assert!(size.is_finite() && size >= 0.0);
}

#[test]
fn test_zero_radius_is_zero() {
    let camera = make_camera(std::f32::consts::FRAC_PI_2, 1000.0, 1000.0);
    let size = project_sphere_diameter(Vec3::new(0.0, 0.0, -10.0), 0.0, &camera);
    assert_eq!(size, 0.0);
}
