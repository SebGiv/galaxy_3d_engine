/// Tests for Scene
///
/// These tests validate Scene creation, RenderInstance lifecycle via SlotMap keys,
/// iteration, and edge cases.

use super::*;
use crate::graphics_device::mock_graphics_device::{MockGraphicsDevice, MockShader};
use crate::graphics_device::{
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
use crate::resource::buffer::{Buffer, BufferDesc, BufferKind, FieldDesc, FieldType};
use crate::scene::render_instance::{AABB, RenderInstanceKey};
use glam::{Vec3, Mat4};
use std::sync::{Arc, Mutex};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_mock_graphics_device() -> Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(MockGraphicsDevice::new()))
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

fn create_test_geometry(graphics_device: Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>) -> Arc<Geometry> {
    let desc = GeometryDesc {
        name: "geo".to_string(),
        graphics_device,
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

fn create_test_pipeline(graphics_device: Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>) -> Arc<Pipeline> {
    let desc = PipelineDesc {
        graphics_device,
        variants: vec![PipelineVariantDesc {
            name: "default".to_string(),
            passes: vec![PipelinePassDesc {
                pipeline: crate::graphics_device::PipelineDesc {
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
    Arc::new(Material::from_desc(0, desc).unwrap())
}

fn create_test_buffers(
    graphics_device: Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>,
) -> (Arc<Buffer>, Arc<Buffer>, Arc<Buffer>) {
    let frame_buffer = Arc::new(Buffer::from_desc(BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Uniform,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap());
    let instance_buffer = Arc::new(Buffer::from_desc(BufferDesc {
        graphics_device: graphics_device.clone(),
        kind: BufferKind::Storage,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap());
    let material_buffer = Arc::new(Buffer::from_desc(BufferDesc {
        graphics_device,
        kind: BufferKind::Storage,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap());
    (frame_buffer, instance_buffer, material_buffer)
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

/// Helper: create all resources and return (graphics_device, buffers, geometry, pipeline, material, mesh)
fn setup_resources() -> (
    Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>,
    (Arc<Buffer>, Arc<Buffer>, Arc<Buffer>),
    Arc<Geometry>,
    Arc<Pipeline>,
    Arc<Material>,
    Mesh,
) {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let geometry = create_test_geometry(graphics_device.clone());
    let pipeline = create_test_pipeline(graphics_device.clone());
    let material = create_test_material(&pipeline, 1.0);
    let mesh = create_test_mesh(&geometry, &material);
    (graphics_device, buffers, geometry, pipeline, material, mesh)
}

/// Helper: mark for deferred removal + commit immediately.
/// For tests that don't care about deferred behavior.
fn remove_and_commit(scene: &mut Scene, key: RenderInstanceKey) -> bool {
    let marked = scene.remove_render_instance(key);
    if marked {
        let removed = scene.take_removed_instances();
        scene.commit_removals(&removed);
    }
    marked
}

// ============================================================================
// Tests: Scene Creation
// ============================================================================

#[test]
fn test_scene_new_is_empty() {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);
    assert_eq!(scene.render_instance_count(), 0);
}

// ============================================================================
// Tests: Create RenderInstance
// ============================================================================

#[test]
fn test_create_render_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());
}

#[test]
fn test_create_multiple_render_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

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
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_ne!(key1, key2);
}

// ============================================================================
// Tests: Remove RenderInstance (Deferred)
// ============================================================================

#[test]
fn test_remove_render_instance_deferred() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.render_instance_count(), 1);

    // Mark for deferred removal
    assert!(scene.remove_render_instance(key));

    // Instance still in scene (deferred)
    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());

    // Commit the removal
    let removed = scene.take_removed_instances();
    scene.commit_removals(&removed);

    assert_eq!(scene.render_instance_count(), 0);
    assert!(scene.render_instance(key).is_none());
}

#[test]
fn test_remove_render_instance_key_becomes_invalid() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    remove_and_commit(&mut scene, key);

    // Key is now invalid
    assert!(scene.render_instance(key).is_none());
    assert!(!scene.set_world_matrix(key, Mat4::IDENTITY));
}

#[test]
fn test_remove_nonexistent_key_returns_false() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove once → succeeds
    remove_and_commit(&mut scene, key);

    // Remove again → returns false
    assert!(!scene.remove_render_instance(key));
}

#[test]
fn test_remove_does_not_invalidate_other_keys() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

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
    remove_and_commit(&mut scene, key2);

    // Other keys remain valid
    assert!(scene.render_instance(key1).is_some());
    assert!(scene.render_instance(key3).is_some());
    assert!(scene.render_instance(key2).is_none());
    assert_eq!(scene.render_instance_count(), 2);
}

#[test]
fn test_slot_reuse_after_remove() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove and commit (frees the slot)
    remove_and_commit(&mut scene, key1);

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
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)),
        create_test_aabb(), 0,
    ).unwrap();

    let instance = scene.render_instance(key).unwrap();
    assert_eq!(*instance.world_matrix(),
               Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
}

#[test]
fn test_set_world_matrix() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Mutate via Scene method
    let new_matrix = Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0));
    assert!(scene.set_world_matrix(key, new_matrix));

    // Verify mutation persisted
    let instance = scene.render_instance(key).unwrap();
    assert_eq!(*instance.world_matrix(), new_matrix);
}

// ============================================================================
// Tests: render_instance_keys
// ============================================================================

#[test]
fn test_render_instance_keys_empty() {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);
    assert_eq!(scene.render_instance_keys().count(), 0);
}

#[test]
fn test_render_instance_keys_returns_all() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let keys: Vec<_> = scene.render_instance_keys().collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&key1));
    assert!(keys.contains(&key2));
}

// ============================================================================
// Tests: Iteration
// ============================================================================

#[test]
fn test_render_instances_iteration() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

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
fn test_set_world_matrix_all_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Mutate all instances via Scene methods
    let offset = Vec3::new(10.0, 0.0, 0.0);
    scene.set_world_matrix(key1, Mat4::from_translation(offset));
    scene.set_world_matrix(key2, Mat4::from_translation(offset));

    // Verify all were updated
    for (_, instance) in scene.render_instances() {
        assert_eq!(*instance.world_matrix(), Mat4::from_translation(offset));
    }
}

#[test]
fn test_iteration_after_removal() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let _key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let _key3 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove middle one and commit
    remove_and_commit(&mut scene, key2);

    // Iteration should yield exactly 2 items
    let count = scene.render_instances().count();
    assert_eq!(count, 2);
}

#[test]
fn test_iteration_empty_scene() {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);
    let count = scene.render_instances().count();
    assert_eq!(count, 0);
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

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
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

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
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());
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

    // Mark every other one for removal and commit
    for i in (0..100).step_by(2) {
        scene.remove_render_instance(keys[i]);
    }
    let removed = scene.take_removed_instances();
    scene.commit_removals(&removed);

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
// Tests: Draw (via ForwardDrawer + BruteForceCuller)
// ============================================================================

use crate::graphics_device::mock_graphics_device::MockCommandList;
use crate::graphics_device::Viewport;
use crate::camera::{Camera, Frustum};
use crate::scene::culler::{CameraCuller, BruteForceCuller};
use crate::scene::drawer::{Drawer, ForwardDrawer};

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
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);
    let camera = create_test_camera();

    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let view = culler.cull(&scene, &camera, None);

    let mut cmd = MockCommandList::new();
    drawer.draw(&scene, &view, &mut cmd).unwrap();

    // Only viewport + scissor, no draw calls
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
}

#[test]
fn test_draw_single_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let view = culler.cull(&scene, &camera, None);

    let mut cmd = MockCommandList::new();
    drawer.draw(&scene, &view, &mut cmd).unwrap();

    assert_eq!(cmd.commands, vec![
        "set_viewport",
        "set_scissor",
        "bind_vertex_buffer",
        "bind_index_buffer",
        "bind_pipeline",
        "bind_binding_group",  // global set 0 (frame + instance + material buffers)
        "push_constants",  // draw slot index
        "draw_indexed",
    ]);
}

#[test]
fn test_draw_skips_committed_removal() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();

    // Remove and commit before culling (normal frame flow)
    remove_and_commit(&mut scene, key);

    let view = culler.cull(&scene, &camera, None);

    let mut cmd = MockCommandList::new();
    drawer.draw(&scene, &view, &mut cmd).unwrap();

    // Instance was removed — only viewport + scissor
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
}

#[test]
fn test_draw_multiple_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::X), create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::from_translation(Vec3::Y), create_test_aabb(), 0,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let view = culler.cull(&scene, &camera, None);

    let mut cmd = MockCommandList::new();
    drawer.draw(&scene, &view, &mut cmd).unwrap();

    // 2 instances: viewport + scissor + 2x (bind_vb, bind_ib, bind_pipeline, bind_bg, push, draw_indexed)
    assert_eq!(cmd.commands.len(), 2 + 2 * 6);
    assert_eq!(cmd.commands[0], "set_viewport");
    assert_eq!(cmd.commands[1], "set_scissor");
}

// ============================================================================
// Tests: Draw Slot Allocator
// ============================================================================

#[test]
fn test_draw_slot_count_empty() {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);
    assert_eq!(scene.draw_slot_count(), 0);
    assert_eq!(scene.draw_slot_high_water_mark(), 0);
}

#[test]
fn test_draw_slot_count_tracks_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    // Each test mesh has 1 LOD with 1 submesh = 1 draw slot per instance
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    assert_eq!(scene.draw_slot_count(), 1);
    assert_eq!(scene.draw_slot_high_water_mark(), 1);

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    assert_eq!(scene.draw_slot_count(), 2);
    assert_eq!(scene.draw_slot_high_water_mark(), 2);
}

#[test]
fn test_draw_slot_count_after_remove() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let _key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.draw_slot_count(), 2);

    remove_and_commit(&mut scene, key1);
    assert_eq!(scene.draw_slot_count(), 1);
    // High water mark stays at 2
    assert_eq!(scene.draw_slot_high_water_mark(), 2);
}

#[test]
fn test_draw_slot_recycling_after_remove() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Remove first instance (frees slot 0)
    remove_and_commit(&mut scene, key1);
    assert_eq!(scene.draw_slot_high_water_mark(), 2);

    // New instance should recycle the freed slot
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // High water mark should NOT increase (slot was recycled)
    assert_eq!(scene.draw_slot_count(), 2);
    assert_eq!(scene.draw_slot_high_water_mark(), 2);
}

#[test]
fn test_draw_slot_count_after_clear() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.draw_slot_count(), 2);

    scene.clear();

    // Clear resets everything (allocator recreated)
    assert_eq!(scene.draw_slot_count(), 0);
    assert_eq!(scene.draw_slot_high_water_mark(), 0);
}

// ============================================================================
// Tests: Dirty Transform Tracking
// ============================================================================

#[test]
fn test_create_marks_new_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert!(scene.new_instances().contains(&key));
    assert!(!scene.dirty_transforms().contains(&key));
}

#[test]
fn test_set_world_matrix_marks_dirty_transform() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Creation goes to new_instances, not dirty_transforms
    assert!(scene.dirty_transforms().is_empty());

    // set_world_matrix should mark dirty
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    assert!(scene.dirty_transforms().contains(&key));
}

#[test]
fn test_take_dirty_transforms_clears_set() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // set_world_matrix to populate dirty_transforms
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));

    let taken = scene.take_dirty_transforms();
    assert!(taken.contains(&key));
    assert!(scene.dirty_transforms().is_empty());
}

#[test]
fn test_remove_cleans_dirty_transform() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Populate dirty_transforms via set_world_matrix
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    assert!(scene.dirty_transforms().contains(&key));

    scene.remove_render_instance(key);
    assert!(!scene.dirty_transforms().contains(&key));
}

#[test]
fn test_clear_cleans_dirty_transforms() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key1 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    let key2 = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Populate dirty_transforms via set_world_matrix
    scene.set_world_matrix(key1, Mat4::from_translation(Vec3::X));
    scene.set_world_matrix(key2, Mat4::from_translation(Vec3::Y));
    assert_eq!(scene.dirty_transforms().len(), 2);

    scene.clear();
    assert!(scene.dirty_transforms().is_empty());
}

#[test]
fn test_set_world_matrix_on_invalid_key_returns_false() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    remove_and_commit(&mut scene, key);

    assert!(!scene.set_world_matrix(key, Mat4::IDENTITY));
}

#[test]
fn test_dirty_transform_deduplication() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // 3 mutations on the same instance → 1 entry in dirty set
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::Y));
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::Z));

    assert_eq!(scene.dirty_transforms().len(), 1);
    assert!(scene.dirty_transforms().contains(&key));
}

// ============================================================================
// Tests: New Instance Tracking
// ============================================================================

#[test]
fn test_take_new_instances_clears_set() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    let taken = scene.take_new_instances();
    assert!(taken.contains(&key));
    assert!(scene.new_instances().is_empty());
}

#[test]
fn test_remove_cleans_new_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert!(scene.new_instances().contains(&key));

    scene.remove_render_instance(key);
    assert!(!scene.new_instances().contains(&key));
}

#[test]
fn test_clear_cleans_new_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();
    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert_eq!(scene.new_instances().len(), 2);

    scene.clear();
    assert!(scene.new_instances().is_empty());
}

#[test]
fn test_create_then_set_matrix_in_both_sets() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Create puts in new_instances only
    assert!(scene.new_instances().contains(&key));
    assert!(!scene.dirty_transforms().contains(&key));

    // set_world_matrix adds to dirty_transforms too
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    assert!(scene.new_instances().contains(&key));
    assert!(scene.dirty_transforms().contains(&key));
}

// ============================================================================
// Tests: Deferred Removal Tracking
// ============================================================================

#[test]
fn test_remove_marks_removed_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    scene.remove_render_instance(key);
    let removed = scene.take_removed_instances();
    assert!(removed.contains(&key));
}

#[test]
fn test_take_removed_instances_clears_set() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    scene.remove_render_instance(key);
    let _ = scene.take_removed_instances();

    // Second take returns empty
    assert!(scene.take_removed_instances().is_empty());
}

#[test]
fn test_remove_deduplication() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Mark same key twice → 1 entry in removed set
    scene.remove_render_instance(key);
    scene.remove_render_instance(key);

    let removed = scene.take_removed_instances();
    assert_eq!(removed.len(), 1);
}

#[test]
fn test_clear_cleans_removed_instances() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    let key = scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    scene.remove_render_instance(key);
    scene.clear();

    assert!(scene.take_removed_instances().is_empty());
}

// ============================================================================
// Tests: Global Binding Group
// ============================================================================

#[test]
fn test_global_binding_group_none_before_first_instance() {
    let graphics_device = create_mock_graphics_device();
    let buffers = create_test_buffers(graphics_device.clone());
    let scene = Scene::new(graphics_device, buffers.0, buffers.1, buffers.2);

    // No instance created yet → global binding group is None
    assert!(scene.global_binding_group().is_none());
}

#[test]
fn test_global_binding_group_created_on_first_instance() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    // Global binding group should exist after first instance
    let bg = scene.global_binding_group();
    assert!(bg.is_some());
    assert_eq!(bg.unwrap().set_index(), 0);
}

#[test]
fn test_global_binding_group_survives_clear() {
    let (graphics_device, buffers, _, _, _, mesh) = setup_resources();
    let mut scene = Scene::new(graphics_device, buffers.0.clone(), buffers.1.clone(), buffers.2.clone());

    scene.create_render_instance(
        &mesh, Mat4::IDENTITY, create_test_aabb(), 0,
    ).unwrap();

    assert!(scene.global_binding_group().is_some());

    scene.clear();

    // Global binding group survives clear (buffers unchanged)
    assert!(scene.global_binding_group().is_some());
}
