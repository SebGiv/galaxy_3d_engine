use super::*;
use crate::engine::Engine;
use crate::camera::VisibleInstances;
use crate::graphics_device::{TextureFormat, SampleCount, mock_graphics_device::{MockGraphicsDevice, MockCommandList, MockBindingGroup}};
use crate::resource::resource_manager::PassInfo;
use crate::scene::{Scene, BruteForceCuller, CameraCuller, RenderView};
use crate::scene::scene_test_helpers::{create_test_aabb, create_test_camera};
use crate::scene::view_dispatcher::ViewDispatcher;
use serial_test::serial;
use std::sync::Arc;
use glam::Mat4;

// ============================================================================
// Trivial constructor tests (no Engine setup)
// ============================================================================

#[test]
fn test_forward_drawer_new() {
    let _d = ForwardDrawer::new();
}

#[test]
fn test_forward_drawer_with_capacity_zero() {
    let _d = ForwardDrawer::with_capacity(0);
}

#[test]
fn test_forward_drawer_with_capacity_large() {
    let _d = ForwardDrawer::with_capacity(65_536);
}

// ============================================================================
// Engine-backed integration tests
// ============================================================================

fn setup_engine_with_main_device() {
    Engine::initialize().unwrap();
    Engine::reset_for_testing();
    Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();
}

fn populate_resource_manager() -> (
    crate::resource::resource_manager::MeshKey,
    crate::resource::resource_manager::ShaderKey,
) {
    use crate::graphics_device::{
        BufferFormat, ShaderStage, VertexLayout, VertexBinding, VertexAttribute,
        VertexInputRate, IndexType, PolygonMode, PrimitiveTopology,
    };
    use crate::resource::geometry::{
        GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
    };
    use crate::resource::pipeline::PipelineDesc;
    use crate::resource::material::{MaterialDesc, MaterialPassDesc, ParamValue};
    use crate::resource::mesh::{MeshDesc, MeshSubMeshDesc, GeometryMeshRef, GeometrySubMeshRef};
    use crate::resource::shader::ShaderDesc;

    let rm_arc = Engine::resource_manager().unwrap();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut rm = rm_arc.lock().unwrap();

    let layout = VertexLayout {
        bindings: vec![VertexBinding { binding: 0, stride: 8, input_rate: VertexInputRate::Vertex }],
        attributes: vec![VertexAttribute { location: 0, binding: 0, format: BufferFormat::R32G32_SFLOAT, offset: 0 }],
    };

    let geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
        name: "geo".to_string(),
        graphics_device: gd_arc.clone(),
        vertex_data: vec![0u8; 48],
        index_data: Some(vec![0u8; 12]),
        vertex_layout: layout.clone(),
        index_type: IndexType::U16,
        meshes: vec![GeometryMeshDesc {
            name: "cube".to_string(),
            submeshes: vec![GeometrySubMeshDesc {
                name: "main".to_string(),
                lods: vec![GeometrySubMeshLODDesc {
                    vertex_offset: 0, vertex_count: 6,
                    index_offset: 0, index_count: 6,
                    topology: PrimitiveTopology::TriangleList,
                }],
                lod_thresholds: Vec::new(),
            }],
        }],
    }).unwrap();

    let vk = rm.create_shader("vert".to_string(),
        ShaderDesc { code: &[], stage: ShaderStage::Vertex, entry_point: "main".to_string() },
        &mut *gd_arc.lock().unwrap()).unwrap();
    let fk = rm.create_shader("frag".to_string(),
        ShaderDesc { code: &[], stage: ShaderStage::Fragment, entry_point: "main".to_string() },
        &mut *gd_arc.lock().unwrap()).unwrap();
    let _pk = rm.create_pipeline("p".to_string(), PipelineDesc {
        vertex_shader: vk, fragment_shader: fk,
        vertex_layout: layout, topology: PrimitiveTopology::TriangleList,
        rasterization: Default::default(), color_blend: Default::default(),
        multisample: Default::default(), color_formats: vec![], depth_format: None,
    }, &mut *gd_arc.lock().unwrap()).unwrap();
    let mk = rm.create_material("m".to_string(), MaterialDesc {
        passes: vec![MaterialPassDesc {
            pass_type: 0,
            fragment_shader: fk, color_blend: Default::default(), polygon_mode: PolygonMode::Fill,
            textures: vec![],
            params: vec![("value".to_string(), ParamValue::Float(1.0))],
            render_state: None,
        }],
    }, &*gd_arc.lock().unwrap()).unwrap();
    let mesh_key = rm.create_mesh("mesh".to_string(), MeshDesc {
        geometry: geo_key,
        geometry_mesh: GeometryMeshRef::Name("cube".to_string()),
        submeshes: vec![MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name("main".to_string()),
            material: mk,
        }],
    }).unwrap();

    (mesh_key, vk)
}

fn make_pass_info() -> PassInfo {
    PassInfo::new(vec![TextureFormat::R8G8B8A8_UNORM], None, SampleCount::S1)
}

#[test]
#[serial]
fn test_forward_drawer_draw_empty_view() {
    setup_engine_with_main_device();
    let (_mk, _vk) = populate_resource_manager();

    let mut scene = Scene::new();
    let camera = create_test_camera();
    let view = RenderView::new(camera, 0);
    let mut drawer = ForwardDrawer::new();
    let mut cmd = MockCommandList::new();
    let bg: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("test_bg".to_string(), 1));
    let info = make_pass_info();
    let result = drawer.draw(&mut scene, &view, &mut cmd, &info, &bg, true);
    assert!(result.is_ok());
    // viewport + scissor recorded even for an empty view.
    assert!(cmd.commands.iter().any(|c| c == "set_viewport"));
    assert!(cmd.commands.iter().any(|c| c == "set_scissor"));
}

#[test]
#[serial]
fn test_forward_drawer_draw_with_one_visible_submesh() {
    setup_engine_with_main_device();
    let (mesh_key, vertex_shader_key) = populate_resource_manager();

    let mut scene = Scene::new();
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(
            mesh_key, Mat4::IDENTITY, create_test_aabb(),
            vertex_shader_key, &[], &rm,
        ).unwrap();
    }

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut view = RenderView::new(camera.clone(), 0);
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        ViewDispatcher::dispatch(&visible, &mut scene, &rm, std::slice::from_mut(&mut view));
    }

    let mut drawer = ForwardDrawer::new();
    let mut cmd = MockCommandList::new();
    let bg: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("test_bg".to_string(), 1));
    let info = make_pass_info();
    let result = drawer.draw(&mut scene, &view, &mut cmd, &info, &bg, true);
    assert!(result.is_ok(), "draw failed: {:?}", result);
    // At least one bind_pipeline + draw_indexed recorded.
    assert!(cmd.commands.iter().any(|c| c == "bind_pipeline"));
    assert!(cmd.commands.iter().any(|c| c == "draw_indexed"));
}

#[test]
#[serial]
fn test_forward_drawer_draw_skips_invalid_render_instance() {
    setup_engine_with_main_device();
    let (mesh_key, vertex_shader_key) = populate_resource_manager();

    let mut scene = Scene::new();
    let key = {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(
            mesh_key, Mat4::IDENTITY, create_test_aabb(),
            vertex_shader_key, &[], &rm,
        ).unwrap()
    };

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut view = RenderView::new(camera.clone(), 0);
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        ViewDispatcher::dispatch(&visible, &mut scene, &rm, std::slice::from_mut(&mut view));
    }

    // Remove the instance after dispatch — drawer should skip the stale entry.
    scene.remove_render_instance(key);
    scene.removed_instances();

    let mut drawer = ForwardDrawer::new();
    let mut cmd = MockCommandList::new();
    let bg: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("test_bg".to_string(), 1));
    let info = make_pass_info();
    let result = drawer.draw(&mut scene, &view, &mut cmd, &info, &bg, true);
    assert!(result.is_ok());
    // No draw_indexed expected — the instance was removed.
    assert!(!cmd.commands.iter().any(|c| c == "draw_indexed"));
}

#[test]
#[serial]
fn test_forward_drawer_draw_skips_textures_when_disabled() {
    setup_engine_with_main_device();
    let (mesh_key, vertex_shader_key) = populate_resource_manager();

    let mut scene = Scene::new();
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        scene.create_render_instance(
            mesh_key, Mat4::IDENTITY, create_test_aabb(),
            vertex_shader_key, &[], &rm,
        ).unwrap();
    }

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut view = RenderView::new(camera.clone(), 0);
    {
        let rm_arc = Engine::resource_manager().unwrap();
        let rm = rm_arc.lock().unwrap();
        ViewDispatcher::dispatch(&visible, &mut scene, &rm, std::slice::from_mut(&mut view));
    }

    let mut drawer = ForwardDrawer::new();
    let mut cmd = MockCommandList::new();
    let bg: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("test_bg".to_string(), 1));
    let info = make_pass_info();
    drawer.draw(&mut scene, &view, &mut cmd, &info, &bg, false).unwrap();
    // bind_textures should NOT have been emitted.
    assert!(!cmd.commands.iter().any(|c| c == "bind_textures"));
}
