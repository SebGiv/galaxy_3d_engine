/// Tests for ResourceManager
///
/// These tests use MockRenderer to test ResourceManager logic without requiring a GPU.

use super::*;
use crate::renderer::mock_renderer::MockRenderer;
use crate::renderer::{
    IndexType, PrimitiveTopology, TextureFormat, TextureUsage, BufferFormat,
    MipmapMode, TextureData, VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate,
};
use crate::resource::{AtlasRegion, AtlasRegionDesc, LayerDesc, PipelinePassDesc};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a MockRenderer wrapped in Arc<Mutex<>>
fn create_mock_renderer() -> Arc<Mutex<dyn crate::renderer::Renderer>> {
    Arc::new(Mutex::new(MockRenderer::new()))
}

/// Create a simple texture descriptor for testing
fn create_test_texture_desc(
    renderer: Arc<Mutex<dyn crate::renderer::Renderer>>,
   _name: &str,
    width: u32,
    height: u32,
) -> TextureDesc {
    TextureDesc {
        renderer,
        texture: crate::renderer::TextureDesc {
            width,
            height,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            data: Some(TextureData::Single(vec![255u8; (width * height * 4) as usize])),
            mipmap: MipmapMode::None,
        },
        layers: vec![LayerDesc {
            name: "default".to_string(),
            layer_index: 0,
            data: None,
            regions: vec![],
        }],
    }
}

/// Create a simple mesh descriptor for testing
fn create_test_mesh_desc(
    renderer: Arc<Mutex<dyn crate::renderer::Renderer>>,
    name: &str,
) -> MeshDesc {
    // Simple quad: 4 vertices (Position2D + UV), 6 indices
    let vertex_data = vec![
        // Position (x, y) + UV (u, v)
        -0.5f32, -0.5, 0.0, 0.0, // Bottom-left
         0.5, -0.5, 1.0, 0.0,     // Bottom-right
         0.5,  0.5, 1.0, 1.0,     // Top-right
        -0.5,  0.5, 0.0, 1.0,     // Top-left
    ];

    let index_data = vec![
        0u16, 1, 2,  // First triangle
        2, 3, 0,     // Second triangle
    ];

    let vertex_bytes: Vec<u8> = vertex_data.iter()
        .flat_map(|&f| f.to_ne_bytes())
        .collect();

    let index_bytes: Vec<u8> = index_data.iter()
        .flat_map(|&i| i.to_ne_bytes())
        .collect();

    // Simple Position2D + UV layout
    let vertex_layout = VertexLayout {
        bindings: vec![VertexBinding {
            binding: 0,
            stride: 16, // 4 floats * 4 bytes
            input_rate: VertexInputRate::Vertex,
        }],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
            VertexAttribute {
                location: 1,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 8,
            },
        ],
    };

    MeshDesc {
        name: name.to_string(),
        renderer,
        vertex_data: vertex_bytes,
        index_data: Some(index_bytes),
        vertex_layout,
        index_type: IndexType::U16,
        meshes: vec![MeshEntryDesc {
            name: name.to_string(),
            lods: vec![MeshLODDesc {
                lod_index: 0,
                submeshes: vec![SubMeshDesc {
                    name: "default".to_string(),
                    vertex_offset: 0,
                    vertex_count: 4,
                    index_offset: 0,
                    index_count: 6,
                    topology: PrimitiveTopology::TriangleList,
                }],
            }],
        }],
    }
}

/// Create a simple pipeline descriptor for testing
fn create_test_pipeline_desc(
    renderer: Arc<Mutex<dyn crate::renderer::Renderer>>,
    name: &str,
) -> PipelineDesc {
    let vertex_layout = VertexLayout {
        bindings: vec![VertexBinding {
            binding: 0,
            stride: 16,
            input_rate: VertexInputRate::Vertex,
        }],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
        ],
    };

    PipelineDesc {
        renderer,
        variants: vec![PipelineVariantDesc {
            name: name.to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: crate::renderer::PipelineDesc {
                    vertex_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("vert".to_string())),
                    fragment_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("frag".to_string())),
                    vertex_layout,
                    topology: PrimitiveTopology::TriangleList,
                    push_constant_ranges: vec![],
                    descriptor_set_layouts: vec![],
                    rasterization: Default::default(),
                    depth_stencil: Default::default(),
                    color_blend: Default::default(),
                    multisample: Default::default(),
                },
            }],
        }],
    }
}

// ============================================================================
// Tests: ResourceManager Creation
// ============================================================================

#[test]
fn test_resource_manager_new() {
    let rm = ResourceManager::new();
    assert_eq!(rm.texture_count(), 0);
    assert_eq!(rm.mesh_count(), 0);
    assert_eq!(rm.pipeline_count(), 0);
}

// ============================================================================
// Tests: Texture Management
// ============================================================================

#[test]
fn test_create_texture() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_texture_desc(renderer.clone(), "test_texture", 256, 256);
    let _texture = rm.create_texture("test_texture".to_string(), desc).unwrap();

    assert_eq!(rm.texture_count(), 1);
    // texture has been created successfully (no direct width/height access)
}

#[test]
fn test_get_texture() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_texture_desc(renderer.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc).unwrap();

    let texture = rm.texture("test_texture");
    assert!(texture.is_some());
    // texture exists (no direct width/height access on resource::Texture)
}

#[test]
fn test_get_texture_not_found() {
    let rm = ResourceManager::new();
    let texture = rm.texture("nonexistent");
    assert!(texture.is_none());
}

#[test]
fn test_remove_texture() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_texture_desc(renderer.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc).unwrap();

    assert_eq!(rm.texture_count(), 1);

    let removed = rm.remove_texture("test_texture");
    assert!(removed);
    assert_eq!(rm.texture_count(), 0);
}

#[test]
fn test_remove_texture_not_found() {
    let mut rm = ResourceManager::new();
    let removed = rm.remove_texture("nonexistent");
    assert!(!removed);
}

#[test]
fn test_duplicate_texture_fails() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_texture_desc(renderer.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc1).unwrap();

    let desc2 = create_test_texture_desc(renderer.clone(), "test_texture", 512, 512);
    let result = rm.create_texture("test_texture".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.texture_count(), 1); // Still only one texture
}

#[test]
fn test_multiple_textures() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_texture_desc(renderer.clone(), "texture1", 256, 256);
    let desc2 = create_test_texture_desc(renderer.clone(), "texture2", 512, 512);
    let desc3 = create_test_texture_desc(renderer.clone(), "texture3", 128, 128);

    rm.create_texture("texture1".to_string(), desc1).unwrap();
    rm.create_texture("texture2".to_string(), desc2).unwrap();
    rm.create_texture("texture3".to_string(), desc3).unwrap();

    assert_eq!(rm.texture_count(), 3);
    assert!(rm.texture("texture1").is_some());
    assert!(rm.texture("texture2").is_some());
    assert!(rm.texture("texture3").is_some());
}

#[test]
fn test_texture_count() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    assert_eq!(rm.texture_count(), 0);

    let desc1 = create_test_texture_desc(renderer.clone(), "texture1", 256, 256);
    rm.create_texture("texture1".to_string(), desc1).unwrap();
    assert_eq!(rm.texture_count(), 1);

    let desc2 = create_test_texture_desc(renderer.clone(), "texture2", 512, 512);
    rm.create_texture("texture2".to_string(), desc2).unwrap();
    assert_eq!(rm.texture_count(), 2);

    rm.remove_texture("texture1");
    assert_eq!(rm.texture_count(), 1);

    rm.remove_texture("texture2");
    assert_eq!(rm.texture_count(), 0);
}

// ============================================================================
// Tests: Mesh Management
// ============================================================================

#[test]
fn test_create_mesh() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_mesh_desc(renderer.clone(), "test_mesh");
    let mesh = rm.create_mesh("test_mesh".to_string(), desc).unwrap();

    assert_eq!(rm.mesh_count(), 1);
    assert_eq!(mesh.mesh_entry_count(), 1);
}

#[test]
fn test_get_mesh() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_mesh_desc(renderer.clone(), "test_mesh");
    rm.create_mesh("test_mesh".to_string(), desc).unwrap();

    let mesh = rm.mesh("test_mesh");
    assert!(mesh.is_some());
}

#[test]
fn test_get_mesh_not_found() {
    let rm = ResourceManager::new();
    let mesh = rm.mesh("nonexistent");
    assert!(mesh.is_none());
}

#[test]
fn test_remove_mesh() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_mesh_desc(renderer.clone(), "test_mesh");
    rm.create_mesh("test_mesh".to_string(), desc).unwrap();

    assert_eq!(rm.mesh_count(), 1);

    let removed = rm.remove_mesh("test_mesh");
    assert!(removed);
    assert_eq!(rm.mesh_count(), 0);
}

#[test]
fn test_remove_mesh_not_found() {
    let mut rm = ResourceManager::new();
    let removed = rm.remove_mesh("nonexistent");
    assert!(!removed);
}

#[test]
fn test_duplicate_mesh_fails() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_mesh_desc(renderer.clone(), "test_mesh");
    rm.create_mesh("test_mesh".to_string(), desc1).unwrap();

    let desc2 = create_test_mesh_desc(renderer.clone(), "test_mesh");
    let result = rm.create_mesh("test_mesh".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.mesh_count(), 1);
}

#[test]
fn test_multiple_meshes() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_mesh_desc(renderer.clone(), "mesh1");
    let desc2 = create_test_mesh_desc(renderer.clone(), "mesh2");
    let desc3 = create_test_mesh_desc(renderer.clone(), "mesh3");

    rm.create_mesh("mesh1".to_string(), desc1).unwrap();
    rm.create_mesh("mesh2".to_string(), desc2).unwrap();
    rm.create_mesh("mesh3".to_string(), desc3).unwrap();

    assert_eq!(rm.mesh_count(), 3);
    assert!(rm.mesh("mesh1").is_some());
    assert!(rm.mesh("mesh2").is_some());
    assert!(rm.mesh("mesh3").is_some());
}

#[test]
fn test_mesh_count() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    assert_eq!(rm.mesh_count(), 0);

    let desc1 = create_test_mesh_desc(renderer.clone(), "mesh1");
    rm.create_mesh("mesh1".to_string(), desc1).unwrap();
    assert_eq!(rm.mesh_count(), 1);

    let desc2 = create_test_mesh_desc(renderer.clone(), "mesh2");
    rm.create_mesh("mesh2".to_string(), desc2).unwrap();
    assert_eq!(rm.mesh_count(), 2);

    rm.remove_mesh("mesh1");
    assert_eq!(rm.mesh_count(), 1);

    rm.remove_mesh("mesh2");
    assert_eq!(rm.mesh_count(), 0);
}

// ============================================================================
// Tests: Pipeline Management
// ============================================================================

#[test]
fn test_create_pipeline() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    let pipeline = rm.create_pipeline("test_pipeline".to_string(), desc).unwrap();

    assert_eq!(rm.pipeline_count(), 1);
    assert_eq!(pipeline.variant_count(), 1);
}

#[test]
fn test_get_pipeline() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    rm.create_pipeline("test_pipeline".to_string(), desc).unwrap();

    let pipeline = rm.pipeline("test_pipeline");
    assert!(pipeline.is_some());
}

#[test]
fn test_get_pipeline_not_found() {
    let rm = ResourceManager::new();
    let pipeline = rm.pipeline("nonexistent");
    assert!(pipeline.is_none());
}

#[test]
fn test_remove_pipeline() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    rm.create_pipeline("test_pipeline".to_string(), desc).unwrap();

    assert_eq!(rm.pipeline_count(), 1);

    let removed = rm.remove_pipeline("test_pipeline");
    assert!(removed);
    assert_eq!(rm.pipeline_count(), 0);
}

#[test]
fn test_remove_pipeline_not_found() {
    let mut rm = ResourceManager::new();
    let removed = rm.remove_pipeline("nonexistent");
    assert!(!removed);
}

#[test]
fn test_duplicate_pipeline_fails() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    rm.create_pipeline("test_pipeline".to_string(), desc1).unwrap();

    let desc2 = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    let result = rm.create_pipeline("test_pipeline".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.pipeline_count(), 1);
}

#[test]
fn test_multiple_pipelines() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    let desc1 = create_test_pipeline_desc(renderer.clone(), "pipeline1");
    let desc2 = create_test_pipeline_desc(renderer.clone(), "pipeline2");
    let desc3 = create_test_pipeline_desc(renderer.clone(), "pipeline3");

    rm.create_pipeline("pipeline1".to_string(), desc1).unwrap();
    rm.create_pipeline("pipeline2".to_string(), desc2).unwrap();
    rm.create_pipeline("pipeline3".to_string(), desc3).unwrap();

    assert_eq!(rm.pipeline_count(), 3);
    assert!(rm.pipeline("pipeline1").is_some());
    assert!(rm.pipeline("pipeline2").is_some());
    assert!(rm.pipeline("pipeline3").is_some());
}

#[test]
fn test_pipeline_count() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    assert_eq!(rm.pipeline_count(), 0);

    let desc1 = create_test_pipeline_desc(renderer.clone(), "pipeline1");
    rm.create_pipeline("pipeline1".to_string(), desc1).unwrap();
    assert_eq!(rm.pipeline_count(), 1);

    let desc2 = create_test_pipeline_desc(renderer.clone(), "pipeline2");
    rm.create_pipeline("pipeline2".to_string(), desc2).unwrap();
    assert_eq!(rm.pipeline_count(), 2);

    rm.remove_pipeline("pipeline1");
    assert_eq!(rm.pipeline_count(), 1);

    rm.remove_pipeline("pipeline2");
    assert_eq!(rm.pipeline_count(), 0);
}

// ============================================================================
// Tests: Mixed Resource Management
// ============================================================================

#[test]
fn test_mixed_resources() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create one of each resource type
    let texture_desc = create_test_texture_desc(renderer.clone(), "texture", 256, 256);
    let mesh_desc = create_test_mesh_desc(renderer.clone(), "mesh");
    let pipeline_desc = create_test_pipeline_desc(renderer.clone(), "pipeline");

    rm.create_texture("texture".to_string(), texture_desc).unwrap();
    rm.create_mesh("mesh".to_string(), mesh_desc).unwrap();
    rm.create_pipeline("pipeline".to_string(), pipeline_desc).unwrap();

    assert_eq!(rm.texture_count(), 1);
    assert_eq!(rm.mesh_count(), 1);
    assert_eq!(rm.pipeline_count(), 1);

    assert!(rm.texture("texture").is_some());
    assert!(rm.mesh("mesh").is_some());
    assert!(rm.pipeline("pipeline").is_some());
}

#[test]
fn test_clear_all_resources() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create multiple resources
    for i in 0..3 {
        let texture_desc = create_test_texture_desc(renderer.clone(), &format!("texture{}", i), 256, 256);
        let mesh_desc = create_test_mesh_desc(renderer.clone(), &format!("mesh{}", i));
        let pipeline_desc = create_test_pipeline_desc(renderer.clone(), &format!("pipeline{}", i));

        rm.create_texture(format!("texture{}", i), texture_desc).unwrap();
        rm.create_mesh(format!("mesh{}", i), mesh_desc).unwrap();
        rm.create_pipeline(format!("pipeline{}", i), pipeline_desc).unwrap();
    }

    assert_eq!(rm.texture_count(), 3);
    assert_eq!(rm.mesh_count(), 3);
    assert_eq!(rm.pipeline_count(), 3);

    // Remove all
    for i in 0..3 {
        rm.remove_texture(&format!("texture{}", i));
        rm.remove_mesh(&format!("mesh{}", i));
        rm.remove_pipeline(&format!("pipeline{}", i));
    }

    assert_eq!(rm.texture_count(), 0);
    assert_eq!(rm.mesh_count(), 0);
    assert_eq!(rm.pipeline_count(), 0);
}

// ============================================================================
// Tests: MockRenderer Verification
// ============================================================================

#[test]
fn test_mock_renderer_tracks_buffers() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(MockRenderer::new()));
    let renderer: Arc<Mutex<dyn crate::renderer::Renderer>> = mock.clone();

    let desc = create_test_mesh_desc(renderer.clone(), "test_mesh");
    rm.create_mesh("test_mesh".to_string(), desc).unwrap();

    // Verify buffers were created
    let created_buffers = mock.lock().unwrap().get_created_buffers();
    assert!(created_buffers.len() >= 2); // At least vertex + index buffers
}

#[test]
fn test_mock_renderer_tracks_textures() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(MockRenderer::new()));
    let renderer: Arc<Mutex<dyn crate::renderer::Renderer>> = mock.clone();

    let desc = create_test_texture_desc(renderer.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc).unwrap();

    // Verify texture was created
    let created_textures = mock.lock().unwrap().get_created_textures();
    assert!(created_textures.len() >= 1);
}

#[test]
fn test_mock_renderer_tracks_pipelines() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(MockRenderer::new()));
    let renderer: Arc<Mutex<dyn crate::renderer::Renderer>> = mock.clone();

    let desc = create_test_pipeline_desc(renderer.clone(), "test_pipeline");
    rm.create_pipeline("test_pipeline".to_string(), desc).unwrap();

    // Verify pipeline was created
    let created_pipelines = mock.lock().unwrap().get_created_pipelines();
    assert_eq!(created_pipelines.len(), 1);
}

// ============================================================================
// Tests: Resource Modification (add_* methods)
// ============================================================================

#[test]
fn test_add_texture_layer() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create an indexed texture with 2 initial layers
    let desc = TextureDesc {
        renderer: renderer.clone(),
        texture: crate::renderer::TextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 4, // Indexed texture with 4 slots
            data: None,
            mipmap: MipmapMode::None,
        },
        layers: vec![
            LayerDesc {
                name: "layer0".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "layer1".to_string(),
                layer_index: 1,
                data: None,
                regions: vec![],
            },
        ],
    };

    rm.create_texture("test_texture".to_string(), desc).unwrap();

    // Add a new layer
    let new_layer = LayerDesc {
        name: "layer2".to_string(),
        layer_index: 2,
        data: None,
        regions: vec![],
    };

    let result = rm.add_texture_layer("test_texture", new_layer);
    assert!(result.is_ok());

    // Verify layer was added
    let texture = rm.texture("test_texture").unwrap();
    assert_eq!(texture.layer_count(), 3);
}

#[test]
fn test_add_texture_layer_to_nonexistent_texture() {
    let mut rm = ResourceManager::new();

    let new_layer = LayerDesc {
        name: "layer".to_string(),
        layer_index: 0,
        data: None,
        regions: vec![],
    };

    let result = rm.add_texture_layer("nonexistent", new_layer);
    assert!(result.is_err());
}

#[test]
fn test_add_texture_layer_to_simple_texture() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a simple texture (array_layers=1)
    let desc = create_test_texture_desc(renderer.clone(), "simple", 256, 256);
    rm.create_texture("simple".to_string(), desc).unwrap();

    // Try to add a layer (should fail - simple textures can't have layers added)
    let new_layer = LayerDesc {
        name: "layer1".to_string(),
        layer_index: 1,
        data: None,
        regions: vec![],
    };

    let result = rm.add_texture_layer("simple", new_layer);
    assert!(result.is_err());
}

#[test]
fn test_add_texture_region() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a texture with one layer
    let desc = create_test_texture_desc(renderer.clone(), "atlas", 256, 256);
    rm.create_texture("atlas".to_string(), desc).unwrap();

    // Add a region to the default layer
    let region = AtlasRegionDesc {
        name: "sprite1".to_string(),
        region: AtlasRegion {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        },
    };

    let result = rm.add_texture_region("atlas", "default", region);
    assert!(result.is_ok());

    // Verify region was added
    let texture = rm.texture("atlas").unwrap();
    let layer = texture.layer_by_name("default").unwrap();
    assert_eq!(layer.region_count(), 1);
}

#[test]
fn test_add_texture_region_to_nonexistent_texture() {
    let mut rm = ResourceManager::new();

    let region = AtlasRegionDesc {
        name: "sprite".to_string(),
        region: AtlasRegion { x: 0, y: 0, width: 64, height: 64 },
    };

    let result = rm.add_texture_region("nonexistent", "layer", region);
    assert!(result.is_err());
}

#[test]
fn test_add_mesh_entry_to_existing_mesh() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a mesh with no initial entries
    let desc = MeshDesc {
        name: "test_mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: vec![0u8; 32], // 4 vertices * 8 bytes
        index_data: Some(vec![0u8; 12]), // 6 indices * 2 bytes
        vertex_layout: VertexLayout {
            bindings: vec![VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }],
            attributes: vec![VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }],
        },
        index_type: IndexType::U16,
        meshes: vec![], // No initial entries
    };

    rm.create_mesh("mesh".to_string(), desc).unwrap();

    // Add a mesh entry
    let entry = MeshEntryDesc {
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
    };

    let result = rm.add_mesh_entry("mesh", entry);
    assert!(result.is_ok());

    // Verify entry was added
    let mesh = rm.mesh("mesh").unwrap();
    assert_eq!(mesh.mesh_entry_count(), 1);
}

#[test]
fn test_add_mesh_entry_to_nonexistent_mesh() {
    let mut rm = ResourceManager::new();

    let entry = MeshEntryDesc {
        name: "hero".to_string(),
        lods: vec![],
    };

    let result = rm.add_mesh_entry("nonexistent", entry);
    assert!(result.is_err());
}

#[test]
fn test_add_mesh_lod() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a mesh with one entry
    let desc = MeshDesc {
        name: "mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: vec![0u8; 64], // 8 vertices * 8 bytes
        index_data: Some(vec![0u8; 24]), // 12 indices * 2 bytes
        vertex_layout: VertexLayout {
            bindings: vec![VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }],
            attributes: vec![VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }],
        },
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

    rm.create_mesh("mesh".to_string(), desc).unwrap();

    // Add a LOD to the mesh entry
    let lod = MeshLODDesc {
        lod_index: 1,
        submeshes: vec![
            SubMeshDesc {
                name: "body_lod1".to_string(),
                vertex_offset: 4,
                vertex_count: 4,
                index_offset: 6,
                index_count: 6,
                topology: PrimitiveTopology::TriangleList,
            }
        ],
    };

    let result = rm.add_mesh_lod("mesh", 0, lod);
    assert!(result.is_ok());

    // Verify LOD was added
    let mesh = rm.mesh("mesh").unwrap();
    let entry = mesh.mesh_entry(0).unwrap();
    assert_eq!(entry.lod_count(), 2);
}

#[test]
fn test_add_mesh_lod_to_nonexistent_mesh() {
    let mut rm = ResourceManager::new();

    let lod = MeshLODDesc {
        lod_index: 0,
        submeshes: vec![],
    };

    let result = rm.add_mesh_lod("nonexistent", 0, lod);
    assert!(result.is_err());
}

#[test]
fn test_add_submesh() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a mesh with one entry and one LOD
    let desc = MeshDesc {
        name: "mesh".to_string(),
        renderer: renderer.clone(),
        vertex_data: vec![0u8; 64], // 8 vertices * 8 bytes
        index_data: Some(vec![0u8; 24]), // 12 indices * 2 bytes
        vertex_layout: VertexLayout {
            bindings: vec![VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }],
            attributes: vec![VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }],
        },
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

    rm.create_mesh("mesh".to_string(), desc).unwrap();

    // Add a submesh to the LOD
    let submesh = SubMeshDesc {
        name: "armor".to_string(),
        vertex_offset: 4,
        vertex_count: 4,
        index_offset: 6,
        index_count: 6,
        topology: PrimitiveTopology::TriangleList,
    };

    let result = rm.add_submesh("mesh", 0, 0, submesh);
    assert!(result.is_ok());

    // Verify submesh was added
    let mesh = rm.mesh("mesh").unwrap();
    let entry = mesh.mesh_entry(0).unwrap();
    let lod = entry.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 2);
}

#[test]
fn test_add_submesh_to_nonexistent_mesh() {
    let mut rm = ResourceManager::new();

    let submesh = SubMeshDesc {
        name: "submesh".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: PrimitiveTopology::TriangleList,
    };

    let result = rm.add_submesh("nonexistent", 0, 0, submesh);
    assert!(result.is_err());
}

#[test]
fn test_add_pipeline_variant() {
    let mut rm = ResourceManager::new();
    let renderer = create_mock_renderer();

    // Create a pipeline with one variant
    let desc = create_test_pipeline_desc(renderer.clone(), "default");
    rm.create_pipeline("pipeline".to_string(), desc).unwrap();

    // Add another variant
    let variant = PipelineVariantDesc {
        name: "wireframe".to_string(),
        passes: vec![PipelinePassDesc {
            pipeline: crate::renderer::PipelineDesc {
                vertex_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("vert".to_string())),
                fragment_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("frag".to_string())),
                vertex_layout: VertexLayout {
                    bindings: vec![VertexBinding {
                        binding: 0,
                        stride: 8,
                        input_rate: VertexInputRate::Vertex,
                    }],
                    attributes: vec![VertexAttribute {
                        location: 0,
                        binding: 0,
                        format: BufferFormat::R32G32_SFLOAT,
                        offset: 0,
                    }],
                },
                topology: PrimitiveTopology::LineList,
                push_constant_ranges: vec![],
                descriptor_set_layouts: vec![],
                rasterization: Default::default(),
                depth_stencil: Default::default(),
                color_blend: Default::default(),
                multisample: Default::default(),
            },
        }],
    };

    let result = rm.add_pipeline_variant("pipeline", variant);
    assert!(result.is_ok());

    // Verify variant was added
    let pipeline = rm.pipeline("pipeline").unwrap();
    assert_eq!(pipeline.variant_count(), 2);
}

#[test]
fn test_add_pipeline_variant_to_nonexistent_pipeline() {
    let mut rm = ResourceManager::new();

    let variant = PipelineVariantDesc {
        name: "variant".to_string(),
        passes: vec![PipelinePassDesc {
            pipeline: crate::renderer::PipelineDesc {
                vertex_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("vert".to_string())),
                fragment_shader: Arc::new(crate::renderer::mock_renderer::MockShader::new("frag".to_string())),
                vertex_layout: VertexLayout {
                    bindings: vec![],
                    attributes: vec![],
                },
                topology: PrimitiveTopology::TriangleList,
                push_constant_ranges: vec![],
                descriptor_set_layouts: vec![],
                rasterization: Default::default(),
                depth_stencil: Default::default(),
                color_blend: Default::default(),
                multisample: Default::default(),
            },
        }],
    };

    let result = rm.add_pipeline_variant("nonexistent", variant);
    assert!(result.is_err());
}
