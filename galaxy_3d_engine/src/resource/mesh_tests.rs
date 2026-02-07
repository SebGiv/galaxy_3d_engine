/// Unit tests for mesh.rs
///
/// Tests the Mesh, MeshEntry, MeshLOD, and SubMesh hierarchy without requiring GPU.
/// Uses MockRenderer for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::renderer::{
    mock_renderer::MockRenderer,
    IndexType, PrimitiveTopology, VertexLayout,
    VertexBinding, VertexAttribute, BufferFormat, VertexInputRate,
};
#[cfg(test)]
use crate::resource::{
    Mesh, MeshDesc, MeshEntryDesc, MeshLODDesc, SubMeshDesc, SubMesh,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a mock renderer for testing
fn create_mock_renderer() -> Arc<Mutex<dyn crate::renderer::Renderer>> {
    let renderer = MockRenderer::new();
    Arc::new(Mutex::new(renderer))
}

/// Create a simple vertex layout (Position2D)
fn create_simple_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8, // 2 floats (x, y) = 8 bytes
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT, // vec2 of floats
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
fn create_simple_submesh_desc() -> SubMeshDesc {
    SubMeshDesc {
        name: "quad".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: PrimitiveTopology::TriangleList,
    }
}

// ============================================================================
// MESH CREATION TESTS
// ============================================================================

#[test]
fn test_create_mesh_indexed() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    assert_eq!(mesh.name(), "test_mesh");
    assert_eq!(mesh.total_vertex_count(), 4);
    assert_eq!(mesh.total_index_count(), 6);
    assert!(mesh.is_indexed());
    assert_eq!(mesh.index_type(), IndexType::U16);
}

#[test]
fn test_create_mesh_non_indexed() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: None, // Non-indexed
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    assert_eq!(mesh.name(), "test_mesh");
    assert_eq!(mesh.total_vertex_count(), 4);
    assert_eq!(mesh.total_index_count(), 0);
    assert!(!mesh.is_indexed());
    assert!(mesh.index_buffer().is_none());
}

#[test]
fn test_create_mesh_with_entry() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![create_simple_submesh_desc()],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    assert_eq!(mesh.mesh_entry_count(), 1);
    assert!(mesh.mesh_entry_by_name("hero").is_some());
}

#[test]
fn test_create_mesh_invalid_vertex_stride() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: vec![1, 2, 3], // 3 bytes, not divisible by stride 8
        index_data: None,
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let result = Mesh::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_create_mesh_invalid_index_stride() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(vec![1, 2, 3]), // 3 bytes, not divisible by u16 size (2)
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let result = Mesh::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// MESH ENTRY TESTS
// ============================================================================

#[test]
fn test_add_mesh_entry() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let entry_desc = MeshEntryDesc {
        name: "hero".to_string(),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![create_simple_submesh_desc()],
            }
        ],
    };

    let entry_id = mesh.add_mesh_entry(entry_desc).unwrap();

    assert_eq!(entry_id, 0);
    assert_eq!(mesh.mesh_entry_count(), 1);
    assert_eq!(mesh.mesh_entry_names(), vec!["hero"]);
}

#[test]
fn test_get_mesh_entry_by_name() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry = mesh.mesh_entry_by_name("hero");
    assert!(entry.is_some());

    let entry = mesh.mesh_entry_by_name("nonexistent");
    assert!(entry.is_none());
}

#[test]
fn test_get_mesh_entry_by_id() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry_id = mesh.mesh_entry_id("hero");
    assert_eq!(entry_id, Some(0));

    let entry = mesh.mesh_entry(0);
    assert!(entry.is_some());
}

#[test]
fn test_add_duplicate_mesh_entry() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![],
            }
        ],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let entry_desc = MeshEntryDesc {
        name: "hero".to_string(), // Duplicate
        lods: vec![],
    };

    let result = mesh.add_mesh_entry(entry_desc);
    assert!(result.is_err());
}

// ============================================================================
// LOD TESTS
// ============================================================================

#[test]
fn test_add_lod() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![],
            }
        ],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let lod_desc = MeshLODDesc {
        lod_index: 0,
        submeshes: vec![create_simple_submesh_desc()],
    };

    let entry_id = mesh.mesh_entry_id("hero").unwrap();
    let lod_index = mesh.add_mesh_lod(entry_id, lod_desc).unwrap();

    assert_eq!(lod_index, 0);

    let entry = mesh.mesh_entry(entry_id).unwrap();
    assert_eq!(entry.lod_count(), 1);
}

#[test]
fn test_multiple_lods() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![create_simple_submesh_desc()],
                    },
                    MeshLODDesc {
                        lod_index: 1,
                        submeshes: vec![create_simple_submesh_desc()],
                    },
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry = mesh.mesh_entry_by_name("hero").unwrap();
    assert_eq!(entry.lod_count(), 2);
    assert!(entry.lod(0).is_some());
    assert!(entry.lod(1).is_some());
}

// ============================================================================
// SUBMESH TESTS
// ============================================================================

#[test]
fn test_submesh_accessors() {
    let submesh_desc = SubMeshDesc {
        name: "test".to_string(),
        vertex_offset: 10,
        vertex_count: 20,
        index_offset: 5,
        index_count: 30,
        topology: PrimitiveTopology::TriangleStrip,
    };

    let renderer = create_mock_renderer();
    // Create mesh with enough vertices/indices to accommodate submesh
    let vertex_data = vec![0u8; 30 * 8]; // 30 vertices * 8 bytes stride
    let index_data = vec![0u8; 35 * 2]; // 35 indices * 2 bytes (u16)

    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![submesh_desc],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let submesh = mesh.submesh_by_name("hero", 0, "test").unwrap();

    assert_eq!(submesh.vertex_offset(), 10);
    assert_eq!(submesh.vertex_count(), 20);
    assert_eq!(submesh.index_offset(), 5);
    assert_eq!(submesh.index_count(), 30);
    assert_eq!(submesh.topology(), PrimitiveTopology::TriangleStrip);
}

#[test]
fn test_add_submesh() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![],
                    }
                ],
            }
        ],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let submesh_desc = SubMeshDesc {
        name: "body".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: PrimitiveTopology::TriangleList,
    };

    let entry_id = mesh.mesh_entry_id("hero").unwrap();
    let submesh_id = mesh.add_submesh(entry_id, 0, submesh_desc).unwrap();

    assert_eq!(submesh_id, 0);

    let entry = mesh.mesh_entry(entry_id).unwrap();
    let lod = entry.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 1);
}

#[test]
fn test_submesh_validation_vertex_overflow() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(), // 4 vertices
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let entry_desc = MeshEntryDesc {
        name: "hero".to_string(),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc {
                        name: "invalid".to_string(),
                        vertex_offset: 0,
                        vertex_count: 10, // Exceeds total_vertex_count (4)
                        index_offset: 0,
                        index_count: 6,
                        topology: PrimitiveTopology::TriangleList,
                    }
                ],
            }
        ],
    };

    let result = mesh.add_mesh_entry(entry_desc);
    assert!(result.is_err());
}

#[test]
fn test_submesh_validation_index_overflow() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()), // 6 indices
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mut mesh = Mesh::from_desc(desc).unwrap();

    let entry_desc = MeshEntryDesc {
        name: "hero".to_string(),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc {
                        name: "invalid".to_string(),
                        vertex_offset: 0,
                        vertex_count: 4,
                        index_offset: 0,
                        index_count: 20, // Exceeds total_index_count (6)
                        topology: PrimitiveTopology::TriangleList,
                    }
                ],
            }
        ],
    };

    let result = mesh.add_mesh_entry(entry_desc);
    assert!(result.is_err());
}

// ============================================================================
// LOOKUP TESTS
// ============================================================================

#[test]
fn test_submesh_lookup_by_name() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 4,
                                index_offset: 0,
                                index_count: 6,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry = mesh.mesh_entry_by_name("hero").unwrap();
    let lod = entry.lod(0).unwrap();

    let submesh_id = lod.submesh_id("body");
    assert_eq!(submesh_id, Some(0));

    let submesh = lod.submesh_by_name("body");
    assert!(submesh.is_some());

    let submesh_names = lod.submesh_names();
    assert_eq!(submesh_names, vec!["body"]);
}

#[test]
fn test_multiple_submeshes_in_lod() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            SubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry = mesh.mesh_entry_by_name("hero").unwrap();
    let lod = entry.lod(0).unwrap();

    assert_eq!(lod.submesh_count(), 2);
    assert!(lod.submesh_by_name("body").is_some());
    assert!(lod.submesh_by_name("armor").is_some());
}

// ============================================================================
// COMPLEX HIERARCHY TESTS
// ============================================================================

#[test]
fn test_complex_mesh_hierarchy() {
    let renderer = create_mock_renderer();

    // Create a large buffer to accommodate multiple mesh entries
    let vertex_data = vec![0u8; 100 * 8]; // 100 vertices
    let index_data = vec![0u8; 150 * 2]; // 150 indices

    let desc = MeshDesc {
        name: "characters".to_string(),
        renderer: renderer.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 10,
                                index_offset: 0,
                                index_count: 15,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            SubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 10,
                                vertex_count: 5,
                                index_offset: 15,
                                index_count: 9,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                    MeshLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 8,
                                index_offset: 0,
                                index_count: 12,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                ],
            },
            MeshEntryDesc {
                name: "enemy".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 20,
                                vertex_count: 12,
                                index_offset: 30,
                                index_count: 18,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // Verify mesh structure
    assert_eq!(mesh.mesh_entry_count(), 2);
    assert_eq!(mesh.mesh_entry_names().len(), 2);

    // Verify hero entry
    let hero = mesh.mesh_entry_by_name("hero").unwrap();
    assert_eq!(hero.lod_count(), 2);

    let hero_lod0 = hero.lod(0).unwrap();
    assert_eq!(hero_lod0.submesh_count(), 2);

    let hero_lod1 = hero.lod(1).unwrap();
    assert_eq!(hero_lod1.submesh_count(), 1);

    // Verify enemy entry
    let enemy = mesh.mesh_entry_by_name("enemy").unwrap();
    assert_eq!(enemy.lod_count(), 1);

    let enemy_lod0 = enemy.lod(0).unwrap();
    assert_eq!(enemy_lod0.submesh_count(), 1);
}

// ============================================================================
// GETTER TESTS
// ============================================================================

#[test]
fn test_mesh_getters() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // Test renderer()
    assert!(Arc::ptr_eq(&mesh.renderer(), &renderer));

    // Test vertex_buffer()
    let vb = mesh.vertex_buffer();
    assert!(Arc::strong_count(vb) >= 1);

    // Test vertex_layout()
    let layout = mesh.vertex_layout();
    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.bindings[0].stride, 8);

    // Test total_vertex_count() - already tested but verify here
    assert_eq!(mesh.total_vertex_count(), 4);

    // Test total_index_count() - already tested but verify here
    assert_eq!(mesh.total_index_count(), 6);
}

#[test]
fn test_submesh_by_id() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            SubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    // Get entry id
    let entry_id = mesh.mesh_entry_id("hero").unwrap();
    assert_eq!(entry_id, 0);

    // Test submesh() with ids (not by name)
    let submesh0 = mesh.submesh(entry_id, 0, 0);
    assert!(submesh0.is_some());
    assert_eq!(submesh0.unwrap().vertex_offset(), 0);
    assert_eq!(submesh0.unwrap().vertex_count(), 2);

    let submesh1 = mesh.submesh(entry_id, 0, 1);
    assert!(submesh1.is_some());
    assert_eq!(submesh1.unwrap().vertex_offset(), 2);
    assert_eq!(submesh1.unwrap().vertex_count(), 2);

    // Test invalid ids
    let invalid_submesh = mesh.submesh(entry_id, 0, 99);
    assert!(invalid_submesh.is_none());

    let invalid_lod = mesh.submesh(entry_id, 99, 0);
    assert!(invalid_lod.is_none());

    let invalid_entry = mesh.submesh(99, 0, 0);
    assert!(invalid_entry.is_none());
}

#[test]
fn test_submeshes_iterator() {
    let renderer = create_mock_renderer();
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: create_quad_vertex_data(),
        index_data: Some(create_quad_index_data_u16()),
        vertex_layout: create_simple_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            MeshEntryDesc {
                name: "hero".to_string(),
                lods: vec![
                    MeshLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            SubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0,
                                vertex_count: 2,
                                index_offset: 0,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            SubMeshDesc {
                                name: "armor".to_string(),
                                vertex_offset: 2,
                                vertex_count: 2,
                                index_offset: 3,
                                index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            SubMeshDesc {
                                name: "weapon".to_string(),
                                vertex_offset: 0,
                                vertex_count: 4,
                                index_offset: 0,
                                index_count: 6,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    }
                ],
            }
        ],
    };

    let mesh = Mesh::from_desc(desc).unwrap();

    let entry = mesh.mesh_entry_by_name("hero").unwrap();
    let lod = entry.lod(0).unwrap();

    // Test submeshes() iterator
    let submesh_vec: Vec<(&str, &SubMesh)> = lod.submeshes().collect();

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
