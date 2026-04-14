/// Unit tests for geometry.rs
///
/// Tests the Geometry, GeometryMesh, GeometrySubMesh, and GeometrySubMeshLOD
/// hierarchy without requiring GPU. Uses MockGraphicsDevice for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::graphics_device;
#[cfg(test)]
use crate::resource::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
    GeometrySubMesh,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a mock graphics_device for testing
fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    let graphics_device = graphics_device::mock_graphics_device::MockGraphicsDevice::new();
    Arc::new(Mutex::new(graphics_device))
}

/// Create a simple vertex layout (Position2D)
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

/// Create vertex data for 4 vertices (quad)
fn create_quad_vertex_data() -> Vec<u8> {
    let vertices: Vec<f32> = vec![
        0.0, 0.0,
        1.0, 0.0,
        1.0, 1.0,
        0.0, 1.0,
    ];
    vertices.iter().flat_map(|&f| f.to_le_bytes()).collect()
}

/// Create index data for quad (2 triangles)
fn create_quad_index_data_u16() -> Vec<u8> {
    let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
    indices.iter().flat_map(|&i| i.to_le_bytes()).collect()
}

/// Create a simple submesh LOD descriptor (covers all 4 vertices / 6 indices)
fn make_quad_lod_desc() -> GeometrySubMeshLODDesc {
    GeometrySubMeshLODDesc {
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: graphics_device::PrimitiveTopology::TriangleList,
    }
}

/// Create a simple submesh descriptor with a single LOD
fn make_quad_submesh_desc(name: &str) -> GeometrySubMeshDesc {
    GeometrySubMeshDesc {
        name: name.to_string(),
        lods: vec![make_quad_lod_desc()],
    }
}

// ============================================================================
// GEOMETRY CREATION TESTS
// ============================================================================

#[test]
fn test_create_geometry_indexed() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    assert_eq!(geom.name(), "test_geom");
    assert_eq!(geom.total_vertex_count(), 4);
    assert_eq!(geom.total_index_count(), 6);
    assert!(geom.is_indexed());
    assert_eq!(geom.index_type(), graphics_device::IndexType::U16);
}

#[test]
fn test_create_geometry_non_indexed() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: None,
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    assert_eq!(geom.name(), "test_geom");
    assert_eq!(geom.total_vertex_count(), 4);
    assert_eq!(geom.total_index_count(), 0);
    assert!(!geom.is_indexed());
    assert!(geom.index_buffer().is_none());
}

#[test]
fn test_create_geometry_with_mesh() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![make_quad_submesh_desc("quad")],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    assert_eq!(geom.mesh_count(), 1);
    assert!(geom.mesh_by_name("hero").is_some());
}

#[test]
fn test_create_geometry_invalid_vertex_stride() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![1, 2, 3],
        index_data: None,
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let result = Geometry::from_desc(desc, 0);
    assert!(result.is_err());
}

#[test]
fn test_create_geometry_invalid_index_stride() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(vec![1, 2, 3]),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let result = Geometry::from_desc(desc, 0);
    assert!(result.is_err());
}

// ============================================================================
// GEOMETRY MESH TESTS
// ============================================================================

#[test]
fn test_add_mesh() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        submeshes: vec![make_quad_submesh_desc("quad")],
    };

    let mesh_id = geom.add_mesh(mesh_desc).unwrap();

    assert_eq!(mesh_id, 0);
    assert_eq!(geom.mesh_count(), 1);
    assert_eq!(geom.mesh_names(), vec!["hero"]);
}

#[test]
fn test_get_mesh_by_name() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh = geom.mesh_by_name("hero");
    assert!(mesh.is_some());

    let mesh = geom.mesh_by_name("nonexistent");
    assert!(mesh.is_none());
}

#[test]
fn test_get_mesh_by_id() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_id = geom.mesh_id("hero");
    assert_eq!(mesh_id, Some(0));

    let mesh = geom.mesh(0);
    assert!(mesh.is_some());
}

#[test]
fn test_add_duplicate_mesh() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        submeshes: vec![],
    };

    let result = geom.add_mesh(mesh_desc);
    assert!(result.is_err());
}

// ============================================================================
// SUBMESH TESTS
// ============================================================================

#[test]
fn test_add_submesh_to_mesh() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_id = geom.mesh_id("hero").unwrap();
    let submesh_id = geom.add_submesh(mesh_id, make_quad_submesh_desc("body")).unwrap();
    assert_eq!(submesh_id, 0);

    let mesh = geom.mesh(mesh_id).unwrap();
    assert_eq!(mesh.submesh_count(), 1);
    assert!(mesh.submesh_by_name("body").is_some());
}

#[test]
fn test_add_submesh_lod_to_existing_submesh() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![make_quad_submesh_desc("body")],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_id = geom.mesh_id("hero").unwrap();
    let submesh_id = geom.mesh(mesh_id).unwrap().submesh_id("body").unwrap();

    // Initial: 1 LOD
    assert_eq!(geom.mesh(mesh_id).unwrap().submesh(submesh_id).unwrap().lod_count(), 1);

    // Add a second LOD
    let new_lod_index = geom.add_submesh_lod(mesh_id, submesh_id, make_quad_lod_desc()).unwrap();
    assert_eq!(new_lod_index, 1);
    assert_eq!(geom.mesh(mesh_id).unwrap().submesh(submesh_id).unwrap().lod_count(), 2);
}

#[test]
fn test_submesh_lod_accessors() {
    let lod_desc = GeometrySubMeshLODDesc {
        vertex_offset: 10,
        vertex_count: 20,
        index_offset: 5,
        index_count: 30,
        topology: graphics_device::PrimitiveTopology::TriangleStrip,
    };

    let graphics_device = create_mock_graphics_device();
    let vertex_data = vec![0u8; 30 * 8];
    let index_data = vec![0u8; 35 * 2];

    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![GeometrySubMeshDesc {
                    name: "test".to_string(),
                    lods: vec![lod_desc],
                }],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let submesh = geom.submesh_by_name("hero", "test").unwrap();
    let lod0 = submesh.lod(0).unwrap();

    assert_eq!(lod0.vertex_offset(), 10);
    assert_eq!(lod0.vertex_count(), 20);
    assert_eq!(lod0.index_offset(), 5);
    assert_eq!(lod0.index_count(), 30);
    assert_eq!(lod0.topology(), graphics_device::PrimitiveTopology::TriangleStrip);
}

#[test]
fn test_submesh_validation_vertex_overflow() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        submeshes: vec![GeometrySubMeshDesc {
            name: "invalid".to_string(),
            lods: vec![GeometrySubMeshLODDesc {
                vertex_offset: 0,
                vertex_count: 10, // exceeds total_vertex_count (4)
                index_offset: 0,
                index_count: 6,
                topology: graphics_device::PrimitiveTopology::TriangleList,
            }],
        }],
    };

    let result = geom.add_mesh(mesh_desc);
    assert!(result.is_err());
}

#[test]
fn test_submesh_validation_index_overflow() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let mut geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        submeshes: vec![GeometrySubMeshDesc {
            name: "invalid".to_string(),
            lods: vec![GeometrySubMeshLODDesc {
                vertex_offset: 0,
                vertex_count: 4,
                index_offset: 0,
                index_count: 20, // exceeds total_index_count (6)
                topology: graphics_device::PrimitiveTopology::TriangleList,
            }],
        }],
    };

    let result = geom.add_mesh(mesh_desc);
    assert!(result.is_err());
}

// ============================================================================
// LOOKUP TESTS
// ============================================================================

#[test]
fn test_submesh_lookup_by_name() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![make_quad_submesh_desc("body")],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();

    let submesh_id = mesh.submesh_id("body");
    assert_eq!(submesh_id, Some(0));

    let submesh = mesh.submesh_by_name("body");
    assert!(submesh.is_some());

    let submesh_names = mesh.submesh_names();
    assert_eq!(submesh_names, vec!["body"]);
}

#[test]
fn test_multiple_submeshes() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "body".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 0, vertex_count: 2,
                            index_offset: 0, index_count: 3,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    },
                    GeometrySubMeshDesc {
                        name: "armor".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 2, vertex_count: 2,
                            index_offset: 3, index_count: 3,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    },
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();

    assert_eq!(mesh.submesh_count(), 2);
    assert!(mesh.submesh_by_name("body").is_some());
    assert!(mesh.submesh_by_name("armor").is_some());
}

// ============================================================================
// COMPLEX HIERARCHY TESTS
// ============================================================================

#[test]
fn test_complex_geometry_hierarchy() {
    let graphics_device = create_mock_graphics_device();

    let vertex_data = vec![0u8; 100 * 8];
    let index_data = vec![0u8; 150 * 2];

    let desc = GeometryDesc {
        name: "characters".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![
                    // body has 2 LODs
                    GeometrySubMeshDesc {
                        name: "body".to_string(),
                        lods: vec![
                            GeometrySubMeshLODDesc {
                                vertex_offset: 0, vertex_count: 10,
                                index_offset: 0, index_count: 15,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshLODDesc {
                                vertex_offset: 0, vertex_count: 8,
                                index_offset: 0, index_count: 12,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                    // armor has only 1 LOD (disappears at lower LOD)
                    GeometrySubMeshDesc {
                        name: "armor".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 10, vertex_count: 5,
                            index_offset: 15, index_count: 9,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    },
                ],
            },
            GeometryMeshDesc {
                name: "enemy".to_string(),
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "body".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 20, vertex_count: 12,
                            index_offset: 30, index_count: 18,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    assert_eq!(geom.mesh_count(), 2);
    assert_eq!(geom.mesh_names().len(), 2);

    // Hero
    let hero = geom.mesh_by_name("hero").unwrap();
    assert_eq!(hero.submesh_count(), 2);
    assert_eq!(hero.submesh_by_name("body").unwrap().lod_count(), 2);
    assert_eq!(hero.submesh_by_name("armor").unwrap().lod_count(), 1);

    // Enemy
    let enemy = geom.mesh_by_name("enemy").unwrap();
    assert_eq!(enemy.submesh_count(), 1);
    assert_eq!(enemy.submesh_by_name("body").unwrap().lod_count(), 1);
}

// ============================================================================
// GETTER TESTS
// ============================================================================

#[test]
fn test_geometry_getters() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    assert!(Arc::ptr_eq(&geom.graphics_device(), &graphics_device));
    let vb = geom.vertex_buffer();
    assert!(Arc::strong_count(vb) >= 1);

    let layout = geom.vertex_layout();
    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.bindings[0].stride, 8);

    assert_eq!(geom.total_vertex_count(), 4);
    assert_eq!(geom.total_index_count(), 6);
}

#[test]
fn test_submesh_by_id_path() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "body".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 0, vertex_count: 2,
                            index_offset: 0, index_count: 3,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    },
                    GeometrySubMeshDesc {
                        name: "armor".to_string(),
                        lods: vec![GeometrySubMeshLODDesc {
                            vertex_offset: 2, vertex_count: 2,
                            index_offset: 3, index_count: 3,
                            topology: graphics_device::PrimitiveTopology::TriangleList,
                        }],
                    },
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh_id = geom.mesh_id("hero").unwrap();
    assert_eq!(mesh_id, 0);

    // submesh(mesh_id, submesh_id)
    let body = geom.submesh(mesh_id, 0).unwrap();
    assert_eq!(body.lod_count(), 1);
    assert_eq!(body.lod(0).unwrap().vertex_offset(), 0);

    let armor = geom.submesh(mesh_id, 1).unwrap();
    assert_eq!(armor.lod(0).unwrap().vertex_offset(), 2);

    // submesh_lod helper
    let body_lod0 = geom.submesh_lod(mesh_id, 0, 0).unwrap();
    assert_eq!(body_lod0.vertex_offset(), 0);

    // Invalid ids
    assert!(geom.submesh(mesh_id, 99).is_none());
    assert!(geom.submesh(99, 0).is_none());
    assert!(geom.submesh_lod(mesh_id, 0, 99).is_none());
}

#[test]
fn test_submeshes_iterator() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "hero".to_string(),
                submeshes: vec![
                    make_quad_submesh_desc("body"),
                    make_quad_submesh_desc("armor"),
                    make_quad_submesh_desc("weapon"),
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc, 0).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();

    let submesh_vec: Vec<(&str, &GeometrySubMesh)> = mesh.submeshes().collect();
    assert_eq!(submesh_vec.len(), 3);

    let names: Vec<&str> = submesh_vec.iter().map(|(name, _)| *name).collect();
    assert!(names.contains(&"body"));
    assert!(names.contains(&"armor"));
    assert!(names.contains(&"weapon"));

    for (name, submesh) in mesh.submeshes() {
        assert!(matches!(name, "body" | "armor" | "weapon"));
        let lod0 = submesh.lod(0).unwrap();
        assert_eq!(lod0.vertex_count(), 4);
        assert_eq!(lod0.index_count(), 6);
    }
}
