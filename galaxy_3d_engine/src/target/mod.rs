//! Render target management module
//!
//! Provides render target creation and management.
//! A render target defines where a scene gets rendered to
//! (screen, texture, etc.).

mod render_target;
mod target_manager;

pub use render_target::RenderTarget;
pub use target_manager::TargetManager;
