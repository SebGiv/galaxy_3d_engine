/// Camera — low-level passive data container.
///
/// The Camera computes nothing. The caller (game engine) is responsible
/// for computing and setting all fields: view matrix, projection matrix,
/// frustum, viewport, and scissor.
///
/// The engine does NOT store or manage cameras. They are tools provided
/// by the engine, owned and driven by the caller.

use glam::Mat4;
use crate::renderer::command_list::{Viewport, Rect2D};
use super::frustum::Frustum;

/// Low-level camera. A passive data container — computes nothing.
///
/// The caller is responsible for computing and setting all fields.
/// Typically, the game engine computes view/projection/frustum from
/// high-level parameters (position, rotation, FOV, etc.) and passes
/// the results here.
#[derive(Debug, Clone)]
pub struct Camera {
    view_matrix: Mat4,
    projection_matrix: Mat4,
    frustum: Frustum,
    viewport: Viewport,
    scissor: Option<Rect2D>,
}

impl Camera {
    /// Create a new camera with the given parameters.
    ///
    /// The scissor defaults to `None` (same as viewport).
    pub fn new(view: Mat4, projection: Mat4, frustum: Frustum, viewport: Viewport) -> Self {
        Self {
            view_matrix: view,
            projection_matrix: projection,
            frustum,
            viewport,
            scissor: None,
        }
    }

    // ===== GETTERS =====

    /// View matrix (inverse of the camera's world transform).
    pub fn view_matrix(&self) -> &Mat4 {
        &self.view_matrix
    }

    /// Projection matrix (perspective or orthographic).
    pub fn projection_matrix(&self) -> &Mat4 {
        &self.projection_matrix
    }

    /// Combined view-projection matrix (projection * view).
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix * self.view_matrix
    }

    /// Frustum planes for culling.
    pub fn frustum(&self) -> &Frustum {
        &self.frustum
    }

    /// Viewport dimensions and depth range.
    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    /// Scissor rectangle, if set.
    pub fn scissor(&self) -> Option<&Rect2D> {
        self.scissor.as_ref()
    }

    /// Effective scissor: explicit scissor or viewport bounds as Rect2D.
    pub fn effective_scissor(&self) -> Rect2D {
        self.scissor.unwrap_or(Rect2D {
            x: self.viewport.x as i32,
            y: self.viewport.y as i32,
            width: self.viewport.width as u32,
            height: self.viewport.height as u32,
        })
    }

    // ===== SETTERS — store, compute nothing =====

    /// Set the view matrix.
    pub fn set_view(&mut self, matrix: Mat4) {
        self.view_matrix = matrix;
    }

    /// Set the projection matrix.
    pub fn set_projection(&mut self, matrix: Mat4) {
        self.projection_matrix = matrix;
    }

    /// Set the frustum.
    pub fn set_frustum(&mut self, frustum: Frustum) {
        self.frustum = frustum;
    }

    /// Set the viewport.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
    }

    /// Set the scissor rectangle. `None` means same as viewport.
    pub fn set_scissor(&mut self, scissor: Option<Rect2D>) {
        self.scissor = scissor;
    }
}

#[cfg(test)]
#[path = "camera_tests.rs"]
mod tests;
