//! Unit tests for Pipeline resource
//!
//! Tests Pipeline, PipelineVariant, and PipelinePass hierarchy without requiring GPU.
//! Uses MockGraphicsDevice for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::graphics_device;
#[cfg(test)]
use graphics_device::GraphicsDevice as _;
#[cfg(test)]
use crate::resource::{
    Pipeline, PipelineDesc, PipelineVariantDesc, PipelinePassDesc,
};

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
fn create_mock_render_pipeline_desc() -> graphics_device::PipelineDesc {
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
        depth_stencil: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
    }
}

/// Create a single-pass PipelineVariantDesc for convenience
fn create_single_pass_variant(name: &str) -> PipelineVariantDesc {
    PipelineVariantDesc {
        name: name.to_string(),
        passes: vec![
            PipelinePassDesc {
                pipeline: create_mock_render_pipeline_desc(),
            }
        ],
    }
}

/// Create a multi-pass PipelineVariantDesc for convenience
fn create_multi_pass_variant(name: &str, pass_count: usize) -> PipelineVariantDesc {
    let passes = (0..pass_count)
        .map(|_| PipelinePassDesc {
            pipeline: create_mock_render_pipeline_desc(),
        })
        .collect();

    PipelineVariantDesc {
        name: name.to_string(),
        passes,
    }
}

// ============================================================================
// PIPELINE CREATION TESTS
// ============================================================================

#[test]
fn test_create_pipeline_single_variant_single_pass() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("default")],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_count(), 1);
    assert!(pipeline.variant(0).is_some());
    assert_eq!(pipeline.variant(0).unwrap().name(), "default");
    assert_eq!(pipeline.variant(0).unwrap().pass_count(), 1);
}

#[test]
fn test_create_pipeline_multiple_variants() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("static"),
            create_single_pass_variant("animated"),
            create_single_pass_variant("transparent"),
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_count(), 3);
    assert_eq!(pipeline.variant(0).unwrap().name(), "static");
    assert_eq!(pipeline.variant(1).unwrap().name(), "animated");
    assert_eq!(pipeline.variant(2).unwrap().name(), "transparent");
}

#[test]
fn test_create_pipeline_multi_pass_variant() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_multi_pass_variant("toon_outline", 2)],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_count(), 1);
    let variant = pipeline.variant(0).unwrap();
    assert_eq!(variant.name(), "toon_outline");
    assert_eq!(variant.pass_count(), 2);
    assert!(variant.pass(0).is_some());
    assert!(variant.pass(1).is_some());
    assert!(variant.pass(2).is_none());
}

#[test]
fn test_create_pipeline_mixed_pass_counts() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("standard"),     // 1 pass
            create_multi_pass_variant("toon", 2),       // 2 passes
            create_multi_pass_variant("fur", 4),         // 4 passes
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant(0).unwrap().pass_count(), 1);
    assert_eq!(pipeline.variant(1).unwrap().pass_count(), 2);
    assert_eq!(pipeline.variant(2).unwrap().pass_count(), 4);
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn test_create_pipeline_duplicate_variant_names_fails() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("default"),
            create_single_pass_variant("default"), // DUPLICATE!
        ],
    };

    let result = Pipeline::from_desc(desc);

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("Duplicate variant name"));
    }
}

#[test]
fn test_create_pipeline_empty_passes_fails() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            PipelineVariantDesc {
                name: "empty".to_string(),
                passes: vec![], // No passes!
            }
        ],
    };

    let result = Pipeline::from_desc(desc);

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("at least one pass"));
    }
}

#[test]
fn test_add_variant_duplicate_name_fails() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("default")],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();

    let result = pipeline.add_variant(create_single_pass_variant("default"));

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("already exists"));
    }
}

#[test]
fn test_add_variant_empty_passes_fails() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("default")],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();

    let result = pipeline.add_variant(PipelineVariantDesc {
        name: "empty".to_string(),
        passes: vec![],
    });

    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.to_string().contains("at least one pass"));
    }
}

// ============================================================================
// VARIANT SELECTION TESTS
// ============================================================================

#[test]
fn test_variant_by_name_found() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("alpha"),
            create_single_pass_variant("beta"),
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant_by_name("beta");
    assert!(variant.is_some());
    assert_eq!(variant.unwrap().name(), "beta");
}

#[test]
fn test_variant_by_name_not_found() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("alpha")],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    let variant = pipeline.variant_by_name("nonexistent");
    assert!(variant.is_none());
}

#[test]
fn test_variant_by_index_found() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("first"),
            create_single_pass_variant("second"),
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
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("only")],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert!(pipeline.variant(0).is_some());
    assert!(pipeline.variant(1).is_none());
    assert!(pipeline.variant(999).is_none());
}

#[test]
fn test_variant_index_from_name() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("zero"),
            create_single_pass_variant("one"),
            create_single_pass_variant("two"),
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();

    assert_eq!(pipeline.variant_index("zero"), Some(0));
    assert_eq!(pipeline.variant_index("one"), Some(1));
    assert_eq!(pipeline.variant_index("two"), Some(2));
    assert_eq!(pipeline.variant_index("nonexistent"), None);
}

// ============================================================================
// PASS ACCESS TESTS
// ============================================================================

#[test]
fn test_pass_by_index() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_multi_pass_variant("toon", 3)],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();
    let variant = pipeline.variant(0).unwrap();

    assert_eq!(variant.pass_count(), 3);
    assert!(variant.pass(0).is_some());
    assert!(variant.pass(1).is_some());
    assert!(variant.pass(2).is_some());
    assert!(variant.pass(3).is_none());
}

#[test]
fn test_pass_graphics_device_pipeline_getter() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_multi_pass_variant("toon", 2)],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();
    let variant = pipeline.variant(0).unwrap();

    let pass_0 = variant.pass(0).unwrap();
    let pass_1 = variant.pass(1).unwrap();

    // Both passes should have valid graphics_device pipelines
    assert!(Arc::strong_count(pass_0.graphics_device_pipeline()) >= 1);
    assert!(Arc::strong_count(pass_1.graphics_device_pipeline()) >= 1);

    // Passes should have different graphics_device pipelines
    assert!(!Arc::ptr_eq(pass_0.graphics_device_pipeline(), pass_1.graphics_device_pipeline()));
}

// ============================================================================
// MAX PASS COUNT TESTS
// ============================================================================

#[test]
fn test_max_pass_count_single_variant() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_multi_pass_variant("toon", 3)],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.max_pass_count(), 3);
}

#[test]
fn test_max_pass_count_mixed_variants() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("standard"),     // 1 pass
            create_multi_pass_variant("toon", 2),       // 2 passes
            create_multi_pass_variant("fur", 4),        // 4 passes
        ],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.max_pass_count(), 4);
}

#[test]
fn test_max_pass_count_empty_pipeline() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![],
    };

    let pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.max_pass_count(), 0);
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_variant_names_case_sensitive() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![
            create_single_pass_variant("Default"),
            create_single_pass_variant("default"), // Different case
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
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("initial")],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.variant_count(), 1);

    let new_variant_idx = pipeline.add_variant(
        create_single_pass_variant("added")
    ).unwrap();

    assert_eq!(pipeline.variant_count(), 2);
    assert_eq!(new_variant_idx, 1);
    assert!(pipeline.variant_by_name("added").is_some());
}

#[test]
fn test_add_multi_pass_variant() {
    let graphics_device = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));

    let desc = PipelineDesc {
        graphics_device: graphics_device.clone(),
        variants: vec![create_single_pass_variant("standard")],
    };

    let mut pipeline = Pipeline::from_desc(desc).unwrap();
    assert_eq!(pipeline.max_pass_count(), 1);

    pipeline.add_variant(create_multi_pass_variant("toon", 3)).unwrap();

    assert_eq!(pipeline.variant_count(), 2);
    assert_eq!(pipeline.max_pass_count(), 3);
    assert_eq!(pipeline.variant_by_name("toon").unwrap().pass_count(), 3);
}
