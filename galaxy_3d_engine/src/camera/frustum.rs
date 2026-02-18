/// Frustum — six clipping planes for visibility culling.
///
/// Each plane is represented as a Vec4 (A, B, C, D) where:
/// - (A, B, C) is the inward-pointing normal
/// - D is the signed distance
/// - A point P is inside the frustum if dot(plane, P_homogeneous) >= 0 for all planes
///
/// The caller is responsible for computing and setting the frustum.
/// The engine provides `from_view_projection()` as a utility, but
/// the caller may compute the frustum by other means.

use glam::{Mat4, Vec3, Vec4};
use crate::scene::AABB;

/// Frustum plane indices
pub const PLANE_LEFT: usize = 0;
pub const PLANE_RIGHT: usize = 1;
pub const PLANE_BOTTOM: usize = 2;
pub const PLANE_TOP: usize = 3;
pub const PLANE_NEAR: usize = 4;
pub const PLANE_FAR: usize = 5;

/// Six frustum planes for culling.
///
/// Each plane is (A, B, C, D) where Ax + By + Cz + D = 0.
/// Normal (A, B, C) points inward (toward the visible volume).
/// Works with both perspective and orthographic projections.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    /// Frustum planes: left, right, bottom, top, near, far
    pub planes: [Vec4; 6],
}

impl Frustum {
    /// Extract frustum planes from a view-projection matrix.
    ///
    /// Uses the Gribb & Hartmann method. Works for both perspective
    /// and orthographic projections.
    pub fn from_view_projection(vp: &Mat4) -> Self {
        let m = vp.to_cols_array_2d();

        // Gribb & Hartmann: extract planes from rows of the VP matrix
        // Each plane is normalized so that (A, B, C) is a unit vector
        let mut planes = [
            // Left:   row3 + row0
            Vec4::new(m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0], m[3][3] + m[3][0]),
            // Right:  row3 - row0
            Vec4::new(m[0][3] - m[0][0], m[1][3] - m[1][0], m[2][3] - m[2][0], m[3][3] - m[3][0]),
            // Bottom: row3 + row1
            Vec4::new(m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1], m[3][3] + m[3][1]),
            // Top:    row3 - row1
            Vec4::new(m[0][3] - m[0][1], m[1][3] - m[1][1], m[2][3] - m[2][1], m[3][3] - m[3][1]),
            // Near:   row3 + row2
            Vec4::new(m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2], m[3][3] + m[3][2]),
            // Far:    row3 - row2
            Vec4::new(m[0][3] - m[0][2], m[1][3] - m[1][2], m[2][3] - m[2][2], m[3][3] - m[3][2]),
        ];

        // Normalize each plane
        for plane in &mut planes {
            let normal_len = Vec3::new(plane.x, plane.y, plane.z).length();
            if normal_len > 0.0 {
                *plane /= normal_len;
            }
        }

        Self { planes }
    }

    /// Test if an AABB intersects this frustum.
    ///
    /// Uses the "positive vertex" test: for each plane, find the AABB corner
    /// most in the direction of the plane normal. If that corner is outside,
    /// the AABB is fully outside.
    ///
    /// Returns `true` if the AABB is (potentially) inside or intersecting.
    /// May return false positives (conservative), never false negatives.
    pub fn intersects_aabb(&self, aabb: &AABB) -> bool {
        for plane in &self.planes {
            let normal = Vec3::new(plane.x, plane.y, plane.z);

            // Find the positive vertex (corner most aligned with the normal)
            let p_vertex = Vec3::new(
                if normal.x >= 0.0 { aabb.max.x } else { aabb.min.x },
                if normal.y >= 0.0 { aabb.max.y } else { aabb.min.y },
                if normal.z >= 0.0 { aabb.max.z } else { aabb.min.z },
            );

            // If the positive vertex is outside this plane, the AABB is fully outside
            if normal.dot(p_vertex) + plane.w < 0.0 {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
#[path = "frustum_tests.rs"]
mod tests;
