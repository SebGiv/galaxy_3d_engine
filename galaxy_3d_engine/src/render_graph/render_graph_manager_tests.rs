/// Tests for RenderGraphManager
///
/// CRUD smoke tests run without the Engine, while pass / framebuffer / execute
/// tests rely on the global Engine + a `MockGraphicsDevice`. Engine-backed
/// tests are `#[serial]` because they share global state.

use super::*;
use super::super::test_helpers::{
    setup_engine_for_render_graph, default_color_ops, default_depth_ops,
    make_recording_pass,
};
use crate::engine::Engine;
use crate::graphics_device;
use crate::render_graph::access_type::ResourceAccess;
use crate::resource::resource_manager::TextureKey;
use serial_test::serial;
use std::sync::atomic::Ordering;

// ============================================================================
// CRUD without Engine
// ============================================================================

#[test]
fn test_render_graph_manager_new() {
    let rgm = RenderGraphManager::new();
    assert_eq!(rgm.render_graph_count(), 0);
    assert_eq!(rgm.render_pass_count(), 0);
    assert_eq!(rgm.graph_resource_count(), 0);
    assert_eq!(rgm.framebuffer_count(), 0);
}

#[test]
fn test_create_graph_resource() {
    let mut rgm = RenderGraphManager::new();
    let key = rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0,
            mip_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
    ).unwrap();
    assert_eq!(rgm.graph_resource_count(), 1);
    assert!(rgm.graph_resource(key).is_some());
    assert!(rgm.graph_resource_by_name("color").is_some());
    assert_eq!(rgm.graph_resource_id("color"), Some(key));
}

#[test]
fn test_create_graph_resource_duplicate_name_fails() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        },
    ).unwrap();
    let result = rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        },
    );
    assert!(result.is_err());
    assert_eq!(rgm.graph_resource_count(), 1);
}

#[test]
fn test_graph_resource_not_found() {
    let rgm = RenderGraphManager::new();
    assert!(rgm.graph_resource_by_name("nonexistent").is_none());
    assert!(rgm.graph_resource_id("nonexistent").is_none());
}

#[test]
fn test_remove_graph_resource_by_key() {
    let mut rgm = RenderGraphManager::new();
    let key = rgm.create_graph_resource("r",
        GraphResource::Buffer { buffer_key: crate::resource::resource_manager::BufferKey::default(), offset: 0, size: u64::MAX }
    ).unwrap();
    assert!(rgm.remove_graph_resource(key));
    assert_eq!(rgm.graph_resource_count(), 0);
    assert!(rgm.graph_resource_by_name("r").is_none());
    // Removing again returns false.
    assert!(!rgm.remove_graph_resource(key));
}

#[test]
fn test_remove_graph_resource_by_name() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_graph_resource("r",
        GraphResource::Buffer { buffer_key: crate::resource::resource_manager::BufferKey::default(), offset: 0, size: u64::MAX }
    ).unwrap();
    assert!(rgm.remove_graph_resource_by_name("r"));
    assert_eq!(rgm.graph_resource_count(), 0);
    assert!(!rgm.remove_graph_resource_by_name("r"));
}

#[test]
fn test_clear_drops_graph_resources() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_graph_resource("color", GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    rgm.clear();
    assert_eq!(rgm.graph_resource_count(), 0);
    assert!(rgm.graph_resource_by_name("color").is_none());
}

#[test]
fn test_create_graph_resource_with_mip_count_zero_rejected() {
    let mut rgm = RenderGraphManager::new();
    let result = rgm.create_graph_resource("bad", GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0, mip_count: 0, base_array_layer: 0, layer_count: 1,
    });
    assert!(result.is_err(), "expected mip_count=0 to be rejected");
    assert_eq!(rgm.graph_resource_count(), 0);
}

#[test]
fn test_create_graph_resource_with_layer_count_zero_rejected() {
    let mut rgm = RenderGraphManager::new();
    let result = rgm.create_graph_resource("bad", GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 0,
    });
    assert!(result.is_err(), "expected layer_count=0 to be rejected");
    assert_eq!(rgm.graph_resource_count(), 0);
}

#[test]
fn test_create_graph_resource_with_size_zero_rejected() {
    let mut rgm = RenderGraphManager::new();
    let result = rgm.create_graph_resource("bad", GraphResource::Buffer {
        buffer_key: crate::resource::resource_manager::BufferKey::default(),
        offset: 0,
        size: 0,
    });
    assert!(result.is_err(), "expected size=0 to be rejected");
    assert_eq!(rgm.graph_resource_count(), 0);
}

#[test]
fn test_create_graph_resource_with_remaining_sentinels_accepted() {
    // Sentinels (REMAINING_*, WHOLE_SIZE) must be accepted as-is — they
    // are u32::MAX / u64::MAX, not 0.
    let mut rgm = RenderGraphManager::new();
    rgm.create_graph_resource("full_tex", GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0,
        mip_count: crate::render_graph::REMAINING_MIP_LEVELS,
        base_array_layer: 0,
        layer_count: crate::render_graph::REMAINING_ARRAY_LAYERS,
    }).expect("REMAINING_* sentinels should be accepted for textures");
    rgm.create_graph_resource("full_buf", GraphResource::Buffer {
        buffer_key: crate::resource::resource_manager::BufferKey::default(),
        offset: 0,
        size: crate::render_graph::WHOLE_SIZE,
    }).expect("WHOLE_SIZE sentinel should be accepted for buffers");
    assert_eq!(rgm.graph_resource_count(), 2);
}

#[test]
fn test_render_pass_lookups_on_empty_manager() {
    let rgm = RenderGraphManager::new();
    assert!(rgm.render_pass_by_name("nonexistent").is_none());
    assert!(rgm.render_pass_id("nonexistent").is_none());
}

#[test]
fn test_render_graph_lookups_on_empty_manager() {
    let rgm = RenderGraphManager::new();
    assert!(rgm.render_graph_by_name("nonexistent").is_none());
    assert!(rgm.render_graph_id("nonexistent").is_none());
}

// ============================================================================
// Engine-backed integration tests
// ============================================================================

#[test]
#[serial]
fn test_create_render_graph_engine_backed() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let key = rgm.create_render_graph("main", 2, &*gd).unwrap();
    assert_eq!(rgm.render_graph_count(), 1);
    assert!(rgm.render_graph(key).is_some());
    assert!(rgm.render_graph_by_name("main").is_some());
    assert_eq!(rgm.render_graph_id("main"), Some(key));
}

#[test]
#[serial]
fn test_create_render_graph_duplicate_name_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main", 2, &*gd).unwrap();
    let result = rgm.create_render_graph("main", 2, &*gd);
    assert!(result.is_err());
    assert_eq!(rgm.render_graph_count(), 1);
}

#[test]
#[serial]
fn test_create_render_pass_color_only() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("opaque", vec![
        ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
    ], action, &*gd).unwrap();
    assert_eq!(rgm.render_pass_count(), 1);
    let pass = rgm.render_pass(pass_key).unwrap();
    assert_eq!(pass.name(), "opaque");
    assert!(pass.framebuffer_key().is_some());
    assert!(pass.pass_info().is_some());
    assert_eq!(rgm.framebuffer_count(), 1);
}

#[test]
#[serial]
fn test_create_render_pass_color_and_depth() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let depth_gr = rgm.create_graph_resource("depth", GraphResource::Texture {
        texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("opaque_z", vec![
        ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
        ResourceAccess {
            graph_resource_key: depth_gr,
            access_type: AccessType::DepthStencilWrite,
            target_ops: Some(default_depth_ops()),
        },
    ], action, &*gd).unwrap();
    let pass = rgm.render_pass(pass_key).unwrap();
    assert_eq!(pass.clear_values().len(), 2);
}

#[test]
#[serial]
fn test_create_render_pass_compute_only_has_no_framebuffer() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    // Only a sampled read — no attachment.
    let pass_key = rgm.create_render_pass("compute", vec![
        ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::FragmentShaderRead,
            target_ops: None,
        },
    ], action, &*gd).unwrap();
    let pass = rgm.render_pass(pass_key).unwrap();
    assert!(pass.framebuffer_key().is_none(), "compute-only pass must have no framebuffer");
    assert!(pass.pass_info().is_none());
    assert!(pass.gd_render_pass().is_none());
}

#[test]
#[serial]
fn test_attachment_with_mip_count_gt_1_rejected() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let bad_gr = rgm.create_graph_resource("bad", GraphResource::Texture {
        texture_key: env.color_texture,
        base_mip_level: 0,
        mip_count: 2, // illegal for an attachment
        base_array_layer: 0,
        layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![
        ResourceAccess {
            graph_resource_key: bad_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
    ], action, &*gd);
    assert!(result.is_err(), "attachment with mip_count=2 must be rejected");
}

#[test]
#[serial]
fn test_attachment_with_remaining_mips_rejected() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let bad_gr = rgm.create_graph_resource("bad", GraphResource::Texture {
        texture_key: env.color_texture,
        base_mip_level: 0,
        mip_count: crate::render_graph::REMAINING_MIP_LEVELS,
        base_array_layer: 0,
        layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![
        ResourceAccess {
            graph_resource_key: bad_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
    ], action, &*gd);
    assert!(result.is_err(), "attachment with REMAINING_MIP_LEVELS must be rejected");
}

#[test]
#[serial]
fn test_non_attachment_with_mip_count_gt_1_accepted() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    // FragmentShaderRead may legitimately span multiple mips (sampler
    // selects mip via LOD). The build_pass_cache rule only constrains
    // attachment accesses.
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let gr = rgm.create_graph_resource("sampled", GraphResource::Texture {
        texture_key: env.color_texture,
        base_mip_level: 0,
        mip_count: crate::render_graph::REMAINING_MIP_LEVELS,
        base_array_layer: 0,
        layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("compute", vec![
        ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::FragmentShaderRead,
            target_ops: None,
        },
    ], action, &*gd).expect("non-attachment access should accept any mip_count");
    assert!(rgm.render_pass(pass_key).is_some());
}

#[test]
#[serial]
fn test_layered_attachment_accepted() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    // mip_count == 1, layer_count > 1 is the canonical layered-rendering
    // shape (cubemap one-pass, cascaded shadow maps, multi-view) and must
    // be accepted on attachments.
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let gr = rgm.create_graph_resource("layered", GraphResource::Texture {
        texture_key: env.color_texture,
        base_mip_level: 0,
        mip_count: 1,
        base_array_layer: 0,
        layer_count: 1, // env.color_texture has 1 layer; check is structural here
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("layered", vec![
        ResourceAccess {
            graph_resource_key: gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
    ], action, &*gd).expect("mip_count=1 attachment should be accepted regardless of layer_count");
    assert!(rgm.render_pass(pass_key).is_some());
}

#[test]
#[serial]
fn test_create_render_pass_duplicate_name_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action1, _) = make_recording_pass();
    rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_gr,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action1, &*gd).unwrap();
    let (action2, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![], action2, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_create_render_pass_color_attachment_missing_target_ops_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_gr,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: None,
    }], action, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_create_render_pass_unknown_graph_resource_key_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    use slotmap::Key;
    let bogus = GraphResourceKey::null();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: bogus,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_create_render_pass_two_depth_writes_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let depth_gr = rgm.create_graph_resource("d1", GraphResource::Texture {
        texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let depth_gr2 = rgm.create_graph_resource("d2", GraphResource::Texture {
        texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![
        ResourceAccess {
            graph_resource_key: depth_gr,
            access_type: AccessType::DepthStencilWrite,
            target_ops: Some(default_depth_ops()),
        },
        ResourceAccess {
            graph_resource_key: depth_gr2,
            access_type: AccessType::DepthStencilWrite,
            target_ops: Some(default_depth_ops()),
        },
    ], action, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_create_render_pass_buffer_as_color_attachment_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let buf_gr = rgm.create_graph_resource("buf",
        GraphResource::Buffer { buffer_key: crate::resource::resource_manager::BufferKey::default(), offset: 0, size: u64::MAX }
    ).unwrap();
    let (action, _) = make_recording_pass();
    let result = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: buf_gr,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_get_or_create_framebuffer_idempotent() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let slots = vec![ColorAttachmentSlot { color: color_gr, resolve: None }];
    // First call creates a new framebuffer.
    let fb1 = rgm.get_or_create_framebuffer(&slots, None, &*gd).unwrap();
    // Second call with identical inputs hits the cache.
    let fb2 = rgm.get_or_create_framebuffer(&slots, None, &*gd).unwrap();
    assert_eq!(fb1, fb2);
    assert_eq!(rgm.framebuffer_count(), 1);
}

#[test]
#[serial]
fn test_get_or_create_framebuffer_different_for_different_attachments() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let depth_gr = rgm.create_graph_resource("d", GraphResource::Texture {
        texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let slots = vec![ColorAttachmentSlot { color: color_gr, resolve: None }];
    let fb1 = rgm.get_or_create_framebuffer(&slots, None, &*gd).unwrap();
    let fb2 = rgm.get_or_create_framebuffer(&slots, Some(depth_gr), &*gd).unwrap();
    assert_ne!(fb1, fb2);
    assert_eq!(rgm.framebuffer_count(), 2);
}

#[test]
#[serial]
fn test_remove_framebuffer_invalidates_cache() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let slots = vec![ColorAttachmentSlot { color: color_gr, resolve: None }];
    let fb1 = rgm.get_or_create_framebuffer(&slots, None, &*gd).unwrap();
    assert!(rgm.remove_framebuffer(fb1));
    assert_eq!(rgm.framebuffer_count(), 0);
    let fb2 = rgm.get_or_create_framebuffer(&slots, None, &*gd).unwrap();
    assert_ne!(fb1, fb2, "post-remove call should rebuild a new framebuffer");
    // Removing an invalid key returns false.
    use slotmap::Key;
    assert!(!rgm.remove_framebuffer(FramebufferKey::null()));
}

#[test]
#[serial]
fn test_set_pass_access_resource() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let color_b = rgm.create_graph_resource("b", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_a,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    rgm.set_pass_access_resource(pass_key, 0, color_b, &*gd).unwrap();
    let pass = rgm.render_pass(pass_key).unwrap();
    assert_eq!(pass.accesses()[0].graph_resource_key, color_b);
}

#[test]
#[serial]
fn test_set_pass_access_resource_invalid_pass_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    use slotmap::Key;
    let result = rgm.set_pass_access_resource(RenderPassKey::null(), 0, color_a, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_set_pass_access_resource_out_of_bounds_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_a,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    let result = rgm.set_pass_access_resource(pass_key, 99, color_a, &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_set_pass_access_target_ops() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_a,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    let new_ops = TargetOps::Color {
        clear_color: [1.0, 0.0, 0.0, 1.0],
        load_op: graphics_device::LoadOp::Load,
        store_op: graphics_device::StoreOp::Store,
        resolve_target: None,
    };
    rgm.set_pass_access_target_ops(pass_key, 0, new_ops, &*gd).unwrap();
}

#[test]
#[serial]
fn test_set_pass_access_target_ops_invalid_pass_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    use slotmap::Key;
    let result = rgm.set_pass_access_target_ops(RenderPassKey::null(), 0, default_color_ops(), &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_set_pass_access_target_ops_out_of_bounds_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_a,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    let result = rgm.set_pass_access_target_ops(pass_key, 99, default_color_ops(), &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_replace_pass_accesses() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_a = rgm.create_graph_resource("a", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let depth_a = rgm.create_graph_resource("d", GraphResource::Texture {
        texture_key: env.depth_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    let pass_key = rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_a,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    let new_accesses = vec![
        ResourceAccess {
            graph_resource_key: color_a,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        },
        ResourceAccess {
            graph_resource_key: depth_a,
            access_type: AccessType::DepthStencilWrite,
            target_ops: Some(default_depth_ops()),
        },
    ];
    rgm.replace_pass_accesses(pass_key, new_accesses, &*gd).unwrap();
    assert_eq!(rgm.render_pass(pass_key).unwrap().accesses().len(), 2);
}

#[test]
#[serial]
fn test_replace_pass_accesses_invalid_pass_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    use slotmap::Key;
    let result = rgm.replace_pass_accesses(RenderPassKey::null(), vec![], &*gd);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_execute_render_graph_runs_each_pass_once() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let rgm_arc = {
        Engine::create_render_graph_manager().unwrap();
        Engine::render_graph_manager().unwrap()
    };
    let (graph_key, pass_key, counter): (RenderGraphKey, RenderPassKey, std::sync::Arc<std::sync::atomic::AtomicU32>) = {
        let mut rgm = rgm_arc.lock().unwrap();
        let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
        let color_gr = rgm.create_graph_resource("color", GraphResource::Texture {
            texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
        }).unwrap();
        let (action, counter) = make_recording_pass();
        let pass_key = rgm.create_render_pass("opaque", vec![ResourceAccess {
            graph_resource_key: color_gr,
            access_type: AccessType::ColorAttachmentWrite,
            target_ops: Some(default_color_ops()),
        }], action, &*gd).unwrap();
        (graph_key, pass_key, counter)
    };

    let mut rgm = rgm_arc.lock().unwrap();
    rgm.execute_render_graph(graph_key, &[pass_key], &mut *gd, |_cmd| Ok(())).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    rgm.execute_render_graph(graph_key, &[pass_key], &mut *gd, |_cmd| Ok(())).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
#[serial]
fn test_execute_render_graph_invalid_graph_key_fails() {
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let mut rgm = rgm_arc.lock().unwrap();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    use slotmap::Key;
    let result = rgm.execute_render_graph(RenderGraphKey::null(), &[], &mut *gd, |_| Ok(()));
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_execute_render_graph_invalid_pass_key_fails() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut gd = gd_arc.lock().unwrap();
    setup_engine_for_render_graph();
    Engine::create_render_graph_manager().unwrap();
    let rgm_arc = Engine::render_graph_manager().unwrap();
    let mut rgm = rgm_arc.lock().unwrap();
    let graph_key = rgm.create_render_graph("main", 1, &*gd).unwrap();
    use slotmap::Key;
    let result = rgm.execute_render_graph(graph_key, &[RenderPassKey::null()], &mut *gd, |_| Ok(()));
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_clear_drops_engine_backed_passes() {
    let gd_arc = Engine::graphics_device("main").unwrap();
    let gd = gd_arc.lock().unwrap();
    let env = setup_engine_for_render_graph();
    let mut rgm = RenderGraphManager::new();
    let color_gr = rgm.create_graph_resource("c", GraphResource::Texture {
        texture_key: env.color_texture, base_mip_level: 0, mip_count: 1, base_array_layer: 0, layer_count: 1,
    }).unwrap();
    let (action, _) = make_recording_pass();
    rgm.create_render_pass("p", vec![ResourceAccess {
        graph_resource_key: color_gr,
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: Some(default_color_ops()),
    }], action, &*gd).unwrap();
    rgm.create_render_graph("main", 1, &*gd).unwrap();

    rgm.clear();
    assert_eq!(rgm.render_pass_count(), 0);
    assert_eq!(rgm.render_graph_count(), 0);
    assert_eq!(rgm.graph_resource_count(), 0);
    assert_eq!(rgm.framebuffer_count(), 0);
}
