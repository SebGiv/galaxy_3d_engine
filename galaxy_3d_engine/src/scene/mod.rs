//! Scene management module
//!
//! Provides scene, render instance management, and rendering strategies
//! (culling, dispatching, drawing, updating).

mod render_instance;
mod light;
mod scene;
mod scene_manager;
mod scene_index;
mod octree_scene_index;
mod culler;
mod drawer;
mod updater;
mod render_view;
mod view_dispatcher;
mod render_queue;

pub use render_instance::{
    RenderInstance, RenderInstanceKey, RenderSubMesh, RenderSubMeshPass,
    VertexShaderOverride,
    AABB, FLAG_VISIBLE, FLAG_CAST_SHADOW, FLAG_RECEIVE_SHADOW,
};
pub use render_view::{RenderView, VisibleSubMesh};
pub use view_dispatcher::ViewDispatcher;
pub use light::{Light, LightKey, LightType, LightDesc};
pub use scene::Scene;
pub use scene_manager::SceneManager;
pub use scene_index::SceneIndex;
pub use octree_scene_index::OctreeSceneIndex;
pub use culler::{CameraCuller, BruteForceCuller, FrustumCuller};
pub use drawer::{Drawer, ForwardDrawer};
pub use updater::{Updater, NoOpUpdater, DefaultUpdater};
pub use render_queue::{RenderQueue, DrawCall, distance_to_u16, build_sort_key};
