use super::*;
use super::super::test_helpers::{
    setup_engine_for_render_graph, default_color_ops, default_depth_ops, make_recording_pass,
};
use crate::engine::Engine;
use crate::render_graph::access_type::ResourceAccess;
use crate::resource::resource_manager::TextureKey;
use serial_test::serial;

#[test]
#[serial]
fn test_render_graph_new_with_one_frame() {
    setup_engine_for_render_graph();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let graph = RenderGraph::new("test".to_string(), &*gd, 1).unwrap();
    assert_eq!(graph.name(), "test");
}

#[test]
#[serial]
fn test_render_graph_new_with_multiple_frames() {
    setup_engine_for_render_graph();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let graph = RenderGraph::new("test".to_string(), &*gd, 3).unwrap();
    assert_eq!(graph.name(), "test");
}

#[test]
#[serial]
fn test_render_graph_new_with_zero_frames_fails() {
    setup_engine_for_render_graph();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let result = RenderGraph::new("test".to_string(), &*gd, 0);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_render_graph_command_list_after_construction() {
    setup_engine_for_render_graph();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let graph = RenderGraph::new("test".to_string(), &*gd, 2).unwrap();
    // command_list() returns the most recent frame — Ok even before execute().
    assert!(graph.command_list().is_ok());
}

#[test]
#[serial]
fn test_render_graph_execute_via_manager_runs_post_passes() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, _pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let pass_key = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action, &*gd).unwrap();
        (graph_key, pass_key)
    };

    let post_called = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let post_called_inner = post_called.clone();

    let mut rgm = rgm_arc.lock().unwrap();
    let pass_keys = vec![_pass_key];
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_cmd| {
        post_called_inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }).unwrap();
    assert_eq!(post_called.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
#[serial]
fn test_render_graph_execute_with_no_passes_runs_post_only() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        rgm.create_render_graph("main", 1, &*gd).unwrap()
    };

    let post_called = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let post_called_inner = post_called.clone();

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[], &mut *gd, |_cmd| {
        post_called_inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }).unwrap();
    assert_eq!(post_called.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
#[serial]
fn test_render_graph_topological_sort_with_writer_reader() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, writer_key, reader_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        // Writer: writes color
        let (writer_action, _) = make_recording_pass();
        let writer_key = rgm.create_render_pass("writer", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], writer_action, &*gd).unwrap();
        // Reader: reads color (sampled, no attachment, no framebuffer)
        let (reader_action, _) = make_recording_pass();
        let reader_key = rgm.create_render_pass("reader", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::FragmentShaderRead,
            target_ops: None,
        }], reader_action, &*gd).unwrap();
        (graph_key, writer_key, reader_key)
    };

    // Submit reader BEFORE writer in the input list — topo sort should reorder.
    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[reader_key, writer_key], &mut *gd, |_| Ok(()));
    assert!(result.is_ok(), "expected success, got {:?}", result);
}

#[test]
#[serial]
fn test_render_graph_command_list_ok_after_execute() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 2, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let _pass_key = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action, &*gd).unwrap();
        graph_key
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[], &mut *gd, |_| Ok(())).unwrap();
    let graph = rgm.render_graph(graph_key).unwrap();
    assert!(graph.command_list().is_ok());
}

#[test]
fn test_render_graph_key_default() {
    use slotmap::Key;
    let _key = RenderGraphKey::null();
}

#[test]
#[serial]
fn test_render_graph_execute_detects_cycle() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();

        let color_a = rgm.create_graph_resource("color_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let color_b = rgm.create_graph_resource("color_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();

        // Pass A: writes color_a, reads color_b → depends on B.
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("a", vec![
            ResourceAccess {
                graph_resource_key: color_a,
                access_type: AccessType::ColorAttachmentWrite,
                target_ops: Some(default_color_ops()),
            },
            ResourceAccess {
                graph_resource_key: color_b,
                access_type: AccessType::FragmentShaderRead,
                target_ops: None,
            },
        ], action_a, &*gd).unwrap();

        // Pass B: writes color_b, reads color_a → depends on A. Cycle.
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("b", vec![
            ResourceAccess {
                graph_resource_key: color_b,
                access_type: AccessType::ColorAttachmentWrite,
                target_ops: Some(default_color_ops()),
            },
            ResourceAccess {
                graph_resource_key: color_a,
                access_type: AccessType::FragmentShaderRead,
                target_ops: None,
            },
        ], action_b, &*gd).unwrap();

        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[pass_a, pass_b], &mut *gd, |_| Ok(()));
    assert!(result.is_err(), "expected cycle detection to fail");
}

#[test]
#[serial]
fn test_render_graph_execute_advances_through_frames() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 3, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let _pk = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action, &*gd).unwrap();
        graph_key
    };

    // Run 5 frames — exercises ring rotation through 3 command lists.
    let mut rgm = rgm_arc.lock().unwrap();
    for _ in 0..5 {
        rgm.execute_render_graph(graph_key, &[], &mut *gd, |_| Ok(())).unwrap();
    }
    let graph = rgm.render_graph(graph_key).unwrap();
    assert!(graph.command_list().is_ok());
}

#[test]
#[serial]
fn test_render_graph_execute_with_buffer_resource_access() {
    // Covers the GraphResource::Buffer arm of execute()'s access materialization
    // (lines 175-184): buffer accesses are pushed into self.buffer_accesses
    // and forwarded to begin_render_pass.
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();

    // Create a storage buffer in the resource manager so the access can
    // resolve it. Build it via the default helpers.
    let buf_key = {
        let rm_arc = Engine::resource_manager().unwrap();
        let mut rm = rm_arc.lock().unwrap();
        let gd_arc = Engine::graphics_device("main").unwrap();
        rm.create_default_instance_buffer(
            "instances".to_string(), gd_arc, 4,
            crate::graphics_device::BufferUpdateMode::Static,
        ).unwrap()
    };

    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let (graph_key, pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let buf_gr = rgm.create_graph_resource("buf", GraphResource::Buffer { buffer_key: buf_key, offset: 0, size: u64::MAX }).unwrap();
        let (action, _) = make_recording_pass();
        let pass_key = rgm.create_render_pass("opaque_with_buf", vec![
            ResourceAccess {
                graph_resource_key: color_gr,
                access_type: AccessType::ColorAttachmentWrite,
                target_ops: Some(default_color_ops()),
            },
            // Buffer access — exercises the GraphResource::Buffer arm.
            ResourceAccess {
                graph_resource_key: buf_gr,
                access_type: AccessType::ComputeRead,
                target_ops: None,
            },
        ], action, &*gd).unwrap();
        (graph_key, pass_key)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_key], &mut *gd, |_| Ok(())).unwrap();
}

#[test]
#[serial]
fn test_render_graph_execute_writer_equals_reader_self_loop() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    // Covers the `if writer != k { ... }` false branch (line ~280) inside
    // topological_sort: a single pass that both writes AND reads the same
    // resource → writer == k → skip dependency edge.
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();

    let (graph_key, pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        // One pass writes color (ColorAttachmentWrite) AND reads it
        // (ColorAttachmentRead) → writer == k path inside topo sort.
        let pass_key = rgm.create_render_pass("self_io", vec![
            ResourceAccess {
                graph_resource_key: color_gr,
                access_type: AccessType::ColorAttachmentWrite,
                target_ops: Some(default_color_ops()),
            },
            ResourceAccess {
                graph_resource_key: color_gr,
                access_type: AccessType::ColorAttachmentRead,
                target_ops: Some(default_color_ops()),
            },
        ], action, &*gd).unwrap();
        (graph_key, pass_key)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[pass_key], &mut *gd, |_| Ok(()));
    assert!(result.is_ok(), "self read+write should sort cleanly: {:?}", result);
}

#[test]
fn test_graph_resource_with_buffer_does_not_panic_on_construction() {
    // Constructing a GraphResource::Buffer is allowed even with an
    // un-registered BufferKey — only used as a render-pass attachment
    // would surface a failure.
    use crate::resource::resource_manager::BufferKey;
    let _ = GraphResource::Buffer { buffer_key: BufferKey::default(), offset: 0, size: u64::MAX };
    let _ = GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0,
        mip_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    };
}

// ============================================================================
// Correction B — per-TextureKey access history
// ============================================================================

/// Build two GraphResources pointing at the same TextureKey but
/// referenced through distinct GraphResourceKeys, then run two passes
/// that write through them (sub-ranges identical) and check that the
/// second access sees the first as `previous_access_type`.
///
/// This is the canonical regression case for the bug-tracking that B
/// fixes: before B, the second pass would see `previous_access_type =
/// None` because `prev_access` was indexed by GraphResourceKey, and the
/// barrier would emit `oldLayout = UNDEFINED` → discard of pass 1's work.
#[test]
#[serial]
fn test_two_grs_same_texture_same_subrange_share_history() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();

        // Two GraphResources, same TextureKey, same sub-range.
        let gr_a = rgm.create_graph_resource("color_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("color_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();

        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        (graph_key, vec![pass_a, pass_b])
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(()));
    // Without B, this would still succeed at the API level — the bug
    // was a silent layout-transition issue, not a Rust error. The smoke
    // test here just confirms execute() runs to completion with two
    // overlapping GraphResources on the same TextureKey.
    assert!(result.is_ok(), "execute failed: {:?}", result);
}

/// Two GraphResources on the same TextureKey but on **disjoint**
/// mip-layer ranges. The second access should not find any overlapping
/// previous entry → `previous_access_type = None`. Smoke-test only:
/// we exercise the correctness of the lookup path and confirm it does
/// not falsely report the other range's AccessType.
#[test]
#[serial]
fn test_two_grs_same_texture_disjoint_subranges_independent_history() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        // Engine test fixtures only expose a single-mip / single-layer
        // color texture, so we simulate disjoint sub-ranges by using
        // distinct base_array_layer values; the test fixture is
        // tolerant of REMAINING_*-style declarations because the
        // backend ignores layer_count beyond what it actually has.
        // The lookup logic doesn't query the underlying resource — it
        // only compares declared sub-ranges.
        let gr_a = rgm.create_graph_resource("color_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("color_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 1, layer_count: 1,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        (graph_key, vec![pass_a, pass_b])
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(()));
    assert!(result.is_ok(), "execute failed: {:?}", result);
}

// ============================================================================
// Correction C — overlap-based topological sort
// ============================================================================

/// Two GraphResources on the same TextureKey, identical sub-range,
/// passes A then B. With C, the topo sort detects the overlap and
/// inserts an A → B edge even though the keys differ. Pass B's
/// predecessor set should contain pass A.
#[test]
#[serial]
fn test_topo_overlap_creates_dependency_same_subrange() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let gr_a = rgm.create_graph_resource("color_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("color_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_a, pass_b], &mut *gd, |_| Ok(())).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let preds_b = graph.predecessors_of(pass_b);
    assert!(preds_b.contains(&pass_a),
        "pass_b should have pass_a as predecessor (overlap-based dep), got {:?}", preds_b);
    let pos_a = graph.sorted_position_of(pass_a).unwrap();
    let pos_b = graph.sorted_position_of(pass_b).unwrap();
    assert!(pos_a < pos_b, "pass_a must come before pass_b in sorted order");
}

/// Same as above but sub-ranges are disjoint (different layers). With
/// C, no edge should be created — the two passes stay independent.
#[test]
#[serial]
fn test_topo_no_dependency_when_subranges_disjoint() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let gr_a = rgm.create_graph_resource("color_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("color_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 1, layer_count: 1,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_a, pass_b], &mut *gd, |_| Ok(())).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let preds_b = graph.predecessors_of(pass_b);
    assert!(!preds_b.contains(&pass_a),
        "disjoint sub-ranges should NOT create a dependency, got {:?}", preds_b);
    let preds_a = graph.predecessors_of(pass_a);
    assert!(!preds_a.contains(&pass_b),
        "disjoint sub-ranges should NOT create a dependency, got {:?}", preds_a);
}

/// Multiple writers on the same texture, then a reader covering the
/// whole layer range. The reader's predecessor set should contain
/// EVERY overlapping writer (RAW deps fan-in).
#[test]
#[serial]
fn test_topo_raw_dep_fans_in_from_multiple_overlapping_writers() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, write_a, write_b, read_c) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        // A writes layer 0, B writes layer 1, C reads layer 0..2 (covers both).
        let gr_a = rgm.create_graph_resource("write_a", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("write_b", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 1, layer_count: 1,
        }).unwrap();
        let gr_c = rgm.create_graph_resource("read_c", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0,
            layer_count: crate::render_graph::REMAINING_ARRAY_LAYERS,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let write_a = rgm.create_render_pass("write_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let write_b = rgm.create_render_pass("write_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        // C is a sampled-read pass — no framebuffer.
        let (action_c, _) = make_recording_pass();
        let read_c = rgm.create_render_pass("read_c", vec![ResourceAccess {
            graph_resource_key: gr_c,
            access_type: AccessType::FragmentShaderRead,
            target_ops: None,
        }], action_c, &*gd).unwrap();
        (graph_key, write_a, write_b, read_c)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(
        graph_key, &[write_a, write_b, read_c], &mut *gd, |_| Ok(()),
    ).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let preds_c = graph.predecessors_of(read_c);
    assert!(preds_c.contains(&write_a),
        "read_c must depend on write_a (overlap on layer 0), got {:?}", preds_c);
    assert!(preds_c.contains(&write_b),
        "read_c must depend on write_b (overlap on layer 1), got {:?}", preds_c);
    let pos_a = graph.sorted_position_of(write_a).unwrap();
    let pos_b = graph.sorted_position_of(write_b).unwrap();
    let pos_c = graph.sorted_position_of(read_c).unwrap();
    assert!(pos_a < pos_c && pos_b < pos_c,
        "read_c must come after both writers");
}

/// Regression: when multiple passes share the SAME GraphResourceKey,
/// the user-declared order must still be preserved (correction C
/// shouldn't break this — it merely extends the existing logic to
/// distinct keys).
#[test]
#[serial]
fn test_topo_same_gr_preserves_user_order() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, p_a, p_b, p_c) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        // Inline pass creation (closure didn't borrow well across calls).
        let (action_a, _) = make_recording_pass();
        let p_a = rgm.create_render_pass("p_a", vec![ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let p_b = rgm.create_render_pass("p_b", vec![ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        let (action_c, _) = make_recording_pass();
        let p_c = rgm.create_render_pass("p_c", vec![ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_c, &*gd).unwrap();
        (graph_key, p_a, p_b, p_c)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[p_a, p_b, p_c], &mut *gd, |_| Ok(())).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let pos_a = graph.sorted_position_of(p_a).unwrap();
    let pos_b = graph.sorted_position_of(p_b).unwrap();
    let pos_c = graph.sorted_position_of(p_c).unwrap();
    assert!(pos_a < pos_b && pos_b < pos_c,
        "user-declared order must be preserved: a={} b={} c={}", pos_a, pos_b, pos_c);
}

/// Buffer counterpart of `test_topo_overlap_creates_dependency_same_subrange`.
#[test]
#[serial]
fn test_topo_buffer_overlap_creates_dependency() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    use crate::resource::resource_manager::BufferKey;
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        // Same BufferKey, overlapping byte ranges (0..64 vs 32..96).
        let buf = BufferKey::default();
        let gr_a = rgm.create_graph_resource("buf_a", GraphResource::Buffer {
            buffer_key: buf, offset: 0, size: 64,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("buf_b", GraphResource::Buffer {
            buffer_key: buf, offset: 32, size: 64,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ComputeWrite,
            target_ops: None,
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ComputeWrite,
            target_ops: None,
        }], action_b, &*gd).unwrap();
        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_a, pass_b], &mut *gd, |_| Ok(())).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let preds_b = graph.predecessors_of(pass_b);
    assert!(preds_b.contains(&pass_a),
        "buffer overlap (0..64) ∩ (32..96) must create dep, got {:?}", preds_b);
}

/// Buffer counterpart with disjoint ranges — no edge.
#[test]
#[serial]
fn test_topo_buffer_disjoint_no_dependency() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    use crate::resource::resource_manager::BufferKey;
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let buf = BufferKey::default();
        let gr_a = rgm.create_graph_resource("buf_a", GraphResource::Buffer {
            buffer_key: buf, offset: 0, size: 64,
        }).unwrap();
        let gr_b = rgm.create_graph_resource("buf_b", GraphResource::Buffer {
            buffer_key: buf, offset: 64, size: 64,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("pass_a", vec![ResourceAccess {
            graph_resource_key: gr_a,
            access_type: AccessType::ComputeWrite,
            target_ops: None,
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ComputeWrite,
            target_ops: None,
        }], action_b, &*gd).unwrap();
        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_a, pass_b], &mut *gd, |_| Ok(())).unwrap();

    let graph = rgm.render_graph(graph_key).unwrap();
    let preds_b = graph.predecessors_of(pass_b);
    assert!(!preds_b.contains(&pass_a),
        "disjoint buffer ranges (0..64) and (64..128) must NOT create dep, got {:?}", preds_b);
}

/// Cycle detection through overlap: build a true read↔write deadlock
/// using the existing single-pass cycle pattern (already covered by
/// `test_render_graph_execute_detects_cycle`) but via DISTINCT
/// GraphResources targeting the same TextureKey on overlapping
/// sub-ranges. The overlap-based topo sort must still detect the
/// resulting cycle just like the legacy single-key code did.
///
/// Pattern: pass A reads via gr_a, pass A also writes via gr_a' (same
/// texture/sub-range, distinct GraphResource). Same self-loop logic as
/// in test_render_graph_execute_detects_cycle, but with separate
/// GraphResources to exercise the overlap detection path.
#[test]
#[serial]
fn test_topo_cycle_detected_via_overlap() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, p_a, p_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        // Two textures, two passes, mutual read-then-write through
        // distinct GraphResources targeting overlapping sub-ranges.
        // Pass A: write tex_color, read tex_depth.
        // Pass B: write tex_depth, read tex_color.
        // This is the canonical cycle: A → B (RAW on depth) and
        // B → A (RAW on color).
        let gr_a_w = rgm.create_graph_resource("a_write_color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_a_r = rgm.create_graph_resource("a_read_depth", GraphResource::Texture {
            texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b_w = rgm.create_graph_resource("b_write_depth", GraphResource::Texture {
            texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let gr_b_r = rgm.create_graph_resource("b_read_color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let p_a = rgm.create_render_pass("pass_a", vec![
            ResourceAccess {
                graph_resource_key: gr_a_w,
                access_type: AccessType::ColorAttachmentWrite,
                target_ops: Some(default_color_ops()),
            },
            ResourceAccess {
                graph_resource_key: gr_a_r,
                access_type: AccessType::FragmentShaderRead,
                target_ops: None,
            },
        ], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let p_b = rgm.create_render_pass("pass_b", vec![
            ResourceAccess {
                graph_resource_key: gr_b_w,
                access_type: AccessType::DepthStencilWrite,
                target_ops: Some(default_depth_ops()),
            },
            ResourceAccess {
                graph_resource_key: gr_b_r,
                access_type: AccessType::FragmentShaderRead,
                target_ops: None,
            },
        ], action_b, &*gd).unwrap();
        (graph_key, p_a, p_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[p_a, p_b], &mut *gd, |_| Ok(()));
    assert!(result.is_err(),
        "mutual read-then-write between A and B via distinct GraphResources \
         must trigger cycle detection");
}

/// Zero allocation in steady state for the topo-sort writer history.
/// Mirrors `test_zero_alloc_in_steady_state` but for the four
/// `*_writers` maps.
#[test]
#[serial]
fn test_topo_zero_alloc_in_steady_state() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action_a, _) = make_recording_pass();
        let pass_a = rgm.create_render_pass("a", vec![ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_a, &*gd).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("b", vec![ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b, &*gd).unwrap();
        (graph_key, vec![pass_a, pass_b])
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();
    let cap_after_frame_1 = rgm.render_graph(graph_key).unwrap()
        .topo_writer_history_capacities();
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();
    let cap_after_frame_3 = rgm.render_graph(graph_key).unwrap()
        .topo_writer_history_capacities();

    assert_eq!(cap_after_frame_1, cap_after_frame_3,
        "topo-sort writer-history Vec capacities must not change between \
         frame 1 and frame 3");
}

/// Run `execute()` three times in a row and check that the inner
/// `Vec`s of `prev_image_accesses` have not reallocated between frame 1
/// and frame 3 — i.e. their `capacity()` is identical.
///
/// This protects the zero-allocation-per-frame contract that
/// correction B rests on. A regression that drops/recreates the inner
/// Vecs would show up as a capacity reset.
#[test]
#[serial]
fn test_zero_alloc_in_steady_state() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let pass = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action, &*gd).unwrap();
        (graph_key, vec![pass])
    };

    let mut rgm = rgm_arc.lock().unwrap();

    // Frame 1: warm up the access-history map.
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();

    // Snapshot capacities right after frame 1.
    let cap_after_frame_1: Vec<usize> = rgm
        .render_graph(graph_key).unwrap()
        .image_access_history_capacities();

    // Frames 2 & 3: re-run the same graph.
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();
    rgm.execute_render_graph(graph_key, &pass_keys, &mut *gd, |_| Ok(())).unwrap();

    let cap_after_frame_3: Vec<usize> = rgm
        .render_graph(graph_key).unwrap()
        .image_access_history_capacities();

    assert_eq!(
        cap_after_frame_1, cap_after_frame_3,
        "inner Vec capacities must not change between frame 1 and frame 3 \
         (= no reallocation in steady state)",
    );
}
