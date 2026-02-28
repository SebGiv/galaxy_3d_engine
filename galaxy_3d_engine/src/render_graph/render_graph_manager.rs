/// Central render graph manager for the engine.
///
/// Manages named render graphs. Each render graph describes
/// a complete rendering pipeline as a DAG of passes and targets.

use rustc_hash::FxHashMap;
use crate::error::Result;
use crate::engine_bail;
use super::render_graph::RenderGraph;

/// Render graph manager singleton (managed by Engine)
///
/// Stores named render graphs. Multiple render graphs can exist
/// simultaneously (e.g. different rendering configurations).
pub struct RenderGraphManager {
    render_graphs: FxHashMap<String, RenderGraph>,
}

impl RenderGraphManager {
    /// Create a new empty render graph manager
    pub fn new() -> Self {
        Self {
            render_graphs: FxHashMap::default(),
        }
    }

    /// Create a new named render graph
    ///
    /// Returns a reference to the created render graph.
    ///
    /// # Errors
    ///
    /// Returns an error if a render graph with the same name already exists.
    pub fn create_render_graph(&mut self, name: &str) -> Result<&RenderGraph> {
        if self.render_graphs.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraphManager",
                "RenderGraph '{}' already exists", name);
        }

        self.render_graphs.insert(name.to_string(), RenderGraph::new());
        Ok(self.render_graphs.get(name).unwrap())
    }

    /// Get a render graph by name
    pub fn render_graph(&self, name: &str) -> Option<&RenderGraph> {
        self.render_graphs.get(name)
    }

    /// Get a mutable render graph by name
    pub fn render_graph_mut(&mut self, name: &str) -> Option<&mut RenderGraph> {
        self.render_graphs.get_mut(name)
    }

    /// Remove a render graph by name
    ///
    /// Returns the removed render graph, or None if not found.
    pub fn remove_render_graph(&mut self, name: &str) -> Option<RenderGraph> {
        self.render_graphs.remove(name)
    }

    /// Get the number of render graphs
    pub fn render_graph_count(&self) -> usize {
        self.render_graphs.len()
    }

    /// Get all render graph names
    pub fn render_graph_names(&self) -> Vec<&str> {
        self.render_graphs.keys().map(|k| k.as_str()).collect()
    }

    /// Remove all render graphs
    pub fn clear(&mut self) {
        self.render_graphs.clear();
    }
}

#[cfg(test)]
#[path = "render_graph_manager_tests.rs"]
mod tests;
