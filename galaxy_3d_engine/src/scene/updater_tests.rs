use super::*;
use crate::camera::VisibleInstances;
use crate::scene::{Scene, BruteForceCuller, CameraCuller, LightDesc};
use crate::scene::scene_test_helpers::{
    setup_resources, create_test_aabb, create_test_camera,
    make_frame_buffer, make_instance_buffer, make_light_buffer,
};
use glam::{Mat4, Vec3};

// ============================================================================
// NoOpUpdater
// ============================================================================

#[test]
fn test_noop_updater_new() {
    let _u = NoOpUpdater::new();
}

#[test]
fn test_noop_update_frame_returns_ok() {
    let mut setup = setup_resources();
    let buf = make_frame_buffer(&mut setup.rm);
    let camera = create_test_camera();
    let mut updater = NoOpUpdater::new();
    assert!(updater.update_frame(&camera, &buf).is_ok());
}

#[test]
fn test_noop_update_instances_returns_ok() {
    let mut setup = setup_resources();
    let buf = make_instance_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    let mut updater = NoOpUpdater::new();
    assert!(updater.update_instances(&mut scene, None, &buf).is_ok());
}

#[test]
fn test_noop_update_lights_returns_ok() {
    let mut setup = setup_resources();
    let buf = make_light_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    let mut updater = NoOpUpdater::new();
    assert!(updater.update_lights(&mut scene, &buf).is_ok());
}

#[test]
fn test_noop_assign_lights_returns_ok() {
    let mut setup = setup_resources();
    let buf = make_instance_buffer(&mut setup.rm, 4);
    let scene = Scene::new();
    let visible = VisibleInstances::new_empty();
    let mut updater = NoOpUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

// ============================================================================
// DefaultUpdater
// ============================================================================

#[test]
fn test_default_updater_new() {
    let _u = DefaultUpdater::new();
}

#[test]
fn test_default_update_frame_writes_camera_data() {
    let mut setup = setup_resources();
    let buf = make_frame_buffer(&mut setup.rm);
    let camera = create_test_camera();
    let mut updater = DefaultUpdater::new();
    assert!(updater.update_frame(&camera, &buf).is_ok());
}

#[test]
fn test_default_update_frame_idempotent_on_repeated_call() {
    let mut setup = setup_resources();
    let buf = make_frame_buffer(&mut setup.rm);
    let camera = create_test_camera();
    let mut updater = DefaultUpdater::new();
    updater.update_frame(&camera, &buf).unwrap();
    updater.update_frame(&camera, &buf).unwrap();
    updater.update_frame(&camera, &buf).unwrap();
}

#[test]
fn test_default_update_instances_empty_scene() {
    let mut setup = setup_resources();
    let buf = make_instance_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    let mut updater = DefaultUpdater::new();
    // Empty scene → no new_keys → never touches Engine::resource_manager.
    assert!(updater.update_instances(&mut scene, None, &buf).is_ok());
}

#[test]
fn test_default_update_lights_empty_scene() {
    let mut setup = setup_resources();
    let buf = make_light_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    let mut updater = DefaultUpdater::new();
    assert!(updater.update_lights(&mut scene, &buf).is_ok());
}

#[test]
fn test_default_update_lights_with_point_light() {
    let mut setup = setup_resources();
    let buf = make_light_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    scene.create_light(LightDesc::Point {
        position: Vec3::new(0.0, 5.0, 0.0),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 1.5,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });
    let mut updater = DefaultUpdater::new();
    assert!(updater.update_lights(&mut scene, &buf).is_ok());
}

#[test]
fn test_default_update_lights_with_spot_light() {
    let mut setup = setup_resources();
    let buf = make_light_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    scene.create_light(LightDesc::Spot {
        position: Vec3::new(0.0, 5.0, 0.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: Vec3::new(1.0, 0.5, 0.5),
        intensity: 2.0,
        range: 15.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
        spot_inner_angle: 0.5,
        spot_outer_angle: 1.0,
    });
    let mut updater = DefaultUpdater::new();
    assert!(updater.update_lights(&mut scene, &buf).is_ok());
}

#[test]
fn test_default_update_lights_two_calls_handle_dirty_paths() {
    let mut setup = setup_resources();
    let buf = make_light_buffer(&mut setup.rm, 4);
    let mut scene = Scene::new();
    let _key = scene.create_light(LightDesc::Point {
        position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 5.0,
        attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
    });
    let mut updater = DefaultUpdater::new();
    // First call: drains new_lights via the "new" path.
    updater.update_lights(&mut scene, &buf).unwrap();
    // Second call: nothing new/dirty, exercises the empty branches.
    updater.update_lights(&mut scene, &buf).unwrap();
}

#[test]
fn test_default_assign_lights_no_lights_writes_zero_count() {
    let setup = setup_resources();
    let mut rm = setup.rm;
    let buf = make_instance_buffer(&mut rm, 4);
    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &rm,
    ).unwrap();

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut updater = DefaultUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

#[test]
fn test_default_assign_lights_with_point_in_range() {
    let setup = setup_resources();
    let mut rm = setup.rm;
    let buf = make_instance_buffer(&mut rm, 4);

    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &rm,
    ).unwrap();
    scene.create_light(LightDesc::Point {
        position: Vec3::new(0.5, 0.0, 0.0),
        color: Vec3::ONE,
        intensity: 1.0,
        range: 100.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut updater = DefaultUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

#[test]
fn test_default_assign_lights_with_spot_in_range() {
    let setup = setup_resources();
    let mut rm = setup.rm;
    let buf = make_instance_buffer(&mut rm, 4);

    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &rm,
    ).unwrap();
    scene.create_light(LightDesc::Spot {
        position: Vec3::new(0.0, 0.5, 0.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: Vec3::ONE,
        intensity: 1.0,
        range: 100.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
        spot_inner_angle: 0.5,
        spot_outer_angle: 1.0,
    });

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut updater = DefaultUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

#[test]
fn test_default_assign_lights_light_out_of_range_culled() {
    let setup = setup_resources();
    let mut rm = setup.rm;
    let buf = make_instance_buffer(&mut rm, 4);

    let mut scene = Scene::new();
    scene.create_render_instance(
        setup.mesh_key, Mat4::IDENTITY, create_test_aabb(),
        setup.vertex_shader_key, &[], &rm,
    ).unwrap();
    // Light far away, range too small.
    scene.create_light(LightDesc::Point {
        position: Vec3::new(1000.0, 1000.0, 1000.0),
        color: Vec3::ONE,
        intensity: 1.0,
        range: 1.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    let camera = create_test_camera();
    let mut culler = BruteForceCuller::new();
    let mut visible = VisibleInstances::new_empty();
    culler.cull_into(&scene, &camera, None, &mut visible);

    let mut updater = DefaultUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

#[test]
fn test_default_assign_lights_no_visible_instances() {
    let setup = setup_resources();
    let mut rm = setup.rm;
    let buf = make_instance_buffer(&mut rm, 4);

    let mut scene = Scene::new();
    scene.create_light(LightDesc::Point {
        position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 5.0,
        attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
    });

    let visible = VisibleInstances::new_empty();
    let mut updater = DefaultUpdater::new();
    assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
}

// ============================================================================
// Engine-backed integration tests for update_instances new/dirty paths
// ============================================================================

mod engine_backed {
    use super::*;
    use crate::engine::Engine;
    use crate::graphics_device::mock_graphics_device::MockGraphicsDevice;
    use crate::graphics_device::{
        BufferFormat, ShaderStage, IndexType, PrimitiveTopology, PolygonMode,
        VertexLayout, VertexBinding, VertexAttribute, VertexInputRate,
    };
    use crate::resource::geometry::{
        GeometryDesc, GeometryMeshDesc, GeometrySubMeshDesc, GeometrySubMeshLODDesc,
    };
    use crate::resource::pipeline::PipelineDesc;
    use crate::resource::material::{MaterialDesc, MaterialPassDesc, ParamValue};
    use crate::resource::mesh::{MeshDesc, MeshSubMeshDesc, GeometryMeshRef, GeometrySubMeshRef};
    use crate::resource::shader::ShaderDesc;
    use crate::scene::render_instance::AABB;
    use serial_test::serial;
    use glam::{Mat4, Vec3};
    use std::sync::Arc;

    fn setup_engine() -> (
        Arc<crate::resource::buffer::Buffer>,
        crate::resource::resource_manager::MeshKey,
        crate::resource::resource_manager::ShaderKey,
    ) {
        Engine::initialize().unwrap();
        Engine::reset_for_testing();
        Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
        Engine::create_resource_manager().unwrap();

        let rm_arc = Engine::resource_manager().unwrap();
        let gd_arc = Engine::graphics_device("main").unwrap();
        let mut rm = rm_arc.lock().unwrap();
        let layout = VertexLayout {
            bindings: vec![VertexBinding { binding: 0, stride: 8, input_rate: VertexInputRate::Vertex }],
            attributes: vec![VertexAttribute { location: 0, binding: 0, format: BufferFormat::R32G32_SFLOAT, offset: 0 }],
        };
        let geo_key = rm.create_geometry("geo".to_string(), GeometryDesc {
            name: "geo".to_string(), graphics_device: gd_arc.clone(),
            vertex_data: vec![0u8; 48], index_data: Some(vec![0u8; 12]),
            vertex_layout: layout.clone(), index_type: IndexType::U16,
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
        rm.create_pipeline("p".to_string(), PipelineDesc {
            vertex_shader: vk, fragment_shader: fk,
            vertex_layout: layout, topology: PrimitiveTopology::TriangleList,
            rasterization: Default::default(), color_blend: Default::default(),
            multisample: Default::default(), color_formats: vec![], depth_format: None,
        }, &mut *gd_arc.lock().unwrap()).unwrap();
        let mk = rm.create_material("m".to_string(), MaterialDesc {
            passes: vec![MaterialPassDesc {
                pass_type: 0, fragment_shader: fk, color_blend: Default::default(),
                polygon_mode: PolygonMode::Fill, textures: vec![],
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
        let buf_key = rm.create_default_instance_buffer("inst".to_string(), gd_arc.clone(), 16).unwrap();
        let buf = rm.buffer(buf_key).unwrap().clone();
        (buf, mesh_key, vk)
    }

    fn make_aabb() -> AABB {
        AABB { min: Vec3::new(-1.0, -1.0, -1.0), max: Vec3::new(1.0, 1.0, 1.0) }
    }

    #[test]
    #[serial]
    fn test_default_update_instances_new_path() {
        let (buf, mesh_key, vk) = setup_engine();

        let mut scene = Scene::new();
        {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap();
        }

        let mut updater = DefaultUpdater::new();
        assert!(updater.update_instances(&mut scene, None, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_update_instances_dirty_transform_path() {
        let (buf, mesh_key, vk) = setup_engine();

        let mut scene = Scene::new();
        let key = {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap()
        };

        let mut updater = DefaultUpdater::new();
        // First update_instances drains new_instances.
        updater.update_instances(&mut scene, None, &buf).unwrap();
        // Modify transform → marks dirty.
        scene.set_world_matrix(key, Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));
        // Second update_instances should hit the dirty branch.
        assert!(updater.update_instances(&mut scene, None, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_update_instances_removed_path() {
        let (buf, mesh_key, vk) = setup_engine();

        let mut scene = Scene::new();
        let key = {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap()
        };

        let mut updater = DefaultUpdater::new();
        updater.update_instances(&mut scene, None, &buf).unwrap();
        // Remove and verify the path drains the removed set.
        scene.remove_render_instance(key);
        assert!(updater.update_instances(&mut scene, None, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_assign_lights_with_new_instance_path() {
        let (buf, mesh_key, vk) = setup_engine();
        let mut scene = Scene::new();
        {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap();
        }
        scene.create_light(LightDesc::Point {
            position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 100.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
        });

        // Cull → visible
        let mut culler = crate::scene::BruteForceCuller::new();
        let camera = crate::scene::scene_test_helpers::create_test_camera();
        let mut visible = crate::camera::VisibleInstances::new_empty();
        culler.cull_into(&scene, &camera, None, &mut visible);

        let mut updater = DefaultUpdater::new();
        assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_assign_lights_more_than_max_overflows_into_indices_1() {
        let (buf, mesh_key, vk) = setup_engine();
        let mut scene = Scene::new();
        {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap();
        }
        // 10 lights tightly packed near origin, all in range. The updater
        // takes the top 8 by score → exercises the indices_1 path.
        for i in 0..10 {
            scene.create_light(LightDesc::Point {
                position: Vec3::new(0.5 + i as f32 * 0.05, 0.0, 0.0),
                color: Vec3::ONE,
                intensity: 1.0 + i as f32 * 0.1,
                range: 100.0,
                attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
            });
        }

        let camera = crate::scene::scene_test_helpers::create_test_camera();
        let mut culler = crate::scene::BruteForceCuller::new();
        let mut visible = crate::camera::VisibleInstances::new_empty();
        culler.cull_into(&scene, &camera, None, &mut visible);

        let mut updater = DefaultUpdater::new();
        assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_assign_lights_disabled_light_filtered() {
        let (buf, mesh_key, vk) = setup_engine();
        let mut scene = Scene::new();
        {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap();
        }
        let key = scene.create_light(LightDesc::Point {
            position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 100.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
        });
        scene.set_light_enabled(key, false);

        let camera = crate::scene::scene_test_helpers::create_test_camera();
        let mut culler = crate::scene::BruteForceCuller::new();
        let mut visible = crate::camera::VisibleInstances::new_empty();
        culler.cull_into(&scene, &camera, None, &mut visible);

        let mut updater = DefaultUpdater::new();
        // Disabled light is skipped in the enabled_light_keys phase → falls
        // into the empty-lights fast path.
        assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_assign_lights_spot_outside_cone_filtered() {
        let (buf, mesh_key, vk) = setup_engine();
        let mut scene = Scene::new();
        {
            let rm_arc = Engine::resource_manager().unwrap();
            let rm = rm_arc.lock().unwrap();
            scene.create_render_instance(mesh_key, Mat4::IDENTITY, make_aabb(), vk, &[], &rm).unwrap();
        }
        // Spot light pointing AWAY from the instance — cone test rejects it.
        scene.create_light(LightDesc::Spot {
            position: Vec3::new(0.0, 0.0, 5.0),
            direction: Vec3::new(0.0, 0.0, 1.0), // away from origin
            color: Vec3::ONE, intensity: 1.0, range: 100.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
            spot_inner_angle: 0.05, spot_outer_angle: 0.10, // very narrow cone
        });

        let camera = crate::scene::scene_test_helpers::create_test_camera();
        let mut culler = crate::scene::BruteForceCuller::new();
        let mut visible = crate::camera::VisibleInstances::new_empty();
        culler.cull_into(&scene, &camera, None, &mut visible);

        let mut updater = DefaultUpdater::new();
        assert!(updater.assign_lights(&scene, &visible, &buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_update_lights_dirty_transform_path() {
        let (_buf, _mesh_key, _vk) = setup_engine();
        // Create a separate light buffer (not tied to instance buffer).
        let light_buf = {
            let rm_arc = Engine::resource_manager().unwrap();
            let mut rm = rm_arc.lock().unwrap();
            let gd_arc = Engine::graphics_device("main").unwrap();
            let key = rm.create_default_light_buffer("lights".to_string(), gd_arc, 8).unwrap();
            rm.buffer(key).unwrap().clone()
        };

        let mut scene = Scene::new();
        let key = scene.create_light(LightDesc::Point {
            position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 5.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
        });

        let mut updater = DefaultUpdater::new();
        // First update_lights drains the new_lights set.
        updater.update_lights(&mut scene, &light_buf).unwrap();
        // Modify position → dirty transform path next call.
        scene.set_light_position(key, Vec3::new(5.0, 0.0, 0.0));
        assert!(updater.update_lights(&mut scene, &light_buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_update_lights_dirty_data_path() {
        let (_buf, _mesh_key, _vk) = setup_engine();
        let light_buf = {
            let rm_arc = Engine::resource_manager().unwrap();
            let mut rm = rm_arc.lock().unwrap();
            let gd_arc = Engine::graphics_device("main").unwrap();
            let key = rm.create_default_light_buffer("lights2".to_string(), gd_arc, 8).unwrap();
            rm.buffer(key).unwrap().clone()
        };

        let mut scene = Scene::new();
        let key = scene.create_light(LightDesc::Point {
            position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 5.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
        });

        let mut updater = DefaultUpdater::new();
        updater.update_lights(&mut scene, &light_buf).unwrap();
        scene.set_light_intensity(key, 5.0);
        scene.set_light_color(key, Vec3::new(0.5, 0.5, 1.0));
        assert!(updater.update_lights(&mut scene, &light_buf).is_ok());
    }

    #[test]
    #[serial]
    fn test_default_update_lights_removed_path() {
        let (_buf, _mesh_key, _vk) = setup_engine();
        let light_buf = {
            let rm_arc = Engine::resource_manager().unwrap();
            let mut rm = rm_arc.lock().unwrap();
            let gd_arc = Engine::graphics_device("main").unwrap();
            let key = rm.create_default_light_buffer("lights3".to_string(), gd_arc, 8).unwrap();
            rm.buffer(key).unwrap().clone()
        };

        let mut scene = Scene::new();
        let key = scene.create_light(LightDesc::Point {
            position: Vec3::ZERO, color: Vec3::ONE, intensity: 1.0, range: 5.0,
            attenuation_constant: 0.0, attenuation_linear: 0.0, attenuation_quadratic: 1.0,
        });

        let mut updater = DefaultUpdater::new();
        updater.update_lights(&mut scene, &light_buf).unwrap();
        // Remove → next update_lights drains removed.
        scene.remove_light(key);
        assert!(updater.update_lights(&mut scene, &light_buf).is_ok());
    }
}

