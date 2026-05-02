//! Unit tests for Pipeline resource
//!
//! Tests the simplified Pipeline wrapper around graphics_device::Pipeline.
//! Uses MockGraphicsDevice for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::graphics_device;
#[cfg(test)]
use graphics_device::GraphicsDevice as _;
#[cfg(test)]
use crate::resource::resource_manager::ResourceManager;
#[cfg(test)]
use crate::resource::shader::ShaderDesc;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a simple vertex layout for testing
fn create_simple_vertex_layout() -> graphics_device::VertexLayout {
    graphics_device::VertexLayout {
        bindings: vec![
            graphics_device::VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: graphics_device::VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            graphics_device::VertexAttribute {
                location: 0,
                binding: 0,
                format: graphics_device::BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    }
}

/// Create a Pipeline via MockGraphicsDevice + ResourceManager
fn create_test_pipeline() -> crate::resource::Pipeline {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let mut rm = ResourceManager::new();
    let mut gd_lock = graphics_device.lock().unwrap();

    let vk = rm.create_shader("vert".to_string(), ShaderDesc { code: &[], stage: graphics_device::ShaderStage::Vertex, entry_point: "main".to_string() }, &mut *gd_lock).unwrap();
    let fk = rm.create_shader("frag".to_string(), ShaderDesc { code: &[], stage: graphics_device::ShaderStage::Fragment, entry_point: "main".to_string() }, &mut *gd_lock).unwrap();

    let vertex_shader = gd_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    let fragment_shader = gd_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    let desc = graphics_device::PipelineDesc {
        vertex_layout: create_simple_vertex_layout(),
        topology: graphics_device::PrimitiveTopology::TriangleList,
        rasterization: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
        color_formats: vec![],
        depth_format: None,
    };

    let gd_pipeline = gd_lock.create_pipeline(desc, &vertex_shader, &fragment_shader).unwrap();
    crate::resource::Pipeline::from_gpu_pipeline(gd_pipeline, vk, fk, 0, 0)
}

// ============================================================================
// PIPELINE CREATION TESTS
// ============================================================================

#[test]
fn test_create_pipeline() {
    let pipeline = create_test_pipeline();
    assert!(Arc::strong_count(pipeline.graphics_device_pipeline()) >= 1);
}

#[test]
fn test_pipeline_reflection() {
    let pipeline = create_test_pipeline();
    let reflection = pipeline.reflection();
    // Mock pipelines have empty reflection
    assert_eq!(reflection.binding_count(), 0);
}

#[test]
fn test_pipeline_binding_group_layout_count() {
    let pipeline = create_test_pipeline();
    // Mock pipelines report 0 binding group layouts
    assert_eq!(pipeline.binding_group_layout_count(), 0);
}

#[test]
fn test_pipeline_graphics_device_pipeline_accessor() {
    let pipeline = create_test_pipeline();
    let gd_pipeline = pipeline.graphics_device_pipeline();
    // Should be a valid Arc
    assert!(Arc::strong_count(gd_pipeline) >= 1);
}

#[test]
fn test_pipeline_signature_id_accessor() {
    let pipeline = create_test_pipeline();
    // create_test_pipeline assigns signature_id = 0
    assert_eq!(pipeline.signature_id(), 0);
}

#[test]
fn test_pipeline_sort_id_accessor() {
    let pipeline = create_test_pipeline();
    // create_test_pipeline assigns sort_id = 0
    assert_eq!(pipeline.sort_id(), 0);
}

#[test]
fn test_pipeline_vertex_shader_key() {
    let pipeline = create_test_pipeline();
    let _key = pipeline.vertex_shader();
    // ShaderKey is opaque (slotmap key); we just verify the call returns Copy.
}

#[test]
fn test_pipeline_fragment_shader_key() {
    let pipeline = create_test_pipeline();
    let _key = pipeline.fragment_shader();
}

#[test]
fn test_pipeline_from_gpu_pipeline_with_explicit_ids() {
    use crate::resource::resource_manager::ShaderKey;
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let mut gd_lock = graphics_device.lock().unwrap();
    let vs = gd_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();
    let fs = gd_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();
    let desc = graphics_device::PipelineDesc {
        vertex_layout: create_simple_vertex_layout(),
        topology: graphics_device::PrimitiveTopology::TriangleList,
        rasterization: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
        color_formats: vec![],
        depth_format: None,
    };
    let gd_pipeline = gd_lock.create_pipeline(desc, &vs, &fs).unwrap();
    let pipeline = crate::resource::Pipeline::from_gpu_pipeline(
        gd_pipeline,
        ShaderKey::default(),
        ShaderKey::default(),
        42,
        99,
    );
    assert_eq!(pipeline.signature_id(), 42);
    assert_eq!(pipeline.sort_id(), 99);
}
