/// Tests for RenderGraphManager
///
/// These tests validate render graph creation, retrieval, removal,
/// naming, and lifecycle management.

use super::*;

// ============================================================================
// Tests: RenderGraphManager Creation
// ============================================================================

#[test]
fn test_render_graph_manager_new() {
    let rgm = RenderGraphManager::new();
    assert_eq!(rgm.render_graph_count(), 0);
}

// ============================================================================
// Tests: Create RenderGraph
// ============================================================================

#[test]
fn test_create_render_graph() {
    let mut rgm = RenderGraphManager::new();
    let result = rgm.create_render_graph("main");
    assert!(result.is_ok());
    assert_eq!(rgm.render_graph_count(), 1);
}

#[test]
fn test_create_multiple_render_graphs() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.create_render_graph("shadow").unwrap();
    rgm.create_render_graph("post_process").unwrap();

    assert_eq!(rgm.render_graph_count(), 3);
}

#[test]
fn test_create_render_graph_duplicate_name_fails() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();

    let result = rgm.create_render_graph("main");
    assert!(result.is_err());
    assert_eq!(rgm.render_graph_count(), 1);
}

// ============================================================================
// Tests: Get RenderGraph
// ============================================================================

#[test]
fn test_render_graph_found() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();

    assert!(rgm.render_graph("main").is_some());
}

#[test]
fn test_render_graph_not_found() {
    let rgm = RenderGraphManager::new();
    assert!(rgm.render_graph("nonexistent").is_none());
}

#[test]
fn test_render_graph_mut_found() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();

    assert!(rgm.render_graph_mut("main").is_some());
}

#[test]
fn test_render_graph_mut_not_found() {
    let mut rgm = RenderGraphManager::new();
    assert!(rgm.render_graph_mut("nonexistent").is_none());
}

// ============================================================================
// Tests: Remove RenderGraph
// ============================================================================

#[test]
fn test_remove_render_graph() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();

    let removed = rgm.remove_render_graph("main");
    assert!(removed.is_some());
    assert_eq!(rgm.render_graph_count(), 0);
}

#[test]
fn test_remove_render_graph_not_found() {
    let mut rgm = RenderGraphManager::new();
    let removed = rgm.remove_render_graph("nonexistent");
    assert!(removed.is_none());
}

#[test]
fn test_remove_render_graph_then_get_returns_none() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.remove_render_graph("main");

    assert!(rgm.render_graph("main").is_none());
}

#[test]
fn test_remove_render_graph_does_not_affect_others() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.create_render_graph("shadow").unwrap();

    rgm.remove_render_graph("main");

    assert!(rgm.render_graph("main").is_none());
    assert!(rgm.render_graph("shadow").is_some());
    assert_eq!(rgm.render_graph_count(), 1);
}

#[test]
fn test_remove_and_recreate_render_graph() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.remove_render_graph("main");

    let result = rgm.create_render_graph("main");
    assert!(result.is_ok());
    assert_eq!(rgm.render_graph_count(), 1);
}

// ============================================================================
// Tests: RenderGraph Names
// ============================================================================

#[test]
fn test_render_graph_names_empty() {
    let rgm = RenderGraphManager::new();
    assert!(rgm.render_graph_names().is_empty());
}

#[test]
fn test_render_graph_names_multiple() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.create_render_graph("shadow").unwrap();
    rgm.create_render_graph("post_process").unwrap();

    let names = rgm.render_graph_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"main"));
    assert!(names.contains(&"shadow"));
    assert!(names.contains(&"post_process"));
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.create_render_graph("shadow").unwrap();

    rgm.clear();

    assert_eq!(rgm.render_graph_count(), 0);
    assert!(rgm.render_graph("main").is_none());
    assert!(rgm.render_graph("shadow").is_none());
}

#[test]
fn test_clear_then_create() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();
    rgm.clear();

    let result = rgm.create_render_graph("main");
    assert!(result.is_ok());
    assert_eq!(rgm.render_graph_count(), 1);
}

// ============================================================================
// Tests: RenderGraph Count
// ============================================================================

#[test]
fn test_render_graph_count_tracks_correctly() {
    let mut rgm = RenderGraphManager::new();
    assert_eq!(rgm.render_graph_count(), 0);

    rgm.create_render_graph("a").unwrap();
    assert_eq!(rgm.render_graph_count(), 1);

    rgm.create_render_graph("b").unwrap();
    assert_eq!(rgm.render_graph_count(), 2);

    rgm.remove_render_graph("a");
    assert_eq!(rgm.render_graph_count(), 1);

    rgm.remove_render_graph("b");
    assert_eq!(rgm.render_graph_count(), 0);
}

// ============================================================================
// Tests: Error Messages
// ============================================================================

#[test]
fn test_create_render_graph_duplicate_error_message() {
    let mut rgm = RenderGraphManager::new();
    rgm.create_render_graph("main").unwrap();

    let result = rgm.create_render_graph("main");
    match result {
        Err(crate::error::Error::BackendError(msg)) => {
            assert!(msg.contains("already exists"));
        }
        _ => panic!("Expected BackendError with 'already exists'"),
    }
}
