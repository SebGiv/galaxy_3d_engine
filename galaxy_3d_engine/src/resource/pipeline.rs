/// Resource-level pipeline type.
///
/// A Pipeline wraps a single graphics_device::Pipeline (GPU pipeline object)
/// and exposes its reflection data. Created by the ResourceManager which
/// handles the GPU pipeline creation via the GraphicsDevice.

use std::sync::Arc;
use crate::graphics_device;

// ===== PIPELINE =====

/// Pipeline resource wrapping a single GPU pipeline
pub struct Pipeline {
    graphics_device_pipeline: Arc<dyn graphics_device::Pipeline>,
}

// ===== DESCRIPTOR =====

/// Pipeline creation descriptor
pub struct PipelineDesc {
    pub pipeline: graphics_device::PipelineDesc,
}

// ===== PIPELINE IMPLEMENTATION =====

impl Pipeline {
    /// Create pipeline from a pre-built GPU pipeline (internal use by ResourceManager)
    pub(crate) fn from_gpu_pipeline(graphics_device_pipeline: Arc<dyn graphics_device::Pipeline>) -> Self {
        Self { graphics_device_pipeline }
    }

    /// Get the underlying graphics device pipeline
    pub fn graphics_device_pipeline(&self) -> &Arc<dyn graphics_device::Pipeline> {
        &self.graphics_device_pipeline
    }

    /// Get shader reflection data
    pub fn reflection(&self) -> &graphics_device::PipelineReflection {
        self.graphics_device_pipeline.reflection()
    }

    /// Get the number of binding group layouts
    pub fn binding_group_layout_count(&self) -> u32 {
        self.graphics_device_pipeline.binding_group_layout_count()
    }
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
