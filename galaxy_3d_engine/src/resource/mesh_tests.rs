/// Tests for Mesh resource
///
/// These tests use MockGraphicsDevice and ResourceManager to create real Geometry,
/// Pipeline, and Material resources via SlotMap keys, then validate Mesh creation,
/// ref resolution, ordering, and error handling.

use super::*;
use crate::graphics_device;
use crate::resource::geometry::{
    GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::material::{MaterialDesc, ParamValue};
use crate::resource::resource_manager::{ResourceManager, GeometryKey, MaterialKey, PipelineKey, ShaderKey};
use crate::resource::shader::ShaderDesc;
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()))
}

fn create_test_resources() -> (
    GeometryKey,
    ResourceManager,
    Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
) {
    let graphics_device = create_mock_graphics_device();
    let mut rm = ResourceManager::new();

    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding {
            binding: 0, stride: 8, input_rate: graphics_device::VertexInputRate::Vertex,
        }],
        attributes: vec![graphics_device::VertexAttribute {
            location: 0, binding: 0,
            format: graphics_device::BufferFormat::R32G32_SFLOAT, offset: 0,
        }],
    };

    let desc = GeometryDesc {
        name: "characters".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![0u8; 160],
        index_data: Some(vec![0u8; 48]),
        vertex_layout,
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "body".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc { name: "head".to_string(), vertex_offset: 0, vertex_count: 4, index_offset: 0, index_count: 6, topology: graphics_device::PrimitiveTopology::TriangleList },
                            GeometrySubMeshDesc { name: "torso".to_string(), vertex_offset: 4, vertex_count: 4, index_offset: 6, index_count: 6, topology: graphics_device::PrimitiveTopology::TriangleList },
                            GeometrySubMeshDesc { name: "legs".to_string(), vertex_offset: 8, vertex_count: 4, index_offset: 12, index_count: 6, topology: graphics_device::PrimitiveTopology::TriangleList },
                        ],
                    },
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            GeometrySubMeshDesc { name: "head".to_string(), vertex_offset: 0, vertex_count: 4, index_offset: 0, index_count: 6, topology: graphics_device::PrimitiveTopology::TriangleList },
                            GeometrySubMeshDesc { name: "body".to_string(), vertex_offset: 4, vertex_count: 6, index_offset: 6, index_count: 12, topology: graphics_device::PrimitiveTopology::TriangleList },
                        ],
                    },
                ],
            },
            GeometryMeshDesc {
                name: "weapon".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc { name: "blade".to_string(), vertex_offset: 12, vertex_count: 4, index_offset: 18, index_count: 6, topology: graphics_device::PrimitiveTopology::TriangleList },
                        ],
                    },
                ],
            },
        ],
    };

    let geom_key = rm.create_geometry("characters".to_string(), desc).unwrap();
    (geom_key, rm, graphics_device)
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
        bindings: vec![graphics_device::VertexBinding { binding: 0, stride: 8, input_rate: graphics_device::VertexInputRate::Vertex }],
        attributes: vec![graphics_device::VertexAttribute { location: 0, binding: 0, format: graphics_device::BufferFormat::R32G32_SFLOAT, offset: 0 }],
    };
    let desc = PipelineDesc {
        vertex_shader: vk, fragment_shader: fk,
        vertex_layout, topology: graphics_device::PrimitiveTopology::TriangleList,
        rasterization: Default::default(), color_blend: Default::default(),
        multisample: Default::default(), color_formats: vec![], depth_format: None,
    };
    rm.create_pipeline(name.to_string(), desc, &mut *gd.lock().unwrap()).unwrap()
}

fn create_test_material(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, pipeline: PipelineKey, name: &str, value: f32) -> MaterialKey {
    rm.create_material(name.to_string(), MaterialDesc {
        pipeline, textures: vec![],
        params: vec![("value".to_string(), ParamValue::Float(value))],
        render_state: None,
    }, &*gd.lock().unwrap()).unwrap()
}

fn material_value(rm: &ResourceManager, key: MaterialKey) -> f32 {
    rm.material(key).unwrap().param_by_name("value").unwrap().as_float().unwrap()
}

// ============================================================================
// Tests: Basic Mesh Creation
// ============================================================================

#[test]
fn test_create_mesh_single_lod_single_submesh() {
    let (geom_key, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: geom_key, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).unwrap();
    assert_eq!(mesh.lod_count(), 1);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 1);
    assert_eq!(mesh.lod(0).unwrap().submesh(0).unwrap().material(), mk);
}

#[test]
fn test_create_mesh_multi_lod() {
    let (geom_key, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: geom_key, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
            ]},
        ],
    }, &rm).unwrap();
    assert_eq!(mesh.lod_count(), 2);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

// ============================================================================
// Tests: GeometryMeshRef Resolution
// ============================================================================

#[test]
fn test_geometry_mesh_ref_by_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).unwrap();
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_by_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Index(1),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Index(0), material: mk }] }],
    }, &rm).unwrap();
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_invalid_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("nonexistent".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).is_err());
}

#[test]
fn test_geometry_mesh_ref_invalid_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Index(99),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Index(0), material: mk }] }],
    }, &rm).is_err());
}

// ============================================================================
// Tests: GeometrySubMeshRef Resolution
// ============================================================================

#[test]
fn test_submesh_ref_by_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).unwrap();
    assert_eq!(mesh.lod(0).unwrap().submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_by_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Index(0), material: mk }] }],
    }, &rm).unwrap();
    assert_eq!(mesh.lod(0).unwrap().submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_invalid_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("nonexistent".to_string()), material: armor },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
            ]},
        ],
    }, &rm).is_err());
}

#[test]
fn test_submesh_ref_invalid_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Index(99), material: mk }] }],
    }, &rm).is_err());
}

// ============================================================================
// Tests: Submesh Ordering
// ============================================================================

#[test]
fn test_submesh_order_matches_geometry_lod() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);

    // Provide submeshes in REVERSE order
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            ]},
        ],
    }, &rm).unwrap();

    let lod0 = mesh.lod(0).unwrap();
    assert_eq!(lod0.submesh(0).unwrap().submesh_id(), 0);
    assert_eq!(lod0.submesh(1).unwrap().submesh_id(), 1);
    assert_eq!(lod0.submesh(2).unwrap().submesh_id(), 2);
    assert!((material_value(&rm, lod0.submesh(0).unwrap().material()) - 1.0).abs() < f32::EPSILON);
    assert!((material_value(&rm, lod0.submesh(1).unwrap().material()) - 2.0).abs() < f32::EPSILON);
    assert!((material_value(&rm, lod0.submesh(2).unwrap().material()) - 3.0).abs() < f32::EPSILON);

    let lod1 = mesh.lod(1).unwrap();
    assert!((material_value(&rm, lod1.submesh(0).unwrap().material()) - 1.0).abs() < f32::EPSILON);
    assert!((material_value(&rm, lod1.submesh(1).unwrap().material()) - 2.0).abs() < f32::EPSILON);
}

#[test]
fn test_lod_order_matches_geometry_mesh() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);

    // Provide LODs in REVERSE order (1, 0)
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
            ]},
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
            ]},
        ],
    }, &rm).unwrap();

    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

// ============================================================================
// Tests: Validation Errors
// ============================================================================

#[test]
fn test_duplicate_lod_index_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] },
            MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] },
        ],
    }, &rm).is_err());
}

#[test]
fn test_incomplete_lod_coverage_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);
    // "body" has 2 LODs but we only provide 1
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![
            SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
            SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
        ]}],
    }, &rm).is_err());
}

#[test]
fn test_lod_index_out_of_range_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 5, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).is_err());
}

#[test]
fn test_duplicate_submesh_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![
            SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk },
            SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk },
        ]}],
    }, &rm).is_err());
}

#[test]
fn test_incomplete_submesh_coverage_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc { lod_index: 0, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
            ]},
            MeshLODDesc { lod_index: 1, submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
            ]},
        ],
    }, &rm).is_err());
}

// ============================================================================
// Tests: Accessors
// ============================================================================

#[test]
fn test_mesh_accessors() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m42", 42.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk, geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc { lod_index: 0, submeshes: vec![SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk }] }],
    }, &rm).unwrap();

    assert_eq!(mesh.geometry(), gk);
    assert_eq!(mesh.geometry_mesh_id(), 1);
    assert_eq!(mesh.lod_count(), 1);
    assert!(mesh.lod(0).is_some());
    assert!(mesh.lod(1).is_none());

    let lod = mesh.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 1);
    assert!(lod.submesh(0).is_some());
    assert!(lod.submesh(1).is_none());

    let submesh = lod.submesh(0).unwrap();
    assert_eq!(submesh.submesh_id(), 0);
    assert_eq!(submesh.material(), mk);
}

// ============================================================================
// Tests: Helper - mesh_desc_from_name_mapping
// ============================================================================

#[test]
fn test_mesh_desc_from_name_mapping() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, "pants", 3.0);

    let mapping = FxHashMap::from_iter([
        ("head".to_string(), skin), ("torso".to_string(), armor),
        ("legs".to_string(), pants), ("body".to_string(), armor),
    ]);
    let desc = mesh_desc_from_name_mapping(gk, GeometryMeshRef::Name("body".to_string()), &mapping, &rm).unwrap();
    assert_eq!(desc.lods.len(), 2);
    let mesh = Mesh::from_desc(desc, &rm).unwrap();
    assert_eq!(mesh.lod_count(), 2);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

#[test]
fn test_mesh_desc_from_name_mapping_missing_material() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, "skin", 1.0);
    let mapping = FxHashMap::from_iter([("head".to_string(), skin)]);
    assert!(mesh_desc_from_name_mapping(gk, GeometryMeshRef::Name("body".to_string()), &mapping, &rm).is_err());
}

#[test]
fn test_mesh_desc_from_name_mapping_invalid_mesh_ref() {
    let (gk, rm, _gd) = create_test_resources();
    let mapping = FxHashMap::default();
    assert!(mesh_desc_from_name_mapping(gk, GeometryMeshRef::Name("nonexistent".to_string()), &mapping, &rm).is_err());
}

#[test]
fn test_mesh_desc_from_name_mapping_by_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let pk = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, "m", 1.0);
    let mapping = FxHashMap::from_iter([("blade".to_string(), mk)]);
    let desc = mesh_desc_from_name_mapping(gk, GeometryMeshRef::Index(1), &mapping, &rm).unwrap();
    let mesh = Mesh::from_desc(desc, &rm).unwrap();
    assert_eq!(mesh.lod_count(), 1);
    assert_eq!(mesh.geometry_mesh_id(), 1);
}
