/// Shader - Vulkan implementation of RendererShader trait

use galaxy_3d_engine::galaxy3d::render::{
    Shader as RendererShader,
    ReflectedBinding,
    ReflectedPushConstant,
};
use ash::vk;

/// Vulkan shader implementation
pub struct Shader {
    /// Vulkan shader module
    pub(crate) module: vk::ShaderModule,
    /// Shader stage flags
    pub(crate) stage: vk::ShaderStageFlags,
    /// Entry point name
    pub(crate) entry_point: String,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
    /// SPIR-V reflected bindings (parsed at shader creation, used at pipeline creation)
    pub(crate) reflected_bindings: Vec<ReflectedBinding>,
    /// SPIR-V reflected push constants (parsed at shader creation, used at pipeline creation)
    pub(crate) reflected_push_constants: Vec<ReflectedPushConstant>,
}

impl RendererShader for Shader {
    // No public methods
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            // Destroy shader module
            self.device.destroy_shader_module(self.module, None);
        }
    }
}
