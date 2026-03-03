use super::*;

// ============================================================================
// Tests: Basic insert / contains / len
// ============================================================================

#[test]
fn test_new_is_empty() {
    let set: SwapSet<u32> = SwapSet::new();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
}

#[test]
fn test_insert_and_contains() {
    let mut set: SwapSet<u32> = SwapSet::new();
    assert!(set.insert(1));
    assert!(set.insert(2));

    assert!(set.contains(&1));
    assert!(set.contains(&2));
    assert!(!set.contains(&3));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_insert_duplicate_returns_false() {
    let mut set: SwapSet<u32> = SwapSet::new();
    assert!(set.insert(1));
    assert!(!set.insert(1));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_remove() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);
    set.insert(2);

    assert!(set.remove(&1));
    assert!(!set.contains(&1));
    assert!(set.contains(&2));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_remove_nonexistent_returns_false() {
    let mut set: SwapSet<u32> = SwapSet::new();
    assert!(!set.remove(&42));
}

// ============================================================================
// Tests: Flip
// ============================================================================

#[test]
fn test_flip_returns_previous_front() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(10);
    set.insert(20);

    let back = set.flip();
    assert_eq!(back.len(), 2);
    assert!(back.contains(&10));
    assert!(back.contains(&20));
}

#[test]
fn test_front_is_empty_after_flip() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(10);

    let _back = set.flip();

    // Front is logically empty (pending clear)
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);
    assert!(!set.contains(&10));
}

#[test]
fn test_insert_after_flip_goes_to_new_front() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);

    let back = set.flip();
    assert!(back.contains(&1));

    // Insert into new front
    set.insert(2);
    assert!(set.contains(&2));
    assert!(!set.contains(&1)); // 1 is in the back, not the front

    assert_eq!(set.len(), 1);
}

#[test]
fn test_double_flip() {
    let mut set: SwapSet<u32> = SwapSet::new();

    // Frame 1: insert A, B
    set.insert(1);
    set.insert(2);

    let back1 = set.flip();
    assert!(back1.contains(&1));
    assert!(back1.contains(&2));

    // Frame 2: insert C
    set.insert(3);

    let back2 = set.flip();
    assert_eq!(back2.len(), 1);
    assert!(back2.contains(&3));

    // Back1 (now front, pending clear) is logically empty
    assert!(set.is_empty());
}

#[test]
fn test_flip_preserves_capacity() {
    let mut set: SwapSet<u32> = SwapSet::new();

    // Insert enough elements to force allocation
    for i in 0..100 {
        set.insert(i);
    }

    set.flip();

    // Insert one element into new front — triggers lazy clear
    set.insert(999);

    // The old front (now back) should still have its capacity
    let back = set.back();
    assert!(back.capacity() >= 100);
}

// ============================================================================
// Tests: Remove after flip
// ============================================================================

#[test]
fn test_remove_on_logically_empty_front_returns_false() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);
    set.flip();

    // Front is logically empty (pending clear), remove returns false
    assert!(!set.remove(&1));
}

// ============================================================================
// Tests: Back accessor
// ============================================================================

#[test]
fn test_back_without_flip_is_empty() {
    let set: SwapSet<u32> = SwapSet::new();
    assert!(set.back().is_empty());
}

#[test]
fn test_back_after_flip() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);
    set.flip();

    assert!(set.back().contains(&1));
}

// ============================================================================
// Tests: Clear
// ============================================================================

#[test]
fn test_clear_empties_both_buffers() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);
    set.flip();
    set.insert(2);

    set.clear();

    assert!(set.is_empty());
    assert!(set.back().is_empty());
}

#[test]
fn test_insert_after_clear() {
    let mut set: SwapSet<u32> = SwapSet::new();
    set.insert(1);
    set.flip();
    set.clear();

    set.insert(42);
    assert!(set.contains(&42));
    assert_eq!(set.len(), 1);
}

// ============================================================================
// Tests: Multi-frame simulation
// ============================================================================

#[test]
fn test_three_frame_cycle() {
    let mut set: SwapSet<u32> = SwapSet::new();

    // Frame 1
    set.insert(1);
    set.insert(2);
    let f1 = set.flip();
    assert_eq!(f1.len(), 2);

    // Frame 2
    set.insert(3);
    let f2 = set.flip();
    assert_eq!(f2.len(), 1);
    assert!(f2.contains(&3));

    // Frame 3: no inserts
    let f3 = set.flip();
    assert_eq!(f3.len(), 0);

    // Frame 4: insert again
    set.insert(4);
    set.insert(5);
    let f4 = set.flip();
    assert_eq!(f4.len(), 2);
    assert!(f4.contains(&4));
    assert!(f4.contains(&5));
}
