//! Camera module — low-level camera, frustum, and render view.
//!
//! Provides passive data containers for the rendering pipeline.
//! The engine does NOT store or manage cameras — they are tools
//! provided by the engine, owned and driven by the caller.

mod camera;
mod frustum;
mod render_view;

pub use camera::Camera;
pub use frustum::{
    Frustum,
    PLANE_LEFT, PLANE_RIGHT, PLANE_BOTTOM, PLANE_TOP, PLANE_NEAR, PLANE_FAR,
};
pub use render_view::RenderView;
