/// Unit tests for geometry.rs
///
/// Tests the Geometry, GeometryMesh, GeometryLOD, and GeometrySubMesh hierarchy without requiring GPU.
/// Uses MockGraphicsDevice for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::graphics_device;
#[cfg(test)]
use crate::resource::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc, GeometrySubMesh,
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
                stride: 8, // 2 floats (x, y) = 8 bytes
                input_rate: graphics_device::VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            graphics_device::VertexAttribute {
                location: 0,
                binding: 0,
                format: graphics_device::BufferFormat::R32G32_SFLOAT, // vec2 of floats
                offset: 0,
            }
        ],
    }
}

/// Create vertex data for 4 vertices (quad)
fn create_quad_vertex_data() -> Vec<u8> {
    // 4 vertices with 2 floats each (x, y)
    // Vertex 0: (0.0, 0.0)
    // Vertex 1: (1.0, 0.0)
    // Vertex 2: (1.0, 1.0)
    // Vertex 3: (0.0, 1.0)
    let vertices: Vec<f32> = vec![
        0.0, 0.0,
        1.0, 0.0,
        1.0, 1.0,
        0.0, 1.0,
    ];

    vertices.iter()
        .flat_map(|&f| f.to_le_bytes())
        .collect()
}

/// Create index data for quad (2 triangles)
fn create_quad_index_data_u16() -> Vec<u8> {
    let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
    indices.iter()
        .flat_map(|&i| i.to_le_bytes())
        .collect()
}

/// Create a simple submesh descriptor
fn create_simple_submesh_desc() -> GeometrySubMeshDesc {
    GeometrySubMeshDesc {
        name: "quad".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: graphics_device::PrimitiveTopology::TriangleList,
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

    let geom = Geometry::from_desc(desc).unwrap();

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
        index_data: None, // Non-indexed
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let geom = Geometry::from_desc(desc).unwrap();

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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![create_simple_submesh_desc()],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    assert_eq!(geom.mesh_count(), 1);
    assert!(geom.mesh_by_name("hero").is_some());
}

#[test]
fn test_create_geometry_invalid_vertex_stride() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![1, 2, 3], // 3 bytes, not divisible by stride 8
        index_data: None,
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let result = Geometry::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_create_geometry_invalid_index_stride() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(vec![1, 2, 3]), // 3 bytes, not divisible by u16 size (2)
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let result = Geometry::from_desc(desc);
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

    let mut geom = Geometry::from_desc(desc).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        lods: vec![
            GeometryLODDesc {
                lod_index: 0,
                submeshes: vec![create_simple_submesh_desc()],
            }
        ],
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
                lods: vec![],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

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
                lods: vec![],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

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
                lods: vec![],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(), // Duplicate
        lods: vec![],
    };

    let result = geom.add_mesh(mesh_desc);
    assert!(result.is_err());
}

// ============================================================================
// LOD TESTS
// ============================================================================

#[test]
fn test_add_lod() {
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
                lods: vec![],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc).unwrap();

    let lod_desc = GeometryLODDesc {
        lod_index: 0,
        submeshes: vec![create_simple_submesh_desc()],
    };

    let mesh_id = geom.mesh_id("hero").unwrap();
    let lod_index = geom.add_lod(mesh_id, lod_desc).unwrap();

    assert_eq!(lod_index, 0);

    let mesh = geom.mesh(mesh_id).unwrap();
    assert_eq!(mesh.lod_count(), 1);
}

#[test]
fn test_multiple_lods() {
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![create_simple_submesh_desc()],
                    },
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![create_simple_submesh_desc()],
                    },
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();
    assert_eq!(mesh.lod_count(), 2);
    assert!(mesh.lod(0).is_some());
    assert!(mesh.lod(1).is_some());
}

// ============================================================================
// SUBMESH TESTS
// ============================================================================

#[test]
fn test_submesh_accessors() {
    let submesh_desc = GeometrySubMeshDesc {
        name: "test".to_string(),
        vertex_offset: 10,
        vertex_count: 20,
        index_offset: 5,
        index_count: 30,
        topology: graphics_device::PrimitiveTopology::TriangleStrip,
    };

    let graphics_device = create_mock_graphics_device();
    // Create geometry with enough vertices/indices to accommodate submesh
    let vertex_data = vec![0u8; 30 * 8]; // 30 vertices * 8 bytes stride
    let index_data = vec![0u8; 35 * 2]; // 35 indices * 2 bytes (u16)

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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![submesh_desc],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    let submesh = geom.submesh_by_name("hero", 0, "test").unwrap();

    assert_eq!(submesh.vertex_offset(), 10);
    assert_eq!(submesh.vertex_count(), 20);
    assert_eq!(submesh.index_offset(), 5);
    assert_eq!(submesh.index_count(), 30);
    assert_eq!(submesh.topology(), graphics_device::PrimitiveTopology::TriangleStrip);
}

#[test]
fn test_add_submesh() {
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![],
                    }
                ],
            }
        ],
    };

    let mut geom = Geometry::from_desc(desc).unwrap();

    let submesh_desc = GeometrySubMeshDesc {
        name: "body".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: graphics_device::PrimitiveTopology::TriangleList,
    };

    let mesh_id = geom.mesh_id("hero").unwrap();
    let submesh_id = geom.add_submesh(mesh_id, 0, submesh_desc).unwrap();

    assert_eq!(submesh_id, 0);

    let mesh = geom.mesh(mesh_id).unwrap();
    let lod = mesh.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 1);
}

#[test]
fn test_submesh_validation_vertex_overflow() {
    let graphics_device = create_mock_graphics_device();
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: create_quad_vertex_data(), // 4 vertices
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let mut geom = Geometry::from_desc(desc).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        lods: vec![
            GeometryLODDesc {
                lod_index: 0,
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "invalid".to_string(),
                        vertex_offset: 0,
                        vertex_count: 10, // Exceeds total_vertex_count (4)
                        index_offset: 0,
                        index_count: 6,
                        topology: graphics_device::PrimitiveTopology::TriangleList,
                    }
                ],
            }
        ],
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
        index_data: Some(create_quad_index_data_u16()), // 6 indices
        vertex_layout: create_simple_vertex_layout(),
        index_type: graphics_device::IndexType::U16,
        meshes: vec![],
    };

    let mut geom = Geometry::from_desc(desc).unwrap();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        lods: vec![
            GeometryLODDesc {
                lod_index: 0,
                submeshes: vec![
                    GeometrySubMeshDesc {
                        name: "invalid".to_string(),
                        vertex_offset: 0,
                        vertex_count: 4,
                        index_offset: 0,
                        index_count: 20, // Exceeds total_index_count (6)
                        topology: graphics_device::PrimitiveTopology::TriangleList,
                    }
                ],
            }
        ],
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 4,
                                index_offset: 0,
                                index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();
    let lod = mesh.lod(0).unwrap();

    let submesh_id = lod.submesh_id("body");
    assert_eq!(submesh_id, Some(0));

    let submesh = lod.submesh_by_name("body");
    assert!(submesh.is_some());

    let submesh_names = lod.submesh_names();
    assert_eq!(submesh_names, vec!["body"]);
}

#[test]
fn test_multiple_submeshes_in_lod() {
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();
    let lod = mesh.lod(0).unwrap();

    assert_eq!(lod.submesh_count(), 2);
    assert!(lod.submesh_by_name("body").is_some());
    assert!(lod.submesh_by_name("armor").is_some());
}

// ============================================================================
// COMPLEX HIERARCHY TESTS
// ============================================================================

#[test]
fn test_complex_geometry_hierarchy() {
    let graphics_device = create_mock_graphics_device();

    // Create a large buffer to accommodate multiple meshes
    let vertex_data = vec![0u8; 100 * 8]; // 100 vertices
    let index_data = vec![0u8; 150 * 2]; // 150 indices

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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 10,
                                index_offset: 0,
                                index_count: 15,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 10,
                                vertex_count: 5,
                                index_offset: 15,
                                index_count: 9,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 8,
                                index_offset: 0,
                                index_count: 12,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                ],
            },
            GeometryMeshDesc {
                name: "enemy".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 20,
                                vertex_count: 12,
                                index_offset: 30,
                                index_count: 18,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    // Verify geometry structure
    assert_eq!(geom.mesh_count(), 2);
    assert_eq!(geom.mesh_names().len(), 2);

    // Verify hero mesh
    let hero = geom.mesh_by_name("hero").unwrap();
    assert_eq!(hero.lod_count(), 2);

    let hero_lod0 = hero.lod(0).unwrap();
    assert_eq!(hero_lod0.submesh_count(), 2);

    let hero_lod1 = hero.lod(1).unwrap();
    assert_eq!(hero_lod1.submesh_count(), 1);

    // Verify enemy mesh
    let enemy = geom.mesh_by_name("enemy").unwrap();
    assert_eq!(enemy.lod_count(), 1);

    let enemy_lod0 = enemy.lod(0).unwrap();
    assert_eq!(enemy_lod0.submesh_count(), 1);
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

    let geom = Geometry::from_desc(desc).unwrap();

    // Test graphics_device()
    assert!(Arc::ptr_eq(&geom.graphics_device(), &graphics_device));

    // Test vertex_buffer()
    let vb = geom.vertex_buffer();
    assert!(Arc::strong_count(vb) >= 1);

    // Test vertex_layout()
    let layout = geom.vertex_layout();
    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.bindings[0].stride, 8);

    // Test total_vertex_count() - already tested but verify here
    assert_eq!(geom.total_vertex_count(), 4);

    // Test total_index_count() - already tested but verify here
    assert_eq!(geom.total_index_count(), 6);
}

#[test]
fn test_submesh_by_id() {
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    // Get mesh id
    let mesh_id = geom.mesh_id("hero").unwrap();
    assert_eq!(mesh_id, 0);

    // Test submesh() with ids (not by name)
    let submesh0 = geom.submesh(mesh_id, 0, 0);
    assert!(submesh0.is_some());
    assert_eq!(submesh0.unwrap().vertex_offset(), 0);
    assert_eq!(submesh0.unwrap().vertex_count(), 2);

    let submesh1 = geom.submesh(mesh_id, 0, 1);
    assert!(submesh1.is_some());
    assert_eq!(submesh1.unwrap().vertex_offset(), 2);
    assert_eq!(submesh1.unwrap().vertex_count(), 2);

    // Test invalid ids
    let invalid_submesh = geom.submesh(mesh_id, 0, 99);
    assert!(invalid_submesh.is_none());

    let invalid_lod = geom.submesh(mesh_id, 99, 0);
    assert!(invalid_lod.is_none());

    let invalid_mesh = geom.submesh(99, 0, 0);
    assert!(invalid_mesh.is_none());
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
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "weapon".to_string(),
                                vertex_offset: 0,
                                vertex_count: 4,
                                index_offset: 0,
                                index_count: 6,
                                topology: graphics_device::PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let geom = Geometry::from_desc(desc).unwrap();

    let mesh = geom.mesh_by_name("hero").unwrap();
    let lod = mesh.lod(0).unwrap();

    // Test submeshes() iterator
    let submesh_vec: Vec<(&str, &GeometrySubMesh)> = lod.submeshes().collect();

    assert_eq!(submesh_vec.len(), 3);

    // Verify all submeshes are present (order not guaranteed by HashMap iterator)
    let names: Vec<&str> = submesh_vec.iter().map(|(name, _)| *name).collect();
    assert!(names.contains(&"body"));
    assert!(names.contains(&"armor"));
    assert!(names.contains(&"weapon"));

    // Verify we can access submesh data through iterator
    for (name, submesh) in lod.submeshes() {
        match name {
            "body" => {
                assert_eq!(submesh.vertex_count(), 2);
                assert_eq!(submesh.index_count(), 3);
            }
            "armor" => {
                assert_eq!(submesh.vertex_count(), 2);
                assert_eq!(submesh.index_count(), 3);
            }
            "weapon" => {
                assert_eq!(submesh.vertex_count(), 4);
                assert_eq!(submesh.index_count(), 6);
            }
            _ => panic!("Unexpected submesh name: {}", name),
        }
    }
}
