/// Tests for Shader resource

use super::*;
use crate::graphics_device::mock_graphics_device::MockShader;

#[test]
fn test_from_gpu_shader_vertex() {
    let mock = Arc::new(MockShader::new("vert".to_string()));
    let shader = Shader::from_gpu_shader(mock.clone(), ShaderStage::Vertex);

    assert_eq!(shader.stage(), ShaderStage::Vertex);
    assert!(Arc::ptr_eq(shader.graphics_device_shader(), &(mock as Arc<dyn graphics_device::Shader>)));
}

#[test]
fn test_from_gpu_shader_fragment() {
    let mock = Arc::new(MockShader::new("frag".to_string()));
    let shader = Shader::from_gpu_shader(mock.clone(), ShaderStage::Fragment);

    assert_eq!(shader.stage(), ShaderStage::Fragment);
    assert!(Arc::ptr_eq(shader.graphics_device_shader(), &(mock as Arc<dyn graphics_device::Shader>)));
}

#[test]
fn test_from_gpu_shader_compute() {
    let mock = Arc::new(MockShader::new("comp".to_string()));
    let shader = Shader::from_gpu_shader(mock.clone(), ShaderStage::Compute);

    assert_eq!(shader.stage(), ShaderStage::Compute);
    assert!(Arc::ptr_eq(shader.graphics_device_shader(), &(mock as Arc<dyn graphics_device::Shader>)));
}
