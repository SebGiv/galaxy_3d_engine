/// Tests for Material resource
///
/// These tests use MockRenderer to create real Texture and Pipeline resources,
/// then validate Material creation, LayerRef/RegionRef resolution, and error handling.

use super::*;
use crate::renderer::{self, SamplerType};
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
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_count(), 1);

    let slot = material.texture_slot_by_name("albedo").unwrap();
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
    let roughness = material.param_by_name("roughness").unwrap();
    assert!((roughness.as_float().unwrap() - 0.8).abs() < f32::EPSILON);

    let base_color = material.param_by_name("base_color").unwrap();
    let v = base_color.as_vec4().unwrap();
    assert!((v[0] - 1.0).abs() < f32::EPSILON);
    assert!((v[1] - 0.5).abs() < f32::EPSILON);
    assert!((v[2] - 0.2).abs() < f32::EPSILON);
    assert!((v[3] - 1.0).abs() < f32::EPSILON);

    // Access by index
    let param0 = material.param(0).unwrap();
    assert_eq!(param0.name(), "roughness");
    assert!((param0.as_float().unwrap() - 0.8).abs() < f32::EPSILON);
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
            ("b".to_string(), ParamValue::Bool(true)),
            ("m3".to_string(), ParamValue::Mat3([
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ])),
            ("m4".to_string(), ParamValue::Mat4([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])),
        ],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.param_count(), 9);

    assert_eq!(material.param_by_name("i").unwrap().as_int().unwrap(), -42);
    assert_eq!(material.param_by_name("u").unwrap().as_uint().unwrap(), 99);
    assert_eq!(material.param_by_name("b").unwrap().as_bool().unwrap(), true);

    let m3 = material.param_by_name("m3").unwrap().as_mat3().unwrap();
    assert!((m3[0][0] - 1.0).abs() < f32::EPSILON);
    assert!((m3[1][1] - 1.0).abs() < f32::EPSILON);

    let m4 = material.param_by_name("m4").unwrap().as_mat4().unwrap();
    assert!((m4[0][0] - 1.0).abs() < f32::EPSILON);
    assert!((m4[3][3] - 1.0).abs() < f32::EPSILON);
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
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot_by_name("diffuse_map").unwrap();
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
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot_by_name("normal_map").unwrap();
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
            sampler_type: SamplerType::LinearRepeat,
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
            sampler_type: SamplerType::LinearRepeat,
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
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot_by_name("tile").unwrap();
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
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slot = material.texture_slot_by_name("tile").unwrap();
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
            sampler_type: SamplerType::LinearRepeat,
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
            sampler_type: SamplerType::LinearRepeat,
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
            sampler_type: SamplerType::LinearRepeat,
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
                sampler_type: SamplerType::LinearRepeat,
            },
            MaterialTextureSlotDesc {
                name: "albedo".to_string(), // duplicate
                texture,
                layer: None,
                region: None,
                sampler_type: SamplerType::LinearRepeat,
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
                sampler_type: SamplerType::LinearRepeat,
            },
            MaterialTextureSlotDesc {
                name: "normal".to_string(),
                texture: texture2.clone(),
                layer: None,
                region: None,
                sampler_type: SamplerType::LinearRepeat,
            },
        ],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_count(), 2);

    // Access by name
    assert!(material.texture_slot_by_name("albedo").is_some());
    assert!(material.texture_slot_by_name("normal").is_some());
    assert!(material.texture_slot_by_name("nonexistent").is_none());

    // Access by index
    let slot0 = material.texture_slot(0).unwrap();
    assert_eq!(slot0.name(), "albedo");
    let slot1 = material.texture_slot(1).unwrap();
    assert_eq!(slot1.name(), "normal");
    assert!(material.texture_slot(2).is_none());
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
                sampler_type: SamplerType::LinearRepeat,
            },
            MaterialTextureSlotDesc {
                name: "normal".to_string(),
                texture: normal_tex.clone(),
                layer: None,
                region: None,
                sampler_type: SamplerType::LinearRepeat,
            },
            MaterialTextureSlotDesc {
                name: "detail".to_string(),
                texture: indexed_tex.clone(),
                layer: Some(LayerRef::Name("diffuse".to_string())),
                region: Some(RegionRef::Name("grass".to_string())),
                sampler_type: SamplerType::LinearRepeat,
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
    let detail = material.texture_slot_by_name("detail").unwrap();
    assert_eq!(detail.layer(), Some(0));   // "diffuse" = index 0
    assert_eq!(detail.region(), Some(0));  // "grass" = index 0

    // Verify params
    assert_eq!(material.param_count(), 4);
    let metallic = material.param_by_name("metallic").unwrap();
    assert!((metallic.as_float().unwrap()).abs() < f32::EPSILON);

    // Verify slice accessors
    assert_eq!(material.texture_slots().len(), 3);
    assert_eq!(material.params().len(), 4);
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
    assert!(material.param_by_name("nonexistent").is_none());
    assert!(material.param(999).is_none());
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
    assert!(material.texture_slot_by_name("nonexistent").is_none());
    assert!(material.texture_slot(0).is_none());
}

// ============================================================================
// Tests: Typed Accessors on MaterialParam
// ============================================================================

#[test]
fn test_typed_accessors_correct_type() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("f".to_string(), ParamValue::Float(1.5)),
            ("v4".to_string(), ParamValue::Vec4([1.0, 2.0, 3.0, 4.0])),
        ],
    };

    let material = Material::from_desc(desc).unwrap();

    let f = material.param_by_name("f").unwrap();
    assert!((f.as_float().unwrap() - 1.5).abs() < f32::EPSILON);
    assert!(f.as_vec4().is_none()); // wrong type returns None

    let v4 = material.param_by_name("v4").unwrap();
    assert!(v4.as_float().is_none()); // wrong type returns None
    let arr = v4.as_vec4().unwrap();
    assert!((arr[2] - 3.0).abs() < f32::EPSILON);
}

// ============================================================================
// Tests: Slice Accessors
// ============================================================================

#[test]
fn test_texture_slots_slice() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_simple_texture(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![
            MaterialTextureSlotDesc {
                name: "a".to_string(),
                texture: texture.clone(),
                layer: None,
                region: None,
                sampler_type: SamplerType::LinearRepeat,
            },
            MaterialTextureSlotDesc {
                name: "b".to_string(),
                texture,
                layer: None,
                region: None,
                sampler_type: SamplerType::LinearRepeat,
            },
        ],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    let slots = material.texture_slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].name(), "a");
    assert_eq!(slots[1].name(), "b");
}

#[test]
fn test_params_slice() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("x".to_string(), ParamValue::Float(1.0)),
            ("y".to_string(), ParamValue::Int(42)),
        ],
    };

    let material = Material::from_desc(desc).unwrap();
    let params = material.params();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name(), "x");
    assert_eq!(params[1].name(), "y");
    assert!((params[0].as_float().unwrap() - 1.0).abs() < f32::EPSILON);
    assert_eq!(params[1].as_int().unwrap(), 42);
}

// ============================================================================
// Tests: param_id and texture_slot_id
// ============================================================================

#[test]
fn test_param_id() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![],
        params: vec![
            ("alpha".to_string(), ParamValue::Float(0.5)),
            ("beta".to_string(), ParamValue::Int(10)),
        ],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.param_id("alpha"), Some(0));
    assert_eq!(material.param_id("beta"), Some(1));
    assert_eq!(material.param_id("nonexistent"), None);
}

#[test]
fn test_texture_slot_id() {
    let renderer = create_mock_renderer();
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_simple_texture(renderer.clone());

    let desc = MaterialDesc {
        pipeline,
        textures: vec![MaterialTextureSlotDesc {
            name: "diffuse".to_string(),
            texture,
            layer: None,
            region: None,
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![],
    };

    let material = Material::from_desc(desc).unwrap();
    assert_eq!(material.texture_slot_id("diffuse"), Some(0));
    assert_eq!(material.texture_slot_id("nonexistent"), None);
}
