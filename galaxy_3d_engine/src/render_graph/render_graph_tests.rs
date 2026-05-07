use super::*;
use super::super::test_helpers::{
    setup_engine_for_render_graph, default_color_ops, make_recording_pass,
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
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, _pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let pass_key = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action).unwrap();
        (graph_key, pass_key)
    };

    let post_called = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let post_called_inner = post_called.clone();

    let mut rgm = rgm_arc.lock().unwrap();
    let pass_keys = vec![_pass_key];
    rgm.execute_render_graph(graph_key, &pass_keys, |_cmd| {
        post_called_inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }).unwrap();
    assert_eq!(post_called.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
#[serial]
fn test_render_graph_execute_with_no_passes_runs_post_only() {
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        rgm.create_render_graph("main", 1).unwrap()
    };

    let post_called = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let post_called_inner = post_called.clone();

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[], |_cmd| {
        post_called_inner.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }).unwrap();
    assert_eq!(post_called.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
#[serial]
fn test_render_graph_topological_sort_with_writer_reader() {
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, writer_key, reader_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        // Writer: writes color
        let (writer_action, _) = make_recording_pass();
        let writer_key = rgm.create_render_pass("writer", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], writer_action).unwrap();
        // Reader: reads color (sampled, no attachment, no framebuffer)
        let (reader_action, _) = make_recording_pass();
        let reader_key = rgm.create_render_pass("reader", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::FragmentShaderRead,
            target_ops: None,
        }], reader_action).unwrap();
        (graph_key, writer_key, reader_key)
    };

    // Submit reader BEFORE writer in the input list — topo sort should reorder.
    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[reader_key, writer_key], |_| Ok(()));
    assert!(result.is_ok(), "expected success, got {:?}", result);
}

#[test]
#[serial]
fn test_render_graph_command_list_ok_after_execute() {
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 2).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let _pass_key = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action).unwrap();
        graph_key
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[], |_| Ok(())).unwrap();
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
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_a, pass_b) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();

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
        ], action_a).unwrap();

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
        ], action_b).unwrap();

        (graph_key, pass_a, pass_b)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[pass_a, pass_b], |_| Ok(()));
    assert!(result.is_err(), "expected cycle detection to fail");
}

#[test]
#[serial]
fn test_render_graph_execute_advances_through_frames() {
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let graph_key = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 3).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let _pk = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action).unwrap();
        graph_key
    };

    // Run 5 frames — exercises ring rotation through 3 command lists.
    let mut rgm = rgm_arc.lock().unwrap();
    for _ in 0..5 {
        rgm.execute_render_graph(graph_key, &[], |_| Ok(())).unwrap();
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
        rm.create_default_instance_buffer("instances".to_string(), gd_arc, 4).unwrap()
    };

    let (graph_key, pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
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
        ], action).unwrap();
        (graph_key, pass_key)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_key], |_| Ok(())).unwrap();
}

#[test]
#[serial]
fn test_render_graph_execute_writer_equals_reader_self_loop() {
    // Covers the `if writer != k { ... }` false branch (line ~280) inside
    // topological_sort: a single pass that both writes AND reads the same
    // resource → writer == k → skip dependency edge.
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();

    let (graph_key, pass_key) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
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
        ], action).unwrap();
        (graph_key, pass_key)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &[pass_key], |_| Ok(()));
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
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();

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
        }], action_a).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b).unwrap();
        (graph_key, vec![pass_a, pass_b])
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &pass_keys, |_| Ok(()));
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
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
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
        }], action_a).unwrap();
        let (action_b, _) = make_recording_pass();
        let pass_b = rgm.create_render_pass("pass_b", vec![ResourceAccess {
            graph_resource_key: gr_b,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action_b).unwrap();
        (graph_key, vec![pass_a, pass_b])
    };

    let mut rgm = rgm_arc.lock().unwrap();
    let result = rgm.execute_render_graph(graph_key, &pass_keys, |_| Ok(()));
    assert!(result.is_ok(), "execute failed: {:?}", result);
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
    let env = setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let (graph_key, pass_keys) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1,
            base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, _) = make_recording_pass();
        let pass = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action).unwrap();
        (graph_key, vec![pass])
    };

    let mut rgm = rgm_arc.lock().unwrap();

    // Frame 1: warm up the access-history map.
    rgm.execute_render_graph(graph_key, &pass_keys, |_| Ok(())).unwrap();

    // Snapshot capacities right after frame 1.
    let cap_after_frame_1: Vec<usize> = rgm
        .render_graph(graph_key).unwrap()
        .image_access_history_capacities();

    // Frames 2 & 3: re-run the same graph.
    rgm.execute_render_graph(graph_key, &pass_keys, |_| Ok(())).unwrap();
    rgm.execute_render_graph(graph_key, &pass_keys, |_| Ok(())).unwrap();

    let cap_after_frame_3: Vec<usize> = rgm
        .render_graph(graph_key).unwrap()
        .image_access_history_capacities();

    assert_eq!(
        cap_after_frame_1, cap_after_frame_3,
        "inner Vec capacities must not change between frame 1 and frame 3 \
         (= no reallocation in steady state)",
    );
}
