/// Tests for Mesh resource
///
/// These tests use MockGraphicsDevice to create real Geometry, Pipeline, and Material
/// resources, then validate Mesh creation, ref resolution, ordering, and error handling.

use super::*;
use crate::graphics_device;
use crate::resource::geometry::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::{Pipeline, PipelineDesc, PipelineVariantDesc, PipelinePassDesc};
use crate::resource::material::{Material, MaterialDesc, ParamValue};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()))
}

/// Create a geometry with:
/// - GeometryMesh "body": LOD 0 (head, torso, legs), LOD 1 (head, body)
/// - GeometryMesh "weapon": LOD 0 (blade)
fn create_test_geometry(graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>) -> Arc<Geometry> {
    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding {
            binding: 0,
            stride: 8,
            input_rate: graphics_device::VertexInputRate::Vertex,
        }],
        attributes: vec![graphics_device::VertexAttribute {
            location: 0,
            binding: 0,
            format: graphics_device::BufferFormat::R32G32_SFLOAT,
            offset: 0,
        }],
    };

    let desc = GeometryDesc {
        name: "characters".to_string(),
        graphics_device,
        vertex_data: vec![0u8; 160],  // 20 vertices * 8 bytes
        index_data: Some(vec![0u8; 48]), // 24 indices * 2 bytes
        vertex_layout,
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "body".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "head".to_string(),
                                vertex_offset: 0, vertex_count: 4,
                                index_offset: 0, index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "torso".to_string(),
                                vertex_offset: 4, vertex_count: 4,
                                index_offset: 6, index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "legs".to_string(),
                                vertex_offset: 8, vertex_count: 4,
                                index_offset: 12, index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "head".to_string(),
                                vertex_offset: 0, vertex_count: 4,
                                index_offset: 0, index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 4, vertex_count: 6,
                                index_offset: 6, index_count: 12,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
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
                            GeometrySubMeshDesc {
                                name: "blade".to_string(),
                                vertex_offset: 12, vertex_count: 4,
                                index_offset: 18, index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                ],
            },
        ],
    };
    Arc::new(Geometry::from_desc(desc).unwrap())
}

fn create_test_pipeline(graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>) -> Arc<Pipeline> {
    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding {
            binding: 0, stride: 8, input_rate: graphics_device::VertexInputRate::Vertex,
        }],
        attributes: vec![graphics_device::VertexAttribute {
            location: 0, binding: 0,
            format: graphics_device::BufferFormat::R32G32_SFLOAT, offset: 0,
        }],
    };

    let desc = PipelineDesc {
        graphics_device,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: graphics_device::PipelineDesc {
                    vertex_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("vert".to_string())),
                    fragment_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("frag".to_string())),
                    vertex_layout,
                    topology: graphics_device::PrimitiveTopology::TriangleList,
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

/// Create a material with a distinguishing param value
fn create_test_material(pipeline: &Arc<Pipeline>, value: f32) -> Arc<Material> {
    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![("value".to_string(), ParamValue::Float(value))],
    };
    Arc::new(Material::from_desc(0, desc).unwrap())
}

/// Extract the "value" param from a material as f32
fn material_value(material: &Arc<Material>) -> f32 {
    material.param_by_name("value").unwrap().as_float().unwrap()
}

// ============================================================================
// Tests: Basic Mesh Creation
// ============================================================================

#[test]
fn test_create_mesh_single_lod_single_submesh() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod_count(), 1);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 1);
    assert!(Arc::ptr_eq(mesh.lod(0).unwrap().submesh(0).unwrap().material(), &mat));
}

#[test]
fn test_create_mesh_multi_lod() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);
    let pants = create_test_material(&pipeline, 3.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants.clone() },
                ],
            },
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor.clone() },
                ],
            },
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod_count(), 2);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

// ============================================================================
// Tests: GeometryMeshRef Resolution
// ============================================================================

#[test]
fn test_geometry_mesh_ref_by_name() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    // "weapon" is the second mesh (id=1)
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_by_index() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Index(1), // "weapon"
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Index(0), // "blade"
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.geometry_mesh_id(), 1);
}

#[test]
fn test_geometry_mesh_ref_invalid_name() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("nonexistent".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat,
            }],
        }],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_geometry_mesh_ref_invalid_index() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Index(99),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Index(0),
                material: mat,
            }],
        }],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

// ============================================================================
// Tests: GeometrySubMeshRef Resolution
// ============================================================================

#[test]
fn test_submesh_ref_by_name() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod(0).unwrap().submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_by_index() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Index(0),
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod(0).unwrap().submesh(0).unwrap().submesh_id(), 0);
}

#[test]
fn test_submesh_ref_invalid_name() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);
    let pants = create_test_material(&pipeline, 3.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("nonexistent".to_string()), material: armor.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants.clone() },
                ],
            },
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
                ],
            },
        ],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_submesh_ref_invalid_index() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Index(99),
                material: mat,
            }],
        }],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

// ============================================================================
// Tests: Submesh Ordering
// ============================================================================

#[test]
fn test_submesh_order_matches_geometry_lod() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());

    // Create materials with distinct values
    let skin = create_test_material(&pipeline, 1.0);   // for "head"
    let armor = create_test_material(&pipeline, 2.0);   // for "torso"
    let pants = create_test_material(&pipeline, 3.0);   // for "legs"

    // Provide submeshes in REVERSE order (legs, torso, head)
    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor.clone() },
                ],
            },
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                ],
            },
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // LOD 0: GeometryLOD submeshes are [head=0, torso=1, legs=2]
    // Despite providing them as [legs, head, torso], the result should be ordered
    let lod0 = mesh.lod(0).unwrap();
    assert_eq!(lod0.submesh(0).unwrap().submesh_id(), 0); // head
    assert_eq!(lod0.submesh(1).unwrap().submesh_id(), 1); // torso
    assert_eq!(lod0.submesh(2).unwrap().submesh_id(), 2); // legs

    // Verify materials match the correct submesh
    assert!((material_value(lod0.submesh(0).unwrap().material()) - 1.0).abs() < f32::EPSILON); // head → skin
    assert!((material_value(lod0.submesh(1).unwrap().material()) - 2.0).abs() < f32::EPSILON); // torso → armor
    assert!((material_value(lod0.submesh(2).unwrap().material()) - 3.0).abs() < f32::EPSILON); // legs → pants

    // LOD 1: GeometryLOD submeshes are [head=0, body=1]
    let lod1 = mesh.lod(1).unwrap();
    assert!((material_value(lod1.submesh(0).unwrap().material()) - 1.0).abs() < f32::EPSILON); // head → skin
    assert!((material_value(lod1.submesh(1).unwrap().material()) - 2.0).abs() < f32::EPSILON); // body → armor
}

#[test]
fn test_lod_order_matches_geometry_mesh() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);
    let pants = create_test_material(&pipeline, 3.0);

    // Provide LODs in REVERSE order (1, 0)
    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor.clone() },
                ],
            },
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants.clone() },
                ],
            },
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // Despite providing LODs as [1, 0], mesh.lod(0) should be LOD 0 (3 submeshes)
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

// ============================================================================
// Tests: Validation Errors
// ============================================================================

#[test]
fn test_duplicate_lod_index_fails() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![SubMeshDesc {
                    submesh: GeometrySubMeshRef::Name("blade".to_string()),
                    material: mat.clone(),
                }],
            },
            MeshLODDesc {
                lod_index: 0, // duplicate
                submeshes: vec![SubMeshDesc {
                    submesh: GeometrySubMeshRef::Name("blade".to_string()),
                    material: mat,
                }],
            },
        ],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_incomplete_lod_coverage_fails() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);
    let pants = create_test_material(&pipeline, 3.0);

    // "body" has LOD 0 and LOD 1, but we only provide LOD 0
    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("legs".to_string()), material: pants },
                ],
            },
        ],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_lod_index_out_of_range_fails() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    // "weapon" only has LOD 0, but we specify LOD 5
    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 5,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat,
            }],
        }],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_duplicate_submesh_fails() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mat.clone() },
                SubMeshDesc { submesh: GeometrySubMeshRef::Name("blade".to_string()), material: mat }, // duplicate
            ],
        }],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

#[test]
fn test_incomplete_submesh_coverage_fails() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);

    // "body" LOD 0 has 3 submeshes (head, torso, legs) but we only provide 2
    let desc = MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Name("body".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin.clone() },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("torso".to_string()), material: armor.clone() },
                    // "legs" missing
                ],
            },
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("head".to_string()), material: skin },
                    SubMeshDesc { submesh: GeometrySubMeshRef::Name("body".to_string()), material: armor },
                ],
            },
        ],
    };

    assert!(Mesh::from_desc(desc).is_err());
}

// ============================================================================
// Tests: Accessors
// ============================================================================

#[test]
fn test_mesh_accessors() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 42.0);

    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("weapon".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("blade".to_string()),
                material: mat.clone(),
            }],
        }],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // Mesh accessors
    assert!(Arc::ptr_eq(mesh.geometry(), &geometry));
    assert_eq!(mesh.geometry_mesh_id(), 1); // "weapon" = id 1
    assert_eq!(mesh.geometry_mesh().lod_count(), 1);
    assert_eq!(mesh.lod_count(), 1);
    assert!(mesh.lod(0).is_some());
    assert!(mesh.lod(1).is_none());

    // MeshLOD accessors
    let lod = mesh.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 1);
    assert!(lod.submesh(0).is_some());
    assert!(lod.submesh(1).is_none());

    // SubMesh accessors
    let submesh = lod.submesh(0).unwrap();
    assert_eq!(submesh.submesh_id(), 0);
    assert!(Arc::ptr_eq(submesh.material(), &mat));
}

// ============================================================================
// Tests: Helper - mesh_desc_from_name_mapping
// ============================================================================

#[test]
fn test_mesh_desc_from_name_mapping() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());

    let skin = create_test_material(&pipeline, 1.0);
    let armor = create_test_material(&pipeline, 2.0);
    let pants = create_test_material(&pipeline, 3.0);

    let mapping = FxHashMap::from_iter([
        ("head".to_string(), skin.clone()),
        ("torso".to_string(), armor.clone()),
        ("legs".to_string(), pants.clone()),
        ("body".to_string(), armor.clone()), // for LOD 1 merged submesh
    ]);

    let desc = mesh_desc_from_name_mapping(
        &geometry,
        GeometryMeshRef::Name("body".to_string()),
        &mapping,
    ).unwrap();

    // Verify the desc was built correctly
    assert_eq!(desc.lods.len(), 2);
    assert_eq!(desc.lods[0].lod_index, 0);
    assert_eq!(desc.lods[0].submeshes.len(), 3);
    assert_eq!(desc.lods[1].lod_index, 1);
    assert_eq!(desc.lods[1].submeshes.len(), 2);

    // Create the Mesh from the generated desc (validates it works end-to-end)
    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod_count(), 2);
    assert_eq!(mesh.lod(0).unwrap().submesh_count(), 3);
    assert_eq!(mesh.lod(1).unwrap().submesh_count(), 2);
}

#[test]
fn test_mesh_desc_from_name_mapping_missing_material() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());

    let skin = create_test_material(&pipeline, 1.0);

    // Only provide "head", missing "torso" and "legs"
    let mapping = FxHashMap::from_iter([
        ("head".to_string(), skin),
    ]);

    let result = mesh_desc_from_name_mapping(
        &geometry,
        GeometryMeshRef::Name("body".to_string()),
        &mapping,
    );

    assert!(result.is_err());
}

#[test]
fn test_mesh_desc_from_name_mapping_invalid_mesh_ref() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());

    let mapping = FxHashMap::default();

    let result = mesh_desc_from_name_mapping(
        &geometry,
        GeometryMeshRef::Name("nonexistent".to_string()),
        &mapping,
    );

    assert!(result.is_err());
}

#[test]
fn test_mesh_desc_from_name_mapping_by_index() {
    let graphics_device = create_mock_graphics_device();
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let mat = create_test_material(&pipeline, 1.0);

    let mapping = FxHashMap::from_iter([
        ("blade".to_string(), mat),
    ]);

    let desc = mesh_desc_from_name_mapping(
        &geometry,
        GeometryMeshRef::Index(1), // "weapon"
        &mapping,
    ).unwrap();

    let mesh = Mesh::from_desc(desc).unwrap();
    assert_eq!(mesh.lod_count(), 1);
    assert_eq!(mesh.geometry_mesh_id(), 1);
}
