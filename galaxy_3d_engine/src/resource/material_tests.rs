/// Tests for Material resource
///
/// These tests use MockRenderer to create real Texture and Pipeline resources,
/// then validate Material creation, LayerRef/RegionRef resolution, and error handling.

use super::*;
use crate::renderer;
use crate::resource::texture::{TextureDesc, LayerDesc, AtlasRegion, AtlasRegionDesc};
use crate::resource::pipeline::{PipelineDesc, PipelineVariantDesc, PipelinePassDesc};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a MockRenderer wrapped in Arc<Mutex<>>
fn create_mock_renderer() -> Arc<Mutex<dyn renderer::Renderer>> {
    Arc::new(Mutex::new(renderer::mock_renderer::MockRenderer::new()))
}

/// Create a simple texture (1 layer, no regions)
fn create_simple_texture(renderer: Arc<Mutex<dyn renderer::Renderer>>) -> Arc<Texture> {
    let desc = TextureDesc {
        renderer,
        texture: renderer::TextureDesc {
            width: 256,
            height: 256,
            format: renderer::TextureFormat::R8G8B8A8_UNORM,
            usage: renderer::TextureUsage::Sampled,
            array_layers: 1,
            data: Some(renderer::TextureData::Single(vec![255u8; 256 * 256 * 4])),
            mipmap: renderer::MipmapMode::None,
        },
        layers: vec![LayerDesc {
            name: "default".to_string(),
            layer_index: 0,
            data: None,
            regions: vec![],
        }],
    };
    Arc::new(Texture::from_desc(desc).unwrap())
}

/// Create an indexed texture (4 layers, with atlas regions on layer 0)
fn create_indexed_texture_with_regions(renderer: Arc<Mutex<dyn renderer::Renderer>>) -> Arc<Texture> {
    let desc = TextureDesc {
        renderer,
        texture: renderer::TextureDesc {
            width: 256,
            height: 256,
            format: renderer::TextureFormat::R8G8B8A8_UNORM,
            usage: renderer::TextureUsage::Sampled,
            array_layers: 4,
            data: None,
            mipmap: renderer::MipmapMode::None,
        },
        layers: vec![
            LayerDesc {
                name: "diffuse".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "grass".to_string(),
                        region: AtlasRegion { x: 0, y: 0, width: 128, height: 128 },
                    },
                    AtlasRegionDesc {
                        name: "stone".to_string(),
                        region: AtlasRegion { x: 128, y: 0, width: 128, height: 128 },
                    },
                ],
            },
            LayerDesc {
                name: "normal".to_string(),
                layer_index: 1,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "roughness".to_string(),
                layer_index: 2,
                data: None,
                regions: vec![],
            },
        ],
    };
    Arc::new(Texture::from_desc(desc).unwrap())
}

/// Create a test pipeline
fn create_test_pipeline(renderer: Arc<Mutex<dyn renderer::Renderer>>) -> Arc<Pipeline> {
    let vertex_layout = renderer::VertexLayout {
        bindings: vec![renderer::VertexBinding {
            binding: 0,
            stride: 16,
            input_rate: renderer::VertexInputRate::Vertex,
        }],
        attributes: vec![renderer::VertexAttribute {
            location: 0,
            binding: 0,
            format: renderer::BufferFormat::R32G32_SFLOAT,
            offset: 0,
        }],
    };

    let desc = PipelineDesc {
        renderer,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: renderer::PipelineDesc {
                    vertex_shader: Arc::new(renderer::mock_renderer::MockShader::new("vert".to_string())),
                    fragment_shader: Arc::new(renderer::mock_renderer::MockShader::new("frag".to_string())),
                    vertex_layout,
                    topology: renderer::PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    binding_group_layouts: vec![],
                    rasterization: Default::default(),
                    depth_stencil: Default::default(),
                    color_blend: Default::default(),
                    multisample: Default::default(),
                },
            }],
        }],
    };
    Arc::new(Pipeline::from_desc(desc).unwrap())
}

// ============================================================================
// Tests: Basic Material Creation
// ============================================================================

#[test]
fn test_create_material_minimal() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_count(), 0);
    assert_eq!(material.param_count(), 0);
    assert!(Arc::ptr_eq(material.pipeline(), &pipeline));
}

#[test]
fn test_create_material_with_simple_texture() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_simple_texture(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "albedo".to_string(),
            texture: texture.clone(),
            layer: None,
            region: None,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_count(), 1);

    let slot = material.texture_slot("albedo").unwrap();
    assert_eq!(slot.name(), "albedo");
    assert!(Arc::ptr_eq(slot.texture(), &texture));
    assert_eq!(slot.layer(), None);
    assert_eq!(slot.region(), None);
}

#[test]
fn test_create_material_with_params() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
            ("base_color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0])),
            ("metallic".to_string(), ParamValue::Float(0.0)),
        ],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.param_count(), 3);

    // Access by name
    match material.param("roughness").unwrap() {
        ParamValue::Float(v) => assert!((v - 0.8).abs() < f32::EPSILON),
        _ => panic!("Expected Float"),
    }

    match material.param("base_color").unwrap() {
        ParamValue::Vec4(v) => {
            assert!((v[0] - 1.0).abs() < f32::EPSILON);
            assert!((v[1] - 0.5).abs() < f32::EPSILON);
            assert!((v[2] - 0.2).abs() < f32::EPSILON);
            assert!((v[3] - 1.0).abs() < f32::EPSILON);
        }
        _ => panic!("Expected Vec4"),
    }

    // Access by index
    let (name, value) = material.param_at(0).unwrap();
    assert_eq!(name, "roughness");
    match value {
        ParamValue::Float(v) => assert!((v - 0.8).abs() < f32::EPSILON),
        _ => panic!("Expected Float"),
    }
}

#[test]
fn test_create_material_with_all_param_types() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("f".to_string(), ParamValue::Float(1.0)),
            ("v2".to_string(), ParamValue::Vec2([1.0, 2.0])),
            ("v3".to_string(), ParamValue::Vec3([1.0, 2.0, 3.0])),
            ("v4".to_string(), ParamValue::Vec4([1.0, 2.0, 3.0, 4.0])),
            ("i".to_string(), ParamValue::Int(-42)),
            ("u".to_string(), ParamValue::UInt(99)),
        ],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.param_count(), 6);

    match material.param("i").unwrap() {
        ParamValue::Int(v) => assert_eq!(*v, -42),
        _ => panic!("Expected Int"),
    }

    match material.param("u").unwrap() {
        ParamValue::UInt(v) => assert_eq!(*v, 99),
        _ => panic!("Expected UInt"),
    }
}

// ============================================================================
// Tests: LayerRef Resolution
// ============================================================================

#[test]
fn test_layer_ref_by_index() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "diffuse_map".to_string(),
            texture: texture.clone(),
            layer: Some(LayerRef::Index(1)),
            region: None,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot("diffuse_map").unwrap();
    assert_eq!(slot.layer(), Some(1));
    assert_eq!(slot.region(), None);
}

#[test]
fn test_layer_ref_by_name() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "normal_map".to_string(),
            texture: texture.clone(),
            layer: Some(LayerRef::Name("normal".to_string())),
            region: None,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot("normal_map").unwrap();
    // "normal" is layer index 1 in the indexed texture
    assert_eq!(slot.layer(), Some(1));
}

#[test]
fn test_layer_ref_invalid_index() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "bad_layer".to_string(),
            texture,
            layer: Some(LayerRef::Index(99)),
            region: None,
        }],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_layer_ref_invalid_name() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "bad_layer".to_string(),
            texture,
            layer: Some(LayerRef::Name("nonexistent".to_string())),
            region: None,
        }],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// Tests: RegionRef Resolution
// ============================================================================

#[test]
fn test_region_ref_by_index() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "tile".to_string(),
            texture: texture.clone(),
            layer: Some(LayerRef::Name("diffuse".to_string())),
            region: Some(RegionRef::Index(0)), // "grass" region
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot("tile").unwrap();
    assert_eq!(slot.layer(), Some(0));   // "diffuse" = layer 0
    assert_eq!(slot.region(), Some(0));  // region index 0 = "grass"
}

#[test]
fn test_region_ref_by_name() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "tile".to_string(),
            texture: texture.clone(),
            layer: Some(LayerRef::Name("diffuse".to_string())),
            region: Some(RegionRef::Name("stone".to_string())),
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot("tile").unwrap();
    assert_eq!(slot.layer(), Some(0));   // "diffuse" = layer 0
    assert_eq!(slot.region(), Some(1));  // "stone" = region index 1
}

#[test]
fn test_region_ref_invalid_index() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "bad_region".to_string(),
            texture,
            layer: Some(LayerRef::Index(0)),
            region: Some(RegionRef::Index(99)),
        }],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_region_ref_invalid_name() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "bad_region".to_string(),
            texture,
            layer: Some(LayerRef::Index(0)),
            region: Some(RegionRef::Name("nonexistent".to_string())),
        }],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_region_without_layer_fails() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "bad_slot".to_string(),
            texture,
            layer: None,
            region: Some(RegionRef::Index(0)),
        }],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// Tests: Validation Errors
// ============================================================================

#[test]
fn test_duplicate_texture_slot_name_fails() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_simple_texture(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![
            MaterialTextureSlotDesc {
                name: "albedo".to_string(),
                texture: texture.clone(),
                layer: None,
                region: None,
            },
            MaterialTextureSlotDesc {
                name: "albedo".to_string(), // duplicate
                texture,
                layer: None,
                region: None,
            },
        ],
        params: vec![],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_duplicate_param_name_fails() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.5)),
            ("roughness".to_string(), ParamValue::Float(0.8)), // duplicate
        ],
    };

    let result = Material::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// Tests: Multiple Texture Slots
// ============================================================================

#[test]
fn test_multiple_texture_slots() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture1 = create_simple_texture(renderer.clone());
    let texture2 = create_simple_texture(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![
            MaterialTextureSlotDesc {
                name: "albedo".to_string(),
                texture: texture1.clone(),
                layer: None,
                region: None,
            },
            MaterialTextureSlotDesc {
                name: "normal".to_string(),
                texture: texture2.clone(),
                layer: None,
                region: None,
            },
        ],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_count(), 2);

    // Access by name
    assert!(material.texture_slot("albedo").is_some());
    assert!(material.texture_slot("normal").is_some());
    assert!(material.texture_slot("nonexistent").is_none());

    // Access by index
    let slot0 = material.texture_slot_at(0).unwrap();
    assert_eq!(slot0.name(), "albedo");
    let slot1 = material.texture_slot_at(1).unwrap();
    assert_eq!(slot1.name(), "normal");
    assert!(material.texture_slot_at(2).is_none());
}

// ============================================================================
// Tests: Full PBR-style Material
// ============================================================================

#[test]
fn test_full_pbr_material() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let albedo_tex = create_simple_texture(renderer.clone());
    let normal_tex = create_simple_texture(renderer.clone());
    let indexed_tex = create_indexed_texture_with_regions(renderer.clone());

    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![
            MaterialTextureSlotDesc {
                name: "albedo".to_string(),
                texture: albedo_tex.clone(),
                layer: None,
                region: None,
            },
            MaterialTextureSlotDesc {
                name: "normal".to_string(),
                texture: normal_tex.clone(),
                layer: None,
                region: None,
            },
            MaterialTextureSlotDesc {
                name: "detail".to_string(),
                texture: indexed_tex.clone(),
                layer: Some(LayerRef::Name("diffuse".to_string())),
                region: Some(RegionRef::Name("grass".to_string())),
            },
        ],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.5)),
            ("metallic".to_string(), ParamValue::Float(0.0)),
            ("base_color".to_string(), ParamValue::Vec4([1.0, 1.0, 1.0, 1.0])),
            ("uv_scale".to_string(), ParamValue::Vec2([1.0, 1.0])),
        ],
    };

    let material = Material::from_desc(desc).unwrap();

    // Verify pipeline
    assert!(Arc::ptr_eq(material.pipeline(), &pipeline));

    // Verify texture slots
    assert_eq!(material.texture_slot_count(), 3);
    let detail = material.texture_slot("detail").unwrap();
    assert_eq!(detail.layer(), Some(0));   // "diffuse" = index 0
    assert_eq!(detail.region(), Some(0));  // "grass" = index 0

    // Verify params
    assert_eq!(material.param_count(), 4);
    match material.param("metallic").unwrap() {
        ParamValue::Float(v) => assert!((*v).abs() < f32::EPSILON),
        _ => panic!("Expected Float"),
    }
}

// ============================================================================
// Tests: Accessor Edge Cases
// ============================================================================

#[test]
fn test_param_not_found() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![("roughness".to_string(), ParamValue::Float(0.5))],
    };

    let material = Material::from_desc(desc).unwrap();
    assert!(material.param("nonexistent").is_none());
    assert!(material.param_at(999).is_none());
}

#[test]
fn test_texture_slot_not_found() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert!(material.texture_slot("nonexistent").is_none());
    assert!(material.texture_slot_at(0).is_none());
}
