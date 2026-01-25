/// RendererShader trait and shader descriptor

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

/// Shader resource trait
///
/// Implemented by backend-specific shader types (e.g., VulkanRendererShader).
/// The shader is automatically destroyed when dropped.
pub trait RendererShader: Send + Sync {
    // No public methods, shaders are used by pipelines
}
