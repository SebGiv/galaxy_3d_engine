/// Tests for Scene
///
/// These tests validate Scene creation, RenderInstance lifecycle via SlotMap keys,
/// iteration, and edge cases.

use super::*;
use crate::renderer::mock_renderer::{MockRenderer, MockShader};
use crate::renderer::{
    PrimitiveTopology, BufferFormat,
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, IndexType,
};
use crate::resource::geometry::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::{
    Pipeline, PipelineDesc, PipelineVariantDesc, PipelinePassDesc,
};
use crate::resource::material::{Material, MaterialDesc, ParamValue};
use crate::resource::mesh::{
    Mesh, MeshDesc, MeshLODDesc, SubMeshDesc,
    GeometryMeshRef, GeometrySubMeshRef,
};
use crate::scene::render_instance::{AABB, RenderInstanceKey};
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

fn create_test_geometry(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Geometry> {
    let desc = GeometryDesc {
        name: "geo".to_string(),
        renderer,
        vertex_data: vec![0u8; 48], // 6 vertices * 8 bytes
        index_data: Some(vec![0u8; 12]), // 6 indices * 2 bytes
        vertex_layout: create_vertex_layout(),
        index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "cube".to_string(),
            lods: vec![GeometryLODDesc {
                lod_index: 0,
                submeshes: vec![GeometrySubMeshDesc {
                    name: "main".to_string(),
                    vertex_offset: 0, vertex_count: 6,
                    index_offset: 0, index_count: 6,
                    topology: PrimitiveTopology::TriangleList,
                }],
            }],
        }],
    };
    Arc::new(Geometry::from_desc(desc).unwrap())
}

fn create_test_pipeline(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> Arc<Pipeline> {
    let desc = PipelineDesc {
        renderer,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: crate::renderer::PipelineDesc {
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
                },
            }],
        }],
    };
    Arc::new(Pipeline::from_desc(desc).unwrap())
}

fn create_test_material(pipeline: &Arc<Pipeline>, value: f32) -> Arc<Material> {
    let desc = MaterialDesc {
        pipeline: pipeline.clone(),
        textures: vec![],
        params: vec![("value".to_string(), ParamValue::Float(value))],
    };
    Arc::new(Material::from_desc(desc).unwrap())
}

fn create_test_aabb() -> AABB {
    AABB {
        min: Vec3::new(-1.0, -1.0, -1.0),
        max: Vec3::new(1.0, 1.0, 1.0),
    }
}

fn create_test_mesh(geometry: &Arc<Geometry>, material: &Arc<Material>) -> Mesh {
    let desc = MeshDesc {
        geometry: geometry.clone(),
        geometry_mesh: GeometryMeshRef::Name("cube".to_string()),
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

/// Helper: create all resources and return (renderer, geometry, pipeline, material, mesh)
fn setup_resources() -> (
    Arc<Mutex<dyn crate::renderer::Renderer>>,
    Arc<Geometry>,
    Arc<Pipeline>,
    Arc<Material>,
    Mesh,
) {
    let renderer = create_mock_renderer();
    let geometry = create_test_geometry(renderer.clone());
    let pipeline = create_test_pipeline(renderer.clone());
    let material = create_test_material(&pipeline, 1.0);
    let mesh = create_test_mesh(&geometry, &material);
    (renderer, geometry, pipeline, material, mesh)
}

// ============================================================================
// Tests: Scene Creation
// ============================================================================

#[test]
fn test_scene_new_is_empty() {
    let renderer = create_mock_renderer();
    let scene = Scene::new(renderer);
    assert_eq!(scene.render_instance_count(), 0);
}

// ============================================================================
// Tests: Create RenderInstance
// ============================================================================

#[test]
fn test_create_render_instance() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());
}

#[test]
fn test_create_multiple_render_instances() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::X), create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Y), create_test_aabb(), 0,
    ).unwrap();
    let key3 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Z), create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 3);
    assert!(scene.render_instance(key1).is_some());
    assert!(scene.render_instance(key2).is_some());
    assert!(scene.render_instance(key3).is_some());

    // Each instance has its own matrix
    assert_eq!(*scene.render_instance(key1).unwrap().world_matrix(),
               Mat4::from_translation(Vec3::X));
    assert_eq!(*scene.render_instance(key2).unwrap().world_matrix(),
               Mat4::from_translation(Vec3::Y));
    assert_eq!(*scene.render_instance(key3).unwrap().world_matrix(),
               Mat4::from_translation(Vec3::Z));
}

#[test]
fn test_create_render_instance_returns_unique_keys() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_ne!(key1, key2);
}

// ============================================================================
// Tests: Remove RenderInstance
// ============================================================================

#[test]
fn test_remove_render_instance() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 1);

    let removed = scene.remove_render_instance(key);
    assert!(removed.is_some());
    assert_eq!(scene.render_instance_count(), 0);
}

#[test]
fn test_remove_render_instance_key_becomes_invalid() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    scene.remove_render_instance(key);

    // Key is now invalid
    assert!(scene.render_instance(key).is_none());
    assert!(scene.render_instance_mut(key).is_none());
}

#[test]
fn test_remove_nonexistent_key_returns_none() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove once → succeeds
    scene.remove_render_instance(key);

    // Remove again → returns None
    let result = scene.remove_render_instance(key);
    assert!(result.is_none());
}

#[test]
fn test_remove_does_not_invalidate_other_keys() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::X), create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Y), create_test_aabb(), 0,
    ).unwrap();
    let key3 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Z), create_test_aabb(), 0,
    ).unwrap();

    // Remove middle instance
    scene.remove_render_instance(key2);

    // Other keys remain valid
    assert!(scene.render_instance(key1).is_some());
    assert!(scene.render_instance(key3).is_some());
    assert!(scene.render_instance(key2).is_none());
    assert_eq!(scene.render_instance_count(), 2);
}

#[test]
fn test_slot_reuse_after_remove() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove it
    scene.remove_render_instance(key1);

    // Create new → may reuse the slot
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Old key is still invalid (generation changed)
    assert!(scene.render_instance(key1).is_none());
    // New key is valid
    assert!(scene.render_instance(key2).is_some());
}

// ============================================================================
// Tests: Access and Mutate
// ============================================================================

#[test]
fn test_render_instance_access() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        create_test_aabb(), 0,
    ).unwrap();

    let instance = scene.render_instance(key).unwrap();
    assert_eq!(*instance.world_matrix(),
               Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
}

#[test]
fn test_render_instance_mut_access() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Mutate the instance
    let instance = scene.render_instance_mut(key).unwrap();
    instance.set_world_matrix(Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0)));
    instance.set_visible(false);

    // Verify mutations persisted
    let instance = scene.render_instance(key).unwrap();
    assert_eq!(*instance.world_matrix(),
               Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0)));
    assert!(!instance.is_visible());
}

// ============================================================================
// Tests: Iteration
// ============================================================================

#[test]
fn test_render_instances_iteration() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::X), create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Y), create_test_aabb(), 0,
    ).unwrap();

    let items: Vec<(RenderInstanceKey, _)> = scene.render_instances().collect();
    assert_eq!(items.len(), 2);

    // Both keys should be present
    let keys: Vec<_> = items.iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&key1));
    assert!(keys.contains(&key2));
}

#[test]
fn test_render_instances_mut_iteration() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Hide all instances via mutable iteration
    for (_, instance) in scene.render_instances_mut() {
        instance.set_visible(false);
    }

    // Verify all are hidden
    for (_, instance) in scene.render_instances() {
        assert!(!instance.is_visible());
    }
}

#[test]
fn test_iteration_after_removal() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let _key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let _key3 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove middle one
    scene.remove_render_instance(key2);

    // Iteration should yield exactly 2 items
    let count = scene.render_instances().count();
    assert_eq!(count, 2);
}

#[test]
fn test_iteration_empty_scene() {
    let renderer = create_mock_renderer();
    let scene = Scene::new(renderer);
    let count = scene.render_instances().count();
    assert_eq!(count, 0);
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 2);

    scene.clear();

    assert_eq!(scene.render_instance_count(), 0);
    assert!(scene.render_instance(key1).is_none());
    assert!(scene.render_instance(key2).is_none());
}

#[test]
fn test_clear_then_add() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    scene.clear();

    // Can add after clear
    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());
}

// ============================================================================
// Tests: Stress / Many Instances
// ============================================================================

#[test]
fn test_many_instances() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);
    let mut keys = Vec::new();

    // Add 100 instances
    for i in 0..100 {
        let key = scene.create_render_instance(
            &mesh,
            Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)),
            create_test_aabb(),
            0,
        ).unwrap();
        keys.push(key);
    }

    assert_eq!(scene.render_instance_count(), 100);

    // Remove every other one
    for i in (0..100).step_by(2) {
        scene.remove_render_instance(keys[i]);
    }

    assert_eq!(scene.render_instance_count(), 50);

    // Remaining keys are still valid
    for i in (1..100).step_by(2) {
        assert!(scene.render_instance(keys[i]).is_some());
    }

    // Removed keys are invalid
    for i in (0..100).step_by(2) {
        assert!(scene.render_instance(keys[i]).is_none());
    }
}

// ============================================================================
// Tests: Draw
// ============================================================================

use crate::renderer::mock_renderer::MockCommandList;
use crate::renderer::Viewport;
use crate::camera::{Camera, Frustum};

fn create_test_camera() -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

#[test]
fn test_draw_empty_view() {
    let renderer = create_mock_renderer();
    let scene = Scene::new(renderer);
    let camera = create_test_camera();
    let view = scene.frustum_cull(&camera);

    let mut cmd = MockCommandList::new();
    scene.draw(&view, &mut cmd).unwrap();

    // Only viewport + scissor, no draw calls
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
}

#[test]
fn test_draw_single_instance() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    let view = scene.frustum_cull(&camera);

    let mut cmd = MockCommandList::new();
    scene.draw(&view, &mut cmd).unwrap();

    assert_eq!(cmd.commands, vec![
        "set_viewport",
        "set_scissor",
        "bind_vertex_buffer",
        "bind_index_buffer",
        "bind_pipeline",
        "push_constants",  // MVP
        "push_constants",  // Model
        "draw_indexed",
    ]);
}

#[test]
fn test_draw_skips_removed_instance() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    // Cull while instance exists
    let view = scene.frustum_cull(&camera);

    // Remove instance after culling
    scene.remove_render_instance(key);

    let mut cmd = MockCommandList::new();
    scene.draw(&view, &mut cmd).unwrap();

    // Instance was skipped — only viewport + scissor
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
}

#[test]
fn test_draw_multiple_instances() {
    let (renderer, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(renderer);

    scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::X), create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Y), create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    let view = scene.frustum_cull(&camera);

    let mut cmd = MockCommandList::new();
    scene.draw(&view, &mut cmd).unwrap();

    // 2 instances: viewport + scissor + 2x (bind_vb, bind_ib, bind_pipeline, push x2, draw_indexed)
    assert_eq!(cmd.commands.len(), 2 + 2 * 6);
    assert_eq!(cmd.commands[0], "set_viewport");
    assert_eq!(cmd.commands[1], "set_scissor");
}
