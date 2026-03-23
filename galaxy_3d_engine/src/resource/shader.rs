/// Resource-level shader type.
///
/// A Shader wraps a single graphics_device::Shader (compiled GPU shader)
/// and stores its stage. Created by the ResourceManager which handles
/// the GPU shader compilation via the GraphicsDevice.

use std::sync::Arc;
use crate::graphics_device;
use crate::graphics_device::ShaderStage;

// ===== SHADER =====

/// Shader resource wrapping a compiled GPU shader
pub struct Shader {
    graphics_device_shader: Arc<dyn graphics_device::Shader>,
    stage: ShaderStage,
}

// ===== DESCRIPTOR =====

/// Shader creation descriptor
pub struct ShaderDesc<'a> {
    /// Compiled shader bytecode (SPIR-V or DXIL)
    pub code: &'a [u8],
    /// Shader stage (Vertex, Fragment, Compute)
    pub stage: ShaderStage,
    /// Entry point function name
    pub entry_point: String,
}

// ===== SHADER IMPLEMENTATION =====

impl Shader {
    /// Create shader from a pre-built GPU shader (internal use by ResourceManager)
    pub(crate) fn from_gpu_shader(
        graphics_device_shader: Arc<dyn graphics_device::Shader>,
        stage: ShaderStage,
    ) -> Self {
        Self { graphics_device_shader, stage }
    }

    /// Get the underlying graphics device shader
    pub fn graphics_device_shader(&self) -> &Arc<dyn graphics_device::Shader> {
        &self.graphics_device_shader
    }

    /// Get the shader stage
    pub fn stage(&self) -> ShaderStage {
        self.stage
    }
}

#[cfg(test)]
#[path = "shader_tests.rs"]
mod tests;
