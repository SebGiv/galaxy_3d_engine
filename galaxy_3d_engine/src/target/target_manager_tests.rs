/// Tests for TargetManager
///
/// These tests validate render target creation, retrieval, removal,
/// naming, and lifecycle management.

use super::*;

// ============================================================================
// Tests: TargetManager Creation
// ============================================================================

#[test]
fn test_target_manager_new() {
    let tm = TargetManager::new();
    assert_eq!(tm.render_target_count(), 0);
}

// ============================================================================
// Tests: Create RenderTarget
// ============================================================================

#[test]
fn test_create_render_target() {
    let mut tm = TargetManager::new();
    let result = tm.create_render_target("screen");
    assert!(result.is_ok());
    assert_eq!(tm.render_target_count(), 1);
}

#[test]
fn test_create_multiple_render_targets() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.create_render_target("shadow_map").unwrap();
    tm.create_render_target("post_process").unwrap();

    assert_eq!(tm.render_target_count(), 3);
}

#[test]
fn test_create_render_target_duplicate_name_fails() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();

    let result = tm.create_render_target("screen");
    assert!(result.is_err());
    assert_eq!(tm.render_target_count(), 1);
}

// ============================================================================
// Tests: Get RenderTarget
// ============================================================================

#[test]
fn test_render_target_found() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();

    assert!(tm.render_target("screen").is_some());
}

#[test]
fn test_render_target_not_found() {
    let tm = TargetManager::new();
    assert!(tm.render_target("nonexistent").is_none());
}

#[test]
fn test_render_target_mut_found() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();

    assert!(tm.render_target_mut("screen").is_some());
}

#[test]
fn test_render_target_mut_not_found() {
    let mut tm = TargetManager::new();
    assert!(tm.render_target_mut("nonexistent").is_none());
}

// ============================================================================
// Tests: Remove RenderTarget
// ============================================================================

#[test]
fn test_remove_render_target() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();

    let removed = tm.remove_render_target("screen");
    assert!(removed.is_some());
    assert_eq!(tm.render_target_count(), 0);
}

#[test]
fn test_remove_render_target_not_found() {
    let mut tm = TargetManager::new();
    let removed = tm.remove_render_target("nonexistent");
    assert!(removed.is_none());
}

#[test]
fn test_remove_render_target_then_get_returns_none() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.remove_render_target("screen");

    assert!(tm.render_target("screen").is_none());
}

#[test]
fn test_remove_render_target_does_not_affect_others() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.create_render_target("shadow_map").unwrap();

    tm.remove_render_target("screen");

    assert!(tm.render_target("screen").is_none());
    assert!(tm.render_target("shadow_map").is_some());
    assert_eq!(tm.render_target_count(), 1);
}

#[test]
fn test_remove_and_recreate_render_target() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.remove_render_target("screen");

    let result = tm.create_render_target("screen");
    assert!(result.is_ok());
    assert_eq!(tm.render_target_count(), 1);
}

// ============================================================================
// Tests: RenderTarget Names
// ============================================================================

#[test]
fn test_render_target_names_empty() {
    let tm = TargetManager::new();
    assert!(tm.render_target_names().is_empty());
}

#[test]
fn test_render_target_names_multiple() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.create_render_target("shadow_map").unwrap();
    tm.create_render_target("post_process").unwrap();

    let names = tm.render_target_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"screen"));
    assert!(names.contains(&"shadow_map"));
    assert!(names.contains(&"post_process"));
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.create_render_target("shadow_map").unwrap();

    tm.clear();

    assert_eq!(tm.render_target_count(), 0);
    assert!(tm.render_target("screen").is_none());
    assert!(tm.render_target("shadow_map").is_none());
}

#[test]
fn test_clear_then_create() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();
    tm.clear();

    let result = tm.create_render_target("screen");
    assert!(result.is_ok());
    assert_eq!(tm.render_target_count(), 1);
}

// ============================================================================
// Tests: RenderTarget Count
// ============================================================================

#[test]
fn test_render_target_count_tracks_correctly() {
    let mut tm = TargetManager::new();
    assert_eq!(tm.render_target_count(), 0);

    tm.create_render_target("a").unwrap();
    assert_eq!(tm.render_target_count(), 1);

    tm.create_render_target("b").unwrap();
    assert_eq!(tm.render_target_count(), 2);

    tm.remove_render_target("a");
    assert_eq!(tm.render_target_count(), 1);

    tm.remove_render_target("b");
    assert_eq!(tm.render_target_count(), 0);
}

// ============================================================================
// Tests: Error Messages
// ============================================================================

#[test]
fn test_create_render_target_duplicate_error_message() {
    let mut tm = TargetManager::new();
    tm.create_render_target("screen").unwrap();

    let result = tm.create_render_target("screen");
    match result {
        Err(crate::error::Error::BackendError(msg)) => {
            assert!(msg.contains("already exists"));
        }
        _ => panic!("Expected BackendError with 'already exists'"),
    }
}
