/// Tests for RenderInstance, RenderLOD, RenderSubMesh, and AABB

use super::*;
use crate::graphics_device::mock_graphics_device::MockGraphicsDevice;
use crate::graphics_device::{
    PrimitiveTopology, BufferFormat, TextureFormat, TextureUsage,
    MipmapMode, TextureData, SamplerType, ShaderStage,
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, IndexType,
};
use crate::resource::geometry::{
    GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::material::{
    MaterialDesc, MaterialTextureSlotDesc, ParamValue,
};
use crate::resource::texture::{TextureDesc, LayerDesc};
use crate::resource::mesh::{
    MeshDesc, MeshLODDesc, SubMeshDesc,
    GeometryMeshRef, GeometrySubMeshRef,
};
use crate::resource::shader::ShaderDesc;
use crate::resource::resource_manager::{
    ResourceManager, GeometryKey, PipelineKey, MaterialKey, MeshKey, ShaderKey,
};
use crate::utils::SlotAllocator;
use glam::{Vec3, Mat4};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_graphics_device() -> Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(MockGraphicsDevice::new()))
}

fn create_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![VertexBinding { binding: 0, stride: 8, input_rate: VertexInputRate::Vertex }],
        attributes: vec![VertexAttribute { location: 0, binding: 0, format: BufferFormat::R32G32_SFLOAT, offset: 0 }],
    }
}

fn create_test_shaders(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>) -> (ShaderKey, ShaderKey) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let vk = rm.create_shader(format!("vert_{}", id), ShaderDesc { code: &[], stage: ShaderStage::Vertex, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    let fk = rm.create_shader(format!("frag_{}", id), ShaderDesc { code: &[], stage: ShaderStage::Fragment, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    (vk, fk)
}

fn create_test_pipeline_desc(vk: ShaderKey, fk: ShaderKey) -> PipelineDesc {
    PipelineDesc {
        vertex_shader: vk, fragment_shader: fk,
        vertex_layout: create_vertex_layout(),
        topology: PrimitiveTopology::TriangleList,
        rasterization: Default::default(), color_blend: Default::default(),
        multisample: Default::default(), color_formats: vec![], depth_format: None,
    }
}

fn create_test_aabb() -> AABB {
    AABB { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) }
}

struct TestResources {
    rm: ResourceManager,
    #[allow(dead_code)]
    gd: Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>,
    geo_key: GeometryKey,
    pipeline_key: PipelineKey,
    material_key: MaterialKey,
}

fn create_test_resources() -> TestResources {
    let gd = create_mock_graphics_device();
    let mut rm = ResourceManager::new();

    let geo_key = rm.create_geometry("test_geo".to_string(), GeometryDesc {
        name: "test_geo".to_string(), graphics_device: gd.clone(),
        vertex_data: vec![0u8; 120], index_data: Some(vec![0u8; 36]),
        vertex_layout: create_vertex_layout(), index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "object".to_string(),
            lods: vec![
                GeometryLODDesc { lod_index: 0, submeshes: vec![
                    GeometrySubMeshDesc { name: "body".to_string(), vertex_offset: 0, vertex_count: 4, index_offset: 0, index_count: 6, topology: PrimitiveTopology::TriangleList },
                    GeometrySubMeshDesc { name: "head".to_string(), vertex_offset: 4, vertex_count: 4, index_offset: 6, index_count: 6, topology: PrimitiveTopology::TriangleList },
                ]},
                GeometryLODDesc { lod_index: 1, submeshes: vec![
                    GeometrySubMeshDesc { name: "body_low".to_string(), vertex_offset: 8, vertex_count: 3, index_offset: 12, index_count: 3, topology: PrimitiveTopology::TriangleList },
                ]},
            ],
        }],
    }).unwrap();

    let (vk, fk) = create_test_shaders(&mut rm, &gd);
    let pipeline_key = rm.create_pipeline("p".to_string(), create_test_pipeline_desc(vk, fk), &mut *gd.lock().unwrap()).unwrap();
    let material_key = rm.create_material("m".to_string(), MaterialDesc {
        pipeline: pipeline_key, textures: vec![],
        params: vec![("color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0]))],
        render_state: None,
    }, &*gd.lock().unwrap()).unwrap();

    TestResources { rm, gd, geo_key, pipeline_key, material_key }
}

fn create_simple_mesh_key(res: &mut TestResources) -> MeshKey {
    res.rm.create_mesh("mesh".to_string(), MeshDesc {
        geometry: res.geo_key, geometry_mesh: GeometryMeshRef::Name("object".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: res.material_key },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: res.material_key },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body_low".to_string()), material: res.material_key },
            ]},
        ],
    }).unwrap()
}

fn create_non_indexed_resources() -> (TestResources, MeshKey) {
    let gd = create_mock_graphics_device();
    let mut rm = ResourceManager::new();

    let geo_key = rm.create_geometry("ni_geo".to_string(), GeometryDesc {
        name: "ni_geo".to_string(), graphics_device: gd.clone(),
        vertex_data: vec![0u8; 48], index_data: None,
        vertex_layout: create_vertex_layout(), index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "simple".to_string(),
            lods: vec![GeometryLODDesc { lod_index: 0, submeshes: vec![
                GeometrySubMeshDesc { name: "main".to_string(), vertex_offset: 0, vertex_count: 6, index_offset: 0, index_count: 0, topology: PrimitiveTopology::TriangleList },
            ]}],
        }],
    }).unwrap();

    let (vk, fk) = create_test_shaders(&mut rm, &gd);
    let pk = rm.create_pipeline("p".to_string(), create_test_pipeline_desc(vk, fk), &mut *gd.lock().unwrap()).unwrap();
    let mk = rm.create_material("m".to_string(), MaterialDesc {
        pipeline: pk, textures: vec![], params: vec![("color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0]))], render_state: None,
    }, &*gd.lock().unwrap()).unwrap();

    let mesh_key = rm.create_mesh("mesh".to_string(), MeshDesc {
        geometry: geo_key, geometry_mesh: GeometryMeshRef::Name("simple".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("main".to_string()), material: mk }] }],
    }).unwrap();

    (TestResources { rm, gd, geo_key, pipeline_key: pk, material_key: mk }, mesh_key)
}

fn create_test_render_instance(mesh_key: MeshKey, matrix: Mat4, aabb: AABB, rm: &ResourceManager) -> crate::error::Result<RenderInstance> {
    let mesh = rm.mesh(mesh_key).unwrap();
    let mut alloc = SlotAllocator::new();
    RenderInstance::from_mesh(mesh, matrix, aabb, &mut alloc, rm)
}

// ============================================================================
// Tests: AABB
// ============================================================================

#[test]
fn test_aabb_creation() {
    let aabb = AABB { min: Vec3::new(-1.0, -2.0, -3.0), max: Vec3::new(1.0, 2.0, 3.0) };
    assert_eq!(aabb.min, Vec3::new(-1.0, -2.0, -3.0));
    assert_eq!(aabb.max, Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn test_aabb_clone() {
    let aabb = create_test_aabb();
    let cloned = aabb;
    assert_eq!(cloned.min, aabb.min);
    assert_eq!(cloned.max, aabb.max);
}

#[test]
fn test_aabb_debug() {
    let aabb = create_test_aabb();
    let debug = format!("{:?}", aabb);
    assert!(debug.contains("AABB"));
}

// ============================================================================
// Tests: Flags
// ============================================================================

#[test]
fn test_flag_values_are_distinct() {
    assert_ne!(FLAG_VISIBLE, FLAG_CAST_SHADOW);
    assert_ne!(FLAG_VISIBLE, FLAG_RECEIVE_SHADOW);
    assert_ne!(FLAG_CAST_SHADOW, FLAG_RECEIVE_SHADOW);
}

#[test]
fn test_flag_combinations() {
    let all = FLAG_VISIBLE | FLAG_CAST_SHADOW | FLAG_RECEIVE_SHADOW;
    assert!(all & FLAG_VISIBLE != 0);
    assert!(all & FLAG_CAST_SHADOW != 0);
    assert!(all & FLAG_RECEIVE_SHADOW != 0);
}

#[test]
fn test_flag_individual_bits() {
    assert_eq!(FLAG_VISIBLE, 1);
    assert_eq!(FLAG_CAST_SHADOW, 2);
    assert_eq!(FLAG_RECEIVE_SHADOW, 4);
}

// ============================================================================
// Tests: RenderInstance Creation (from_mesh)
// ============================================================================

#[test]
fn test_from_mesh_basic() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let instance = create_test_render_instance(mk, matrix, create_test_aabb(), &res.rm).unwrap();
    assert_eq!(*instance.world_matrix(), matrix);
    assert!(instance.is_visible());
    assert_eq!(instance.flags(), FLAG_VISIBLE);
}

#[test]
fn test_from_mesh_lod_count() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert_eq!(instance.lod_count(), 2);
    assert!(instance.lod(0).is_some());
    assert!(instance.lod(1).is_some());
    assert!(instance.lod(2).is_none());
}

#[test]
fn test_from_mesh_submesh_count_per_lod() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert_eq!(instance.lod(0).unwrap().sub_mesh_count(), 2);
    assert_eq!(instance.lod(1).unwrap().sub_mesh_count(), 1);
}

#[test]
fn test_from_mesh_extracts_geometry_data() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();

    let sm0 = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm0.vertex_offset(), 0);
    assert_eq!(sm0.vertex_count(), 4);
    assert_eq!(sm0.index_offset(), 0);
    assert_eq!(sm0.index_count(), 6);
    assert_eq!(sm0.topology(), PrimitiveTopology::TriangleList);

    let sm1 = instance.lod(0).unwrap().sub_mesh(1).unwrap();
    assert_eq!(sm1.vertex_offset(), 4);
    assert_eq!(sm1.vertex_count(), 4);
    assert_eq!(sm1.index_offset(), 6);
    assert_eq!(sm1.index_count(), 6);

    let sm_low = instance.lod(1).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm_low.vertex_offset(), 8);
    assert_eq!(sm_low.vertex_count(), 3);
    assert_eq!(sm_low.index_offset(), 12);
    assert_eq!(sm_low.index_count(), 3);
}

#[test]
fn test_from_mesh_stores_material_key() {
    let mut res = create_test_resources();
    let mat_key = res.material_key;
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.material(), mat_key);
}

#[test]
fn test_from_mesh_material_pipeline_structure() {
    let mut res = create_test_resources();
    let pk = res.pipeline_key;
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    let material = res.rm.material(sm.material()).unwrap();
    assert_eq!(material.pipeline(), pk);
    let pipeline = res.rm.pipeline(material.pipeline()).unwrap();
    assert!(pipeline.graphics_device_pipeline().as_ref().reflection().binding_count() == 0);
}

#[test]
fn test_from_mesh_material_with_texture() {
    let gd = create_mock_graphics_device();
    let mut rm = ResourceManager::new();

    let geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
        name: "geo".to_string(), graphics_device: gd.clone(),
        vertex_data: vec![0u8; 120], index_data: Some(vec![0u8; 36]),
        vertex_layout: create_vertex_layout(), index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "object".to_string(),
            lods: vec![GeometryLODDesc { lod_index: 0, submeshes: vec![
                GeometrySubMeshDesc { name: "body".to_string(), vertex_offset: 0, vertex_count: 4, index_offset: 0, index_count: 6, topology: PrimitiveTopology::TriangleList },
                GeometrySubMeshDesc { name: "head".to_string(), vertex_offset: 4, vertex_count: 4, index_offset: 6, index_count: 6, topology: PrimitiveTopology::TriangleList },
            ]},
            GeometryLODDesc { lod_index: 1, submeshes: vec![
                GeometrySubMeshDesc { name: "body_low".to_string(), vertex_offset: 8, vertex_count: 3, index_offset: 12, index_count: 3, topology: PrimitiveTopology::TriangleList },
            ]}],
        }],
    }).unwrap();

    let tex_key = rm.create_texture("tex".to_string(), TextureDesc {
        graphics_device: gd.clone(),
        texture: crate::graphics_device::TextureDesc {
            width: 64, height: 64, format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled, array_layers: 1,
            data: Some(TextureData::Single(vec![255u8; 64*64*4])),
            mipmap: MipmapMode::None, texture_type: crate::graphics_device::TextureType::Tex2D,
            sample_count: crate::graphics_device::SampleCount::S1,
        },
        layers: vec![LayerDesc { name: "default".to_string(), layer_index: 0, data: None, regions: vec![] }],
    }).unwrap();

    let (vk, fk) = create_test_shaders(&mut rm, &gd);
    let pk = rm.create_pipeline("p".to_string(), create_test_pipeline_desc(vk, fk), &mut *gd.lock().unwrap()).unwrap();
    let mk = rm.create_material("m".to_string(), MaterialDesc {
        pipeline: pk, textures: vec![MaterialTextureSlotDesc {
            name: "diffuse".to_string(), texture: tex_key, layer: None, region: None, sampler_type: SamplerType::LinearRepeat,
        }], params: vec![("roughness".to_string(), ParamValue::Float(0.8)), ("metallic".to_string(), ParamValue::Float(0.0))], render_state: None,
    }, &*gd.lock().unwrap()).unwrap();

    let mesh_key = rm.create_mesh("mesh".to_string(), MeshDesc {
        geometry: geo_key, geometry_mesh: GeometryMeshRef::Name("object".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: mk },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: mk },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body_low".to_string()), material: mk },
            ]},
        ],
    }).unwrap();

    let instance = create_test_render_instance(mesh_key, Mat4::IDENTITY, create_test_aabb(), &rm).unwrap();
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.material(), mk);
    assert_eq!(rm.material(sm.material()).unwrap().texture_slot_count(), 1);
}

#[test]
fn test_from_mesh_non_indexed_geometry() {
    let (res, mesh_key) = create_non_indexed_resources();
    let instance = create_test_render_instance(mesh_key, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert!(instance.index_buffer().is_none());
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.vertex_count(), 6);
    assert_eq!(sm.index_count(), 0);
}

#[test]
fn test_from_mesh_indexed_geometry_has_buffers() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert!(instance.index_buffer().is_some());
}

#[test]
fn test_from_mesh_default_flags() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert_eq!(instance.flags(), FLAG_VISIBLE);
    assert!(instance.is_visible());
}

#[test]
fn test_from_mesh_stores_bounding_box() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let aabb = AABB { min: Vec3::new(-5.0, 0.0, -5.0), max: Vec3::new(5.0, 10.0, 5.0) };
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, aabb, &res.rm).unwrap();
    assert_eq!(instance.bounding_box().min, Vec3::new(-5.0, 0.0, -5.0));
    assert_eq!(instance.bounding_box().max, Vec3::new(5.0, 10.0, 5.0));
}

// ============================================================================
// Tests: RenderInstance Accessors and Mutators
// ============================================================================

#[test]
fn test_set_world_matrix() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let mut instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert_eq!(*instance.world_matrix(), Mat4::IDENTITY);
    let new_matrix = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
    instance.set_world_matrix(new_matrix);
    assert_eq!(*instance.world_matrix(), new_matrix);
}

#[test]
fn test_set_flags() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let mut instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    let new_flags = FLAG_VISIBLE | FLAG_CAST_SHADOW | FLAG_RECEIVE_SHADOW;
    instance.set_flags(new_flags);
    assert_eq!(instance.flags(), new_flags);
}

#[test]
fn test_set_visible_true() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let mut instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    assert!(instance.is_visible());
    instance.set_visible(false);
    assert!(!instance.is_visible());
    instance.set_visible(true);
    assert!(instance.is_visible());
}

#[test]
fn test_set_visible_preserves_other_flags() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let mut instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    instance.set_flags(FLAG_VISIBLE | FLAG_CAST_SHADOW);
    instance.set_visible(false);
    assert!(!instance.is_visible());
    assert_ne!(instance.flags() & FLAG_CAST_SHADOW, 0);
    instance.set_visible(true);
    assert!(instance.is_visible());
    assert_ne!(instance.flags() & FLAG_CAST_SHADOW, 0);
}

// ============================================================================
// Tests: RenderLOD Accessors
// ============================================================================

#[test]
fn test_render_lod_sub_mesh_access() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    let lod = instance.lod(0).unwrap();
    assert!(lod.sub_mesh(0).is_some());
    assert!(lod.sub_mesh(1).is_some());
    assert!(lod.sub_mesh(2).is_none());
}

// ============================================================================
// Tests: RenderSubMesh Accessors
// ============================================================================

#[test]
fn test_render_submesh_all_accessors() {
    let mut res = create_test_resources();
    let mk = create_simple_mesh_key(&mut res);
    let instance = create_test_render_instance(mk, Mat4::IDENTITY, create_test_aabb(), &res.rm).unwrap();
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.vertex_offset(), 0);
    assert_eq!(sm.vertex_count(), 4);
    assert_eq!(sm.index_offset(), 0);
    assert_eq!(sm.index_count(), 6);
    assert_eq!(sm.topology(), PrimitiveTopology::TriangleList);
    assert_eq!(sm.material(), res.material_key);
}

// ============================================================================
// Tests: RenderInstanceKey
// ============================================================================

#[test]
fn test_render_instance_key_is_copy() {
    let key: RenderInstanceKey = slotmap::KeyData::from_ffi(0).into();
    let _copy = key;
    let _another = key;
}

// ============================================================================
// Tests: Draw Slot Allocation
// ============================================================================

#[test]
fn test_draw_slot_allocation() {
    let mut res = create_test_resources();
    let mesh_key = create_simple_mesh_key(&mut res);
    let mesh = res.rm.mesh(mesh_key).unwrap();
    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(mesh, Mat4::IDENTITY, create_test_aabb(), &mut alloc, &res.rm).unwrap();
    assert_eq!(alloc.len(), 3);
    assert_eq!(alloc.high_water_mark(), 3);
    let slot0 = instance.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot();
    let slot1 = instance.lod(0).unwrap().sub_mesh(1).unwrap().draw_slot();
    let slot2 = instance.lod(1).unwrap().sub_mesh(0).unwrap().draw_slot();
    assert_ne!(slot0, slot1);
    assert_ne!(slot0, slot2);
    assert_ne!(slot1, slot2);
}

#[test]
fn test_draw_slot_sequential_allocation() {
    let mut res = create_test_resources();
    let mesh_key = create_simple_mesh_key(&mut res);
    let mesh = res.rm.mesh(mesh_key).unwrap();
    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(mesh, Mat4::IDENTITY, create_test_aabb(), &mut alloc, &res.rm).unwrap();
    assert_eq!(instance.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot(), 0);
    assert_eq!(instance.lod(0).unwrap().sub_mesh(1).unwrap().draw_slot(), 1);
    assert_eq!(instance.lod(1).unwrap().sub_mesh(0).unwrap().draw_slot(), 2);
}

#[test]
fn test_draw_slot_shared_allocator() {
    let mut res = create_test_resources();
    let mesh_key = create_simple_mesh_key(&mut res);
    let mesh = res.rm.mesh(mesh_key).unwrap();
    let mut alloc = SlotAllocator::new();
    let inst1 = RenderInstance::from_mesh(mesh, Mat4::IDENTITY, create_test_aabb(), &mut alloc, &res.rm).unwrap();
    let inst2 = RenderInstance::from_mesh(mesh, Mat4::IDENTITY, create_test_aabb(), &mut alloc, &res.rm).unwrap();
    assert_eq!(alloc.len(), 6);
    assert_eq!(inst1.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot(), 0);
    assert_eq!(inst2.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot(), 3);
}

#[test]
fn test_free_draw_slots() {
    let mut res = create_test_resources();
    let mesh_key = create_simple_mesh_key(&mut res);
    let mesh = res.rm.mesh(mesh_key).unwrap();
    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(mesh, Mat4::IDENTITY, create_test_aabb(), &mut alloc, &res.rm).unwrap();
    assert_eq!(alloc.len(), 3);
    instance.free_draw_slots(&mut alloc);
    assert_eq!(alloc.len(), 0);
    assert_eq!(alloc.high_water_mark(), 3);
}
