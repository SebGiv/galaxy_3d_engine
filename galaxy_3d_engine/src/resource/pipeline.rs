/// Resource-level pipeline type with variant and multi-pass support.
///
/// A Pipeline groups related pipeline configurations under named variants.
/// Each variant contains one or more ordered rendering passes.
/// For example: "toon_outline" pipeline with variant "static" containing
/// pass 0 (toon base, cull back) and pass 1 (outline, cull front).
///
/// Can be created empty and variants added later (user responsibility).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::renderer;

// ===== PIPELINE =====

/// Pipeline resource with variant support
pub struct Pipeline {
    renderer: Arc<Mutex<dyn renderer::Renderer>>,
    variants: Vec<PipelineVariant>,
    variant_names: HashMap<String, usize>,
}

/// A single pipeline variant with one or more rendering passes
pub struct PipelineVariant {
    name: String,
    passes: Vec<PipelinePass>,
}

/// A single rendering pass within a pipeline variant
pub struct PipelinePass {
    renderer_pipeline: Arc<dyn renderer::Pipeline>,
}

// ===== DESCRIPTORS =====

/// Pipeline creation descriptor
pub struct PipelineDesc {
    pub renderer: Arc<Mutex<dyn renderer::Renderer>>,
    pub variants: Vec<PipelineVariantDesc>,
}

/// Pipeline variant descriptor with one or more passes
pub struct PipelineVariantDesc {
    pub name: String,
    pub passes: Vec<PipelinePassDesc>,
}

/// Descriptor for a single rendering pass
pub struct PipelinePassDesc {
    pub pipeline: renderer::PipelineDesc,
}

// ===== PIPELINE IMPLEMENTATION =====

impl Pipeline {
    /// Create pipeline from descriptor (internal use by ResourceManager)
    pub(crate) fn from_desc(desc: PipelineDesc) -> Result<Self> {
        // ========== VALIDATION 1: No duplicate variant names ==========
        let mut seen_names = std::collections::HashSet::new();
        for variant_desc in &desc.variants {
            if !seen_names.insert(&variant_desc.name) {
                crate::engine_bail!("galaxy3d::Pipeline",
                    "Duplicate variant name '{}'", variant_desc.name);
            }
        }

        // ========== VALIDATION 2: Each variant must have at least one pass ==========
        for variant_desc in &desc.variants {
            if variant_desc.passes.is_empty() {
                crate::engine_bail!("galaxy3d::Pipeline",
                    "Variant '{}' must have at least one pass", variant_desc.name);
            }
        }

        // ========== CREATE VARIANTS ==========
        let mut variants = Vec::new();
        let mut variant_names = HashMap::new();

        for (vec_index, variant_desc) in desc.variants.into_iter().enumerate() {
            // Create GPU pipelines for each pass
            let mut passes = Vec::new();
            for pass_desc in variant_desc.passes {
                let renderer_pipeline = desc.renderer.lock().unwrap()
                    .create_pipeline(pass_desc.pipeline)?;
                passes.push(PipelinePass { renderer_pipeline });
            }

            let variant = PipelineVariant {
                name: variant_desc.name.clone(),
                passes,
            };

            variants.push(variant);
            variant_names.insert(variant_desc.name, vec_index);
        }

        Ok(Self {
            renderer: desc.renderer,
            variants,
            variant_names,
        })
    }

    // ===== VARIANT ACCESS =====

    /// Get variant by index
    pub fn variant(&self, index: u32) -> Option<&PipelineVariant> {
        self.variants.get(index as usize)
    }

    /// Get variant by name
    pub fn variant_by_name(&self, name: &str) -> Option<&PipelineVariant> {
        let index = self.variant_names.get(name)?;
        self.variants.get(*index)
    }

    /// Get variant index from name
    pub fn variant_index(&self, name: &str) -> Option<u32> {
        self.variant_names.get(name).map(|&idx| idx as u32)
    }

    /// Get total number of variants
    pub fn variant_count(&self) -> usize {
        self.variants.len()
    }

    /// Get maximum pass count across all variants
    pub fn max_pass_count(&self) -> usize {
        self.variants.iter().map(|v| v.passes.len()).max().unwrap_or(0)
    }

    /// Get the renderer reference (needed by Material for BindingGroup creation)
    pub fn renderer(&self) -> &Arc<Mutex<dyn renderer::Renderer>> {
        &self.renderer
    }

    // ===== MODIFICATION =====

    /// Add a new variant
    ///
    /// Uses the stored renderer to create GPU pipelines for each pass.
    pub fn add_variant(&mut self, desc: PipelineVariantDesc) -> Result<u32> {
        // Check for duplicate name
        if self.variant_names.contains_key(&desc.name) {
            crate::engine_bail!("galaxy3d::Pipeline",
                "Variant '{}' already exists", desc.name);
        }

        // Validate at least one pass
        if desc.passes.is_empty() {
            crate::engine_bail!("galaxy3d::Pipeline",
                "Variant '{}' must have at least one pass", desc.name);
        }

        // Create GPU pipelines for each pass
        let mut passes = Vec::new();
        for pass_desc in desc.passes {
            let renderer_pipeline = self.renderer.lock().unwrap()
                .create_pipeline(pass_desc.pipeline)?;
            passes.push(PipelinePass { renderer_pipeline });
        }

        let variant = PipelineVariant {
            name: desc.name.clone(),
            passes,
        };

        let vec_index = self.variants.len();
        self.variants.push(variant);
        self.variant_names.insert(desc.name, vec_index);

        Ok(vec_index as u32)
    }
}

// ===== PIPELINE VARIANT IMPLEMENTATION =====

impl PipelineVariant {
    /// Get variant name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get number of rendering passes
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get pass by index
    pub fn pass(&self, index: u32) -> Option<&PipelinePass> {
        self.passes.get(index as usize)
    }
}

// ===== PIPELINE PASS IMPLEMENTATION =====

impl PipelinePass {
    /// Get the underlying renderer pipeline
    pub fn renderer_pipeline(&self) -> &Arc<dyn renderer::Pipeline> {
        &self.renderer_pipeline
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;