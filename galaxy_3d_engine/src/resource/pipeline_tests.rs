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

/// Create a mock graphics_device::PipelineDesc for testing
fn create_mock_pipeline_desc() -> graphics_device::PipelineDesc {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let mut graphics_device_lock = graphics_device.lock().unwrap();

    let vertex_shader = graphics_device_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    let fragment_shader = graphics_device_lock.create_shader(graphics_device::ShaderDesc {
        stage: graphics_device::ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    drop(graphics_device_lock);

    graphics_device::PipelineDesc {
        vertex_shader,
        fragment_shader,
        vertex_layout: create_simple_vertex_layout(),
        topology: graphics_device::PrimitiveTopology::TriangleList,
        push_constant_ranges: vec![],
        binding_group_layouts: vec![],
        rasterization: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
        color_formats: vec![],
        depth_format: None,
    }
}

/// Create a Pipeline via MockGraphicsDevice
fn create_test_pipeline() -> crate::resource::Pipeline {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let gd_pipeline = graphics_device.lock().unwrap()
        .create_pipeline(create_mock_pipeline_desc()).unwrap();
    crate::resource::Pipeline::from_gpu_pipeline(gd_pipeline)
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
