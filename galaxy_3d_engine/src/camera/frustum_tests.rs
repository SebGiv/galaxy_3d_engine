use glam::{Mat4, Vec3};
use crate::scene::AABB;
use super::*;

// ============================================================================
// Frustum::from_view_projection
// ============================================================================

#[test]
fn test_frustum_from_identity_matrix() {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);

    // Identity VP → NDC cube: x,y,z in [-1, 1]
    // All 6 planes should exist and be normalized
    for plane in &frustum.planes {
        let normal_len = Vec3::new(plane.x, plane.y, plane.z).length();
        assert!((normal_len - 1.0).abs() < 1e-5, "plane normal should be unit length");
    }
}

#[test]
fn test_frustum_from_perspective_projection() {
    let projection = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_4, // 45° FOV
        16.0 / 9.0,                  // aspect ratio
        0.1,                         // near
        100.0,                       // far
    );
    let view = Mat4::look_at_rh(
        Vec3::new(0.0, 0.0, 5.0),   // eye
        Vec3::ZERO,                  // target
        Vec3::Y,                     // up
    );
    let vp = projection * view;

    let frustum = Frustum::from_view_projection(&vp);

    // Planes should be normalized
    for plane in &frustum.planes {
        let normal_len = Vec3::new(plane.x, plane.y, plane.z).length();
        assert!((normal_len - 1.0).abs() < 1e-4, "plane normal should be unit length");
    }
}

#[test]
fn test_frustum_from_orthographic_projection() {
    let projection = Mat4::orthographic_rh(
        -10.0, 10.0, // left, right
        -10.0, 10.0, // bottom, top
        0.1, 100.0,  // near, far
    );
    let vp = projection * Mat4::IDENTITY;

    let frustum = Frustum::from_view_projection(&vp);

    // All planes should be normalized
    for plane in &frustum.planes {
        let normal_len = Vec3::new(plane.x, plane.y, plane.z).length();
        assert!((normal_len - 1.0).abs() < 1e-4, "plane normal should be unit length");
    }
}

// ============================================================================
// Frustum::intersects_aabb
// ============================================================================

#[test]
fn test_aabb_inside_frustum() {
    let projection = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_2, // 90° FOV
        1.0,
        0.1,
        100.0,
    );
    let view = Mat4::look_at_rh(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
    );
    let vp = projection * view;
    let frustum = Frustum::from_view_projection(&vp);

    // AABB at the origin — should be inside the frustum
    let aabb = AABB {
        min: Vec3::new(-1.0, -1.0, -1.0),
        max: Vec3::new(1.0, 1.0, 1.0),
    };

    assert!(frustum.intersects_aabb(&aabb));
}

#[test]
fn test_aabb_outside_frustum() {
    let projection = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_4, // 45° FOV
        1.0,
        0.1,
        100.0,
    );
    let view = Mat4::look_at_rh(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
    );
    let vp = projection * view;
    let frustum = Frustum::from_view_projection(&vp);

    // AABB far to the right — should be outside the frustum
    let aabb = AABB {
        min: Vec3::new(100.0, 100.0, 100.0),
        max: Vec3::new(101.0, 101.0, 101.0),
    };

    assert!(!frustum.intersects_aabb(&aabb));
}

#[test]
fn test_aabb_behind_camera() {
    let projection = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_2,
        1.0,
        0.1,
        100.0,
    );
    let view = Mat4::look_at_rh(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
    );
    let vp = projection * view;
    let frustum = Frustum::from_view_projection(&vp);

    // AABB behind the camera (z > 5)
    let aabb = AABB {
        min: Vec3::new(-1.0, -1.0, 10.0),
        max: Vec3::new(1.0, 1.0, 12.0),
    };

    assert!(!frustum.intersects_aabb(&aabb));
}

#[test]
fn test_aabb_beyond_far_plane() {
    let projection = Mat4::perspective_rh(
        std::f32::consts::FRAC_PI_2,
        1.0,
        0.1,
        10.0, // far = 10
    );
    let view = Mat4::look_at_rh(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        Vec3::Y,
    );
    let vp = projection * view;
    let frustum = Frustum::from_view_projection(&vp);

    // AABB beyond far plane (more than 10 units from camera)
    let aabb = AABB {
        min: Vec3::new(-1.0, -1.0, -20.0),
        max: Vec3::new(1.0, 1.0, -18.0),
    };

    assert!(!frustum.intersects_aabb(&aabb));
}

#[test]
fn test_aabb_intersecting_frustum_boundary() {
    let projection = Mat4::orthographic_rh(
        -5.0, 5.0,
        -5.0, 5.0,
        0.1, 100.0,
    );
    let view = Mat4::IDENTITY;
    let vp = projection * view;
    let frustum = Frustum::from_view_projection(&vp);

    // AABB partially inside (straddles the right boundary at x=5)
    let aabb = AABB {
        min: Vec3::new(4.0, 0.0, -10.0),
        max: Vec3::new(6.0, 1.0, -5.0),
    };

    assert!(frustum.intersects_aabb(&aabb));
}

// ============================================================================
// Plane constants
// ============================================================================

#[test]
fn test_plane_constants() {
    assert_eq!(PLANE_LEFT, 0);
    assert_eq!(PLANE_RIGHT, 1);
    assert_eq!(PLANE_BOTTOM, 2);
    assert_eq!(PLANE_TOP, 3);
    assert_eq!(PLANE_NEAR, 4);
    assert_eq!(PLANE_FAR, 5);
}
