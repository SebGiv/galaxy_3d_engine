use super::*;

// ============================================================================
// Basic allocation tests
// ============================================================================

#[test]
fn test_sequential_alloc() {
    let mut alloc = SlotAllocator::new();
    assert_eq!(alloc.alloc(), 0);
    assert_eq!(alloc.alloc(), 1);
    assert_eq!(alloc.alloc(), 2);
}

#[test]
fn test_new_is_empty() {
    let alloc = SlotAllocator::new();
    assert!(alloc.is_empty());
    assert_eq!(alloc.len(), 0);
    assert_eq!(alloc.high_water_mark(), 0);
}

#[test]
fn test_default_is_empty() {
    let alloc = SlotAllocator::default();
    assert!(alloc.is_empty());
}

// ============================================================================
// Free and recycle tests
// ============================================================================

#[test]
fn test_free_and_recycle() {
    let mut alloc = SlotAllocator::new();
    let a = alloc.alloc(); // 0
    let b = alloc.alloc(); // 1
    alloc.free(a);          // 0 goes to free list
    let c = alloc.alloc(); // 0 (recycled)
    assert_eq!(c, 0);
    assert_eq!(b, 1);
}

#[test]
fn test_free_multiple_recycle_lifo() {
    // Free list is a stack (LIFO): last freed = first recycled
    let mut alloc = SlotAllocator::new();
    let a = alloc.alloc(); // 0
    let _b = alloc.alloc(); // 1
    let c = alloc.alloc(); // 2
    alloc.free(a);          // free list: [0]
    alloc.free(c);          // free list: [0, 2]

    // Next alloc pops from the end → 2 first, then 0
    assert_eq!(alloc.alloc(), 2);
    assert_eq!(alloc.alloc(), 0);
    // Free list exhausted, next is fresh
    assert_eq!(alloc.alloc(), 3);
}

// ============================================================================
// len() and high_water_mark() tests
// ============================================================================

#[test]
fn test_len_tracks_active_slots() {
    let mut alloc = SlotAllocator::new();
    assert_eq!(alloc.len(), 0);

    alloc.alloc();
    assert_eq!(alloc.len(), 1);

    alloc.alloc();
    assert_eq!(alloc.len(), 2);

    alloc.free(0);
    assert_eq!(alloc.len(), 1);

    alloc.free(1);
    assert_eq!(alloc.len(), 0);
    assert!(alloc.is_empty());
}

#[test]
fn test_high_water_mark_never_decreases() {
    let mut alloc = SlotAllocator::new();
    assert_eq!(alloc.high_water_mark(), 0);

    alloc.alloc(); // 0
    assert_eq!(alloc.high_water_mark(), 1);

    alloc.alloc(); // 1
    assert_eq!(alloc.high_water_mark(), 2);

    // Freeing does NOT reduce the high water mark
    alloc.free(0);
    alloc.free(1);
    assert_eq!(alloc.high_water_mark(), 2);

    // Recycled alloc doesn't increase it either
    alloc.alloc(); // 1 (recycled)
    assert_eq!(alloc.high_water_mark(), 2);

    // Fresh alloc does
    alloc.alloc(); // 0 (recycled)
    alloc.alloc(); // 2 (fresh)
    assert_eq!(alloc.high_water_mark(), 3);
}

// ============================================================================
// Stress / pattern tests
// ============================================================================

#[test]
fn test_alloc_free_alloc_cycle() {
    let mut alloc = SlotAllocator::new();

    // Allocate 100 slots
    let ids: Vec<u32> = (0..100).map(|_| alloc.alloc()).collect();
    assert_eq!(alloc.len(), 100);
    assert_eq!(alloc.high_water_mark(), 100);

    // Free all odd indices
    for &id in ids.iter().filter(|id| *id % 2 == 1) {
        alloc.free(id);
    }
    assert_eq!(alloc.len(), 50);
    assert_eq!(alloc.high_water_mark(), 100); // unchanged

    // Allocate 50 more — should recycle the freed ones
    let new_ids: Vec<u32> = (0..50).map(|_| alloc.alloc()).collect();
    assert_eq!(alloc.len(), 100);
    assert_eq!(alloc.high_water_mark(), 100); // still 100 — all recycled

    // All new_ids should be odd numbers (the ones we freed)
    for id in &new_ids {
        assert!(id % 2 == 1, "expected recycled odd id, got {}", id);
    }

    // One more alloc should be fresh
    assert_eq!(alloc.alloc(), 100);
    assert_eq!(alloc.high_water_mark(), 101);
}

#[test]
fn test_indices_are_unique() {
    let mut alloc = SlotAllocator::new();
    let mut seen = std::collections::HashSet::new();

    // Allocate, free some, allocate again — all live indices must be unique
    for _ in 0..50 {
        seen.insert(alloc.alloc());
    }
    // Free 10 of them
    for id in 0..10 {
        alloc.free(id);
        seen.remove(&id);
    }
    // Allocate 10 more
    for _ in 0..10 {
        let id = alloc.alloc();
        assert!(seen.insert(id), "duplicate slot id: {}", id);
    }
    assert_eq!(seen.len(), 50);
}
