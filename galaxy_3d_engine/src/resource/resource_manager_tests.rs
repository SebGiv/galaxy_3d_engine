/// Tests for ResourceManager
///
/// These tests use MockGraphicsDevice to test ResourceManager logic without requiring a GPU.

use super::*;
use crate::graphics_device;
use crate::resource::{
    AtlasRegion, AtlasRegionDesc, LayerDesc, PipelinePassDesc,
    MeshLODDesc, SubMeshDesc, GeometryMeshRef, GeometrySubMeshRef,
    BufferKind, FieldDesc,
    MaterialTextureSlotDesc, LayerRef,
};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a MockGraphicsDevice wrapped in Arc<Mutex<>>
fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()))
}

/// Create a simple texture descriptor for testing
fn create_test_texture_desc(
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
   _name: &str,
    width: u32,
    height: u32,
) -> TextureDesc {
    TextureDesc {
        graphics_device,
        texture: graphics_device::TextureDesc {
            width,
            height,
            format: graphics_device::TextureFormat::R8G8B8A8_UNORM,
            usage: graphics_device::TextureUsage::Sampled,
            array_layers: 1,
            data: Some(graphics_device::TextureData::Single(vec![255u8; (width * height * 4) as usize])),
            mipmap: graphics_device::MipmapMode::None,
            texture_type: graphics_device::TextureType::Tex2D,
        },
        layers: vec![LayerDesc {
            name: "default".to_string(),
            layer_index: 0,
            data: None,
            regions: vec![],
        }],
    }
}

/// Create a simple geometry descriptor for testing
fn create_test_geometry_desc(
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    name: &str,
) -> GeometryDesc {
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
    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding {
            binding: 0,
            stride: 16, // 4 floats * 4 bytes
            input_rate: graphics_device::VertexInputRate::Vertex,
        }],
        attributes: vec![
            graphics_device::VertexAttribute {
                location: 0,
                binding: 0,
                format: graphics_device::BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
            graphics_device::VertexAttribute {
                location: 1,
                binding: 0,
                format: graphics_device::BufferFormat::R32G32_SFLOAT,
                offset: 8,
            },
        ],
    };

    GeometryDesc {
        name: name.to_string(),
        graphics_device,
        vertex_data: vertex_bytes,
        index_data: Some(index_bytes),
        vertex_layout,
        index_type: graphics_device::IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: name.to_string(),
            lods: vec![GeometryLODDesc {
                lod_index: 0,
                submeshes: vec![GeometrySubMeshDesc {
                    name: "default".to_string(),
                    vertex_offset: 0,
                    vertex_count: 4,
                    index_offset: 0,
                    index_count: 6,
                    topology: graphics_device::PrimitiveTopology::TriangleList,
                }],
            }],
        }],
    }
}

/// Create a simple pipeline descriptor for testing
fn create_test_pipeline_desc(
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    name: &str,
) -> PipelineDesc {
    let vertex_layout = graphics_device::VertexLayout {
        bindings: vec![graphics_device::VertexBinding {
            binding: 0,
            stride: 16,
            input_rate: graphics_device::VertexInputRate::Vertex,
        }],
        attributes: vec![
            graphics_device::VertexAttribute {
                location: 0,
                binding: 0,
                format: graphics_device::BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
        ],
    };

    PipelineDesc {
        graphics_device,
        variants: vec![PipelineVariantDesc {
            name: name.to_string(),
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
    }
}

/// Create a simple material descriptor for testing (no textures, no params)
fn create_test_material_desc(pipeline: &Arc<Pipeline>) -> MaterialDesc {
    MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![],
    }
}

/// Create a simple mesh descriptor for testing
///
/// Requires a geometry with at least one mesh (index 0) with one LOD (index 0)
/// and one submesh (index 0).
fn create_test_mesh_desc(
    geometry: &Arc<Geometry>,
    material: &Arc<Material>,
) -> MeshDesc {
    MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Index(0),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Index(0),
                material: material.clone(),
            }],
        }],
    }
}

/// Create prerequisites for mesh testing (geometry + pipeline + material)
///
/// Returns (geometry, material) Arcs for use in MeshDesc.
fn create_mesh_prerequisites(
    rm: &mut ResourceManager,
    graphics_device: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    suffix: &str,
) -> (Arc<Geometry>, Arc<Material>) {
    let geom_desc = create_test_geometry_desc(graphics_device.clone(), suffix);
    let geometry = rm.create_geometry(format!("geom_{}", suffix), geom_desc).unwrap();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), suffix);
    let pipeline = rm.create_pipeline(format!("pipe_{}", suffix), pipe_desc).unwrap();

    let mat_desc = create_test_material_desc(&pipeline);
    let material = rm.create_material(format!("mat_{}", suffix), mat_desc).unwrap();

    (geometry, material)
}

// ============================================================================
// Tests: ResourceManager Creation
// ============================================================================

#[test]
fn test_resource_manager_new() {
    let rm = ResourceManager::new();
    assert_eq!(rm.texture_count(), 0);
    assert_eq!(rm.geometry_count(), 0);
    assert_eq!(rm.pipeline_count(), 0);
    assert_eq!(rm.material_count(), 0);
    assert_eq!(rm.mesh_count(), 0);
}

// ============================================================================
// Tests: Texture Management
// ============================================================================

#[test]
fn test_create_texture() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_texture_desc(graphics_device.clone(), "test_texture", 256, 256);
    let _texture = rm.create_texture("test_texture".to_string(), desc).unwrap();

    assert_eq!(rm.texture_count(), 1);
    // texture has been created successfully (no direct width/height access)
}

#[test]
fn test_get_texture() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_texture_desc(graphics_device.clone(), "test_texture", 256, 256);
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
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_texture_desc(graphics_device.clone(), "test_texture", 256, 256);
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
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_texture_desc(graphics_device.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc1).unwrap();

    let desc2 = create_test_texture_desc(graphics_device.clone(), "test_texture", 512, 512);
    let result = rm.create_texture("test_texture".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.texture_count(), 1); // Still only one texture
}

#[test]
fn test_multiple_textures() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_texture_desc(graphics_device.clone(), "texture1", 256, 256);
    let desc2 = create_test_texture_desc(graphics_device.clone(), "texture2", 512, 512);
    let desc3 = create_test_texture_desc(graphics_device.clone(), "texture3", 128, 128);

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
    let graphics_device = create_mock_graphics_device();

    assert_eq!(rm.texture_count(), 0);

    let desc1 = create_test_texture_desc(graphics_device.clone(), "texture1", 256, 256);
    rm.create_texture("texture1".to_string(), desc1).unwrap();
    assert_eq!(rm.texture_count(), 1);

    let desc2 = create_test_texture_desc(graphics_device.clone(), "texture2", 512, 512);
    rm.create_texture("texture2".to_string(), desc2).unwrap();
    assert_eq!(rm.texture_count(), 2);

    rm.remove_texture("texture1");
    assert_eq!(rm.texture_count(), 1);

    rm.remove_texture("texture2");
    assert_eq!(rm.texture_count(), 0);
}

// ============================================================================
// Tests: Geometry Management
// ============================================================================

#[test]
fn test_create_geometry() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    let geom = rm.create_geometry("test_geom".to_string(), desc).unwrap();

    assert_eq!(rm.geometry_count(), 1);
    assert_eq!(geom.mesh_count(), 1);
}

#[test]
fn test_get_geometry() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    rm.create_geometry("test_geom".to_string(), desc).unwrap();

    let geom = rm.geometry("test_geom");
    assert!(geom.is_some());
}

#[test]
fn test_get_geometry_not_found() {
    let rm = ResourceManager::new();
    let geom = rm.geometry("nonexistent");
    assert!(geom.is_none());
}

#[test]
fn test_remove_geometry() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    rm.create_geometry("test_geom".to_string(), desc).unwrap();

    assert_eq!(rm.geometry_count(), 1);

    let removed = rm.remove_geometry("test_geom");
    assert!(removed);
    assert_eq!(rm.geometry_count(), 0);
}

#[test]
fn test_remove_geometry_not_found() {
    let mut rm = ResourceManager::new();
    let removed = rm.remove_geometry("nonexistent");
    assert!(!removed);
}

#[test]
fn test_duplicate_geometry_fails() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    rm.create_geometry("test_geom".to_string(), desc1).unwrap();

    let desc2 = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    let result = rm.create_geometry("test_geom".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.geometry_count(), 1);
}

#[test]
fn test_multiple_geometries() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_geometry_desc(graphics_device.clone(), "geom1");
    let desc2 = create_test_geometry_desc(graphics_device.clone(), "geom2");
    let desc3 = create_test_geometry_desc(graphics_device.clone(), "geom3");

    rm.create_geometry("geom1".to_string(), desc1).unwrap();
    rm.create_geometry("geom2".to_string(), desc2).unwrap();
    rm.create_geometry("geom3".to_string(), desc3).unwrap();

    assert_eq!(rm.geometry_count(), 3);
    assert!(rm.geometry("geom1").is_some());
    assert!(rm.geometry("geom2").is_some());
    assert!(rm.geometry("geom3").is_some());
}

#[test]
fn test_geometry_count() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    assert_eq!(rm.geometry_count(), 0);

    let desc1 = create_test_geometry_desc(graphics_device.clone(), "geom1");
    rm.create_geometry("geom1".to_string(), desc1).unwrap();
    assert_eq!(rm.geometry_count(), 1);

    let desc2 = create_test_geometry_desc(graphics_device.clone(), "geom2");
    rm.create_geometry("geom2".to_string(), desc2).unwrap();
    assert_eq!(rm.geometry_count(), 2);

    rm.remove_geometry("geom1");
    assert_eq!(rm.geometry_count(), 1);

    rm.remove_geometry("geom2");
    assert_eq!(rm.geometry_count(), 0);
}

// ============================================================================
// Tests: Pipeline Management
// ============================================================================

#[test]
fn test_create_pipeline() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
    let pipeline = rm.create_pipeline("test_pipeline".to_string(), desc).unwrap();

    assert_eq!(rm.pipeline_count(), 1);
    assert_eq!(pipeline.variant_count(), 1);
}

#[test]
fn test_get_pipeline() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
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
    let graphics_device = create_mock_graphics_device();

    let desc = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
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
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
    rm.create_pipeline("test_pipeline".to_string(), desc1).unwrap();

    let desc2 = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
    let result = rm.create_pipeline("test_pipeline".to_string(), desc2);

    assert!(result.is_err());
    assert_eq!(rm.pipeline_count(), 1);
}

#[test]
fn test_multiple_pipelines() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let desc1 = create_test_pipeline_desc(graphics_device.clone(), "pipeline1");
    let desc2 = create_test_pipeline_desc(graphics_device.clone(), "pipeline2");
    let desc3 = create_test_pipeline_desc(graphics_device.clone(), "pipeline3");

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
    let graphics_device = create_mock_graphics_device();

    assert_eq!(rm.pipeline_count(), 0);

    let desc1 = create_test_pipeline_desc(graphics_device.clone(), "pipeline1");
    rm.create_pipeline("pipeline1".to_string(), desc1).unwrap();
    assert_eq!(rm.pipeline_count(), 1);

    let desc2 = create_test_pipeline_desc(graphics_device.clone(), "pipeline2");
    rm.create_pipeline("pipeline2".to_string(), desc2).unwrap();
    assert_eq!(rm.pipeline_count(), 2);

    rm.remove_pipeline("pipeline1");
    assert_eq!(rm.pipeline_count(), 1);

    rm.remove_pipeline("pipeline2");
    assert_eq!(rm.pipeline_count(), 0);
}

// ============================================================================
// Tests: Material Management
// ============================================================================

#[test]
fn test_create_material() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = create_test_material_desc(&pipeline);
    let material = rm.create_material("body".to_string(), mat_desc).unwrap();

    assert_eq!(rm.material_count(), 1);
    assert_eq!(material.texture_slot_count(), 0);
    assert_eq!(material.param_count(), 0);
}

#[test]
fn test_get_material() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = create_test_material_desc(&pipeline);
    rm.create_material("body".to_string(), mat_desc).unwrap();

    let material = rm.material("body");
    assert!(material.is_some());
}

#[test]
fn test_get_material_not_found() {
    let rm = ResourceManager::new();
    let material = rm.material("nonexistent");
    assert!(material.is_none());
}

#[test]
fn test_remove_material() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = create_test_material_desc(&pipeline);
    rm.create_material("body".to_string(), mat_desc).unwrap();

    assert_eq!(rm.material_count(), 1);

    let removed = rm.remove_material("body");
    assert!(removed);
    assert_eq!(rm.material_count(), 0);
}

#[test]
fn test_remove_material_not_found() {
    let mut rm = ResourceManager::new();
    let removed = rm.remove_material("nonexistent");
    assert!(!removed);
}

#[test]
fn test_duplicate_material_fails() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc1 = create_test_material_desc(&pipeline);
    rm.create_material("body".to_string(), mat_desc1).unwrap();

    let mat_desc2 = create_test_material_desc(&pipeline);
    let result = rm.create_material("body".to_string(), mat_desc2);

    assert!(result.is_err());
    assert_eq!(rm.material_count(), 1);
}

#[test]
fn test_material_count() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    assert_eq!(rm.material_count(), 0);

    let mat_desc1 = create_test_material_desc(&pipeline);
    rm.create_material("mat1".to_string(), mat_desc1).unwrap();
    assert_eq!(rm.material_count(), 1);

    let mat_desc2 = create_test_material_desc(&pipeline);
    rm.create_material("mat2".to_string(), mat_desc2).unwrap();
    assert_eq!(rm.material_count(), 2);

    rm.remove_material("mat1");
    assert_eq!(rm.material_count(), 1);

    rm.remove_material("mat2");
    assert_eq!(rm.material_count(), 0);
}

// ============================================================================
// Tests: Mesh Management
// ============================================================================

#[test]
fn test_create_mesh() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let (geometry, material) = create_mesh_prerequisites(&mut rm, &graphics_device, "a");

    let mesh_desc = create_test_mesh_desc(&geometry, &material);
    let mesh = rm.create_mesh("hero".to_string(), mesh_desc).unwrap();

    assert_eq!(rm.mesh_count(), 1);
    assert_eq!(mesh.lod_count(), 1);
}

#[test]
fn test_get_mesh() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let (geometry, material) = create_mesh_prerequisites(&mut rm, &graphics_device, "a");

    let mesh_desc = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("hero".to_string(), mesh_desc).unwrap();

    let mesh = rm.mesh("hero");
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
    let graphics_device = create_mock_graphics_device();

    let (geometry, material) = create_mesh_prerequisites(&mut rm, &graphics_device, "a");

    let mesh_desc = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("hero".to_string(), mesh_desc).unwrap();

    assert_eq!(rm.mesh_count(), 1);

    let removed = rm.remove_mesh("hero");
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
    let graphics_device = create_mock_graphics_device();

    let (geometry, material) = create_mesh_prerequisites(&mut rm, &graphics_device, "a");

    let mesh_desc1 = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("hero".to_string(), mesh_desc1).unwrap();

    let mesh_desc2 = create_test_mesh_desc(&geometry, &material);
    let result = rm.create_mesh("hero".to_string(), mesh_desc2);

    assert!(result.is_err());
    assert_eq!(rm.mesh_count(), 1);
}

#[test]
fn test_mesh_count() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let (geometry, material) = create_mesh_prerequisites(&mut rm, &graphics_device, "a");

    assert_eq!(rm.mesh_count(), 0);

    let mesh_desc1 = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("mesh1".to_string(), mesh_desc1).unwrap();
    assert_eq!(rm.mesh_count(), 1);

    let mesh_desc2 = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("mesh2".to_string(), mesh_desc2).unwrap();
    assert_eq!(rm.mesh_count(), 2);

    rm.remove_mesh("mesh1");
    assert_eq!(rm.mesh_count(), 1);

    rm.remove_mesh("mesh2");
    assert_eq!(rm.mesh_count(), 0);
}

// ============================================================================
// Tests: Mixed Resource Management
// ============================================================================

#[test]
fn test_mixed_resources() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create one of each resource type
    let texture_desc = create_test_texture_desc(graphics_device.clone(), "texture", 256, 256);
    let geom_desc = create_test_geometry_desc(graphics_device.clone(), "geom");
    let pipeline_desc = create_test_pipeline_desc(graphics_device.clone(), "pipeline");

    rm.create_texture("texture".to_string(), texture_desc).unwrap();
    let geometry = rm.create_geometry("geom".to_string(), geom_desc).unwrap();
    let pipeline = rm.create_pipeline("pipeline".to_string(), pipeline_desc).unwrap();

    let mat_desc = create_test_material_desc(&pipeline);
    let material = rm.create_material("material".to_string(), mat_desc).unwrap();

    let mesh_desc = create_test_mesh_desc(&geometry, &material);
    rm.create_mesh("mesh".to_string(), mesh_desc).unwrap();

    assert_eq!(rm.texture_count(), 1);
    assert_eq!(rm.geometry_count(), 1);
    assert_eq!(rm.pipeline_count(), 1);
    assert_eq!(rm.material_count(), 1);
    assert_eq!(rm.mesh_count(), 1);

    assert!(rm.texture("texture").is_some());
    assert!(rm.geometry("geom").is_some());
    assert!(rm.pipeline("pipeline").is_some());
    assert!(rm.material("material").is_some());
    assert!(rm.mesh("mesh").is_some());
}

#[test]
fn test_clear_all_resources() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create multiple base resources
    for i in 0..3 {
        let texture_desc = create_test_texture_desc(graphics_device.clone(), &format!("texture{}", i), 256, 256);
        let geom_desc = create_test_geometry_desc(graphics_device.clone(), &format!("geom{}", i));
        let pipeline_desc = create_test_pipeline_desc(graphics_device.clone(), &format!("pipeline{}", i));

        rm.create_texture(format!("texture{}", i), texture_desc).unwrap();
        rm.create_geometry(format!("geom{}", i), geom_desc).unwrap();
        rm.create_pipeline(format!("pipeline{}", i), pipeline_desc).unwrap();
    }

    // Create materials and meshes (need references to existing resources)
    for i in 0..3 {
        let pipeline = rm.pipeline(&format!("pipeline{}", i)).unwrap().clone();
        let geometry = rm.geometry(&format!("geom{}", i)).unwrap().clone();

        let mat_desc = create_test_material_desc(&pipeline);
        let material = rm.create_material(format!("material{}", i), mat_desc).unwrap();

        let mesh_desc = create_test_mesh_desc(&geometry, &material);
        rm.create_mesh(format!("mesh{}", i), mesh_desc).unwrap();
    }

    assert_eq!(rm.texture_count(), 3);
    assert_eq!(rm.geometry_count(), 3);
    assert_eq!(rm.pipeline_count(), 3);
    assert_eq!(rm.material_count(), 3);
    assert_eq!(rm.mesh_count(), 3);

    // Remove all (meshes first, then materials, then base resources)
    for i in 0..3 {
        rm.remove_mesh(&format!("mesh{}", i));
        rm.remove_material(&format!("material{}", i));
        rm.remove_texture(&format!("texture{}", i));
        rm.remove_geometry(&format!("geom{}", i));
        rm.remove_pipeline(&format!("pipeline{}", i));
    }

    assert_eq!(rm.texture_count(), 0);
    assert_eq!(rm.geometry_count(), 0);
    assert_eq!(rm.pipeline_count(), 0);
    assert_eq!(rm.material_count(), 0);
    assert_eq!(rm.mesh_count(), 0);
}

// ============================================================================
// Tests: MockGraphicsDevice Verification
// ============================================================================

#[test]
fn test_mock_graphics_device_tracks_buffers() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>> = mock.clone();

    let desc = create_test_geometry_desc(graphics_device.clone(), "test_geom");
    rm.create_geometry("test_geom".to_string(), desc).unwrap();

    // Verify buffers were created
    let created_buffers = mock.lock().unwrap().get_created_buffers();
    assert!(created_buffers.len() >= 2); // At least vertex + index buffers
}

#[test]
fn test_mock_graphics_device_tracks_textures() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>> = mock.clone();

    let desc = create_test_texture_desc(graphics_device.clone(), "test_texture", 256, 256);
    rm.create_texture("test_texture".to_string(), desc).unwrap();

    // Verify texture was created
    let created_textures = mock.lock().unwrap().get_created_textures();
    assert!(created_textures.len() >= 1);
}

#[test]
fn test_mock_graphics_device_tracks_pipelines() {
    let mut rm = ResourceManager::new();
    let mock = Arc::new(Mutex::new(graphics_device::mock_graphics_device::MockGraphicsDevice::new()));
    let graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>> = mock.clone();

    let desc = create_test_pipeline_desc(graphics_device.clone(), "test_pipeline");
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
    let graphics_device = create_mock_graphics_device();

    // Create an indexed texture with 2 initial layers
    let desc = TextureDesc {
        graphics_device: graphics_device.clone(),
        texture: graphics_device::TextureDesc {
            width: 256,
            height: 256,
            format: graphics_device::TextureFormat::R8G8B8A8_UNORM,
            usage: graphics_device::TextureUsage::Sampled,
            array_layers: 4, // Indexed texture with 4 slots
            data: None,
            mipmap: graphics_device::MipmapMode::None,
            texture_type: graphics_device::TextureType::Array2D,
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
    let graphics_device = create_mock_graphics_device();

    // Create a simple texture (array_layers=1)
    let desc = create_test_texture_desc(graphics_device.clone(), "simple", 256, 256);
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
    let graphics_device = create_mock_graphics_device();

    // Create a texture with one layer
    let desc = create_test_texture_desc(graphics_device.clone(), "atlas", 256, 256);
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
fn test_add_geometry_mesh_to_existing_geometry() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create a geometry with no initial meshes
    let desc = GeometryDesc {
        name: "test_geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![0u8; 32], // 4 vertices * 8 bytes
        index_data: Some(vec![0u8; 12]), // 6 indices * 2 bytes
        vertex_layout: graphics_device::VertexLayout {
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
        },
        index_type: graphics_device::IndexType::U16,
        meshes: vec![], // No initial meshes
    };

    rm.create_geometry("geom".to_string(), desc).unwrap();

    // Add a mesh
    let mesh_desc = GeometryMeshDesc {
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
    };

    let result = rm.add_geometry_mesh("geom", mesh_desc);
    assert!(result.is_ok());

    // Verify mesh was added
    let geom = rm.geometry("geom").unwrap();
    assert_eq!(geom.mesh_count(), 1);
}

#[test]
fn test_add_geometry_mesh_to_nonexistent_geometry() {
    let mut rm = ResourceManager::new();

    let mesh_desc = GeometryMeshDesc {
        name: "hero".to_string(),
        lods: vec![],
    };

    let result = rm.add_geometry_mesh("nonexistent", mesh_desc);
    assert!(result.is_err());
}

#[test]
fn test_add_geometry_lod() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create a geometry with one mesh
    let desc = GeometryDesc {
        name: "geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![0u8; 64], // 8 vertices * 8 bytes
        index_data: Some(vec![0u8; 24]), // 12 indices * 2 bytes
        vertex_layout: graphics_device::VertexLayout {
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
        },
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

    rm.create_geometry("geom".to_string(), desc).unwrap();

    // Add a LOD to the mesh
    let lod = GeometryLODDesc {
        lod_index: 1,
        submeshes: vec![
            GeometrySubMeshDesc {
                name: "body_lod1".to_string(),
                vertex_offset: 4,
                vertex_count: 4,
                index_offset: 6,
                index_count: 6,
                topology: graphics_device::PrimitiveTopology::TriangleList,
            }
        ],
    };

    let result = rm.add_geometry_lod("geom", 0, lod);
    assert!(result.is_ok());

    // Verify LOD was added
    let geom = rm.geometry("geom").unwrap();
    let mesh = geom.mesh(0).unwrap();
    assert_eq!(mesh.lod_count(), 2);
}

#[test]
fn test_add_geometry_lod_to_nonexistent_geometry() {
    let mut rm = ResourceManager::new();

    let lod = GeometryLODDesc {
        lod_index: 0,
        submeshes: vec![],
    };

    let result = rm.add_geometry_lod("nonexistent", 0, lod);
    assert!(result.is_err());
}

#[test]
fn test_add_geometry_submesh() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create a geometry with one mesh and one LOD
    let desc = GeometryDesc {
        name: "geom".to_string(),
        graphics_device: graphics_device.clone(),
        vertex_data: vec![0u8; 64], // 8 vertices * 8 bytes
        index_data: Some(vec![0u8; 24]), // 12 indices * 2 bytes
        vertex_layout: graphics_device::VertexLayout {
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
        },
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

    rm.create_geometry("geom".to_string(), desc).unwrap();

    // Add a submesh to the LOD
    let submesh = GeometrySubMeshDesc {
        name: "armor".to_string(),
        vertex_offset: 4,
        vertex_count: 4,
        index_offset: 6,
        index_count: 6,
        topology: graphics_device::PrimitiveTopology::TriangleList,
    };

    let result = rm.add_geometry_submesh("geom", 0, 0, submesh);
    assert!(result.is_ok());

    // Verify submesh was added
    let geom = rm.geometry("geom").unwrap();
    let mesh = geom.mesh(0).unwrap();
    let lod = mesh.lod(0).unwrap();
    assert_eq!(lod.submesh_count(), 2);
}

#[test]
fn test_add_geometry_submesh_to_nonexistent_geometry() {
    let mut rm = ResourceManager::new();

    let submesh = GeometrySubMeshDesc {
        name: "submesh".to_string(),
        vertex_offset: 0,
        vertex_count: 4,
        index_offset: 0,
        index_count: 6,
        topology: graphics_device::PrimitiveTopology::TriangleList,
    };

    let result = rm.add_geometry_submesh("nonexistent", 0, 0, submesh);
    assert!(result.is_err());
}

#[test]
fn test_add_pipeline_variant() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create a pipeline with one variant
    let desc = create_test_pipeline_desc(graphics_device.clone(), "default");
    rm.create_pipeline("pipeline".to_string(), desc).unwrap();

    // Add another variant
    let variant = PipelineVariantDesc {
        name: "wireframe".to_string(),
        passes: vec![PipelinePassDesc {
            pipeline: graphics_device::PipelineDesc {
                vertex_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("vert".to_string())),
                fragment_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("frag".to_string())),
                vertex_layout: graphics_device::VertexLayout {
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
                },
                topology: graphics_device::PrimitiveTopology::LineList,
                push_constant_ranges: vec![],
                binding_group_layouts: vec![],
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
            pipeline: graphics_device::PipelineDesc {
                vertex_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("vert".to_string())),
                fragment_shader: Arc::new(graphics_device::mock_graphics_device::MockShader::new("frag".to_string())),
                vertex_layout: graphics_device::VertexLayout {
                    bindings: vec![],
                    attributes: vec![],
                },
                topology: graphics_device::PrimitiveTopology::TriangleList,
                push_constant_ranges: vec![],
                binding_group_layouts: vec![],
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

// ============================================================================
// Tests: Material Slot Allocation
// ============================================================================

#[test]
fn test_material_slot_id_assigned() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat0 = rm.create_material("mat0".to_string(), create_test_material_desc(&pipeline)).unwrap();
    let mat1 = rm.create_material("mat1".to_string(), create_test_material_desc(&pipeline)).unwrap();
    let mat2 = rm.create_material("mat2".to_string(), create_test_material_desc(&pipeline)).unwrap();

    assert_eq!(mat0.slot_id(), 0);
    assert_eq!(mat1.slot_id(), 1);
    assert_eq!(mat2.slot_id(), 2);
}

#[test]
fn test_material_slot_recycled_after_remove() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat0 = rm.create_material("mat0".to_string(), create_test_material_desc(&pipeline)).unwrap();
    let _mat1 = rm.create_material("mat1".to_string(), create_test_material_desc(&pipeline)).unwrap();

    assert_eq!(mat0.slot_id(), 0);

    // Remove mat0, its slot should be recycled
    rm.remove_material("mat0");

    // New material should reuse slot 0
    let mat2 = rm.create_material("mat2".to_string(), create_test_material_desc(&pipeline)).unwrap();
    assert_eq!(mat2.slot_id(), 0);
}

#[test]
fn test_material_slot_high_water_mark() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    assert_eq!(rm.material_slot_high_water_mark(), 0);
    assert_eq!(rm.material_slot_count(), 0);

    rm.create_material("mat0".to_string(), create_test_material_desc(&pipeline)).unwrap();
    assert_eq!(rm.material_slot_high_water_mark(), 1);
    assert_eq!(rm.material_slot_count(), 1);

    rm.create_material("mat1".to_string(), create_test_material_desc(&pipeline)).unwrap();
    assert_eq!(rm.material_slot_high_water_mark(), 2);
    assert_eq!(rm.material_slot_count(), 2);

    rm.remove_material("mat0");
    assert_eq!(rm.material_slot_high_water_mark(), 2); // doesn't shrink
    assert_eq!(rm.material_slot_count(), 1); // but count decreases

    rm.create_material("mat2".to_string(), create_test_material_desc(&pipeline)).unwrap();
    assert_eq!(rm.material_slot_high_water_mark(), 2); // reused slot, no increase
    assert_eq!(rm.material_slot_count(), 2);
}

// ============================================================================
// Tests: sync_materials_to_buffer
// ============================================================================

#[test]
fn test_sync_materials_basic() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
            ("color".to_string(), ParamValue::Vec4([1.0, 0.0, 0.0, 1.0])),
        ],
    };
    rm.create_material("body".to_string(), mat_desc).unwrap();

    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "roughness".to_string(), field_type: FieldType::Float },
            FieldDesc { name: "color".to_string(), field_type: FieldType::Vec4 },
        ],
        count: 4,
    }).unwrap();

    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

#[test]
fn test_sync_materials_type_mismatch_skips() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Material has "roughness" as Float
    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
        ],
    };
    rm.create_material("body".to_string(), mat_desc).unwrap();

    // Buffer has "roughness" as Vec4 (type mismatch)
    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "roughness".to_string(), field_type: FieldType::Vec4 },
        ],
        count: 4,
    }).unwrap();

    // Should succeed (warning, not error)
    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

#[test]
fn test_sync_materials_missing_field_skips() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Material has "roughness" param
    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
        ],
    };
    rm.create_material("body".to_string(), mat_desc).unwrap();

    // Buffer has no "roughness" field
    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "metallic".to_string(), field_type: FieldType::Float },
        ],
        count: 4,
    }).unwrap();

    // Should succeed (warning, not error)
    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

#[test]
fn test_sync_materials_bool_to_uint() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Material has Bool param
    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![
            ("is_metallic".to_string(), ParamValue::Bool(true)),
        ],
    };
    rm.create_material("body".to_string(), mat_desc).unwrap();

    // Buffer has matching UInt field (Bool maps to UInt)
    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "is_metallic".to_string(), field_type: FieldType::UInt },
        ],
        count: 4,
    }).unwrap();

    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

#[test]
fn test_sync_materials_vec3_padding() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Material has Vec3 param (12 bytes raw, needs padding to 16)
    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![
            ("normal".to_string(), ParamValue::Vec3([1.0, 0.0, 0.0])),
        ],
    };
    rm.create_material("body".to_string(), mat_desc).unwrap();

    // Buffer expects Vec3 field (16 bytes with padding)
    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "normal".to_string(), field_type: FieldType::Vec3 },
        ],
        count: 4,
    }).unwrap();

    // This validates that param_to_padded_bytes produces 16 bytes for Vec3,
    // because update_field strictly validates data.len() == field_size
    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

#[test]
fn test_sync_materials_slot_exceeds_buffer() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Create 3 materials → slot ids 0, 1, 2
    for i in 0..3 {
        let mat_desc = MaterialDesc {
            pipeline: pipeline.clone(),
            textures: vec![],
            params: vec![
                ("roughness".to_string(), ParamValue::Float(0.5)),
            ],
        };
        rm.create_material(format!("mat{}", i), mat_desc).unwrap();
    }

    // Buffer only has 1 element (count=1) → slots 1 and 2 exceed
    let buffer = rm.create_buffer("material_buffer".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "roughness".to_string(), field_type: FieldType::Float },
        ],
        count: 1,
    }).unwrap();

    // Should succeed (warning, not error)
    let result = rm.sync_materials_to_buffer(&buffer);
    assert!(result.is_ok());
}

// ============================================================================
// Tests: Private Helpers (compatible_field_type, param_to_padded_bytes)
// ============================================================================

#[test]
fn test_compatible_field_type_all_variants() {
    assert_eq!(compatible_field_type(&ParamValue::Float(0.0)), FieldType::Float);
    assert_eq!(compatible_field_type(&ParamValue::Vec2([0.0; 2])), FieldType::Vec2);
    assert_eq!(compatible_field_type(&ParamValue::Vec3([0.0; 3])), FieldType::Vec3);
    assert_eq!(compatible_field_type(&ParamValue::Vec4([0.0; 4])), FieldType::Vec4);
    assert_eq!(compatible_field_type(&ParamValue::Int(0)), FieldType::Int);
    assert_eq!(compatible_field_type(&ParamValue::UInt(0)), FieldType::UInt);
    assert_eq!(compatible_field_type(&ParamValue::Mat3([[0.0; 3]; 3])), FieldType::Mat3);
    assert_eq!(compatible_field_type(&ParamValue::Mat4([[0.0; 4]; 4])), FieldType::Mat4);
}

#[test]
fn test_compatible_field_type_bool_maps_to_uint() {
    assert_eq!(compatible_field_type(&ParamValue::Bool(true)), FieldType::UInt);
    assert_eq!(compatible_field_type(&ParamValue::Bool(false)), FieldType::UInt);
}

#[test]
fn test_param_to_padded_bytes_vec3() {
    let value = ParamValue::Vec3([1.0, 2.0, 3.0]);
    let bytes = param_to_padded_bytes(&value);
    assert_eq!(bytes.len(), 16); // 12 data + 4 padding

    // Verify the actual float values
    let f0 = f32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    let f1 = f32::from_ne_bytes(bytes[4..8].try_into().unwrap());
    let f2 = f32::from_ne_bytes(bytes[8..12].try_into().unwrap());
    assert!((f0 - 1.0).abs() < f32::EPSILON);
    assert!((f1 - 2.0).abs() < f32::EPSILON);
    assert!((f2 - 3.0).abs() < f32::EPSILON);

    // Last 4 bytes should be zero padding
    assert_eq!(&bytes[12..16], &[0u8; 4]);
}

#[test]
fn test_param_to_padded_bytes_mat3() {
    let value = ParamValue::Mat3([
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ]);
    let bytes = param_to_padded_bytes(&value);
    assert_eq!(bytes.len(), 48); // 3 rows x (12 data + 4 padding)

    // Each row is 16 bytes: 3 floats + 4 bytes padding
    // Row 0: [1.0, 0.0, 0.0, pad]
    let r0f0 = f32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    assert!((r0f0 - 1.0).abs() < f32::EPSILON);
    assert_eq!(&bytes[12..16], &[0u8; 4]); // row 0 padding

    // Row 1: [0.0, 1.0, 0.0, pad]
    let r1f1 = f32::from_ne_bytes(bytes[20..24].try_into().unwrap());
    assert!((r1f1 - 1.0).abs() < f32::EPSILON);
    assert_eq!(&bytes[28..32], &[0u8; 4]); // row 1 padding

    // Row 2: [0.0, 0.0, 1.0, pad]
    let r2f2 = f32::from_ne_bytes(bytes[40..44].try_into().unwrap());
    assert!((r2f2 - 1.0).abs() < f32::EPSILON);
    assert_eq!(&bytes[44..48], &[0u8; 4]); // row 2 padding
}

#[test]
fn test_param_to_padded_bytes_float_unchanged() {
    let value = ParamValue::Float(3.14);
    let bytes = param_to_padded_bytes(&value);
    assert_eq!(bytes.len(), 4);
    let f = f32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    assert!((f - 3.14).abs() < f32::EPSILON);
}

#[test]
fn test_param_to_padded_bytes_vec4_unchanged() {
    let value = ParamValue::Vec4([1.0, 2.0, 3.0, 4.0]);
    let bytes = param_to_padded_bytes(&value);
    assert_eq!(bytes.len(), 16); // same as native, no padding needed
}

// ============================================================================
// Tests: create_default_material_buffer
// ============================================================================

#[test]
fn test_default_material_buffer_creation() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let buffer = rm.create_default_material_buffer(
        "material".to_string(), graphics_device, 4,
    );
    assert!(buffer.is_ok());
    let buffer = buffer.unwrap();
    assert_eq!(buffer.count(), 4);
    assert_eq!(buffer.kind(), BufferKind::Storage);
}

#[test]
fn test_default_material_buffer_fields() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let buffer = rm.create_default_material_buffer(
        "material".to_string(), graphics_device, 1,
    ).unwrap();

    // All 14 fields must exist
    assert!(buffer.field_id("baseColor").is_some());
    assert!(buffer.field_id("emissiveColor").is_some());
    assert!(buffer.field_id("metallic").is_some());
    assert!(buffer.field_id("roughness").is_some());
    assert!(buffer.field_id("normalScale").is_some());
    assert!(buffer.field_id("ao").is_some());
    assert!(buffer.field_id("alphaCutoff").is_some());
    assert!(buffer.field_id("ior").is_some());
    assert!(buffer.field_id("albedoTexture").is_some());
    assert!(buffer.field_id("normalTexture").is_some());
    assert!(buffer.field_id("metallicRoughnessTexture").is_some());
    assert!(buffer.field_id("emissiveTexture").is_some());
    assert!(buffer.field_id("aoTexture").is_some());
    assert!(buffer.field_id("flags").is_some());
}

#[test]
fn test_default_material_buffer_stride() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let buffer = rm.create_default_material_buffer(
        "material".to_string(), graphics_device, 1,
    ).unwrap();

    // 2×Vec4(16) + 6×Float(4) + 6×UInt(4) = 32 + 24 + 24 = 80
    assert_eq!(buffer.stride(), 80);
}

#[test]
fn test_default_material_buffer_defaults() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Verify that creating a buffer with multiple slots and writing all defaults
    // completes without error (MockBuffer::update is a no-op, so we can't read
    // back the values, but we can verify all update_field calls succeed)
    let buffer = rm.create_default_material_buffer(
        "material".to_string(), graphics_device, 4,
    ).unwrap();

    assert_eq!(buffer.count(), 4);
}

// ============================================================================
// sync_materials_to_buffer — texture slots
// ============================================================================

#[test]
fn test_sync_materials_texture_slot_writes_layer() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    // Create a texture with 3 layers
    let tex_desc = TextureDesc {
        graphics_device: graphics_device.clone(),
        texture: graphics_device::TextureDesc {
            width: 64, height: 64,
            format: graphics_device::TextureFormat::R8G8B8A8_UNORM,
            usage: graphics_device::TextureUsage::Sampled,
            array_layers: 3,
            data: None,
            mipmap: graphics_device::MipmapMode::None,
            texture_type: graphics_device::TextureType::Array2D,
        },
        layers: vec![
            LayerDesc { name: "grass".to_string(), layer_index: 0, data: None, regions: vec![] },
            LayerDesc { name: "stone".to_string(), layer_index: 1, data: None, regions: vec![] },
            LayerDesc { name: "sand".to_string(),  layer_index: 2, data: None, regions: vec![] },
        ],
    };
    let texture = rm.create_texture("atlas".to_string(), tex_desc).unwrap();

    // Pipeline + material with a texture slot targeting layer 2
    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![MaterialTextureSlotDesc {
            name: "albedoTexture".to_string(),
            texture: texture.clone(),
            layer: Some(LayerRef::Index(2)),
            region: None,
            sampler_type: graphics_device::SamplerType::LinearRepeat,
        }],
        params: vec![],
    };
    rm.create_material("ground".to_string(), mat_desc).unwrap();

    // Buffer with a UInt field matching the slot name
    let buffer = rm.create_buffer("mat_buf".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "albedoTexture".to_string(), field_type: FieldType::UInt },
        ],
        count: 4,
    }).unwrap();

    // Sync should succeed (layer 2 written to buffer)
    assert!(rm.sync_materials_to_buffer(&buffer).is_ok());
}

#[test]
fn test_sync_materials_texture_slot_no_layer_writes_zero() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let tex_desc = create_test_texture_desc(graphics_device.clone(), "tex", 64, 64);
    let texture = rm.create_texture("tex".to_string(), tex_desc).unwrap();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    // Material with texture slot but NO layer (layer = None → writes 0)
    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![MaterialTextureSlotDesc {
            name: "albedoTexture".to_string(),
            texture: texture.clone(),
            layer: None,
            region: None,
            sampler_type: graphics_device::SamplerType::LinearRepeat,
        }],
        params: vec![],
    };
    rm.create_material("flat".to_string(), mat_desc).unwrap();

    let buffer = rm.create_buffer("mat_buf".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "albedoTexture".to_string(), field_type: FieldType::UInt },
        ],
        count: 4,
    }).unwrap();

    assert!(rm.sync_materials_to_buffer(&buffer).is_ok());
}

#[test]
fn test_sync_materials_texture_slot_missing_field_skips() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let tex_desc = create_test_texture_desc(graphics_device.clone(), "tex", 64, 64);
    let texture = rm.create_texture("tex".to_string(), tex_desc).unwrap();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![MaterialTextureSlotDesc {
            name: "albedoTexture".to_string(),
            texture: texture.clone(),
            layer: None,
            region: None,
            sampler_type: graphics_device::SamplerType::LinearRepeat,
        }],
        params: vec![],
    };
    rm.create_material("mat".to_string(), mat_desc).unwrap();

    // Buffer has NO field named "albedoTexture" → warning, no error
    let buffer = rm.create_buffer("mat_buf".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "roughness".to_string(), field_type: FieldType::Float },
        ],
        count: 4,
    }).unwrap();

    assert!(rm.sync_materials_to_buffer(&buffer).is_ok());
}

#[test]
fn test_sync_materials_texture_slot_wrong_type_skips() {
    let mut rm = ResourceManager::new();
    let graphics_device = create_mock_graphics_device();

    let tex_desc = create_test_texture_desc(graphics_device.clone(), "tex", 64, 64);
    let texture = rm.create_texture("tex".to_string(), tex_desc).unwrap();

    let pipe_desc = create_test_pipeline_desc(graphics_device.clone(), "standard");
    let pipeline = rm.create_pipeline("standard".to_string(), pipe_desc).unwrap();

    let mat_desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![MaterialTextureSlotDesc {
            name: "albedoTexture".to_string(),
            texture: texture.clone(),
            layer: None,
            region: None,
            sampler_type: graphics_device::SamplerType::LinearRepeat,
        }],
        params: vec![],
    };
    rm.create_material("mat".to_string(), mat_desc).unwrap();

    // Buffer has "albedoTexture" but as Float, not UInt → warning, no error
    let buffer = rm.create_buffer("mat_buf".to_string(), BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![
            FieldDesc { name: "albedoTexture".to_string(), field_type: FieldType::Float },
        ],
        count: 4,
    }).unwrap();

    assert!(rm.sync_materials_to_buffer(&buffer).is_ok());
}
