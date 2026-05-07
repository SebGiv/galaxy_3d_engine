//! Render graph management module.
//!
//! A render graph is a per-frame DAG of `RenderPass`es operating on
//! shared `GraphResource`s (textures or buffers). Resources are
//! referenced by stable keys; passes can swap their target at runtime
//! without recreating the graph.
//!
//! `RenderGraph`, `RenderPass`, and `GraphResource` live in
//! `RenderGraphManager` with the (key, name) accessor convention used
//! elsewhere in the engine. `Framebuffer`s also live in the manager but
//! are unnamed and content-addressed via `get_or_create_framebuffer`.

mod access_type;
mod custom_pass_action;
mod debug_pass_action;
mod frame_buffer;
mod fullscreen_pass_action;
mod graph_resource;
mod pass_action;
mod render_graph;
mod render_graph_manager;
mod render_pass;
mod scene_pass_action;

#[cfg(test)]
mod test_helpers;

pub use access_type::{AccessType, ResourceAccess, TargetOps};
pub use custom_pass_action::CustomPassAction;
pub use debug_pass_action::{DebugDisplayMode, DebugDrawEntry, DebugPassAction};
pub use frame_buffer::{ColorAttachmentSlot, Framebuffer, FramebufferKey};
pub use fullscreen_pass_action::FullscreenPassAction;
pub use graph_resource::{
    BufferSubRange, GraphResource, GraphResourceKey, ImageSubRange,
    REMAINING_ARRAY_LAYERS, REMAINING_MIP_LEVELS, WHOLE_SIZE,
};
pub use pass_action::{PassAction, SceneBinding};
pub use render_graph::{RenderGraph, RenderGraphKey};
pub use render_graph_manager::RenderGraphManager;
pub use render_pass::{RenderPass, RenderPassKey};
pub use scene_pass_action::ScenePassAction;
