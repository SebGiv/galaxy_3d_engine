/// Resource-level pipeline type with variant support.
///
/// A Pipeline groups related pipeline configurations under named variants.
/// For example: "mesh" pipeline with variants "static", "animated", "transparent".
///
/// Can be created empty and variants added later (user responsibility).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::{Error, Result};
use crate::renderer::{
    Pipeline as RendererPipeline,
    Renderer,
    PipelineDesc as RenderPipelineDesc,
};

// ===== PIPELINE =====

/// Pipeline resource with variant support
pub struct Pipeline {
    renderer: Arc<Mutex<dyn Renderer>>,
    variants: Vec<PipelineVariant>,
    variant_names: HashMap<String, usize>,
}

/// A single pipeline variant
pub struct PipelineVariant {
    name: String,
    renderer_pipeline: Arc<dyn RendererPipeline>,
}

// ===== DESCRIPTORS =====

/// Pipeline creation descriptor
pub struct PipelineDesc {
    pub renderer: Arc<Mutex<dyn Renderer>>,
    pub variants: Vec<PipelineVariantDesc>,
}

/// Pipeline variant descriptor
pub struct PipelineVariantDesc {
    pub name: String,
    pub pipeline: RenderPipelineDesc,
}

// ===== PIPELINE IMPLEMENTATION =====

impl Pipeline {
    /// Create pipeline from descriptor (internal use by ResourceManager)
    pub(crate) fn from_desc(desc: PipelineDesc) -> Result<Self> {
        // ========== VALIDATION 1: No duplicate variant names ==========
        let mut seen_names = std::collections::HashSet::new();
        for variant_desc in &desc.variants {
            if !seen_names.insert(&variant_desc.name) {
                return Err(Error::BackendError(format!(
                    "Duplicate variant name '{}'", variant_desc.name
                )));
            }
        }

        // ========== CREATE VARIANTS ==========
        let mut variants = Vec::new();
        let mut variant_names = HashMap::new();

        for (vec_index, variant_desc) in desc.variants.into_iter().enumerate() {
            // Create GPU pipeline
            let renderer_pipeline = desc.renderer.lock().unwrap()
                .create_pipeline(variant_desc.pipeline)?;

            let variant = PipelineVariant {
                name: variant_desc.name.clone(),
                renderer_pipeline,
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

    // ===== MODIFICATION =====

    /// Add a new variant
    ///
    /// Uses the stored renderer to create the GPU pipeline.
    pub fn add_variant(&mut self, desc: PipelineVariantDesc) -> Result<u32> {
        // Check for duplicate name
        if self.variant_names.contains_key(&desc.name) {
            return Err(Error::BackendError(format!(
                "Variant '{}' already exists", desc.name
            )));
        }

        // Create GPU pipeline using stored renderer
        let renderer_pipeline = self.renderer.lock().unwrap()
            .create_pipeline(desc.pipeline)?;

        let variant = PipelineVariant {
            name: desc.name.clone(),
            renderer_pipeline,
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

    /// Get the underlying renderer pipeline
    pub fn renderer_pipeline(&self) -> &Arc<dyn RendererPipeline> {
        &self.renderer_pipeline
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
