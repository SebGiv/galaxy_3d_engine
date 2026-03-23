/// Resource-level pipeline type.
///
/// A Pipeline wraps a single graphics_device::Pipeline (GPU pipeline object)
/// and exposes its reflection data. Created by the ResourceManager which
/// handles the GPU pipeline creation via the GraphicsDevice.

use std::sync::Arc;
use crate::graphics_device;
use crate::resource::resource_manager::ShaderKey;

// ===== PIPELINE =====

/// Pipeline resource wrapping a single GPU pipeline
pub struct Pipeline {
    graphics_device_pipeline: Arc<dyn graphics_device::Pipeline>,
    vertex_shader: ShaderKey,
    fragment_shader: ShaderKey,
}

// ===== DESCRIPTOR =====

/// Pipeline creation descriptor
///
/// Shaders are referenced by ShaderKey; the ResourceManager resolves them
/// and passes the GPU shader objects to the backend.
/// Push constant ranges and descriptor set layouts are deduced automatically
/// from shader reflection.
pub struct PipelineDesc {
    /// Vertex shader
    pub vertex_shader: ShaderKey,
    /// Fragment shader
    pub fragment_shader: ShaderKey,
    /// Vertex input layout
    pub vertex_layout: graphics_device::VertexLayout,
    /// Primitive topology
    pub topology: graphics_device::PrimitiveTopology,
    /// Rasterization state
    pub rasterization: graphics_device::RasterizationState,
    /// Color blending state
    pub color_blend: graphics_device::ColorBlendState,
    /// Multisampling state
    pub multisample: graphics_device::MultisampleState,
    /// Color attachment formats
    pub color_formats: Vec<graphics_device::TextureFormat>,
    /// Depth/stencil attachment format (None if no depth/stencil)
    pub depth_format: Option<graphics_device::TextureFormat>,
}

// ===== PIPELINE IMPLEMENTATION =====

impl Pipeline {
    /// Create pipeline from a pre-built GPU pipeline (internal use by ResourceManager)
    pub(crate) fn from_gpu_pipeline(
        graphics_device_pipeline: Arc<dyn graphics_device::Pipeline>,
        vertex_shader: ShaderKey,
        fragment_shader: ShaderKey,
    ) -> Self {
        Self { graphics_device_pipeline, vertex_shader, fragment_shader }
    }

    /// Get the vertex shader key
    pub fn vertex_shader(&self) -> ShaderKey {
        self.vertex_shader
    }

    /// Get the fragment shader key
    pub fn fragment_shader(&self) -> ShaderKey {
        self.fragment_shader
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
