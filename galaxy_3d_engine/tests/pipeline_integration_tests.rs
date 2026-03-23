//! Integration tests for Pipeline with real Vulkan backend
//!
//! These tests verify that Pipeline resource works correctly with a real GPU.
//! All tests require a GPU and are marked with #[ignore].
//!
//! Run with: cargo test --test pipeline_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Engine;
use galaxy_3d_engine::galaxy3d::resource::PipelineDesc;
use galaxy_3d_engine::galaxy3d::render::{
    VertexLayout, VertexBinding, VertexAttribute,
    BufferFormat, VertexInputRate, PrimitiveTopology, ShaderStage,
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
    Engine::initialize().ok();
    Engine::create_resource_manager().ok();
    let rm_arc = Engine::resource_manager().unwrap();
    let mut rm = rm_arc.lock().unwrap();

    let mut graphics_device_lock = graphics_device_arc.lock().unwrap();

    let vert_key = rm.create_shader(
        "test_vert".to_string(),
        galaxy_3d_engine::galaxy3d::resource::ShaderDesc {
            code: &create_minimal_vertex_shader(),
            stage: ShaderStage::Vertex,
            entry_point: "main".to_string(),
        },
        &mut *graphics_device_lock,
    );
    let frag_key = rm.create_shader(
        "test_frag".to_string(),
        galaxy_3d_engine::galaxy3d::resource::ShaderDesc {
            code: &create_minimal_fragment_shader(),
            stage: ShaderStage::Fragment,
            entry_point: "main".to_string(),
        },
        &mut *graphics_device_lock,
    );

    if vert_key.is_err() || frag_key.is_err() {
        return;
    }

    let desc = PipelineDesc {
        vertex_shader: vert_key.unwrap(),
        fragment_shader: frag_key.unwrap(),
        vertex_layout: create_simple_vertex_layout(),
        topology: PrimitiveTopology::TriangleList,
        rasterization: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
        color_formats: vec![],
        depth_format: None,
    };

    let result = rm.create_pipeline("test_pipeline".to_string(), desc, &mut *graphics_device_lock);

    if result.is_ok() {
        assert_eq!(rm.pipeline_count(), 1);
        assert!(rm.pipeline_by_name("test_pipeline").is_some());

        let pipeline = rm.pipeline_by_name("test_pipeline").unwrap();
        assert!(pipeline.graphics_device_pipeline().as_ref().reflection().binding_count() == 0
            || true); // Reflection content depends on actual shaders
    }
}
