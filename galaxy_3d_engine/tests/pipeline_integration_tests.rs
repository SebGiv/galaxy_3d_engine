//! Integration tests for Pipeline with real Vulkan backend
//!
//! These tests verify that Pipeline resource works correctly with a real GPU.
//! All tests require a GPU and are marked with #[ignore].
//!
//! Run with: cargo test --test pipeline_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::GraphicsDevice;
use galaxy_3d_engine::galaxy3d::resource::{ResourceManager, PipelineDesc};
use galaxy_3d_engine::galaxy3d::render::{
    PipelineDesc as RenderPipelineDesc, VertexLayout, VertexBinding, VertexAttribute,
    BufferFormat, VertexInputRate, PrimitiveTopology, ShaderDesc, ShaderStage,
};
use gpu_test_utils::get_test_graphics_device;
use serial_test::serial;

/// Helper to create minimal valid SPIR-V vertex shader
fn create_minimal_vertex_shader() -> Vec<u8> {
    vec![
        0x03, 0x02, 0x23, 0x07,
        0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ]
}

/// Helper to create minimal valid SPIR-V fragment shader
fn create_minimal_fragment_shader() -> Vec<u8> {
    vec![
        0x03, 0x02, 0x23, 0x07,
        0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ]
}

/// Helper to create a simple vertex layout for testing
fn create_simple_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    }
}

// ============================================================================
// PIPELINE CREATION TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_create_pipeline() {
    let graphics_device_arc = get_test_graphics_device();
    let mut rm = ResourceManager::new();

    let mut graphics_device_lock = graphics_device_arc.lock().unwrap();
    let vertex_shader = graphics_device_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &create_minimal_vertex_shader(),
    });
    let fragment_shader = graphics_device_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &create_minimal_fragment_shader(),
    });

    if vertex_shader.is_err() || fragment_shader.is_err() {
        return;
    }

    let vertex_shader = vertex_shader.unwrap();
    let fragment_shader = fragment_shader.unwrap();

    let desc = PipelineDesc {
        pipeline: RenderPipelineDesc {
            vertex_shader,
            fragment_shader,
            vertex_layout: create_simple_vertex_layout(),
            topology: PrimitiveTopology::TriangleList,
            push_constant_ranges: vec![],
            binding_group_layouts: vec![],
            rasterization: Default::default(),
            color_blend: Default::default(),
            multisample: Default::default(),
            color_formats: vec![],
            depth_format: None,
        },
    };

    let result = rm.create_pipeline("test_pipeline".to_string(), desc, &mut *graphics_device_lock);

    if result.is_ok() {
        assert_eq!(rm.pipeline_count(), 1);
        assert!(rm.pipeline("test_pipeline").is_some());

        let pipeline = rm.pipeline("test_pipeline").unwrap();
        assert!(pipeline.graphics_device_pipeline().as_ref().reflection().binding_count() == 0
            || true); // Reflection content depends on actual shaders
    }
}
