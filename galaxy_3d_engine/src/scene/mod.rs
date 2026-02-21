//! Scene management module
//!
//! Provides scene, render instance management, and rendering strategies
//! (culling, drawing, updating).

mod render_instance;
mod scene;
mod scene_manager;
mod culler;
mod drawer;
mod updater;

pub use render_instance::{
    RenderInstance, RenderInstanceKey, RenderLOD, RenderSubMesh,
    RenderPass, ResolvedPushConstant,
    AABB, FLAG_VISIBLE, FLAG_CAST_SHADOW, FLAG_RECEIVE_SHADOW,
};
pub use scene::Scene;
pub use scene_manager::SceneManager;
pub use culler::{CameraCuller, BruteForceCuller};
pub use drawer::{Drawer, ForwardDrawer};
pub use updater::{Updater, NoOpUpdater};
