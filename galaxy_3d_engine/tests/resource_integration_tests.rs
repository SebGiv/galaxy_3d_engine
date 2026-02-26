//! Integration tests for ResourceManager with real Vulkan backend
//!
//! These tests require a GPU and are marked with #[ignore].
//! Run with: cargo test --test resource_integration_tests -- --ignored

mod gpu_test_utils;

use galaxy_3d_engine::galaxy3d::resource::{ResourceManager, TextureDesc, GeometryDesc};
use galaxy_3d_engine::galaxy3d::resource::{LayerDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc};
use galaxy_3d_engine::galaxy3d::render::{
    TextureDesc as RenderTextureDesc, TextureFormat, TextureType, TextureUsage, MipmapMode,
    BufferFormat, VertexLayout, VertexBinding, VertexAttribute, VertexInputRate,
    IndexType, PrimitiveTopology,
};
use gpu_test_utils::get_test_renderer;
use serial_test::serial;

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_create_texture_with_vulkan() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create texture descriptor
    let desc = TextureDesc {
        renderer: renderer_arc.clone(),
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
            texture_type: TextureType::Tex2D,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            }
        ],
    };

    // Create texture
    let _texture = rm.create_texture("test_texture".to_string(), desc).unwrap();

    // Verify
    assert_eq!(rm.texture_count(), 1);
    assert!(rm.texture("test_texture").is_some());
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_create_geometry_with_vulkan() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create vertex data (quad: 4 vertices)
    let vertices: Vec<f32> = vec![
        -0.5, -0.5,  // Bottom-left
         0.5, -0.5,  // Bottom-right
         0.5,  0.5,  // Top-right
        -0.5,  0.5,  // Top-left
    ];
    let vertex_data: Vec<u8> = vertices.iter()
        .flat_map(|&f| f.to_le_bytes())
        .collect();

    // Create index data (2 triangles)
    let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
    let index_data: Vec<u8> = indices.iter()
        .flat_map(|&i| i.to_le_bytes())
        .collect();

    // Create vertex layout
    let vertex_layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8, // 2 floats
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    };

    // Create geometry descriptor
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        renderer: renderer_arc.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout,
        index_type: IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "quad".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "main".to_string(),
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

    // Create geometry
    let _geom = rm.create_geometry("test_geom".to_string(), desc).unwrap();

    // Verify
    assert_eq!(rm.geometry_count(), 1);
    assert!(rm.geometry("test_geom").is_some());

    let geom_ref = rm.geometry("test_geom").unwrap();
    assert_eq!(geom_ref.total_vertex_count(), 4);
    assert_eq!(geom_ref.total_index_count(), 6);
    assert!(geom_ref.is_indexed());
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_multiple_resources() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create multiple textures
    for i in 0..3 {
        let desc = TextureDesc {
            renderer: renderer_arc.clone(),
            texture: RenderTextureDesc {
                width: 128,
                height: 128,
                format: TextureFormat::R8G8B8A8_UNORM,
                usage: TextureUsage::Sampled,
                array_layers: 1,
                mipmap: MipmapMode::None,
                data: None,
                texture_type: TextureType::Tex2D,
            },
            layers: vec![
                LayerDesc {
                    name: "layer".to_string(),
                    layer_index: 0,
                    data: None,
                    regions: vec![],
                }
            ],
        };
        rm.create_texture(format!("texture_{}", i), desc).unwrap();
    }

    // Verify
    assert_eq!(rm.texture_count(), 3);
    assert!(rm.texture("texture_0").is_some());
    assert!(rm.texture("texture_1").is_some());
    assert!(rm.texture("texture_2").is_some());
}

// ============================================================================
// ADVANCED RESOURCE MANAGER TESTS
// ============================================================================

// Note: test_integration_texture_atlas_with_regions removed
// AtlasRegion is not publicly exported in the API

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_geometry_with_multiple_lods() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create vertex data for different LODs
    // LOD 0: 12 vertices (high detail)
    // LOD 1: 8 vertices (medium detail)
    // LOD 2: 4 vertices (low detail)
    let vertices_lod0: Vec<f32> = (0..24).map(|i| i as f32).collect(); // 12 vertices * 2 floats
    let vertices_lod1: Vec<f32> = (0..16).map(|i| i as f32).collect(); // 8 vertices * 2 floats
    let vertices_lod2: Vec<f32> = (0..8).map(|i| i as f32).collect();  // 4 vertices * 2 floats

    let mut vertex_data = Vec::new();
    vertex_data.extend(vertices_lod0.iter().flat_map(|&f| f.to_le_bytes()));
    vertex_data.extend(vertices_lod1.iter().flat_map(|&f| f.to_le_bytes()));
    vertex_data.extend(vertices_lod2.iter().flat_map(|&f| f.to_le_bytes()));

    // Create index data for each LOD
    let indices_lod0: Vec<u16> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    let indices_lod1: Vec<u16> = vec![0, 1, 2, 3, 4, 5, 6, 7];
    let indices_lod2: Vec<u16> = vec![0, 1, 2, 3];

    let mut index_data = Vec::new();
    index_data.extend(indices_lod0.iter().flat_map(|&i| i.to_le_bytes()));
    index_data.extend(indices_lod1.iter().flat_map(|&i| i.to_le_bytes()));
    index_data.extend(indices_lod2.iter().flat_map(|&i| i.to_le_bytes()));

    // Create vertex layout
    let vertex_layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    };

    // Create geometry descriptor with 3 LODs
    let desc = GeometryDesc {
        name: "lod_geom".to_string(),
        renderer: renderer_arc.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout,
        index_type: IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "main_mesh".to_string(),
                lods: vec![
                    // LOD 0 (highest detail)
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "lod0".to_string(),
                                vertex_offset: 0,
                                vertex_count: 12,
                                index_offset: 0,
                                index_count: 12,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                    // LOD 1 (medium detail)
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "lod1".to_string(),
                                vertex_offset: 12,
                                vertex_count: 8,
                                index_offset: 12,
                                index_count: 8,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                    // LOD 2 (lowest detail)
                    GeometryLODDesc {
                        lod_index: 2,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "lod2".to_string(),
                                vertex_offset: 20,
                                vertex_count: 4,
                                index_offset: 20,
                                index_count: 4,
                                topology: PrimitiveTopology::TriangleList,
                            }
                        ],
                    },
                ],
            }
        ],
    };

    // Create geometry
    let _geom = rm.create_geometry("lod_geom".to_string(), desc).unwrap();

    // Verify geometry structure
    let geom_ref = rm.geometry("lod_geom").unwrap();
    assert_eq!(geom_ref.mesh_count(), 1);

    let mesh = geom_ref.mesh(0).unwrap();
    assert_eq!(mesh.lod_count(), 3);

    // Verify each LOD
    assert!(mesh.lod(0).is_some());
    assert!(mesh.lod(1).is_some());
    assert!(mesh.lod(2).is_some());
}

#[test]
#[ignore] // Requires GPU
#[serial]
fn test_integration_geometry_with_multiple_submeshes() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create vertex data (12 vertices for 3 submeshes)
    let vertices: Vec<f32> = (0..24).map(|i| i as f32).collect();
    let vertex_data: Vec<u8> = vertices.iter()
        .flat_map(|&f| f.to_le_bytes())
        .collect();

    // Create index data for 3 submeshes
    let indices: Vec<u16> = vec![
        0, 1, 2, 3,     // Submesh 1
        4, 5, 6, 7,     // Submesh 2
        8, 9, 10, 11,   // Submesh 3
    ];
    let index_data: Vec<u8> = indices.iter()
        .flat_map(|&i| i.to_le_bytes())
        .collect();

    // Create vertex layout
    let vertex_layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    };

    // Create geometry descriptor with multiple submeshes
    let desc = GeometryDesc {
        name: "multi_submesh".to_string(),
        renderer: renderer_arc.clone(),
        vertex_data,
        index_data: Some(index_data),
        vertex_layout,
        index_type: IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "character".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "head".to_string(),
                                vertex_offset: 0,
                                vertex_count: 4,
                                index_offset: 0,
                                index_count: 4,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 4,
                                vertex_count: 4,
                                index_offset: 4,
                                index_count: 4,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "legs".to_string(),
                                vertex_offset: 8,
                                vertex_count: 4,
                                index_offset: 8,
                                index_count: 4,
                                topology: PrimitiveTopology::TriangleList,
                            },
                        ],
                    }
                ],
            }
        ],
    };

    // Create geometry
    let _geom = rm.create_geometry("character_geom".to_string(), desc).unwrap();

    // Verify geometry structure
    let geom_ref = rm.geometry("character_geom").unwrap();
    let mesh = geom_ref.mesh(0).unwrap();
    let lod = mesh.lod(0).unwrap();

    assert_eq!(lod.submesh_count(), 3);
    assert!(lod.submesh_by_name("head").is_some());
    assert!(lod.submesh_by_name("body").is_some());
    assert!(lod.submesh_by_name("legs").is_some());
}

#[test]
#[ignore] // Requires GPU - Stress test
#[serial]
fn test_integration_many_resources_stress_test() {
    // Get shared Vulkan renderer
    let renderer_arc = get_test_renderer();

    // Create ResourceManager
    let mut rm = ResourceManager::new();

    // Create many textures (50 textures)
    for i in 0..50 {
        let desc = TextureDesc {
            renderer: renderer_arc.clone(),
            texture: RenderTextureDesc {
                width: 64,
                height: 64,
                format: TextureFormat::R8G8B8A8_UNORM,
                usage: TextureUsage::Sampled,
                array_layers: 1,
                mipmap: MipmapMode::None,
                data: None,
                texture_type: TextureType::Tex2D,
            },
            layers: vec![
                LayerDesc {
                    name: "layer".to_string(),
                    layer_index: 0,
                    data: None,
                    regions: vec![],
                }
            ],
        };
        rm.create_texture(format!("texture_{}", i), desc).unwrap();
    }

    // Verify all textures were created
    assert_eq!(rm.texture_count(), 50);

    // Create many geometries (20 geometries)
    for i in 0..20 {
        let vertices: Vec<f32> = vec![-0.5, -0.5, 0.5, -0.5, 0.5, 0.5, -0.5, 0.5];
        let vertex_data: Vec<u8> = vertices.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();

        let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
        let index_data: Vec<u8> = indices.iter()
            .flat_map(|&i| i.to_le_bytes())
            .collect();

        let vertex_layout = VertexLayout {
            bindings: vec![
                VertexBinding {
                    binding: 0,
                    stride: 8,
                    input_rate: VertexInputRate::Vertex,
                }
            ],
            attributes: vec![
                VertexAttribute {
                    location: 0,
                    binding: 0,
                    format: BufferFormat::R32G32_SFLOAT,
                    offset: 0,
                }
            ],
        };

        let desc = GeometryDesc {
            name: format!("geom_{}", i),
            renderer: renderer_arc.clone(),
            vertex_data,
            index_data: Some(index_data),
            vertex_layout,
            index_type: IndexType::U16,
            meshes: vec![
                GeometryMeshDesc {
                    name: "quad".to_string(),
                    lods: vec![
                        GeometryLODDesc {
                            lod_index: 0,
                            submeshes: vec![
                                GeometrySubMeshDesc {
                                    name: "main".to_string(),
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

        rm.create_geometry(format!("geom_{}", i), desc).unwrap();
    }

    // Verify all geometries were created
    assert_eq!(rm.geometry_count(), 20);

    // Verify we can access all resources
    for i in 0..50 {
        assert!(rm.texture(&format!("texture_{}", i)).is_some());
    }
    for i in 0..20 {
        assert!(rm.geometry(&format!("geom_{}", i)).is_some());
    }
}
