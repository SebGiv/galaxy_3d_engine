//! Unit tests for Pipeline module
//!
//! Tests IndexType, PrimitiveTopology, VertexInputRate, and VertexLayout types.

use crate::graphics_device::{
    IndexType, PrimitiveTopology, VertexInputRate, VertexLayout,
    VertexBinding, VertexAttribute, BufferFormat,
};

// ============================================================================
// INDEX TYPE TESTS
// ============================================================================

#[test]
fn test_index_type_size_bytes() {
    assert_eq!(IndexType::U16.size_bytes(), 2);
    assert_eq!(IndexType::U32.size_bytes(), 4);
}

#[test]
fn test_index_type_size_calculations() {
    // Example: 1000 indices with U16
    let index_count = 1000u32;
    let index_type = IndexType::U16;
    let buffer_size = index_count * index_type.size_bytes();
    assert_eq!(buffer_size, 2000); // 1000 * 2 bytes

    // Example: 1000 indices with U32
    let index_type = IndexType::U32;
    let buffer_size = index_count * index_type.size_bytes();
    assert_eq!(buffer_size, 4000); // 1000 * 4 bytes
}

#[test]
fn test_index_type_equality() {
    assert_eq!(IndexType::U16, IndexType::U16);
    assert_eq!(IndexType::U32, IndexType::U32);
    assert_ne!(IndexType::U16, IndexType::U32);
}

#[test]
fn test_index_type_debug() {
    let u16_debug = format!("{:?}", IndexType::U16);
    assert!(u16_debug.contains("U16"));

    let u32_debug = format!("{:?}", IndexType::U32);
    assert!(u32_debug.contains("U32"));
}

#[test]
fn test_index_type_clone() {
    let idx1 = IndexType::U16;
    let idx2 = idx1.clone();
    assert_eq!(idx1, idx2);

    let idx3 = IndexType::U32;
    let idx4 = idx3.clone();
    assert_eq!(idx3, idx4);
}

#[test]
fn test_index_type_copy() {
    let idx1 = IndexType::U16;
    let idx2 = idx1; // Copy, not move
    assert_eq!(idx1, idx2);
    // Can still use idx1
    assert_eq!(idx1.size_bytes(), 2);
}

// ============================================================================
// PRIMITIVE TOPOLOGY TESTS
// ============================================================================

#[test]
fn test_primitive_topology_equality() {
    assert_eq!(PrimitiveTopology::TriangleList, PrimitiveTopology::TriangleList);
    assert_eq!(PrimitiveTopology::TriangleStrip, PrimitiveTopology::TriangleStrip);
    assert_eq!(PrimitiveTopology::LineList, PrimitiveTopology::LineList);
    assert_eq!(PrimitiveTopology::PointList, PrimitiveTopology::PointList);

    assert_ne!(PrimitiveTopology::TriangleList, PrimitiveTopology::LineList);
}

#[test]
fn test_primitive_topology_debug() {
    assert!(format!("{:?}", PrimitiveTopology::TriangleList).contains("TriangleList"));
    assert!(format!("{:?}", PrimitiveTopology::TriangleStrip).contains("TriangleStrip"));
    assert!(format!("{:?}", PrimitiveTopology::LineList).contains("LineList"));
    assert!(format!("{:?}", PrimitiveTopology::PointList).contains("PointList"));
}

#[test]
fn test_primitive_topology_clone() {
    let topo1 = PrimitiveTopology::TriangleList;
    let topo2 = topo1.clone();
    assert_eq!(topo1, topo2);
}

#[test]
fn test_primitive_topology_copy() {
    let topo1 = PrimitiveTopology::LineList;
    let topo2 = topo1; // Copy, not move
    assert_eq!(topo1, topo2);
}

// ============================================================================
// VERTEX INPUT RATE TESTS
// ============================================================================

#[test]
fn test_vertex_input_rate_equality() {
    assert_eq!(VertexInputRate::Vertex, VertexInputRate::Vertex);
    assert_eq!(VertexInputRate::Instance, VertexInputRate::Instance);
    assert_ne!(VertexInputRate::Vertex, VertexInputRate::Instance);
}

#[test]
fn test_vertex_input_rate_debug() {
    assert!(format!("{:?}", VertexInputRate::Vertex).contains("Vertex"));
    assert!(format!("{:?}", VertexInputRate::Instance).contains("Instance"));
}

#[test]
fn test_vertex_input_rate_clone() {
    let rate1 = VertexInputRate::Vertex;
    let rate2 = rate1.clone();
    assert_eq!(rate1, rate2);
}

#[test]
fn test_vertex_input_rate_copy() {
    let rate1 = VertexInputRate::Instance;
    let rate2 = rate1; // Copy, not move
    assert_eq!(rate1, rate2);
}

// ============================================================================
// VERTEX LAYOUT TESTS
// ============================================================================

#[test]
fn test_vertex_layout_default() {
    let layout = VertexLayout::default();
    assert_eq!(layout.bindings.len(), 0);
    assert_eq!(layout.attributes.len(), 0);
}

#[test]
fn test_vertex_layout_creation() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12, // 3 floats (x, y, z)
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32B32_SFLOAT, // vec3
                offset: 0,
            }
        ],
    };

    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.attributes.len(), 1);
    assert_eq!(layout.bindings[0].stride, 12);
    assert_eq!(layout.attributes[0].format, BufferFormat::R32G32B32_SFLOAT);
}

#[test]
fn test_vertex_layout_multiple_bindings() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12, // Position: vec3
                input_rate: VertexInputRate::Vertex,
            },
            VertexBinding {
                binding: 1,
                stride: 8, // UV: vec2
                input_rate: VertexInputRate::Vertex,
            },
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32B32_SFLOAT,
                offset: 0,
            },
            VertexAttribute {
                location: 1,
                binding: 1,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
        ],
    };

    assert_eq!(layout.bindings.len(), 2);
    assert_eq!(layout.attributes.len(), 2);
}

#[test]
fn test_vertex_layout_clone() {
    let layout1 = VertexLayout {
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
    };

    let layout2 = layout1.clone();
    assert_eq!(layout1.bindings.len(), layout2.bindings.len());
    assert_eq!(layout1.attributes.len(), layout2.attributes.len());
}

#[test]
fn test_vertex_layout_debug() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![],
    };

    let debug_str = format!("{:?}", layout);
    assert!(debug_str.contains("bindings"));
    assert!(debug_str.contains("attributes"));
}

// ============================================================================
// VERTEX BINDING TESTS
// ============================================================================

#[test]
fn test_vertex_binding_creation() {
    let binding = VertexBinding {
        binding: 0,
        stride: 20, // 5 floats
        input_rate: VertexInputRate::Vertex,
    };

    assert_eq!(binding.binding, 0);
    assert_eq!(binding.stride, 20);
    assert_eq!(binding.input_rate, VertexInputRate::Vertex);
}

#[test]
fn test_vertex_binding_debug() {
    let binding = VertexBinding {
        binding: 1,
        stride: 32,
        input_rate: VertexInputRate::Instance,
    };

    let debug_str = format!("{:?}", binding);
    assert!(debug_str.contains("binding"));
    assert!(debug_str.contains("stride"));
}

#[test]
fn test_vertex_binding_clone() {
    let binding1 = VertexBinding {
        binding: 0,
        stride: 12,
        input_rate: VertexInputRate::Vertex,
    };

    let binding2 = binding1.clone();
    assert_eq!(binding1.binding, binding2.binding);
    assert_eq!(binding1.stride, binding2.stride);
}

#[test]
fn test_vertex_binding_copy() {
    let binding1 = VertexBinding {
        binding: 0,
        stride: 8,
        input_rate: VertexInputRate::Vertex,
    };

    let binding2 = binding1; // Copy
    assert_eq!(binding1.stride, binding2.stride);
    assert_eq!(binding1.stride, 8); // Can still use binding1
}

// ============================================================================
// VERTEX ATTRIBUTE TESTS
// ============================================================================

#[test]
fn test_vertex_attribute_creation() {
    let attr = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32G32B32_SFLOAT,
        offset: 0,
    };

    assert_eq!(attr.location, 0);
    assert_eq!(attr.binding, 0);
    assert_eq!(attr.format, BufferFormat::R32G32B32_SFLOAT);
    assert_eq!(attr.offset, 0);
}

#[test]
fn test_vertex_attribute_with_offset() {
    let attr = VertexAttribute {
        location: 1,
        binding: 0,
        format: BufferFormat::R32G32_SFLOAT,
        offset: 12, // After a vec3 (12 bytes)
    };

    assert_eq!(attr.offset, 12);
}

#[test]
fn test_vertex_attribute_debug() {
    let attr = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32_SFLOAT,
        offset: 0,
    };

    let debug_str = format!("{:?}", attr);
    assert!(debug_str.contains("location"));
    assert!(debug_str.contains("format"));
}

#[test]
fn test_vertex_attribute_clone() {
    let attr1 = VertexAttribute {
        location: 2,
        binding: 1,
        format: BufferFormat::R32G32B32A32_SFLOAT,
        offset: 4,
    };

    let attr2 = attr1.clone();
    assert_eq!(attr1.location, attr2.location);
    assert_eq!(attr1.format, attr2.format);
}

#[test]
fn test_vertex_attribute_copy() {
    let attr1 = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32G32_SFLOAT,
        offset: 0,
    };

    let attr2 = attr1; // Copy
    assert_eq!(attr1.location, attr2.location);
    assert_eq!(attr1.location, 0); // Can still use attr1
}

// ============================================================================
// Default impls / value type tests
// ============================================================================

mod defaults_and_keys {
    use crate::graphics_device::{
        ColorWriteMask, RasterizationState, ColorBlendState, MultisampleState,
        StencilOpState, DynamicRenderState, DynamicRenderStateKey,
        PolygonMode, CullMode, FrontFace, CompareOp, StencilOp, BlendFactor, BlendOp,
        SampleCount, PipelineReflection, PipelineSignatureKey,
        ReflectedBinding, ReflectedPushConstant, BindingType, ShaderStageFlags,
    };

    #[test]
    fn test_color_write_mask_all_constant() {
        assert!(ColorWriteMask::ALL.r);
        assert!(ColorWriteMask::ALL.g);
        assert!(ColorWriteMask::ALL.b);
        assert!(ColorWriteMask::ALL.a);
    }

    #[test]
    fn test_color_write_mask_none_constant() {
        assert!(!ColorWriteMask::NONE.r);
        assert!(!ColorWriteMask::NONE.g);
        assert!(!ColorWriteMask::NONE.b);
        assert!(!ColorWriteMask::NONE.a);
    }

    #[test]
    fn test_color_write_mask_default_is_all() {
        assert_eq!(ColorWriteMask::default(), ColorWriteMask::ALL);
    }

    #[test]
    fn test_rasterization_state_default() {
        let r = RasterizationState::default();
        assert_eq!(r.polygon_mode, PolygonMode::Fill);
        assert!(!r.depth_clamp_enable);
        assert!(r.depth_clip_enable);
    }

    #[test]
    fn test_color_blend_state_default() {
        let c = ColorBlendState::default();
        assert!(!c.blend_enable);
        assert_eq!(c.src_color_factor, BlendFactor::One);
        assert_eq!(c.dst_color_factor, BlendFactor::Zero);
        assert_eq!(c.color_blend_op, BlendOp::Add);
        assert!(c.color_write_enable);
    }

    #[test]
    fn test_multisample_state_default() {
        let m = MultisampleState::default();
        assert_eq!(m.sample_count, SampleCount::S1);
        assert!(!m.alpha_to_coverage_enable);
    }

    #[test]
    fn test_stencil_op_state_default() {
        let s = StencilOpState::default();
        assert_eq!(s.fail_op, StencilOp::Keep);
        assert_eq!(s.pass_op, StencilOp::Keep);
        assert_eq!(s.depth_fail_op, StencilOp::Keep);
        assert_eq!(s.compare_op, CompareOp::Always);
        assert_eq!(s.compare_mask, 0xFF);
        assert_eq!(s.write_mask, 0xFF);
        assert_eq!(s.reference, 0);
    }

    #[test]
    fn test_dynamic_render_state_default() {
        let d = DynamicRenderState::default();
        assert_eq!(d.cull_mode, CullMode::Back);
        assert_eq!(d.front_face, FrontFace::CounterClockwise);
        assert!(d.depth_test_enable);
        assert!(d.depth_write_enable);
        assert_eq!(d.depth_compare_op, CompareOp::Less);
        assert!(!d.depth_bias_enable);
        assert!(!d.depth_bounds_test_enable);
        assert_eq!(d.depth_bounds_min, 0.0);
        assert_eq!(d.depth_bounds_max, 1.0);
        assert!(!d.stencil_test_enable);
        assert_eq!(d.blend_constants, [0.0; 4]);
    }

    #[test]
    fn test_dynamic_render_state_key_from_default_state() {
        let s = DynamicRenderState::default();
        let k = DynamicRenderStateKey::from(&s);
        let k2 = DynamicRenderStateKey::from(&s);
        assert_eq!(k, k2);
    }

    #[test]
    fn test_dynamic_render_state_key_diff_when_state_diff() {
        let mut s1 = DynamicRenderState::default();
        let mut s2 = DynamicRenderState::default();
        s1.cull_mode = CullMode::Front;
        s2.cull_mode = CullMode::Back;
        let k1 = DynamicRenderStateKey::from(&s1);
        let k2 = DynamicRenderStateKey::from(&s2);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_dynamic_render_state_key_handles_blend_constants_bits() {
        let mut s = DynamicRenderState::default();
        s.blend_constants = [0.5, 0.25, 0.125, 0.0625];
        let k1 = DynamicRenderStateKey::from(&s);
        s.blend_constants = [0.5, 0.25, 0.125, 0.0625];
        let k2 = DynamicRenderStateKey::from(&s);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_pipeline_reflection_empty() {
        let r = PipelineReflection::empty();
        assert_eq!(r.binding_count(), 0);
        assert_eq!(r.bindings().len(), 0);
        assert_eq!(r.push_constants().len(), 0);
        assert_eq!(r.push_constant_count(), 0);
        assert!(r.binding(0).is_none());
        assert!(r.binding_by_name("anything").is_none());
        assert!(r.binding_index("anything").is_none());
    }

    #[test]
    fn test_pipeline_reflection_new_with_bindings() {
        let bindings = vec![
            ReflectedBinding {
                name: "Camera".to_string(),
                set: 0, binding: 0,
                binding_type: BindingType::UniformBuffer,
                stage_flags: ShaderStageFlags::VERTEX,
                members: vec![],
            },
            ReflectedBinding {
                name: "AlbedoTex".to_string(),
                set: 1, binding: 0,
                binding_type: BindingType::CombinedImageSampler,
                stage_flags: ShaderStageFlags::FRAGMENT,
                members: vec![],
            },
        ];
        let r = PipelineReflection::new(bindings, vec![]);
        assert_eq!(r.binding_count(), 2);
        assert!(r.binding(0).is_some());
        assert!(r.binding(1).is_some());
        assert!(r.binding(2).is_none());
        assert_eq!(r.binding_by_name("Camera").unwrap().set, 0);
        assert_eq!(r.binding_by_name("AlbedoTex").unwrap().set, 1);
        assert!(r.binding_by_name("Missing").is_none());
        assert_eq!(r.binding_index("Camera"), Some(0));
        assert_eq!(r.binding_index("AlbedoTex"), Some(1));
        assert_eq!(r.binding_index("Missing"), None);
    }

    #[test]
    fn test_pipeline_reflection_new_with_push_constants() {
        let pc = ReflectedPushConstant {
            name: "PushBlock".to_string(),
            stage_flags: ShaderStageFlags::VERTEX_FRAGMENT,
            size: Some(64),
            members: vec![],
        };
        let r = PipelineReflection::new(vec![], vec![pc]);
        assert_eq!(r.push_constant_count(), 1);
        assert_eq!(r.push_constants().len(), 1);
        assert_eq!(r.push_constants()[0].name, "PushBlock");
        assert_eq!(r.push_constants()[0].size, Some(64));
    }

    #[test]
    fn test_pipeline_signature_key_from_empty_reflection() {
        let r = PipelineReflection::empty();
        let k = PipelineSignatureKey::from_reflection(&r);
        assert_eq!(k.descriptor_sets.len(), 0);
        assert_eq!(k.push_constant_ranges.len(), 0);
    }

    #[test]
    fn test_pipeline_signature_key_groups_bindings_by_set() {
        let bindings = vec![
            ReflectedBinding {
                name: "A".to_string(), set: 0, binding: 1,
                binding_type: BindingType::UniformBuffer,
                stage_flags: ShaderStageFlags::VERTEX, members: vec![],
            },
            ReflectedBinding {
                name: "B".to_string(), set: 0, binding: 0,
                binding_type: BindingType::UniformBuffer,
                stage_flags: ShaderStageFlags::FRAGMENT, members: vec![],
            },
            ReflectedBinding {
                name: "C".to_string(), set: 2, binding: 0,
                binding_type: BindingType::CombinedImageSampler,
                stage_flags: ShaderStageFlags::FRAGMENT, members: vec![],
            },
        ];
        let r = PipelineReflection::new(bindings, vec![]);
        let k = PipelineSignatureKey::from_reflection(&r);
        assert_eq!(k.descriptor_sets.len(), 2);
        assert_eq!(k.descriptor_sets[0].set, 0);
        assert_eq!(k.descriptor_sets[1].set, 2);
        assert_eq!(k.descriptor_sets[0].bindings[0].binding, 0);
        assert_eq!(k.descriptor_sets[0].bindings[1].binding, 1);
    }

    #[test]
    fn test_pipeline_signature_key_includes_push_constants() {
        let r = PipelineReflection::new(vec![], vec![
            ReflectedPushConstant {
                name: "PC".to_string(),
                stage_flags: ShaderStageFlags::VERTEX,
                size: Some(32),
                members: vec![],
            },
        ]);
        let k = PipelineSignatureKey::from_reflection(&r);
        assert_eq!(k.push_constant_ranges.len(), 1);
        assert_eq!(k.push_constant_ranges[0].size, 32);
    }

    #[test]
    fn test_pipeline_signature_key_equal_for_identical_reflections() {
        let make_r = || PipelineReflection::new(
            vec![ReflectedBinding {
                name: "A".to_string(), set: 0, binding: 0,
                binding_type: BindingType::UniformBuffer,
                stage_flags: ShaderStageFlags::VERTEX, members: vec![],
            }],
            vec![],
        );
        let k1 = PipelineSignatureKey::from_reflection(&make_r());
        let k2 = PipelineSignatureKey::from_reflection(&make_r());
        assert_eq!(k1, k2);
    }
}
