/// Central render target manager for the engine.
///
/// Manages named render targets. Render targets define where
/// scenes get rendered to (screen, textures, etc.).

use std::collections::HashMap;
use crate::error::Result;
use crate::engine_bail;
use super::render_target::RenderTarget;

/// Target manager singleton (managed by Engine)
///
/// Stores named render targets. Multiple render targets can exist
/// simultaneously (screen, shadow maps, post-processing buffers, etc.).
pub struct TargetManager {
    render_targets: HashMap<String, RenderTarget>,
}

impl TargetManager {
    /// Create a new empty target manager
    pub fn new() -> Self {
        Self {
            render_targets: HashMap::new(),
        }
    }

    /// Create a new named render target
    ///
    /// Returns a reference to the created render target.
    ///
    /// # Errors
    ///
    /// Returns an error if a render target with the same name already exists.
    pub fn create_render_target(&mut self, name: &str) -> Result<&RenderTarget> {
        if self.render_targets.contains_key(name) {
            engine_bail!("galaxy3d::TargetManager",
                "RenderTarget '{}' already exists", name);
        }

        self.render_targets.insert(name.to_string(), RenderTarget::new());
        Ok(self.render_targets.get(name).unwrap())
    }

    /// Get a render target by name
    pub fn render_target(&self, name: &str) -> Option<&RenderTarget> {
        self.render_targets.get(name)
    }

    /// Get a mutable render target by name
    pub fn render_target_mut(&mut self, name: &str) -> Option<&mut RenderTarget> {
        self.render_targets.get_mut(name)
    }

    /// Remove a render target by name
    ///
    /// Returns the removed render target, or None if not found.
    pub fn remove_render_target(&mut self, name: &str) -> Option<RenderTarget> {
        self.render_targets.remove(name)
    }

    /// Get the number of render targets
    pub fn render_target_count(&self) -> usize {
        self.render_targets.len()
    }

    /// Get all render target names
    pub fn render_target_names(&self) -> Vec<&str> {
        self.render_targets.keys().map(|k| k.as_str()).collect()
    }

    /// Remove all render targets
    pub fn clear(&mut self) {
        self.render_targets.clear();
    }
}

#[cfg(test)]
#[path = "target_manager_tests.rs"]
mod tests;
