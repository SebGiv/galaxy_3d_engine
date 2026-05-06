//! Tests for the debug pass action.
//!
//! Most of `DebugPassAction` exercises the live `Engine` (graphics device,
//! resource manager) and is covered by integration tests in the SciFi demo
//! rather than unit tests. This module covers only the small pure helpers.

use super::{DebugDisplayMode, DebugDrawEntry};

#[test]
fn debug_display_mode_is_hashable_and_eq() {
    use std::collections::HashSet;
    let mut s = HashSet::new();
    s.insert(DebugDisplayMode::Wireframe);
    s.insert(DebugDisplayMode::BoundingBox);
    assert!(s.contains(&DebugDisplayMode::Wireframe));
    assert!(s.contains(&DebugDisplayMode::BoundingBox));
    assert_ne!(DebugDisplayMode::Wireframe, DebugDisplayMode::BoundingBox);
}

#[test]
fn debug_draw_entry_is_copy_and_layout_stable() {
    use slotmap::Key;
    let entry = DebugDrawEntry {
        instance_key: crate::scene::RenderInstanceKey::null(),
        color: [1.0, 0.5, 0.0, 0.75],
        line_width: 2.5,
    };
    let copy = entry;
    assert_eq!(copy.color, [1.0, 0.5, 0.0, 0.75]);
    assert_eq!(copy.line_width, 2.5);
}
