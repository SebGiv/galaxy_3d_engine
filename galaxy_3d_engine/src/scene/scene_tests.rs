/// Tests for Scene

use super::*;
use crate::graphics_device::mock_graphics_device::MockGraphicsDevice;
use crate::graphics_device::{
    PrimitiveTopology, BufferFormat, ShaderStage,
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, IndexType, PolygonMode,
};
use crate::resource::geometry::{
    GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::material::{MaterialDesc, MaterialPassDesc, ParamValue};
use crate::resource::mesh::{
    MeshDesc, MeshSubMeshDesc,
    GeometryMeshRef, GeometrySubMeshRef,
};
use crate::resource::buffer::{Buffer, BufferDesc, BufferKind, FieldDesc, FieldType};
use crate::resource::shader::ShaderDesc;
use crate::resource::resource_manager::{ResourceManager, MeshKey, ShaderKey};
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
        bindings: vec![VertexBinding { binding: 0, stride: 8, input_rate: VertexInputRate::Vertex }],
        attributes: vec![VertexAttribute { location: 0, binding: 0, format: BufferFormat::R32G32_SFLOAT, offset: 0 }],
    }
}

fn create_test_aabb() -> AABB {
    AABB { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) }
}

fn create_test_buffers(gd: &Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>) -> (Arc<Buffer>, Arc<Buffer>, Arc<Buffer>, Arc<Buffer>) {
    let mk = |kind| Arc::new(Buffer::from_desc(BufferDesc {
        graphics_device: gd.clone(), kind,
        fields: vec![FieldDesc { name: "dummy".to_string(), field_type: FieldType::Vec4 }],
        count: 1,
    }).unwrap());
    (mk(BufferKind::Uniform), mk(BufferKind::Storage), mk(BufferKind::Storage), mk(BufferKind::Storage))
}

fn to_global_bindings(b: &(Arc<Buffer>, Arc<Buffer>, Arc<Buffer>, Arc<Buffer>)) -> Vec<GlobalBinding> {
    vec![
        GlobalBinding::UniformBuffer(b.0.clone()),
        GlobalBinding::StorageBuffer(b.1.clone()),
        GlobalBinding::StorageBuffer(b.2.clone()),
        GlobalBinding::StorageBuffer(b.3.clone()),
    ]
}

fn create_test_shaders(rm: &mut ResourceManager, gd: &Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>) -> (ShaderKey, ShaderKey) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let vk = rm.create_shader(format!("vert_{}", id), ShaderDesc { code: &[], stage: ShaderStage::Vertex, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    let fk = rm.create_shader(format!("frag_{}", id), ShaderDesc { code: &[], stage: ShaderStage::Fragment, entry_point: "main".to_string() }, &mut *gd.lock().unwrap()).unwrap();
    (vk, fk)
}

struct TestSetup {
    gd: Arc<Mutex<dyn crate::graphics_device::GraphicsDevice>>,
    rm: ResourceManager,
    mesh_key: MeshKey,
    vertex_shader_key: ShaderKey,
    buffers: (Arc<Buffer>, Arc<Buffer>, Arc<Buffer>, Arc<Buffer>),
}

fn setup_resources() -> TestSetup {
    let gd = create_mock_graphics_device();
    let mut rm = ResourceManager::new();
    let buffers = create_test_buffers(&gd);

    let _geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
        name: "geo".to_string(), graphics_device: gd.clone(),
        vertex_data: vec![0u8; 48], index_data: Some(vec![0u8; 12]),
        vertex_layout: create_vertex_layout(), index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "cube".to_string(),
            submeshes: vec![GeometrySubMeshDesc {
                name: "main".to_string(),
                lods: vec![GeometrySubMeshLODDesc {
                    vertex_offset: 0, vertex_count: 6,
                    index_offset: 0, index_count: 6,
                    topology: PrimitiveTopology::TriangleList,
                }],
            }],
        }],
    }).unwrap();

    let (vk, fk) = create_test_shaders(&mut rm, &gd);
    let _pk = rm.create_pipeline("p".to_string(), PipelineDesc {
        vertex_shader: vk, fragment_shader: fk,
        vertex_layout: create_vertex_layout(), topology: PrimitiveTopology::TriangleList,
        rasterization: Default::default(), color_blend: Default::default(),
        multisample: Default::default(), color_formats: vec![], depth_format: None,
    }, &mut *gd.lock().unwrap()).unwrap();

    let mk = rm.create_material("m".to_string(), MaterialDesc {
        passes: vec![MaterialPassDesc {
            pass_type: 0,
            fragment_shader: fk, color_blend: Default::default(), polygon_mode: PolygonMode::Fill, textures: vec![],
            params: vec![("value".to_string(), ParamValue::Float(1.0))],
            render_state: None,
        }],
    }, &*gd.lock().unwrap()).unwrap();

    let mesh_key = rm.create_mesh("mesh".to_string(), MeshDesc {
        geometry: _geo_key,
        geometry_mesh: GeometryMeshRef::Name("cube".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("main".to_string()),
            material: mk,
        }],
    }).unwrap();

    TestSetup { gd, rm, mesh_key, vertex_shader_key: vk, buffers }
}

fn remove_and_commit(scene: &mut Scene, key: RenderInstanceKey) -> bool {
    let marked = scene.remove_render_instance(key);
    if marked { scene.removed_instances(); }
    marked
}

// ============================================================================
// Tests: Scene Creation
// ============================================================================

#[test]
fn test_scene_new_is_empty() {
    let gd = create_mock_graphics_device();
    let b = create_test_buffers(&gd);
    let scene = Scene::new(gd, to_global_bindings(&b));
    assert_eq!(scene.render_instance_count(), 0);
}

// ============================================================================
// Tests: Create RenderInstance
// ============================================================================

#[test]
fn test_create_render_instance() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());
}

#[test]
fn test_create_multiple_render_instances() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::X), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::Y), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k3 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::Z), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.render_instance_count(), 3);
    assert_eq!(*scene.render_instance(k1).unwrap().world_matrix(), Mat4::from_translation(Vec3::X));
    assert_eq!(*scene.render_instance(k2).unwrap().world_matrix(), Mat4::from_translation(Vec3::Y));
    assert_eq!(*scene.render_instance(k3).unwrap().world_matrix(), Mat4::from_translation(Vec3::Z));
}

#[test]
fn test_create_render_instance_returns_unique_keys() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_ne!(k1, k2);
}

// ============================================================================
// Tests: Remove RenderInstance (Deferred)
// ============================================================================

#[test]
fn test_remove_render_instance_deferred() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert!(scene.remove_render_instance(key));
    assert_eq!(scene.render_instance_count(), 1); // still present (deferred)
    scene.removed_instances();
    assert_eq!(scene.render_instance_count(), 0);
}

#[test]
fn test_remove_render_instance_key_becomes_invalid() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, key);
    assert!(scene.render_instance(key).is_none());
    assert!(!scene.set_world_matrix(key, Mat4::IDENTITY));
}

#[test]
fn test_remove_nonexistent_key_returns_false() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, key);
    assert!(!scene.remove_render_instance(key));
}

#[test]
fn test_remove_does_not_invalidate_other_keys() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::X), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::Y), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k3 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::Z), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, k2);
    assert!(scene.render_instance(k1).is_some());
    assert!(scene.render_instance(k3).is_some());
    assert!(scene.render_instance(k2).is_none());
    assert_eq!(scene.render_instance_count(), 2);
}

#[test]
fn test_slot_reuse_after_remove() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, k1);
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert!(scene.render_instance(k1).is_none());
    assert!(scene.render_instance(k2).is_some());
}

// ============================================================================
// Tests: Access and Mutate
// ============================================================================

#[test]
fn test_render_instance_access() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(*scene.render_instance(key).unwrap().world_matrix(), Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
}

#[test]
fn test_set_world_matrix() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let m = Mat4::from_translation(Vec3::new(99.0, 0.0, 0.0));
    assert!(scene.set_world_matrix(key, m));
    assert_eq!(*scene.render_instance(key).unwrap().world_matrix(), m);
}

// ============================================================================
// Tests: render_instance_keys
// ============================================================================

#[test]
fn test_render_instance_keys_empty() {
    let gd = create_mock_graphics_device();
    let b = create_test_buffers(&gd);
    let scene = Scene::new(gd, to_global_bindings(&b));
    assert_eq!(scene.render_instance_keys().count(), 0);
}

#[test]
fn test_render_instance_keys_returns_all() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let keys: Vec<_> = scene.render_instance_keys().collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&k1));
    assert!(keys.contains(&k2));
}

// ============================================================================
// Tests: Iteration
// ============================================================================

#[test]
fn test_render_instances_iteration() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::X), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::Y), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let items: Vec<(RenderInstanceKey, _)> = scene.render_instances().collect();
    assert_eq!(items.len(), 2);
    let keys: Vec<_> = items.iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&k1));
    assert!(keys.contains(&k2));
}

#[test]
fn test_iteration_after_removal() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let _k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let _k3 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, k2);
    assert_eq!(scene.render_instances().count(), 2);
}

#[test]
fn test_iteration_empty_scene() {
    let gd = create_mock_graphics_device();
    let b = create_test_buffers(&gd);
    let scene = Scene::new(gd, to_global_bindings(&b));
    assert_eq!(scene.render_instances().count(), 0);
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.clear();
    assert_eq!(scene.render_instance_count(), 0);
    assert!(scene.render_instance(k1).is_none());
    assert!(scene.render_instance(k2).is_none());
}

#[test]
fn test_clear_then_add() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.clear();
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.render_instance_count(), 1);
    assert!(scene.render_instance(key).is_some());
}

// ============================================================================
// Tests: Stress
// ============================================================================

#[test]
fn test_many_instances() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let mut keys = Vec::new();
    for i in 0..100 {
        let key = scene.create_render_instance(s.mesh_key, Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)), create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
        keys.push(key);
    }
    assert_eq!(scene.render_instance_count(), 100);
    for i in (0..100).step_by(2) { scene.remove_render_instance(keys[i]); }
    scene.removed_instances();
    assert_eq!(scene.render_instance_count(), 50);
    for i in (1..100).step_by(2) { assert!(scene.render_instance(keys[i]).is_some()); }
    for i in (0..100).step_by(2) { assert!(scene.render_instance(keys[i]).is_none()); }
}

// ============================================================================
// Tests: Draw (requires Engine singleton — ignored in unit tests)
// ============================================================================

use crate::engine::Engine;
use serial_test::serial;
use crate::graphics_device::mock_graphics_device::MockCommandList;
use crate::graphics_device::Viewport;
use crate::camera::{Camera, Frustum};
use crate::scene::culler::{CameraCuller, BruteForceCuller};
use crate::scene::drawer::{Drawer, ForwardDrawer};

fn create_test_camera() -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, min_depth: 0.0, max_depth: 1.0 };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

/// Setup the Engine singleton with a ResourceManager populated with test resources.
/// Returns (Scene, ResourceManager mesh_key) for drawer tests.
fn setup_engine_draw_test() -> (Scene, MeshKey, ShaderKey) {
    Engine::initialize().unwrap();
    Engine::reset_for_testing();
    Engine::create_resource_manager().unwrap();

    let rm_arc = Engine::resource_manager().unwrap();
    let gd = create_mock_graphics_device();

    let (mesh_key, vertex_shader_key, buffers) = {
        let mut rm = rm_arc.lock().unwrap();

        let geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
            name: "geo".to_string(), graphics_device: gd.clone(),
            vertex_data: vec![0u8; 48], index_data: Some(vec![0u8; 12]),
            vertex_layout: create_vertex_layout(), index_type: IndexType::U16,
            meshes: vec![GeometryMeshDesc {
                name: "cube".to_string(),
                submeshes: vec![GeometrySubMeshDesc {
                    name: "main".to_string(),
                    lods: vec![GeometrySubMeshLODDesc {
                        vertex_offset: 0, vertex_count: 6,
                        index_offset: 0, index_count: 6,
                        topology: PrimitiveTopology::TriangleList,
                    }],
                }],
            }],
        }).unwrap();

        let (vk, fk) = create_test_shaders(&mut rm, &gd);
        let _pk = rm.create_pipeline("p".to_string(), PipelineDesc {
            vertex_shader: vk, fragment_shader: fk,
            vertex_layout: create_vertex_layout(), topology: PrimitiveTopology::TriangleList,
            rasterization: Default::default(), color_blend: Default::default(),
            multisample: Default::default(), color_formats: vec![], depth_format: None,
        }, &mut *gd.lock().unwrap()).unwrap();

        let mk = rm.create_material("m".to_string(), MaterialDesc {
            passes: vec![MaterialPassDesc {
                pass_type: 0,
                fragment_shader: fk, color_blend: Default::default(), polygon_mode: PolygonMode::Fill, textures: vec![],
                params: vec![("value".to_string(), ParamValue::Float(1.0))],
                render_state: None,
            }],
        }, &*gd.lock().unwrap()).unwrap();

        let mesh_key = rm.create_mesh("mesh".to_string(), MeshDesc {
            geometry: geo_key,
            geometry_mesh: GeometryMeshRef::Name("cube".to_string()),
            submeshes: vec![MeshSubMeshDesc {
                submesh: GeometrySubMeshRef::Name("main".to_string()),
                material: mk,
            }],
        }).unwrap();

        let buffers = create_test_buffers(&gd);
        (mesh_key, vk, buffers)
    };

    let rm_lock = rm_arc.lock().unwrap();
    let scene = Scene::new(gd, to_global_bindings(&buffers));
    drop(rm_lock);

    (scene, mesh_key, vertex_shader_key)
}

#[test]
#[serial]
fn test_draw_empty_view() {
    let (mut scene, _mesh_key, _vertex_shader_key) = setup_engine_draw_test();
    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let mut view = crate::camera::RenderView::new_empty();
    culler.cull_into(&scene, &camera, None, &mut view);
    let mut cmd = MockCommandList::new();
    let pass_info = crate::resource::resource_manager::PassInfo::new(
        vec![], None, crate::graphics_device::SampleCount::S1,
    );
    drawer.draw(&mut scene, &view, &mut cmd, &pass_info).unwrap();
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
    Engine::reset_for_testing();
}

#[test]
#[serial]
fn test_draw_single_instance() {
    let (mut scene, mesh_key, vertex_shader_key) = setup_engine_draw_test();
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(mesh_key, Mat4::IDENTITY, create_test_aabb(), vertex_shader_key, &rm).unwrap();
    }
    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let mut view = crate::camera::RenderView::new_empty();
    culler.cull_into(&scene, &camera, None, &mut view);
    let mut cmd = MockCommandList::new();
    let pass_info = crate::resource::resource_manager::PassInfo::new(
        vec![], None, crate::graphics_device::SampleCount::S1,
    );
    drawer.draw(&mut scene, &view, &mut cmd, &pass_info).unwrap();
    assert_eq!(cmd.commands, vec![
        "set_viewport",
        "set_scissor",
        "bind_vertex_buffer",
        "bind_index_buffer",
        "bind_pipeline",
        "set_dynamic_state",
        "bind_binding_group",  // global set 0
        // push_constants skipped: MockShader has no reflected push constants
        "draw_indexed",
    ]);
    Engine::reset_for_testing();
}

#[test]
#[serial]
fn test_draw_skips_committed_removal() {
    let (mut scene, mesh_key, vertex_shader_key) = setup_engine_draw_test();
    let key = {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(mesh_key, Mat4::IDENTITY, create_test_aabb(), vertex_shader_key, &rm).unwrap()
    };
    remove_and_commit(&mut scene, key);
    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let mut view = crate::camera::RenderView::new_empty();
    culler.cull_into(&scene, &camera, None, &mut view);
    let mut cmd = MockCommandList::new();
    let pass_info = crate::resource::resource_manager::PassInfo::new(
        vec![], None, crate::graphics_device::SampleCount::S1,
    );
    drawer.draw(&mut scene, &view, &mut cmd, &pass_info).unwrap();
    assert_eq!(cmd.commands, vec!["set_viewport", "set_scissor"]);
    Engine::reset_for_testing();
}

#[test]
#[serial]
fn test_draw_multiple_instances() {
    let (mut scene, mesh_key, vertex_shader_key) = setup_engine_draw_test();
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(mesh_key, Mat4::from_translation(Vec3::X), create_test_aabb(), vertex_shader_key, &rm).unwrap();
        scene.create_render_instance(mesh_key, Mat4::from_translation(Vec3::Y), create_test_aabb(), vertex_shader_key, &rm).unwrap();
    }
    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let drawer = ForwardDrawer::new();
    let mut view = crate::camera::RenderView::new_empty();
    culler.cull_into(&scene, &camera, None, &mut view);
    let mut cmd = MockCommandList::new();
    let pass_info = crate::resource::resource_manager::PassInfo::new(
        vec![], None, crate::graphics_device::SampleCount::S1,
    );
    drawer.draw(&mut scene, &view, &mut cmd, &pass_info).unwrap();
    // 2 instances: viewport + scissor + 2x (bind_vb, bind_ib, bind_pipeline, set_dynamic_state, bind_bg, draw_indexed)
    // push_constants skipped: MockShader has no reflected push constants
    assert_eq!(cmd.commands.len(), 2 + 2 * 6);
    Engine::reset_for_testing();
}

// ============================================================================
// Tests: Draw Slot Allocator
// ============================================================================

#[test]
fn test_draw_slot_count_empty() {
    let gd = create_mock_graphics_device();
    let b = create_test_buffers(&gd);
    let scene = Scene::new(gd, to_global_bindings(&b));
    assert_eq!(scene.draw_slot_count(), 0);
    assert_eq!(scene.draw_slot_high_water_mark(), 0);
}

#[test]
fn test_draw_slot_count_tracks_instances() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.draw_slot_count(), 1);
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.draw_slot_count(), 2);
}

#[test]
fn test_draw_slot_count_after_remove() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, k1);
    assert_eq!(scene.draw_slot_count(), 1);
    assert_eq!(scene.draw_slot_high_water_mark(), 2);
}

#[test]
fn test_draw_slot_recycling_after_remove() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, k1);
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.draw_slot_count(), 2);
    assert_eq!(scene.draw_slot_high_water_mark(), 2);
}

#[test]
fn test_draw_slot_count_after_clear() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.clear();
    assert_eq!(scene.draw_slot_count(), 0);
    assert_eq!(scene.draw_slot_high_water_mark(), 0);
}

// ============================================================================
// Tests: Dirty Transform Tracking
// ============================================================================

#[test]
fn test_create_marks_new_instance() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert!(scene.has_new_instance(key));
    assert!(!scene.has_dirty_instance_transform(key));
}

#[test]
fn test_set_world_matrix_marks_dirty_transform() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert_eq!(scene.dirty_instance_transform_count(), 0);
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    assert!(scene.has_dirty_instance_transform(key));
}

#[test]
fn test_dirty_instance_transforms_flip_clears_front() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    let taken = scene.dirty_instance_transforms();
    assert!(taken.contains(&key));
    assert!(!scene.has_dirty_instance_transform(key));
}

#[test]
fn test_remove_cleans_dirty_transform() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    scene.remove_render_instance(key);
    assert!(!scene.has_dirty_instance_transform(key));
}

#[test]
fn test_clear_cleans_dirty_transforms() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let k1 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let k2 = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.set_world_matrix(k1, Mat4::from_translation(Vec3::X));
    scene.set_world_matrix(k2, Mat4::from_translation(Vec3::Y));
    scene.clear();
    assert_eq!(scene.dirty_instance_transform_count(), 0);
}

#[test]
fn test_set_world_matrix_on_invalid_key_returns_false() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    remove_and_commit(&mut scene, key);
    assert!(!scene.set_world_matrix(key, Mat4::IDENTITY));
}

#[test]
fn test_dirty_transform_deduplication() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::Y));
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::Z));
    assert_eq!(scene.dirty_instance_transform_count(), 1);
}

// ============================================================================
// Tests: New Instance Tracking
// ============================================================================

#[test]
fn test_new_instances_flip_clears_front() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let taken = scene.new_instances();
    assert!(taken.contains(&key));
    assert!(!scene.has_new_instance(key));
}

#[test]
fn test_remove_cleans_new_instance() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.remove_render_instance(key);
    assert!(!scene.has_new_instance(key));
}

#[test]
fn test_clear_cleans_new_instances() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.clear();
    assert_eq!(scene.new_instance_count(), 0);
}

#[test]
fn test_create_then_set_matrix_in_both_sets() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert!(scene.has_new_instance(key));
    assert!(!scene.has_dirty_instance_transform(key));
    scene.set_world_matrix(key, Mat4::from_translation(Vec3::X));
    assert!(scene.has_new_instance(key));
    assert!(scene.has_dirty_instance_transform(key));
}

// ============================================================================
// Tests: Deferred Removal Tracking
// ============================================================================

#[test]
fn test_remove_marks_removed_instance() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.remove_render_instance(key);
    let removed = scene.removed_instances();
    assert!(removed.contains(&key));
}

#[test]
fn test_removed_instances_flip_clears_front() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.remove_render_instance(key);
    let _ = scene.removed_instances();
    assert!(scene.removed_instances().is_empty());
}

#[test]
fn test_remove_deduplication() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.remove_render_instance(key);
    scene.remove_render_instance(key);
    assert_eq!(scene.removed_instances().len(), 1);
}

#[test]
fn test_clear_cleans_removed_instances() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    let key = scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    scene.remove_render_instance(key);
    scene.clear();
    assert!(scene.removed_instances().is_empty());
}

// ============================================================================
// Tests: Global Binding Group
// ============================================================================

#[test]
fn test_global_binding_group_none_before_first_instance() {
    let gd = create_mock_graphics_device();
    let b = create_test_buffers(&gd);
    let scene = Scene::new(gd, to_global_bindings(&b));
    assert!(scene.global_binding_group().is_none());
}

#[test]
fn test_global_binding_group_created_on_first_instance() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    let bg = scene.global_binding_group();
    assert!(bg.is_some());
    assert_eq!(bg.unwrap().set_index(), 1); // Set 0 is reserved for bindless textures
}

#[test]
fn test_global_binding_group_survives_clear() {
    let s = setup_resources();
    let mut scene = Scene::new(s.gd, to_global_bindings(&s.buffers));
    scene.create_render_instance(s.mesh_key, Mat4::IDENTITY, create_test_aabb(), s.vertex_shader_key, &s.rm).unwrap();
    assert!(scene.global_binding_group().is_some());
    scene.clear();
    assert!(scene.global_binding_group().is_some());
}
