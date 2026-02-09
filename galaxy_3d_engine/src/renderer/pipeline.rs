/// Pipeline trait and pipeline descriptor

use std::sync::Arc;
use crate::renderer::{Shader, BufferFormat, ShaderStage};

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

/// Index buffer element type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// 16-bit indices (max 65535 vertices)
    U16,
    /// 32-bit indices (max ~4 billion vertices)
    U32,
}

impl IndexType {
    /// Size in bytes of one index element
    pub fn size_bytes(&self) -> u32 {
        match self {
            IndexType::U16 => 2,
            IndexType::U32 => 4,
        }
    }
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
    /// Format of the attribute (data type and component count)
    pub format: BufferFormat,
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

// ===== RASTERIZATION ENUMS =====

/// Face culling mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    /// No culling
    None,
    /// Cull front faces
    Front,
    /// Cull back faces
    Back,
}

/// Front face winding order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontFace {
    /// Counter-clockwise vertices define front face
    CounterClockwise,
    /// Clockwise vertices define front face
    Clockwise,
}

/// Polygon rendering mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonMode {
    /// Fill polygons
    Fill,
    /// Draw edges only (wireframe)
    Line,
    /// Draw vertices only
    Point,
}

// ===== DEPTH/STENCIL ENUMS =====

/// Comparison operator for depth and stencil tests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    /// Never pass
    Never,
    /// Pass if value < reference
    Less,
    /// Pass if value == reference
    Equal,
    /// Pass if value <= reference
    LessOrEqual,
    /// Pass if value > reference
    Greater,
    /// Pass if value != reference
    NotEqual,
    /// Pass if value >= reference
    GreaterOrEqual,
    /// Always pass
    Always,
}

/// Stencil operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StencilOp {
    /// Keep current value
    Keep,
    /// Set to zero
    Zero,
    /// Replace with reference value
    Replace,
    /// Increment and clamp to max
    IncrementAndClamp,
    /// Decrement and clamp to zero
    DecrementAndClamp,
    /// Bitwise invert
    Invert,
    /// Increment and wrap around
    IncrementAndWrap,
    /// Decrement and wrap around
    DecrementAndWrap,
}

// ===== COLOR BLEND ENUMS =====

/// Blend factor for color blending equations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    ConstantColor,
    OneMinusConstantColor,
    SrcAlphaSaturate,
}

/// Blend operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendOp {
    /// result = src * srcFactor + dst * dstFactor
    Add,
    /// result = src * srcFactor - dst * dstFactor
    Subtract,
    /// result = dst * dstFactor - src * srcFactor
    ReverseSubtract,
    /// result = min(src, dst)
    Min,
    /// result = max(src, dst)
    Max,
}

// ===== MULTISAMPLE ENUMS =====

/// Multisample count
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleCount {
    /// 1 sample (no multisampling)
    S1,
    /// 2 samples
    S2,
    /// 4 samples
    S4,
    /// 8 samples
    S8,
}

// ===== RASTERIZATION STATE =====

/// Depth bias parameters
#[derive(Debug, Clone, Copy)]
pub struct DepthBias {
    /// Constant depth offset
    pub constant_factor: f32,
    /// Slope-based depth offset
    pub slope_factor: f32,
    /// Maximum depth bias clamp
    pub clamp: f32,
}

/// Rasterization fixed-function state
#[derive(Debug, Clone, Copy)]
pub struct RasterizationState {
    /// Face culling mode
    pub cull_mode: CullMode,
    /// Front face winding order
    pub front_face: FrontFace,
    /// Polygon rendering mode
    pub polygon_mode: PolygonMode,
    /// Depth bias (None = disabled)
    pub depth_bias: Option<DepthBias>,
}

impl Default for RasterizationState {
    fn default() -> Self {
        Self {
            cull_mode: CullMode::Back,
            front_face: FrontFace::CounterClockwise,
            polygon_mode: PolygonMode::Fill,
            depth_bias: None,
        }
    }
}

// ===== DEPTH/STENCIL STATE =====

/// Stencil operation state (per-face)
#[derive(Debug, Clone, Copy)]
pub struct StencilOpState {
    /// Action on stencil test fail
    pub fail_op: StencilOp,
    /// Action on stencil pass + depth pass
    pub pass_op: StencilOp,
    /// Action on stencil pass + depth fail
    pub depth_fail_op: StencilOp,
    /// Comparison operator
    pub compare_op: CompareOp,
    /// Bits of stencil buffer read for compare
    pub compare_mask: u32,
    /// Bits of stencil buffer written
    pub write_mask: u32,
    /// Reference value for compare/replace
    pub reference: u32,
}

impl Default for StencilOpState {
    fn default() -> Self {
        Self {
            fail_op: StencilOp::Keep,
            pass_op: StencilOp::Keep,
            depth_fail_op: StencilOp::Keep,
            compare_op: CompareOp::Always,
            compare_mask: 0xFF,
            write_mask: 0xFF,
            reference: 0,
        }
    }
}

/// Depth and stencil testing state
#[derive(Debug, Clone, Copy)]
pub struct DepthStencilState {
    /// Enable depth testing
    pub depth_test_enable: bool,
    /// Enable writing to depth buffer
    pub depth_write_enable: bool,
    /// Depth comparison operator
    pub depth_compare_op: CompareOp,
    /// Enable stencil testing
    pub stencil_test_enable: bool,
    /// Stencil operations for front faces
    pub front: StencilOpState,
    /// Stencil operations for back faces
    pub back: StencilOpState,
}

impl Default for DepthStencilState {
    fn default() -> Self {
        Self {
            depth_test_enable: true,
            depth_write_enable: true,
            depth_compare_op: CompareOp::Less,
            stencil_test_enable: false,
            front: StencilOpState::default(),
            back: StencilOpState::default(),
        }
    }
}

// ===== COLOR BLEND STATE =====

/// Color write mask
#[derive(Debug, Clone, Copy)]
pub struct ColorWriteMask {
    pub r: bool,
    pub g: bool,
    pub b: bool,
    pub a: bool,
}

impl ColorWriteMask {
    /// All channels enabled
    pub const ALL: Self = Self { r: true, g: true, b: true, a: true };
    /// No channels enabled
    pub const NONE: Self = Self { r: false, g: false, b: false, a: false };
}

impl Default for ColorWriteMask {
    fn default() -> Self {
        Self::ALL
    }
}

/// Color blending state
#[derive(Debug, Clone, Copy)]
pub struct ColorBlendState {
    /// Enable blending
    pub blend_enable: bool,
    /// Source color blend factor
    pub src_color_factor: BlendFactor,
    /// Destination color blend factor
    pub dst_color_factor: BlendFactor,
    /// Color blend operation
    pub color_blend_op: BlendOp,
    /// Source alpha blend factor
    pub src_alpha_factor: BlendFactor,
    /// Destination alpha blend factor
    pub dst_alpha_factor: BlendFactor,
    /// Alpha blend operation
    pub alpha_blend_op: BlendOp,
    /// Color write mask
    pub color_write_mask: ColorWriteMask,
}

impl Default for ColorBlendState {
    fn default() -> Self {
        Self {
            blend_enable: false,
            src_color_factor: BlendFactor::One,
            dst_color_factor: BlendFactor::Zero,
            color_blend_op: BlendOp::Add,
            src_alpha_factor: BlendFactor::One,
            dst_alpha_factor: BlendFactor::Zero,
            alpha_blend_op: BlendOp::Add,
            color_write_mask: ColorWriteMask::ALL,
        }
    }
}

// ===== MULTISAMPLE STATE =====

/// Multisampling state
#[derive(Debug, Clone, Copy)]
pub struct MultisampleState {
    /// Number of samples per pixel
    pub sample_count: SampleCount,
    /// Enable alpha-to-coverage
    pub alpha_to_coverage: bool,
}

impl Default for MultisampleState {
    fn default() -> Self {
        Self {
            sample_count: SampleCount::S1,
            alpha_to_coverage: false,
        }
    }
}

// ===== PIPELINE DESCRIPTOR =====

/// Descriptor for creating a graphics pipeline
#[derive(Clone)]
pub struct PipelineDesc {
    /// Vertex shader
    pub vertex_shader: Arc<dyn Shader>,
    /// Fragment shader
    pub fragment_shader: Arc<dyn Shader>,
    /// Vertex input layout
    pub vertex_layout: VertexLayout,
    /// Primitive topology
    pub topology: PrimitiveTopology,
    /// Push constant ranges (optional)
    pub push_constant_ranges: Vec<PushConstantRange>,
    /// Descriptor set layouts (for binding textures, uniforms, etc.)
    pub descriptor_set_layouts: Vec<u64>, // vk::DescriptorSetLayout as u64
    /// Rasterization state
    pub rasterization: RasterizationState,
    /// Depth and stencil testing state
    pub depth_stencil: DepthStencilState,
    /// Color blending state
    pub color_blend: ColorBlendState,
    /// Multisampling state
    pub multisample: MultisampleState,
}

/// Pipeline resource trait
///
/// Implemented by backend-specific pipeline types (e.g., VulkanPipeline).
/// The pipeline is automatically destroyed when dropped.
pub trait Pipeline: Send + Sync {
    // No public methods for now, pipelines are created and bound by frames
}

#[cfg(test)]
#[path = "pipeline_tests.rs"]
mod tests;
