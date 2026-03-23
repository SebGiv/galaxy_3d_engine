/// Tests for Material resource
///
/// These tests use MockGraphicsDevice + ResourceManager to create real Texture and Pipeline
/// resources via SlotMap keys, then validate Material creation, LayerRef/RegionRef resolution,
/// and error handling.

use super::*;
use crate::graphics_device::{self, SamplerType};
use crate::resource::texture::{TextureDesc, LayerDesc, AtlasRegion, AtlasRegionDesc};
use crate::resource::resource_manager::{ResourceManager, PipelineKey, TextureKey, ShaderKey};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::shader::ShaderDesc;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()))
}

fn create_test_context() -> (ResourceManager, Arc<Mutex<dyn graphics_device::GraphicsDevice>>) {
    (ResourceManager::new(), create_mock_graphics_device())
}

fn create_simple_texture(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, name: &str) -> TextureKey {
    rm.create_texture(name.to_string(), TextureDesc {
        graphics_device: gd.clone(),
        texture: graphics_device::TextureDesc {
            width: 256, height: 256,
            format: graphics_device::TextureFormat::R8G8B8A8_UNORM,
            usage: graphics_device::TextureUsage::Sampled,
            texture_type: graphics_device::TextureType::Tex2D,
            array_layers: 1,
            data: Some(graphics_device::TextureData::Single(vec![255u8; 256 * 256 * 4])),
            mipmap: graphics_device::MipmapMode::None,
        },
        layers: vec![LayerDesc { name: "default".to_string(), layer_index: 0, data: None, regions: vec![] }],
    }).unwrap()
}

fn create_indexed_texture_with_regions(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, name: &str) -> TextureKey {
    rm.create_texture(name.to_string(), TextureDesc {
        graphics_device: gd.clone(),
        texture: graphics_device::TextureDesc {
            width: 256, height: 256,
            format: graphics_device::TextureFormat::R8G8B8A8_UNORM,
            usage: graphics_device::TextureUsage::Sampled,
            texture_type: graphics_device::TextureType::Array2D,
            array_layers: 4, data: None, mipmap: graphics_device::MipmapMode::None,
        },
        layers: vec![
            LayerDesc { name: "diffuse".to_string(), layer_index: 0, data: None, regions: vec![
                AtlasRegionDesc { name: "grass".to_string(), region: AtlasRegion { x: 0, y: 0, width: 128, height: 128 } },
                AtlasRegionDesc { name: "stone".to_string(), region: AtlasRegion { x: 128, y: 0, width: 128, height: 128 } },
            ]},
            LayerDesc { name: "normal".to_string(), layer_index: 1, data: None, regions: vec![] },
            LayerDesc { name: "roughness".to_string(), layer_index: 2, data: None, regions: vec![] },
        ],
    }).unwrap()
}

fn create_test_shaders(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>) -> (ShaderKey, ShaderKey) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let vk = rm.create_shader(format!("vert_{}", id), ShaderDesc { code: &[], stage: graphics_device::ShaderStage::Vertex, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    let fk = rm.create_shader(format!("frag_{}", id), ShaderDesc { code: &[], stage: graphics_device::ShaderStage::Fragment, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    (vk, fk)
}

fn create_test_pipeline(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, name: &str) -> PipelineKey {
    let (vk, fk) = create_test_shaders(rm, gd);
    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding { binding: 0, stride: 16, input_rate: graphics_device::VertexInputRate::Vertex }],
        attributes: vec![graphics_device::VertexAttribute { location: 0, binding: 0, format: graphics_device::BufferFormat::R32G32_SFLOAT, offset: 0 }],
    };
    rm.create_pipeline(name.to_string(), PipelineDesc {
        vertex_shader: vk, fragment_shader: fk,
        vertex_layout, topology: graphics_device::PrimitiveTopology::TriangleList,
        rasterization: Default::default(), color_blend: Default::default(),
        multisample: Default::default(), color_formats: vec![], depth_format: None,
    }, &mut *gd.lock().unwrap()).unwrap()
}

// ============================================================================
// Tests: Basic Material Creation
// ============================================================================

#[test]
fn test_create_material_minimal() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc { pipeline: pk, textures: vec![], params: vec![], render_state: None }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_count(), 0);
    assert_eq!(mat.param_count(), 0);
    assert_eq!(mat.pipeline(), pk);
}

#[test]
fn test_create_material_with_simple_texture() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_simple_texture(&mut rm, &gd, "tex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "albedo".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_count(), 1);
    let slot = mat.texture_slot_by_name("albedo").unwrap();
    assert_eq!(slot.name(), "albedo");
    assert_eq!(slot.texture(), tk);
    assert_eq!(slot.layer(), None);
    assert_eq!(slot.region(), None);
}

#[test]
fn test_create_material_with_params() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
            ("base_color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0])),
            ("metallic".to_string(), ParamValue::Float(0.0)),
        ], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.param_count(), 3);
    assert!((mat.param_by_name("roughness").unwrap().as_float().unwrap() - 0.8).abs() < f32::EPSILON);
    let v = mat.param_by_name("base_color").unwrap().as_vec4().unwrap();
    assert!((v[0] - 1.0).abs() < f32::EPSILON);
    assert!((v[1] - 0.5).abs() < f32::EPSILON);
    let p0 = mat.param(0).unwrap();
    assert_eq!(p0.name(), "roughness");
}

#[test]
fn test_create_material_with_all_param_types() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![
            ("f".to_string(), ParamValue::Float(1.0)),
            ("v2".to_string(), ParamValue::Vec2([1.0, 2.0])),
            ("v3".to_string(), ParamValue::Vec3([1.0, 2.0, 3.0])),
            ("v4".to_string(), ParamValue::Vec4([1.0, 2.0, 3.0, 4.0])),
            ("i".to_string(), ParamValue::Int(-42)),
            ("u".to_string(), ParamValue::UInt(99)),
            ("b".to_string(), ParamValue::Bool(true)),
            ("m3".to_string(), ParamValue::Mat3([[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]])),
            ("m4".to_string(), ParamValue::Mat4([[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]])),
        ], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.param_count(), 9);
    assert_eq!(mat.param_by_name("i").unwrap().as_int().unwrap(), -42);
    assert_eq!(mat.param_by_name("u").unwrap().as_uint().unwrap(), 99);
    assert_eq!(mat.param_by_name("b").unwrap().as_bool().unwrap(), true);
}

// ============================================================================
// Tests: LayerRef Resolution
// ============================================================================

#[test]
fn test_layer_ref_by_index() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "d".to_string(), texture: tk, layer: Some(LayerRef::Index(1)), region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_by_name("d").unwrap().layer(), Some(1));
}

#[test]
fn test_layer_ref_by_name() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "n".to_string(), texture: tk, layer: Some(LayerRef::Name("normal".to_string())), region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_by_name("n").unwrap().layer(), Some(1));
}

#[test]
fn test_layer_ref_invalid_index() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "x".to_string(), texture: tk, layer: Some(LayerRef::Index(99)), region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

#[test]
fn test_layer_ref_invalid_name() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "x".to_string(), texture: tk, layer: Some(LayerRef::Name("nonexistent".to_string())), region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

// ============================================================================
// Tests: RegionRef Resolution
// ============================================================================

#[test]
fn test_region_ref_by_index() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "t".to_string(), texture: tk, layer: Some(LayerRef::Name("diffuse".to_string())),
            region: Some(RegionRef::Index(0)), sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    let slot = mat.texture_slot_by_name("t").unwrap();
    assert_eq!(slot.layer(), Some(0));
    assert_eq!(slot.region(), Some(0));
}

#[test]
fn test_region_ref_by_name() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "t".to_string(), texture: tk, layer: Some(LayerRef::Name("diffuse".to_string())),
            region: Some(RegionRef::Name("stone".to_string())), sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    let slot = mat.texture_slot_by_name("t").unwrap();
    assert_eq!(slot.layer(), Some(0));
    assert_eq!(slot.region(), Some(1));
}

#[test]
fn test_region_ref_invalid_index() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "x".to_string(), texture: tk, layer: Some(LayerRef::Index(0)),
            region: Some(RegionRef::Index(99)), sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

#[test]
fn test_region_ref_invalid_name() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "x".to_string(), texture: tk, layer: Some(LayerRef::Index(0)),
            region: Some(RegionRef::Name("nonexistent".to_string())), sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

#[test]
fn test_region_without_layer_fails() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_indexed_texture_with_regions(&mut rm, &gd, "itex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "x".to_string(), texture: tk, layer: None,
            region: Some(RegionRef::Index(0)), sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

// ============================================================================
// Tests: Validation Errors
// ============================================================================

#[test]
fn test_duplicate_texture_slot_name_fails() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_simple_texture(&mut rm, &gd, "tex");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![
            MaterialTextureSlotDesc { name: "albedo".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
            MaterialTextureSlotDesc { name: "albedo".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
        ], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

#[test]
fn test_duplicate_param_name_fails() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    assert!(Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![("roughness".to_string(), ParamValue::Float(0.5)), ("roughness".to_string(), ParamValue::Float(0.8))],
        render_state: None,
    }, &rm, &*gd.lock().unwrap()).is_err());
}

// ============================================================================
// Tests: Multiple Texture Slots
// ============================================================================

#[test]
fn test_multiple_texture_slots() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk1 = create_simple_texture(&mut rm, &gd, "tex1");
    let tk2 = create_simple_texture(&mut rm, &gd, "tex2");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![
            MaterialTextureSlotDesc { name: "albedo".to_string(), texture: tk1, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
            MaterialTextureSlotDesc { name: "normal".to_string(), texture: tk2, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
        ], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_count(), 2);
    assert!(mat.texture_slot_by_name("albedo").is_some());
    assert!(mat.texture_slot_by_name("normal").is_some());
    assert!(mat.texture_slot_by_name("nonexistent").is_none());
}

// ============================================================================
// Tests: Full PBR-style Material
// ============================================================================

#[test]
fn test_full_pbr_material() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "pbr");
    let albedo_k = create_simple_texture(&mut rm, &gd, "albedo_tex");
    let normal_k = create_simple_texture(&mut rm, &gd, "normal_tex");
    let indexed_k = create_indexed_texture_with_regions(&mut rm, &gd, "indexed_tex");

    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![
            MaterialTextureSlotDesc { name: "albedo".to_string(), texture: albedo_k, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
            MaterialTextureSlotDesc { name: "normal".to_string(), texture: normal_k, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
            MaterialTextureSlotDesc { name: "detail".to_string(), texture: indexed_k,
                layer: Some(LayerRef::Name("diffuse".to_string())), region: Some(RegionRef::Name("grass".to_string())),
                sampler_type: SamplerType::LinearRepeat },
        ],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.5)),
            ("metallic".to_string(), ParamValue::Float(0.0)),
            ("base_color".to_string(), ParamValue::Vec4([1.0, 1.0, 1.0, 1.0])),
            ("uv_scale".to_string(), ParamValue::Vec2([1.0, 1.0])),
        ], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();

    assert_eq!(mat.pipeline(), pk);
    assert_eq!(mat.texture_slot_count(), 3);
    let detail = mat.texture_slot_by_name("detail").unwrap();
    assert_eq!(detail.layer(), Some(0));
    assert_eq!(detail.region(), Some(0));
    assert_eq!(mat.param_count(), 4);
    assert_eq!(mat.texture_slots().len(), 3);
    assert_eq!(mat.params().len(), 4);
}

// ============================================================================
// Tests: Accessor Edge Cases
// ============================================================================

#[test]
fn test_param_not_found() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![("roughness".to_string(), ParamValue::Float(0.5))],
        render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert!(mat.param_by_name("nonexistent").is_none());
    assert!(mat.param(999).is_none());
}

#[test]
fn test_texture_slot_not_found() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert!(mat.texture_slot_by_name("nonexistent").is_none());
    assert!(mat.texture_slot(0).is_none());
}

// ============================================================================
// Tests: Typed Accessors on MaterialParam
// ============================================================================

#[test]
fn test_typed_accessors_correct_type() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![("f".to_string(), ParamValue::Float(1.5)), ("v4".to_string(), ParamValue::Vec4([1.0,2.0,3.0,4.0]))],
        render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    let f = mat.param_by_name("f").unwrap();
    assert!((f.as_float().unwrap() - 1.5).abs() < f32::EPSILON);
    assert!(f.as_vec4().is_none());
    let v4 = mat.param_by_name("v4").unwrap();
    assert!(v4.as_float().is_none());
    assert!((v4.as_vec4().unwrap()[2] - 3.0).abs() < f32::EPSILON);
}

// ============================================================================
// Tests: Slice Accessors
// ============================================================================

#[test]
fn test_texture_slots_slice() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_simple_texture(&mut rm, &gd, "tex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![
            MaterialTextureSlotDesc { name: "a".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
            MaterialTextureSlotDesc { name: "b".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat },
        ], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slots().len(), 2);
    assert_eq!(mat.texture_slots()[0].name(), "a");
    assert_eq!(mat.texture_slots()[1].name(), "b");
}

#[test]
fn test_params_slice() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![("x".to_string(), ParamValue::Float(1.0)), ("y".to_string(), ParamValue::Int(42))],
        render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.params().len(), 2);
    assert_eq!(mat.params()[0].name(), "x");
    assert_eq!(mat.params()[1].name(), "y");
}

// ============================================================================
// Tests: param_id and texture_slot_id
// ============================================================================

#[test]
fn test_param_id() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![],
        params: vec![("alpha".to_string(), ParamValue::Float(0.5)), ("beta".to_string(), ParamValue::Int(10))],
        render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.param_id("alpha"), Some(0));
    assert_eq!(mat.param_id("beta"), Some(1));
    assert_eq!(mat.param_id("nonexistent"), None);
}

#[test]
fn test_texture_slot_id() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let tk = create_simple_texture(&mut rm, &gd, "tex");
    let mat = Material::from_desc(0, MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "diffuse".to_string(), texture: tk, layer: None, region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.texture_slot_id("diffuse"), Some(0));
    assert_eq!(mat.texture_slot_id("nonexistent"), None);
}

// ============================================================================
// Tests: Slot ID
// ============================================================================

#[test]
fn test_slot_id() {
    let (mut rm, gd) = create_test_context();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mat = Material::from_desc(42, MaterialDesc {
        pipeline: pk, textures: vec![], params: vec![], render_state: None,
    }, &rm, &*gd.lock().unwrap()).unwrap();
    assert_eq!(mat.slot_id(), 42);
}
