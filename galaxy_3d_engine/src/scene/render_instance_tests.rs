/// Tests for RenderInstance, RenderLOD, RenderSubMesh, and AABB
///
/// These tests use MockRenderer to create real Geometry, Pipeline, Texture, Material,
/// and Mesh resources, then validate RenderInstance creation, data extraction, accessors,
/// and error handling.

use super::*;
use crate::renderer::mock_renderer::{MockRenderer, MockShader};
use crate::renderer::{
    PrimitiveTopology, BufferFormat, TextureFormat, TextureUsage,
    MipmapMode, TextureData, SamplerType,
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, IndexType,
};
use crate::resource::geometry::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::{
    Pipeline, PipelineDesc, PipelineVariantDesc, PipelinePassDesc,
};
use crate::resource::material::{
    Material, MaterialDesc, MaterialTextureSlotDesc, ParamValue,
};
use crate::resource::texture::{Texture, TextureDesc, LayerDesc};
use crate::resource::mesh::{
    Mesh, MeshDesc, MeshLODDesc, SubMeshDesc,
    GeometryMeshRef, GeometrySubMeshRef,
};
use crate::utils::SlotAllocator;
use glam::{Vec3, Mat4};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_renderer() -> Arc<Mutex<dyn crate::renderer::Renderer>> {
    Arc::new(Mutex::new(MockRenderer::new()))
}

fn create_vertex_layout() -> VertexLayout {
    VertexLayout {
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
    }
}

/// Create a geometry with:
/// - GeometryMesh "object": LOD 0 (body, head), LOD 1 (body_low)
fn create_test_geometry(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Geometry> {
    let desc = GeometryDesc {
        name: "test_geo".to_string(),
        renderer,
        vertex_data: vec![0u8; 120],  // 15 vertices * 8 bytes
        index_data: Some(vec![0u8; 36]), // 18 indices * 2 bytes
        vertex_layout: create_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "object".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body".to_string(),
                                vertex_offset: 0, vertex_count: 4,
                                index_offset: 0, index_count: 6,
                                topology: PrimitiveTopology::TriangleList,
                            },
                            GeometrySubMeshDesc {
                                name: "head".to_string(),
                                vertex_offset: 4, vertex_count: 4,
                                index_offset: 6, index_count: 6,
                                topology: PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                    GeometryLODDesc {
                        lod_index: 1,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "body_low".to_string(),
                                vertex_offset: 8, vertex_count: 3,
                                index_offset: 12, index_count: 3,
                                topology: PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                ],
            },
        ],
    };
    Arc::new(Geometry::from_desc(desc).unwrap())
}

/// Create a geometry with no index buffer
fn create_non_indexed_geometry(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Geometry> {
    let desc = GeometryDesc {
        name: "non_indexed_geo".to_string(),
        renderer,
        vertex_data: vec![0u8; 48],  // 6 vertices * 8 bytes
        index_data: None,
        vertex_layout: create_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![
            GeometryMeshDesc {
                name: "simple".to_string(),
                lods: vec![
                    GeometryLODDesc {
                        lod_index: 0,
                        submeshes: vec![
                            GeometrySubMeshDesc {
                                name: "main".to_string(),
                                vertex_offset: 0, vertex_count: 6,
                                index_offset: 0, index_count: 0,
                                topology: PrimitiveTopology::TriangleList,
                            },
                        ],
                    },
                ],
            },
        ],
    };
    Arc::new(Geometry::from_desc(desc).unwrap())
}

fn create_render_pipeline_desc() -> crate::renderer::PipelineDesc {
    crate::renderer::PipelineDesc {
        vertex_shader: Arc::new(MockShader::new("vert".to_string())),
        fragment_shader: Arc::new(MockShader::new("frag".to_string())),
        vertex_layout: create_vertex_layout(),
        topology: PrimitiveTopology::TriangleList,
        push_constant_ranges: vec![],
        binding_group_layouts: vec![],
        rasterization: Default::default(),
        depth_stencil: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
    }
}

/// Create a pipeline with a single variant "default" (1 pass)
fn create_test_pipeline(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Pipeline> {
    let desc = PipelineDesc {
        renderer,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: create_render_pipeline_desc(),
            }],
        }],
    };
    Arc::new(Pipeline::from_desc(desc).unwrap())
}

/// Create a pipeline with 2 variants: "default" (1 pass) and "shadow" (1 pass)
fn create_multi_variant_pipeline(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Pipeline> {
    let desc = PipelineDesc {
        renderer,
        variants: vec![
            PipelineVariantDesc {
                name: "default".to_string(),
                passes: vec![PipelinePassDesc {
                    pipeline: create_render_pipeline_desc(),
                }],
            },
            PipelineVariantDesc {
                name: "shadow".to_string(),
                passes: vec![PipelinePassDesc {
                    pipeline: create_render_pipeline_desc(),
                }],
            },
        ],
    };
    Arc::new(Pipeline::from_desc(desc).unwrap())
}

/// Create a pipeline with 1 variant "default" (2 passes: base + outline)
fn create_multi_pass_pipeline(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Pipeline> {
    let desc = PipelineDesc {
        renderer,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![
                PipelinePassDesc { pipeline: create_render_pipeline_desc() },
                PipelinePassDesc { pipeline: create_render_pipeline_desc() },
            ],
        }],
    };
    Arc::new(Pipeline::from_desc(desc).unwrap())
}

fn create_test_texture(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Texture> {
    let desc = TextureDesc {
        renderer,
        texture: crate::renderer::TextureDesc {
            width: 64,
            height: 64,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            data: Some(TextureData::Single(vec![255u8; 64 * 64 * 4])),
            mipmap: MipmapMode::None,
            texture_type: crate::renderer::TextureType::Tex2D,
        },
        layers: vec![LayerDesc {
            name: "default".to_string(),
            layer_index: 0,
            data: None,
            regions: vec![],
        }],
    };
    Arc::new(Texture::from_desc(desc).unwrap())
}

fn create_test_material(pipeline: &Arc<Pipeline>) -> Arc<Material> {
    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![("color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0]))],
    };
    Arc::new(Material::from_desc(0, desc).unwrap())
}

fn create_test_material_with_texture(
    pipeline: &Arc<Pipeline>,
    texture: &Arc<Texture>,
) -> Arc<Material> {
    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![MaterialTextureSlotDesc {
            name: "diffuse".to_string(),
            texture: texture.clone(),
            layer: None,
            region: None,
            sampler_type: SamplerType::LinearRepeat,
        }],
        params: vec![
            ("roughness".to_string(), ParamValue::Float(0.8)),
            ("metallic".to_string(), ParamValue::Float(0.0)),
        ],
    };
    Arc::new(Material::from_desc(0, desc).unwrap())
}

fn create_test_aabb() -> AABB {
    AABB {
        min: Vec3::new(-1.0, -1.0, -1.0),
        max: Vec3::new(1.0, 1.0, 1.0),
    }
}

/// Create a simple mesh (single LOD, single submesh, weapon "blade")
fn create_simple_mesh(
    geometry: &Arc<Geometry>,
    material: &Arc<Material>,
) -> Mesh {
    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("object".to_string()),
        lods: vec![
            MeshLODDesc {
                lod_index: 0,
                submeshes: vec![
                    SubMeshDesc {
                        submesh: GeometrySubMeshRef::Name("body".to_string()),
                        material: material.clone(),
                    },
                    SubMeshDesc {
                        submesh: GeometrySubMeshRef::Name("head".to_string()),
                        material: material.clone(),
                    },
                ],
            },
            MeshLODDesc {
                lod_index: 1,
                submeshes: vec![
                    SubMeshDesc {
                        submesh: GeometrySubMeshRef::Name("body_low".to_string()),
                        material: material.clone(),
                    },
                ],
            },
        ],
    };
    Mesh::from_desc(desc).unwrap()
}

/// Create a mesh from non-indexed geometry
fn create_non_indexed_mesh(
    geometry: &Arc<Geometry>,
    material: &Arc<Material>,
) -> Mesh {
    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("simple".to_string()),
        lods: vec![MeshLODDesc {
            lod_index: 0,
            submeshes: vec![SubMeshDesc {
                submesh: GeometrySubMeshRef::Name("main".to_string()),
                material: material.clone(),
            }],
        }],
    };
    Mesh::from_desc(desc).unwrap()
}

/// Helper: create a RenderInstance with a fresh slot allocator
/// (most tests don't need to inspect draw_slot values)
fn create_test_render_instance(
    mesh: &Mesh,
    matrix: Mat4,
    aabb: AABB,
    variant_index: usize,
) -> crate::error::Result<RenderInstance> {
    let mut alloc = SlotAllocator::new();
    RenderInstance::from_mesh(mesh, matrix, aabb, variant_index, &mut alloc)
}

// ============================================================================
// Tests: AABB
// ============================================================================

#[test]
fn test_aabb_creation() {
    let aabb = AABB {
        min: Vec3::new(-1.0, -2.0, -3.0),
        max: Vec3::new(1.0, 2.0, 3.0),
    };
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
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
    let aabb = create_test_aabb();

    let instance = create_test_render_instance(&mesh, matrix, aabb, 0).unwrap();

    assert_eq!(*instance.world_matrix(), matrix);
    assert_eq!(instance.variant_index(), 0);
    assert!(instance.is_visible());
    assert_eq!(instance.flags(), FLAG_VISIBLE);
}

#[test]
fn test_from_mesh_lod_count() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Mesh has 2 LODs
    assert_eq!(instance.lod_count(), 2);
    assert!(instance.lod(0).is_some());
    assert!(instance.lod(1).is_some());
    assert!(instance.lod(2).is_none());
}

#[test]
fn test_from_mesh_submesh_count_per_lod() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // LOD 0: body + head = 2 submeshes
    assert_eq!(instance.lod(0).unwrap().sub_mesh_count(), 2);
    // LOD 1: body_low = 1 submesh
    assert_eq!(instance.lod(1).unwrap().sub_mesh_count(), 1);
}

#[test]
fn test_from_mesh_extracts_geometry_data() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // LOD 0, submesh 0 = "body": vertex_offset=0, vertex_count=4, index_offset=0, index_count=6
    let sm0 = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm0.vertex_offset(), 0);
    assert_eq!(sm0.vertex_count(), 4);
    assert_eq!(sm0.index_offset(), 0);
    assert_eq!(sm0.index_count(), 6);
    assert_eq!(sm0.topology(), PrimitiveTopology::TriangleList);

    // LOD 0, submesh 1 = "head": vertex_offset=4, vertex_count=4, index_offset=6, index_count=6
    let sm1 = instance.lod(0).unwrap().sub_mesh(1).unwrap();
    assert_eq!(sm1.vertex_offset(), 4);
    assert_eq!(sm1.vertex_count(), 4);
    assert_eq!(sm1.index_offset(), 6);
    assert_eq!(sm1.index_count(), 6);

    // LOD 1, submesh 0 = "body_low": vertex_offset=8, vertex_count=3
    let sm_low = instance.lod(1).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm_low.vertex_offset(), 8);
    assert_eq!(sm_low.vertex_count(), 3);
    assert_eq!(sm_low.index_offset(), 12);
    assert_eq!(sm_low.index_count(), 3);
}

#[test]
fn test_from_mesh_extracts_pipeline_passes() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Single variant, single pass → 1 pass per submesh
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.passes().len(), 1);
}

#[test]
fn test_from_mesh_multi_pass_pipeline() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_multi_pass_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Multi-pass variant → 2 passes per submesh
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.passes().len(), 2);
}

#[test]
fn test_from_mesh_render_pass_structure() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Each submesh has passes with pipeline, binding groups, push constants
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.passes().len(), 1);

    let pass = &sm.passes()[0];
    // Pipeline should be set
    assert!(pass.pipeline().as_ref().reflection().binding_count() == 0);
    // No texture binding groups (global set 0 is owned by Scene)
    assert_eq!(pass.texture_binding_groups().len(), 0);
}

#[test]
fn test_from_mesh_material_with_texture_no_matching_reflection() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_test_texture(renderer.clone());
    let material = create_test_material_with_texture(&pipeline, &texture);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Material has textures and params, but mock reflection has no matching bindings
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.passes().len(), 1);
    // No texture binding groups (no matching reflection; global set 0 is owned by Scene)
    assert_eq!(sm.passes()[0].texture_binding_groups().len(), 0);
}

#[test]
fn test_from_mesh_non_indexed_geometry() {
    let renderer = create_mock_renderer();
    let geometry = create_non_indexed_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_non_indexed_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // No index buffer
    assert!(instance.index_buffer().is_none());

    // Submesh has vertex data but no index data
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.vertex_count(), 6);
    assert_eq!(sm.index_count(), 0);
}

#[test]
fn test_from_mesh_indexed_geometry_has_buffers() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Has both vertex and index buffer
    assert!(instance.index_buffer().is_some());
}

#[test]
fn test_from_mesh_variant_index_selection() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_multi_variant_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    // Use variant 1 ("shadow")
    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 1,
    ).unwrap();

    assert_eq!(instance.variant_index(), 1);
    // Passes should still be extracted (from variant 1)
    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();
    assert_eq!(sm.passes().len(), 1);
}

#[test]
fn test_from_mesh_default_flags() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Default flags = FLAG_VISIBLE only
    assert_eq!(instance.flags(), FLAG_VISIBLE);
    assert!(instance.is_visible());
}

#[test]
fn test_from_mesh_stores_bounding_box() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let aabb = AABB {
        min: Vec3::new(-5.0, 0.0, -5.0),
        max: Vec3::new(5.0, 10.0, 5.0),
    };
    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, aabb, 0,
    ).unwrap();

    assert_eq!(instance.bounding_box().min, Vec3::new(-5.0, 0.0, -5.0));
    assert_eq!(instance.bounding_box().max, Vec3::new(5.0, 10.0, 5.0));
}

// ============================================================================
// Tests: RenderInstance Error Cases
// ============================================================================

#[test]
fn test_from_mesh_invalid_variant_index() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone()); // Only 1 variant
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    // Variant index 99 does not exist
    let result = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 99,
    );
    assert!(result.is_err());
}

// ============================================================================
// Tests: RenderInstance Accessors and Mutators
// ============================================================================

#[test]
fn test_set_world_matrix() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(*instance.world_matrix(), Mat4::IDENTITY);

    let new_matrix = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
    instance.set_world_matrix(new_matrix);
    assert_eq!(*instance.world_matrix(), new_matrix);
}

#[test]
fn test_set_flags() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let new_flags = FLAG_VISIBLE | FLAG_CAST_SHADOW | FLAG_RECEIVE_SHADOW;
    instance.set_flags(new_flags);
    assert_eq!(instance.flags(), new_flags);
}

#[test]
fn test_set_visible_true() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Start visible
    assert!(instance.is_visible());

    // Hide
    instance.set_visible(false);
    assert!(!instance.is_visible());
    assert_eq!(instance.flags() & FLAG_VISIBLE, 0);

    // Show again
    instance.set_visible(true);
    assert!(instance.is_visible());
    assert_ne!(instance.flags() & FLAG_VISIBLE, 0);
}

#[test]
fn test_set_visible_preserves_other_flags() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Set multiple flags
    instance.set_flags(FLAG_VISIBLE | FLAG_CAST_SHADOW);

    // Toggle visibility off
    instance.set_visible(false);
    assert!(!instance.is_visible());
    // CAST_SHADOW should be preserved
    assert_ne!(instance.flags() & FLAG_CAST_SHADOW, 0);

    // Toggle visibility on
    instance.set_visible(true);
    assert!(instance.is_visible());
    assert_ne!(instance.flags() & FLAG_CAST_SHADOW, 0);
}

// ============================================================================
// Tests: RenderLOD Accessors
// ============================================================================

#[test]
fn test_render_lod_sub_mesh_access() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let lod = instance.lod(0).unwrap();
    assert!(lod.sub_mesh(0).is_some());
    assert!(lod.sub_mesh(1).is_some());
    assert!(lod.sub_mesh(2).is_none()); // Out of range
}

// ============================================================================
// Tests: RenderSubMesh Accessors
// ============================================================================

#[test]
fn test_render_submesh_all_accessors() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let texture = create_test_texture(renderer.clone());
    let material = create_test_material_with_texture(&pipeline, &texture);
    let mesh = create_simple_mesh(&geometry, &material);

    let instance = create_test_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let sm = instance.lod(0).unwrap().sub_mesh(0).unwrap();

    // Geometry data
    assert_eq!(sm.vertex_offset(), 0);
    assert_eq!(sm.vertex_count(), 4);
    assert_eq!(sm.index_offset(), 0);
    assert_eq!(sm.index_count(), 6);
    assert_eq!(sm.topology(), PrimitiveTopology::TriangleList);

    // Pipeline passes
    assert_eq!(sm.passes().len(), 1);

    // RenderPass structure: no texture binding groups (global set 0 is owned by Scene)
    let pass = &sm.passes()[0];
    assert_eq!(pass.texture_binding_groups().len(), 0);
}

// ============================================================================
// Tests: RenderInstanceKey
// ============================================================================

#[test]
fn test_render_instance_key_is_copy() {
    // RenderInstanceKey should implement Copy (slotmap guarantee)
    let key: RenderInstanceKey = slotmap::KeyData::from_ffi(0).into();
    let _copy = key;
    let _another = key; // Still valid after copy
}

// ============================================================================
// Tests: Draw Slot Allocation
// ============================================================================

#[test]
fn test_draw_slot_allocation() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0, &mut alloc,
    ).unwrap();

    // Mesh has 2 LODs: LOD 0 (2 submeshes) + LOD 1 (1 submesh) = 3 draw slots
    assert_eq!(alloc.len(), 3);
    assert_eq!(alloc.high_water_mark(), 3);

    // Each submesh has a unique draw_slot
    let slot0 = instance.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot();
    let slot1 = instance.lod(0).unwrap().sub_mesh(1).unwrap().draw_slot();
    let slot2 = instance.lod(1).unwrap().sub_mesh(0).unwrap().draw_slot();
    assert_ne!(slot0, slot1);
    assert_ne!(slot0, slot2);
    assert_ne!(slot1, slot2);
}

#[test]
fn test_draw_slot_sequential_allocation() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0, &mut alloc,
    ).unwrap();

    // First allocation starts at 0 and increments
    let slot0 = instance.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot();
    let slot1 = instance.lod(0).unwrap().sub_mesh(1).unwrap().draw_slot();
    let slot2 = instance.lod(1).unwrap().sub_mesh(0).unwrap().draw_slot();
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);
    assert_eq!(slot2, 2);
}

#[test]
fn test_draw_slot_shared_allocator() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut alloc = SlotAllocator::new();

    // First instance: slots 0, 1, 2
    let inst1 = RenderInstance::from_mesh(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0, &mut alloc,
    ).unwrap();

    // Second instance: slots 3, 4, 5
    let inst2 = RenderInstance::from_mesh(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0, &mut alloc,
    ).unwrap();

    assert_eq!(alloc.len(), 6);
    assert_eq!(alloc.high_water_mark(), 6);

    // Verify no overlap
    assert_eq!(inst1.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot(), 0);
    assert_eq!(inst2.lod(0).unwrap().sub_mesh(0).unwrap().draw_slot(), 3);
}

#[test]
fn test_free_draw_slots() {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline);
    let mesh = create_simple_mesh(&geometry, &material);

    let mut alloc = SlotAllocator::new();
    let instance = RenderInstance::from_mesh(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0, &mut alloc,
    ).unwrap();

    assert_eq!(alloc.len(), 3);

    instance.free_draw_slots(&mut alloc);

    assert_eq!(alloc.len(), 0);
    // High water mark unchanged
    assert_eq!(alloc.high_water_mark(), 3);
}
