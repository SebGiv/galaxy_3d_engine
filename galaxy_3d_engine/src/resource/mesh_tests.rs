/// Tests for Mesh resource
///
/// These tests use MockGraphicsDevice and ResourceManager to create real Geometry,
/// Pipeline, and Material resources via SlotMap keys, then validate Mesh creation,
/// ref resolution, ordering, and error handling.

use super::*;
use crate::graphics_device;
use crate::resource::geometry::{
    GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::material::{MaterialDesc, MaterialPassDesc, ParamValue};
use crate::graphics_device::PolygonMode;
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

/// Build a simple LOD desc with given offsets/counts.
fn lod(vertex_offset: u32, vertex_count: u32, index_offset: u32, index_count: u32) -> GeometrySubMeshLODDesc {
    GeometrySubMeshLODDesc {
        vertex_offset,
        vertex_count,
        index_offset,
        index_count,
        topology: graphics_device::PrimitiveTopology::TriangleList,
    }
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

    // 20 vertices (160 bytes / 8 stride), 24 indices (48 bytes / 2 bytes for U16)
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
                // 3 submeshes — each with its own LOD chain.
                // "head" has 2 LODs, "torso" has 2 LODs, "legs" has 1 LOD
                // (illustrating per-submesh LOD count variability).
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "head".to_string(),
                        lods: vec![
                            lod(0, 4, 0, 6),
                            lod(0, 4, 0, 6),
                        ],
                    },
                    GeometrySubMeshDesc {
                        name: "torso".to_string(),
                        lods: vec![
                            lod(4, 4, 6, 6),
                            lod(4, 4, 6, 6),
                        ],
                    },
                    GeometrySubMeshDesc {
                        name: "legs".to_string(),
                        lods: vec![
                            lod(8, 4, 12, 6),
                        ],
                    },
                ],
            },
            GeometryMeshDesc {
                name: "weapon".to_string(),
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "blade".to_string(),
                        lods: vec![
                            lod(12, 4, 18, 6),
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

fn create_test_pipeline(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, name: &str) -> (PipelineKey, ShaderKey) {
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
    let pk = rm.create_pipeline(name.to_string(), desc, &mut *gd.lock().unwrap()).unwrap();
    (pk, fk)
}

fn create_test_material(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>, _pipeline: PipelineKey, fragment_shader: ShaderKey, name: &str, value: f32) -> MaterialKey {
    rm.create_material(name.to_string(), MaterialDesc {
        passes: vec![MaterialPassDesc {
            pass_type: 0,
            fragment_shader, color_blend: Default::default(), polygon_mode: PolygonMode::Fill, textures: vec![],
            params: vec![("value".to_string(), ParamValue::Float(value))],
            render_state: None,
        }],
    }, &*gd.lock().unwrap()).unwrap()
}

fn material_value(rm: &ResourceManager, key: MaterialKey) -> f32 {
    rm.material(key).unwrap().pass(0).unwrap().param_by_name("value").unwrap().as_float().unwrap()
}

// ============================================================================
// Tests: Basic Mesh Creation
// ============================================================================

#[test]
fn test_create_mesh_single_submesh() {
    let (geom_key, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: geom_key,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("blade".to_string()),
            material: mk,
        }],
    }, &rm).unwrap();
    assert_eq!(mesh.submesh_count(), 1);
    assert_eq!(mesh.submesh(0).unwrap().material(), mk);
}

#[test]
fn test_create_mesh_multi_submesh() {
    let (geom_key, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, fk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, fk, "pants", 3.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: geom_key,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        submeshes: vec![
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
        ],
    }, &rm).unwrap();
    assert_eq!(mesh.submesh_count(), 3);
    assert_eq!(mesh.submesh(0).unwrap().material(), skin);
    assert_eq!(mesh.submesh(1).unwrap().material(), armor);
    assert_eq!(mesh.submesh(2).unwrap().material(), pants);
}

// ============================================================================
// Tests: GeometryMeshRef Resolution
// ============================================================================

#[test]
fn test_geometry_mesh_ref_by_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("blade".to_string()),
            material: mk,
        }],
    }, &rm).unwrap();
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_by_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Index(1),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Index(0),
            material: mk,
        }],
    }, &rm).unwrap();
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_invalid_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("nonexistent".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("blade".to_string()),
            material: mk,
        }],
    }, &rm).is_err());
}

#[test]
fn test_geometry_mesh_ref_invalid_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Index(99),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Index(0),
            material: mk,
        }],
    }, &rm).is_err());
}

// ============================================================================
// Tests: GeometrySubMeshRef Resolution
// ============================================================================

#[test]
fn test_submesh_ref_by_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("blade".to_string()),
            material: mk,
        }],
    }, &rm).unwrap();
    assert_eq!(mesh.submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_by_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Index(0),
            material: mk,
        }],
    }, &rm).unwrap();
    assert_eq!(mesh.submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_invalid_name() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, fk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, fk, "pants", 3.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        submeshes: vec![
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("nonexistent".to_string()), material: armor },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
        ],
    }, &rm).is_err());
}

#[test]
fn test_submesh_ref_invalid_index() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Index(99),
            material: mk,
        }],
    }, &rm).is_err());
}

// ============================================================================
// Tests: Submesh Ordering
// ============================================================================

#[test]
fn test_submesh_order_matches_geometry_mesh() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, fk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, fk, "pants", 3.0);

    // Provide submeshes in REVERSE order — Mesh::from_desc must reorder
    // them to match the parent GeometryMesh submesh order (head, torso, legs).
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        submeshes: vec![
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
        ],
    }, &rm).unwrap();

    assert_eq!(mesh.submesh(0).unwrap().submesh_id(), 0);
    assert_eq!(mesh.submesh(1).unwrap().submesh_id(), 1);
    assert_eq!(mesh.submesh(2).unwrap().submesh_id(), 2);
    assert!((material_value(&rm, mesh.submesh(0).unwrap().material()) - 1.0).abs() < f32::EPSILON);
    assert!((material_value(&rm, mesh.submesh(1).unwrap().material()) - 2.0).abs() < f32::EPSILON);
    assert!((material_value(&rm, mesh.submesh(2).unwrap().material()) - 3.0).abs() < f32::EPSILON);
}

// ============================================================================
// Tests: Validation Errors
// ============================================================================

#[test]
fn test_duplicate_submesh_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mk },
        ],
    }, &rm).is_err());
}

#[test]
fn test_incomplete_submesh_coverage_fails() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, fk, "armor", 2.0);
    // "body" has 3 submeshes but we only provide 2
    assert!(Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        submeshes: vec![
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
            MeshSubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
        ],
    }, &rm).is_err());
}

// ============================================================================
// Tests: Accessors
// ============================================================================

#[test]
fn test_mesh_accessors() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m42", 42.0);
    let mesh = Mesh::from_desc(MeshDesc {
        geometry: gk,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("blade".to_string()),
            material: mk,
        }],
    }, &rm).unwrap();

    assert_eq!(mesh.geometry(), gk);
    assert_eq!(mesh.geometry_mesh_id(), 1);
    assert_eq!(mesh.submesh_count(), 1);
    assert!(mesh.submesh(0).is_some());
    assert!(mesh.submesh(1).is_none());

    let submesh = mesh.submesh(0).unwrap();
    assert_eq!(submesh.submesh_id(), 0);
    assert_eq!(submesh.material(), mk);
}

// ============================================================================
// Tests: Helper - mesh_desc_from_name_mapping
// ============================================================================

#[test]
fn test_mesh_desc_from_name_mapping() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
    let armor = create_test_material(&mut rm, &gd, pk, fk, "armor", 2.0);
    let pants = create_test_material(&mut rm, &gd, pk, fk, "pants", 3.0);

    let mapping = FxHashMap::from_iter([
        ("head".to_string(), skin),
        ("torso".to_string(), armor),
        ("legs".to_string(), pants),
    ]);
    let desc = mesh_desc_from_name_mapping(gk, GeometryMeshRef::Name("body".to_string()), &mapping, &rm).unwrap();
    assert_eq!(desc.submeshes.len(), 3);
    let mesh = Mesh::from_desc(desc, &rm).unwrap();
    assert_eq!(mesh.submesh_count(), 3);
}

#[test]
fn test_mesh_desc_from_name_mapping_missing_material() {
    let (gk, mut rm, gd) = create_test_resources();
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let skin = create_test_material(&mut rm, &gd, pk, fk, "skin", 1.0);
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
    let (pk, fk) = create_test_pipeline(&mut rm, &gd, "p");
    let mk = create_test_material(&mut rm, &gd, pk, fk, "m", 1.0);
    let mapping = FxHashMap::from_iter([("blade".to_string(), mk)]);
    let desc = mesh_desc_from_name_mapping(gk, GeometryMeshRef::Index(1), &mapping, &rm).unwrap();
    let mesh = Mesh::from_desc(desc, &rm).unwrap();
    assert_eq!(mesh.submesh_count(), 1);
    assert_eq!(mesh.geometry_mesh_id(), 1);
}
