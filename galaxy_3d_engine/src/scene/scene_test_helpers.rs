//! Shared test helpers for scene-level tests (culler, view_dispatcher, etc.).
//!
//! Builds a self-contained `ResourceManager` populated with a minimal mesh
//! plus a default pipeline/material — backed by a `MockGraphicsDevice`, so no
//! real GPU is required.

use std::sync::{Arc, Mutex};
use glam::{Mat4, Vec3};

use crate::camera::{Camera, Frustum};
use crate::graphics_device::{
    self, mock_graphics_device::MockGraphicsDevice,
    PrimitiveTopology, BufferFormat, ShaderStage,
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, IndexType, PolygonMode, Viewport,
};
use crate::resource::geometry::{
    GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::material::{MaterialDesc, MaterialPassDesc, ParamValue};
use crate::resource::mesh::{MeshDesc, MeshSubMeshDesc, GeometryMeshRef, GeometrySubMeshRef};
use crate::resource::shader::ShaderDesc;
use crate::resource::buffer::Buffer;
use crate::resource::resource_manager::{ResourceManager, MeshKey, ShaderKey};
use crate::scene::render_instance::AABB;

pub(crate) struct TestSetup {
    pub rm: ResourceManager,
    pub mesh_key: MeshKey,
    pub vertex_shader_key: ShaderKey,
}

pub(crate) fn create_mock_graphics_device() -> Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
    Arc::new(Mutex::new(MockGraphicsDevice::new()))
}

pub(crate) fn create_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![VertexBinding { binding: 0, stride: 8, input_rate: VertexInputRate::Vertex }],
        attributes: vec![VertexAttribute { location: 0, binding: 0, format: BufferFormat::R32G32_SFLOAT, offset: 0 }],
    }
}

pub(crate) fn create_test_aabb() -> AABB {
    AABB { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) }
}

pub(crate) fn create_test_camera() -> Camera {
    let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
    let viewport = Viewport { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, min_depth: 0.0, max_depth: 1.0 };
    Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport)
}

fn create_test_shaders(
    rm: &mut ResourceManager,
    gd: &Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
) -> (ShaderKey, ShaderKey) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let vk = rm.create_shader(
        format!("vert_{}", id),
        ShaderDesc { code: &[], stage: ShaderStage::Vertex, entry_point: "main".to_string() },
        &mut *gd.lock().unwrap(),
    ).unwrap();
    let fk = rm.create_shader(
        format!("frag_{}", id),
        ShaderDesc { code: &[], stage: ShaderStage::Fragment, entry_point: "main".to_string() },
        &mut *gd.lock().unwrap(),
    ).unwrap();
    (vk, fk)
}

/// Build a `ResourceManager` populated with a minimal cube mesh, default
/// pipeline, and default material. Returns the keys needed to instantiate
/// it in a `Scene`.
pub(crate) fn setup_resources() -> TestSetup {
    let gd = create_mock_graphics_device();
    let mut rm = ResourceManager::new();

    let geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
        name: "geo".to_string(),
        graphics_device: gd.clone(),
        vertex_data: vec![0u8; 48],
        index_data: Some(vec![0u8; 12]),
        vertex_layout: create_vertex_layout(),
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

    let (vk, fk) = create_test_shaders(&mut rm, &gd);
    let _pk = rm.create_pipeline("p".to_string(), PipelineDesc {
        vertex_shader: vk,
        fragment_shader: fk,
        vertex_layout: create_vertex_layout(),
        topology: PrimitiveTopology::TriangleList,
        rasterization: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
        color_formats: vec![],
        depth_format: None,
    }, &mut *gd.lock().unwrap()).unwrap();

    let mk = rm.create_material("m".to_string(), MaterialDesc {
        passes: vec![MaterialPassDesc {
            pass_type: 0,
            fragment_shader: fk,
            color_blend: Default::default(),
            polygon_mode: PolygonMode::Fill,
            textures: vec![],
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

    TestSetup { rm, mesh_key, vertex_shader_key: vk }
}

/// Build a frame uniform buffer detached from the global Engine, sized to
/// match `DefaultUpdater::update_frame`'s field layout.
pub(crate) fn make_frame_buffer(rm: &mut ResourceManager) -> Arc<Buffer> {
    let gd = create_mock_graphics_device();
    let key = rm.create_default_frame_uniform_buffer("frame".to_string(), gd).unwrap();
    rm.buffer(key).unwrap().clone()
}

/// Build an instance buffer detached from the global Engine.
pub(crate) fn make_instance_buffer(rm: &mut ResourceManager, count: u32) -> Arc<Buffer> {
    let gd = create_mock_graphics_device();
    let key = rm.create_default_instance_buffer("instance".to_string(), gd, count).unwrap();
    rm.buffer(key).unwrap().clone()
}

/// Build a light buffer detached from the global Engine.
pub(crate) fn make_light_buffer(rm: &mut ResourceManager, count: u32) -> Arc<Buffer> {
    let gd = create_mock_graphics_device();
    let key = rm.create_default_light_buffer("light".to_string(), gd, count).unwrap();
    rm.buffer(key).unwrap().clone()
}
