//! Scene management module
//!
//! Provides scene and render instance management.

mod render_instance;
mod scene;
mod scene_manager;

pub use render_instance::{
    RenderInstance, RenderInstanceKey, RenderLOD, RenderSubMesh,
    RenderPass, ResolvedPushConstant,
    AABB, FLAG_VISIBLE, FLAG_CAST_SHADOW, FLAG_RECEIVE_SHADOW,
};
pub use scene::Scene;
pub use scene_manager::SceneManager;
