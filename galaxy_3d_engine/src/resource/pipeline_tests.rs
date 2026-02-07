//! Unit tests for Pipeline resource
//!
//! Tests Pipeline and PipelineVariant hierarchy without requiring GPU.
//! Uses MockRenderer for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::renderer::{
    Renderer, mock_renderer::MockRenderer,
    PipelineDesc as RenderPipelineDesc, VertexLayout, VertexBinding, VertexAttribute,
    BufferFormat, VertexInputRate, PrimitiveTopology,
};
#[cfg(test)]
use crate::resource::{
    Pipeline, PipelineDesc, PipelineVariantDesc,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a simple vertex layout for testing
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

/// Create a mock RenderPipelineDesc for testing
fn create_mock_render_pipeline_desc() -> RenderPipelineDesc {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));
    let mut renderer_lock = renderer.lock().unwrap();

    let vertex_shader = renderer_lock.create_shader(crate::renderer::ShaderDesc {
        stage: crate::renderer::ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    let fragment_shader = renderer_lock.create_shader(crate::renderer::ShaderDesc {
        stage: crate::renderer::ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &[],
    }).unwrap();

    drop(renderer_lock);

    RenderPipelineDesc {
        vertex_shader,
        fragment_shader,
        vertex_layout: create_simple_vertex_layout(),
        topology: PrimitiveTopology::TriangleList,
        push_constant_ranges: vec![],
        descriptor_set_layouts: vec![],
        enable_blending: false,
    }
}

// ============================================================================
// PIPELINE CREATION TESTS
// ============================================================================

#[test]
fn test_create_pipeline_single_variant() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_count(), 1);
    assert!(pipeline.variant(0).is_some());
    assert_eq!(pipeline.variant(0).unwrap().name(), "default");
}

#[test]
fn test_create_pipeline_multiple_variants() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "static".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "animated".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "transparent".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_count(), 3);
    assert_eq!(pipeline.variant(0).unwrap().name(), "static");
    assert_eq!(pipeline.variant(1).unwrap().name(), "animated");
    assert_eq!(pipeline.variant(2).unwrap().name(), "transparent");
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn test_create_pipeline_duplicate_variant_names_fails() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "default".to_string(), // DUPLICATE!
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    let result = Pipeline::from_desc(desc);

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Duplicate variant name"));
    }
}

#[test]
fn test_add_variant_duplicate_name_fails() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();

    // Try to add variant with duplicate name
    let result = pipeline.add_variant(PipelineVariantDesc {
        name: "default".to_string(),
        pipeline: create_mock_render_pipeline_desc(),
    });

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("already exists"));
    }
}

// ============================================================================
// VARIANT SELECTION TESTS
// ============================================================================

#[test]
fn test_variant_by_name_found() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "alpha".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "beta".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant_by_name("beta");
    assert!(variant.is_some());
    assert_eq!(variant.unwrap().name(), "beta");
}

#[test]
fn test_variant_by_name_not_found() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "alpha".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant_by_name("nonexistent");
    assert!(variant.is_none());
}

#[test]
fn test_variant_by_index_found() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "first".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "second".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert!(pipeline.variant(0).is_some());
    assert!(pipeline.variant(1).is_some());
    assert_eq!(pipeline.variant(0).unwrap().name(), "first");
    assert_eq!(pipeline.variant(1).unwrap().name(), "second");
}

#[test]
fn test_variant_by_index_out_of_bounds() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "only".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert!(pipeline.variant(0).is_some());
    assert!(pipeline.variant(1).is_none());
    assert!(pipeline.variant(999).is_none());
}

#[test]
fn test_variant_index_from_name() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "zero".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "one".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "two".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_index("zero"), Some(0));
    assert_eq!(pipeline.variant_index("one"), Some(1));
    assert_eq!(pipeline.variant_index("two"), Some(2));
    assert_eq!(pipeline.variant_index("nonexistent"), None);
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_variant_names_case_sensitive() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "Default".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            },
            PipelineVariantDesc {
                name: "default".to_string(), // Different case
                pipeline: create_mock_render_pipeline_desc(),
            },
        ],
    };

    // Should succeed - case sensitive
    let pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.variant_count(), 2);
    assert!(pipeline.variant_by_name("Default").is_some());
    assert!(pipeline.variant_by_name("default").is_some());
}

#[test]
fn test_add_variant_increases_count() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "initial".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.variant_count(), 1);

    let new_variant_idx = pipeline.add_variant(PipelineVariantDesc {
        name: "added".to_string(),
        pipeline: create_mock_render_pipeline_desc(),
    }).unwrap();

    assert_eq!(pipeline.variant_count(), 2);
    assert_eq!(new_variant_idx, 1);
    assert!(pipeline.variant_by_name("added").is_some());
}

// ============================================================================
// VARIANT GETTER TESTS
// ============================================================================

#[test]
fn test_variant_renderer_pipeline_getter() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant(0).unwrap();

    // Test renderer_pipeline() getter
    let renderer_pipeline = variant.renderer_pipeline();
    assert!(Arc::strong_count(renderer_pipeline) >= 1);
}

#[test]
fn test_variant_name_getter() {
    let renderer = Arc::new(Mutex::new(MockRenderer::new()));

    let desc = PipelineDesc {
        renderer: renderer.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "test_variant".to_string(),
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant(0).unwrap();

    // Test name() getter (already tested but verify explicitly)
    assert_eq!(variant.name(), "test_variant");
}
