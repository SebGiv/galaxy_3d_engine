/// Render graph — a DAG describing how a frame is rendered.
///
/// A render graph defines the sequence of render passes (nodes) and the
/// render targets (edges) that connect them. It is the high-level description
/// of a complete rendering pipeline.
///
/// Passes and targets are stored in contiguous `Vec`s for cache-friendly
/// iteration, with `HashMap<String, usize>` for name-based lookup.
///
/// Render graphs can only be created via `RenderGraphManager::create_render_graph()`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::engine_bail;
use crate::renderer;
use crate::resource;
use super::render_pass::RenderPass;
use super::render_target::{RenderTarget, RenderTargetKind, TextureTargetView};

pub struct RenderGraph {
    /// Render passes (nodes) stored by index
    passes: Vec<RenderPass>,
    /// Pass name to index mapping
    pass_names: HashMap<String, usize>,
    /// Render targets (edges) stored by index
    targets: Vec<RenderTarget>,
    /// Target name to index mapping
    target_names: HashMap<String, usize>,
}

impl RenderGraph {
    /// Internal only — created via RenderGraphManager::create_render_graph()
    pub(crate) fn new() -> Self {
        Self {
            passes: Vec::new(),
            pass_names: HashMap::new(),
            targets: Vec::new(),
            target_names: HashMap::new(),
        }
    }

    // ===== ADD =====

    /// Add a named render pass (node) to the graph
    ///
    /// Returns the index of the newly added pass.
    ///
    /// # Errors
    ///
    /// Returns an error if a pass with the same name already exists.
    pub fn add_pass(&mut self, name: &str) -> Result<usize> {
        if self.pass_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderPass '{}' already exists in this graph", name);
        }

        let id = self.passes.len();
        self.passes.push(RenderPass::new());
        self.pass_names.insert(name.to_string(), id);
        Ok(id)
    }

    /// Add a swapchain render target (edge) to the graph
    ///
    /// At execution time, `acquire_next_image()` is called on the swapchain
    /// to get the current frame's render target.
    ///
    /// Returns the index of the newly added target.
    ///
    /// # Errors
    ///
    /// Returns an error if a target with the same name already exists.
    pub fn add_swapchain_target(
        &mut self,
        name: &str,
        swapchain: Arc<Mutex<dyn renderer::Swapchain>>,
    ) -> Result<usize> {
        if self.target_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderTarget '{}' already exists in this graph", name);
        }

        let id = self.targets.len();
        self.targets.push(RenderTarget::new(RenderTargetKind::Swapchain(swapchain)));
        self.target_names.insert(name.to_string(), id);
        Ok(id)
    }

    /// Add a texture render target (edge) to the graph
    ///
    /// References a resource texture at a specific array layer and mip level.
    ///
    /// Returns the index of the newly added target.
    ///
    /// # Errors
    ///
    /// Returns an error if a target with the same name already exists.
    pub fn add_texture_target(
        &mut self,
        name: &str,
        texture: Arc<resource::Texture>,
        layer: u32,
        mip_level: u32,
    ) -> Result<usize> {
        if self.target_names.contains_key(name) {
            engine_bail!("galaxy3d::RenderGraph",
                "RenderTarget '{}' already exists in this graph", name);
        }

        let id = self.targets.len();
        self.targets.push(RenderTarget::new(RenderTargetKind::Texture(TextureTargetView {
            texture,
            layer,
            mip_level,
        })));
        self.target_names.insert(name.to_string(), id);
        Ok(id)
    }

    // ===== CONNECT =====

    /// Set a pass as writing to a target (output)
    ///
    /// A target can only be written by one pass (single writer constraint).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pass name is not found
    /// - The target name is not found
    /// - The target is already written by another pass
    pub fn set_output(&mut self, pass_name: &str, target_name: &str) -> Result<()> {
        let pass_id = self.pass_names.get(pass_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderPass '{}' not found", pass_name))?;

        let target_id = self.target_names.get(target_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderTarget '{}' not found", target_name))?;

        // Check single writer constraint
        if let Some(existing_writer) = self.targets[target_id].written_by() {
            if existing_writer != pass_id {
                engine_bail!("galaxy3d::RenderGraph",
                    "RenderTarget '{}' is already written by another pass (index {})",
                    target_name, existing_writer);
            }
            // Same pass already set as writer — no-op
            return Ok(());
        }

        self.targets[target_id].set_written_by(pass_id);
        self.passes[pass_id].add_output(target_id);
        Ok(())
    }

    /// Set a pass as reading from a target (input)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pass name is not found
    /// - The target name is not found
    pub fn set_input(&mut self, pass_name: &str, target_name: &str) -> Result<()> {
        let pass_id = self.pass_names.get(pass_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderPass '{}' not found", pass_name))?;

        let target_id = self.target_names.get(target_name)
            .copied()
            .ok_or_else(|| crate::engine_err!("galaxy3d::RenderGraph",
                "RenderTarget '{}' not found", target_name))?;

        self.passes[pass_id].add_input(target_id);
        Ok(())
    }

    // ===== ACCESS BY INDEX (PRIMARY) =====

    /// Get a render pass by index
    pub fn pass(&self, id: usize) -> Option<&RenderPass> {
        self.passes.get(id)
    }

    /// Get a render target by index
    pub fn target(&self, id: usize) -> Option<&RenderTarget> {
        self.targets.get(id)
    }

    // ===== ACCESS BY NAME (SECONDARY) =====

    /// Get a render pass by name
    pub fn pass_by_name(&self, name: &str) -> Option<&RenderPass> {
        self.pass_names.get(name).and_then(|&id| self.passes.get(id))
    }

    /// Get a render target by name
    pub fn target_by_name(&self, name: &str) -> Option<&RenderTarget> {
        self.target_names.get(name).and_then(|&id| self.targets.get(id))
    }

    // ===== NAME → INDEX RESOLUTION =====

    /// Get a pass index by name
    pub fn pass_id(&self, name: &str) -> Option<usize> {
        self.pass_names.get(name).copied()
    }

    /// Get a target index by name
    pub fn target_id(&self, name: &str) -> Option<usize> {
        self.target_names.get(name).copied()
    }

    // ===== COUNTS =====

    /// Get the number of passes in the graph
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get the number of targets in the graph
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }
}
