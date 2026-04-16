//! Screen-space size projection for LOD selection.
//!
//! Computes the on-screen diameter (in pixels) of a world-space bounding
//! sphere given a camera. This is the standard metric used by modern LOD
//! systems (Unity `relativeHeight`, Simplygon "on-screen-size", Nanite
//! per-cluster screen-space error) — invariant to viewport resolution and
//! field of view.

use glam::Vec3;
use super::camera::Camera;

/// Projected diameter, in pixels, of a world-space bounding sphere.
///
/// Uses the **forward-axis depth** (projection of the center–camera vector
/// onto the camera forward), not the Euclidean distance. This matches how
/// Vulkan/OpenGL compute clip-space `z` and avoids sphere "inflation" on the
/// sides of the frustum.
///
/// The formula uses the vertical FOV derived from the projection matrix:
/// `projection[1][1] = 1 / tan(fov_y / 2)`, so
/// `fov_y_factor = 2 * tan(fov_y / 2) = 2 / projection[1][1]`.
///
/// Returns a non-negative value. If the object is behind the camera or at
/// the exact camera position, the depth is clamped to a small epsilon so
/// the function remains finite — in practice those cases are also removed
/// by frustum culling upstream.
pub fn project_sphere_diameter(
    center_ws: Vec3,
    radius_ws: f32,
    camera: &Camera,
) -> f32 {
    let world = camera.view_matrix().inverse();
    let cam_pos = world.w_axis.truncate();
    let cam_forward = -world.z_axis.truncate();

    let view_depth = (center_ws - cam_pos).dot(cam_forward).max(1e-4);

    // projection[1][1] == 1 / tan(fov_y / 2) for a standard perspective matrix
    let p11 = camera.projection_matrix().y_axis.y;
    // Protect against orthographic / degenerate matrices (p11 == 0).
    let fov_y_factor = if p11.abs() > 1e-6 { 2.0 / p11 } else { 0.0 };

    let viewport_h = camera.viewport().height as f32;

    (2.0 * radius_ws * viewport_h) / (view_depth * fov_y_factor.max(1e-6))
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod tests;
