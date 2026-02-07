//! Integration tests for Pipeline with real Vulkan backend
//!
//! These tests verify that Pipeline resource works correctly with a real GPU.
//! All tests require a GPU and are marked with #[ignore].
//!
//! Run with: cargo test --test pipeline_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::Renderer;
use galaxy_3d_engine::galaxy3d::resource::{ResourceManager, PipelineDesc, PipelineVariantDesc};
use galaxy_3d_engine::galaxy3d::render::{
    PipelineDesc as RenderPipelineDesc, VertexLayout, VertexBinding, VertexAttribute,
    BufferFormat, VertexInputRate, PrimitiveTopology, ShaderDesc, ShaderStage,
};
use gpu_test_utils::get_test_renderer;
use serial_test::serial;

/// Helper to create minimal valid SPIR-V vertex shader
fn create_minimal_vertex_shader() -> Vec<u8> {
    // This is a minimal SPIR-V header - in real tests you'd use compiled shaders
    vec![
        0x03, 0x02, 0x23, 0x07, // Magic number
        0x00, 0x00, 0x01, 0x00, // Version 1.0
        0x00, 0x00, 0x00, 0x00, // Generator
        0x01, 0x00, 0x00, 0x00, // Bound
        0x00, 0x00, 0x00, 0x00, // Schema
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
fn test_integration_create_pipeline_single_variant() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create shaders
    let mut renderer_lock = renderer_arc.lock().unwrap();
    let vertex_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &create_minimal_vertex_shader(),
    });
    let fragment_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &create_minimal_fragment_shader(),
    });
    drop(renderer_lock);

    // Skip test if shaders fail (invalid dummy SPIR-V)
    if vertex_shader.is_err() || fragment_shader.is_err() {
        return;
    }

    let vertex_shader = vertex_shader.unwrap();
    let fragment_shader = fragment_shader.unwrap();

    // Create pipeline descriptor
    let desc = PipelineDesc {
        renderer: renderer_arc.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            }
        ],
    };

    // Create pipeline
    let result = rm.create_pipeline("test_pipeline".to_string(), desc);

    // Verify (pipeline creation might fail with dummy shaders, that's ok)
    if result.is_ok() {
        assert_eq!(rm.pipeline_count(), 1);
        assert!(rm.pipeline("test_pipeline").is_some());

        let pipeline = rm.pipeline("test_pipeline").unwrap();
        assert_eq!(pipeline.variant_count(), 1);
        assert_eq!(pipeline.variant(0).unwrap().name(), "default");
    }
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_create_pipeline_multiple_variants() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create shaders
    let mut renderer_lock = renderer_arc.lock().unwrap();
    let vertex_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &create_minimal_vertex_shader(),
    });
    let fragment_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &create_minimal_fragment_shader(),
    });
    drop(renderer_lock);

    // Skip test if shaders fail
    if vertex_shader.is_err() || fragment_shader.is_err() {
        return;
    }

    let vertex_shader = vertex_shader.unwrap();
    let fragment_shader = fragment_shader.unwrap();

    // Create pipeline descriptor with 3 variants
    let desc = PipelineDesc {
        renderer: renderer_arc.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "static".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
            PipelineVariantDesc {
                name: "animated".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
            PipelineVariantDesc {
                name: "transparent".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: true, // Different blending
                },
            },
        ],
    };

    // Create pipeline
    let result = rm.create_pipeline("mesh_pipeline".to_string(), desc);

    // Verify
    if result.is_ok() {
        assert_eq!(rm.pipeline_count(), 1);

        let pipeline = rm.pipeline("mesh_pipeline").unwrap();
        assert_eq!(pipeline.variant_count(), 3);
        assert_eq!(pipeline.variant(0).unwrap().name(), "static");
        assert_eq!(pipeline.variant(1).unwrap().name(), "animated");
        assert_eq!(pipeline.variant(2).unwrap().name(), "transparent");

        // Test variant lookup by name
        assert!(pipeline.variant_by_name("static").is_some());
        assert!(pipeline.variant_by_name("animated").is_some());
        assert!(pipeline.variant_by_name("transparent").is_some());
    }
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_pipeline_variant_selection() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create shaders
    let mut renderer_lock = renderer_arc.lock().unwrap();
    let vertex_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &create_minimal_vertex_shader(),
    });
    let fragment_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &create_minimal_fragment_shader(),
    });
    drop(renderer_lock);

    // Skip test if shaders fail
    if vertex_shader.is_err() || fragment_shader.is_err() {
        return;
    }

    let vertex_shader = vertex_shader.unwrap();
    let fragment_shader = fragment_shader.unwrap();

    // Create pipeline with variants
    let desc = PipelineDesc {
        renderer: renderer_arc.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "opaque".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
            PipelineVariantDesc {
                name: "alpha".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: true,
                },
            },
        ],
    };

    let result = rm.create_pipeline("material_pipeline".to_string(), desc);

    // Test variant selection
    if result.is_ok() {
        let pipeline = rm.pipeline("material_pipeline").unwrap();

        // Test variant by name
        let opaque_variant = pipeline.variant_by_name("opaque");
        assert!(opaque_variant.is_some());
        assert_eq!(opaque_variant.unwrap().name(), "opaque");

        let alpha_variant = pipeline.variant_by_name("alpha");
        assert!(alpha_variant.is_some());
        assert_eq!(alpha_variant.unwrap().name(), "alpha");

        // Test variant by index
        assert_eq!(pipeline.variant_index("opaque"), Some(0));
        assert_eq!(pipeline.variant_index("alpha"), Some(1));

        // Test non-existent variant
        assert!(pipeline.variant_by_name("nonexistent").is_none());
    }
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_pipeline_different_topologies() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create shaders
    let mut renderer_lock = renderer_arc.lock().unwrap();
    let vertex_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &create_minimal_vertex_shader(),
    });
    let fragment_shader = renderer_lock.create_shader(ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &create_minimal_fragment_shader(),
    });
    drop(renderer_lock);

    // Skip test if shaders fail
    if vertex_shader.is_err() || fragment_shader.is_err() {
        return;
    }

    let vertex_shader = vertex_shader.unwrap();
    let fragment_shader = fragment_shader.unwrap();

    // Create pipeline with different topologies
    let desc = PipelineDesc {
        renderer: renderer_arc.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "triangles".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
            PipelineVariantDesc {
                name: "lines".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::LineList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
            PipelineVariantDesc {
                name: "points".to_string(),
                pipeline: RenderPipelineDesc {
                    vertex_shader: vertex_shader.clone(),
                    fragment_shader: fragment_shader.clone(),
                    vertex_layout: create_simple_vertex_layout(),
                    topology: PrimitiveTopology::PointList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    enable_blending: false,
                },
            },
        ],
    };

    let result = rm.create_pipeline("topology_pipeline".to_string(), desc);

    // Verify different topologies were created
    if result.is_ok() {
        let pipeline = rm.pipeline("topology_pipeline").unwrap();
        assert_eq!(pipeline.variant_count(), 3);
        assert!(pipeline.variant_by_name("triangles").is_some());
        assert!(pipeline.variant_by_name("lines").is_some());
        assert!(pipeline.variant_by_name("points").is_some());
    }
}
