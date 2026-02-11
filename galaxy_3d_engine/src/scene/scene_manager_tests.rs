/// Tests for SceneManager
///
/// These tests validate scene creation, retrieval, removal, naming,
/// and lifecycle management.

use super::*;
use std::sync::Arc;

// ============================================================================
// Tests: SceneManager Creation
// ============================================================================

#[test]
fn test_scene_manager_new() {
    let sm = SceneManager::new();
    assert_eq!(sm.scene_count(), 0);
}

// ============================================================================
// Tests: Create Scene
// ============================================================================

#[test]
fn test_create_scene() {
    let mut sm = SceneManager::new();
    let scene = sm.create_scene("main");
    assert!(scene.is_ok());
    assert_eq!(sm.scene_count(), 1);
}

#[test]
fn test_create_scene_returns_arc() {
    let mut sm = SceneManager::new();
    let scene_arc = sm.create_scene("main").unwrap();

    // The returned Arc should be usable
    let scene = scene_arc.lock().unwrap();
    assert_eq!(scene.render_instance_count(), 0);
}

#[test]
fn test_create_scene_same_as_stored() {
    let mut sm = SceneManager::new();
    let created = sm.create_scene("main").unwrap();
    let retrieved = sm.scene("main").unwrap();

    // Both should point to the same scene
    assert!(Arc::ptr_eq(&created, &retrieved));
}

#[test]
fn test_create_multiple_scenes() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.create_scene("ui").unwrap();
    sm.create_scene("minimap").unwrap();

    assert_eq!(sm.scene_count(), 3);
}

#[test]
fn test_create_scene_duplicate_name_fails() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();

    let result = sm.create_scene("main");
    assert!(result.is_err());
    assert_eq!(sm.scene_count(), 1);
}

// ============================================================================
// Tests: Get Scene
// ============================================================================

#[test]
fn test_scene_found() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();

    assert!(sm.scene("main").is_some());
}

#[test]
fn test_scene_not_found() {
    let sm = SceneManager::new();
    assert!(sm.scene("nonexistent").is_none());
}

#[test]
fn test_scene_distinct_names() {
    let mut sm = SceneManager::new();
    sm.create_scene("scene_a").unwrap();
    sm.create_scene("scene_b").unwrap();

    let a = sm.scene("scene_a").unwrap();
    let b = sm.scene("scene_b").unwrap();

    // Different scenes
    assert!(!Arc::ptr_eq(&a, &b));
}

// ============================================================================
// Tests: Remove Scene
// ============================================================================

#[test]
fn test_remove_scene() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();

    let removed = sm.remove_scene("main");
    assert!(removed.is_some());
    assert_eq!(sm.scene_count(), 0);
}

#[test]
fn test_remove_scene_not_found() {
    let mut sm = SceneManager::new();
    let removed = sm.remove_scene("nonexistent");
    assert!(removed.is_none());
}

#[test]
fn test_remove_scene_then_get_returns_none() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.remove_scene("main");

    assert!(sm.scene("main").is_none());
}

#[test]
fn test_remove_scene_does_not_affect_others() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.create_scene("ui").unwrap();

    sm.remove_scene("main");

    assert!(sm.scene("main").is_none());
    assert!(sm.scene("ui").is_some());
    assert_eq!(sm.scene_count(), 1);
}

#[test]
fn test_remove_and_recreate_scene() {
    let mut sm = SceneManager::new();
    let first = sm.create_scene("main").unwrap();
    sm.remove_scene("main");

    let second = sm.create_scene("main").unwrap();

    // Different Arc (new scene instance)
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(sm.scene_count(), 1);
}

// ============================================================================
// Tests: Scene Names
// ============================================================================

#[test]
fn test_scene_names_empty() {
    let sm = SceneManager::new();
    assert!(sm.scene_names().is_empty());
}

#[test]
fn test_scene_names_multiple() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.create_scene("ui").unwrap();
    sm.create_scene("minimap").unwrap();

    let names = sm.scene_names();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"main"));
    assert!(names.contains(&"ui"));
    assert!(names.contains(&"minimap"));
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.create_scene("ui").unwrap();

    sm.clear();

    assert_eq!(sm.scene_count(), 0);
    assert!(sm.scene("main").is_none());
    assert!(sm.scene("ui").is_none());
}

#[test]
fn test_clear_then_create() {
    let mut sm = SceneManager::new();
    sm.create_scene("main").unwrap();
    sm.clear();

    let result = sm.create_scene("main");
    assert!(result.is_ok());
    assert_eq!(sm.scene_count(), 1);
}

// ============================================================================
// Tests: Scene Count
// ============================================================================

#[test]
fn test_scene_count_tracks_correctly() {
    let mut sm = SceneManager::new();
    assert_eq!(sm.scene_count(), 0);

    sm.create_scene("a").unwrap();
    assert_eq!(sm.scene_count(), 1);

    sm.create_scene("b").unwrap();
    assert_eq!(sm.scene_count(), 2);

    sm.remove_scene("a");
    assert_eq!(sm.scene_count(), 1);

    sm.remove_scene("b");
    assert_eq!(sm.scene_count(), 0);
}

// ============================================================================
// Tests: Thread Safety
// ============================================================================

#[test]
fn test_scene_arc_mutex_is_shareable() {
    let mut sm = SceneManager::new();
    let scene = sm.create_scene("main").unwrap();

    // Clone the Arc (simulating sharing between threads)
    let scene2 = scene.clone();

    // Both references access the same scene
    assert!(Arc::ptr_eq(&scene, &scene2));
}
