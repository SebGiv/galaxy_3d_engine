/// Tests for RenderGraphManager
///
/// Smoke tests that exercise the (key, name) CRUD pattern without
/// touching the GPU. Execution-level tests live next to the
/// `RenderGraph::execute()` integration tests once a mock GraphicsDevice
/// is wired up.

use super::*;
use crate::resource::resource_manager::TextureKey;

// ===== RenderGraphManager creation =====

#[test]
fn test_render_graph_manager_new() {
    let rgm = RenderGraphManager::new();
    assert_eq!(rgm.render_graph_count(), 0);
    assert_eq!(rgm.render_pass_count(), 0);
    assert_eq!(rgm.graph_resource_count(), 0);
}

// ===== GraphResource =====

#[test]
fn test_create_graph_resource() {
    let mut rgm = RenderGraphManager::new();
    let key = rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0,
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
            base_mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
    ).unwrap();
    let result = rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
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
fn test_clear() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_graph_resource(
        "color",
        GraphResource::Texture {
            texture_key: TextureKey::default(),
            base_mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
    ).unwrap();

    rgm.clear();
    assert_eq!(rgm.graph_resource_count(), 0);
    assert!(rgm.graph_resource_by_name("color").is_none());
}
