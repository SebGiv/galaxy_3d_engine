/// RendererPipeline trait and pipeline descriptor

use std::sync::Arc;
use crate::renderer::{RendererShader, TextureFormat, ShaderStage};

/// Primitive topology
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveTopology {
    /// Triangle list
    TriangleList,
    /// Triangle strip
    TriangleStrip,
    /// Line list
    LineList,
    /// Point list
    PointList,
}

/// Vertex input rate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexInputRate {
    /// Data is per-vertex
    Vertex,
    /// Data is per-instance
    Instance,
}

/// Vertex attribute description
#[derive(Debug, Clone, Copy)]
pub struct VertexAttribute {
    /// Attribute location in shader
    pub location: u32,
    /// Binding index
    pub binding: u32,
    /// Format of the attribute
    pub format: TextureFormat,
    /// Offset in bytes from the start of the vertex
    pub offset: u32,
}

/// Vertex binding description
#[derive(Debug, Clone, Copy)]
pub struct VertexBinding {
    /// Binding index
    pub binding: u32,
    /// Stride in bytes between consecutive elements
    pub stride: u32,
    /// Input rate (per-vertex or per-instance)
    pub input_rate: VertexInputRate,
}

/// Vertex input layout
#[derive(Debug, Clone)]
pub struct VertexLayout {
    /// Vertex bindings
    pub bindings: Vec<VertexBinding>,
    /// Vertex attributes
    pub attributes: Vec<VertexAttribute>,
}

impl Default for VertexLayout {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            attributes: Vec::new(),
        }
    }
}

/// Push constant range descriptor
#[derive(Debug, Clone)]
pub struct PushConstantRange {
    /// Shader stages that can access these push constants
    pub stages: Vec<ShaderStage>,
    /// Offset in bytes
    pub offset: u32,
    /// Size in bytes
    pub size: u32,
}

/// Descriptor for creating a graphics pipeline
#[derive(Clone)]
pub struct PipelineDesc {
    /// Vertex shader
    pub vertex_shader: Arc<dyn RendererShader>,
    /// Fragment shader
    pub fragment_shader: Arc<dyn RendererShader>,
    /// Vertex input layout
    pub vertex_layout: VertexLayout,
    /// Primitive topology
    pub topology: PrimitiveTopology,
    /// Push constant ranges (optional)
    pub push_constant_ranges: Vec<PushConstantRange>,
    /// Descriptor set layouts (for binding textures, uniforms, etc.)
    pub descriptor_set_layouts: Vec<u64>, // vk::DescriptorSetLayout as u64
    /// Enable alpha blending (default: false)
    pub enable_blending: bool,
}

/// Pipeline resource trait
///
/// Implemented by backend-specific pipeline types (e.g., VulkanRendererPipeline).
/// The pipeline is automatically destroyed when dropped.
pub trait RendererPipeline: Send + Sync {
    // No public methods for now, pipelines are created and bound by frames
}
