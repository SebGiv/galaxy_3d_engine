//! Render graph management module
//!
//! Provides render graph creation and management.
//! A render graph is a directed acyclic graph (DAG) describing
//! how a frame is rendered — which passes execute, which targets
//! they read/write, and in what order.

mod render_graph;
mod render_graph_manager;
mod render_pass;
mod render_target;

pub use render_graph::RenderGraph;
pub use render_graph_manager::RenderGraphManager;
pub use render_pass::RenderPass;
pub use render_target::{RenderTarget, RenderTargetKind, TextureTargetView};
