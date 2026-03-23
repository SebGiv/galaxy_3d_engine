/// Shader trait and shader descriptor

/// Shader stage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    /// Vertex shader
    Vertex,
    /// Fragment/Pixel shader
    Fragment,
    /// Compute shader
    Compute,
}

/// Descriptor for creating a shader
#[derive(Debug, Clone)]
pub struct ShaderDesc<'a> {
    /// Compiled shader bytecode (SPIR-V or DXIL)
    pub code: &'a [u8],
    /// Shader stage
    pub stage: ShaderStage,
    /// Entry point function name
    pub entry_point: String,
}

use crate::graphics_device::pipeline::{ReflectedBinding, ReflectedPushConstant};

/// Shader resource trait
///
/// Implemented by backend-specific shader types (e.g., VulkanShader).
/// The shader is automatically destroyed when dropped.
/// Exposes SPIR-V reflection data so the backend can deduce
/// descriptor set layouts and push constant ranges automatically.
pub trait Shader: Send + Sync {
    /// Reflected bindings from compiled shader bytecode
    fn reflected_bindings(&self) -> &[ReflectedBinding];
    /// Reflected push constant blocks from compiled shader bytecode
    fn reflected_push_constants(&self) -> &[ReflectedPushConstant];
}
