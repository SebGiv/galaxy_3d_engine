use super::*;
use crate::graphics_device::DynamicRenderState;
use crate::resource::resource_manager::{GeometryKey, PipelineKey};

fn make_dc(draw_slot: u32) -> DrawCall {
    DrawCall {
        pipeline_key: PipelineKey::default(),
        geometry_key: GeometryKey::default(),
        vertex_offset: 0,
        vertex_count: 6,
        index_offset: 0,
        index_count: 6,
        draw_slot,
        render_state: DynamicRenderState::default(),
        render_state_sig: 0,
    }
}

// ============================================================================
// build_sort_key
// ============================================================================

#[test]
fn test_build_sort_key_zero() {
    assert_eq!(build_sort_key(0, 0, 0, 0), 0);
}

#[test]
fn test_build_sort_key_packs_signature_in_msb() {
    let key = build_sort_key(0xABCD, 0, 0, 0);
    assert_eq!(key, 0xABCD_0000_0000_0000);
}

#[test]
fn test_build_sort_key_packs_pipeline_sort() {
    let key = build_sort_key(0, 0xABCD, 0, 0);
    assert_eq!(key, 0x0000_ABCD_0000_0000);
}

#[test]
fn test_build_sort_key_packs_geometry_sort() {
    let key = build_sort_key(0, 0, 0xABCD, 0);
    assert_eq!(key, 0x0000_0000_ABCD_0000);
}

#[test]
fn test_build_sort_key_packs_render_state_in_lsb() {
    let key = build_sort_key(0, 0, 0, 0xABCD);
    assert_eq!(key, 0x0000_0000_0000_ABCD);
}

#[test]
fn test_build_sort_key_full_packing() {
    let key = build_sort_key(0x1111, 0x2222, 0x3333, 0x4444);
    assert_eq!(key, 0x1111_2222_3333_4444);
}

#[test]
fn test_build_sort_key_signature_is_msb_priority() {
    // Higher signature outranks every other byte.
    let high_sig = build_sort_key(2, 0, 0, 0);
    let low_sig_high_other = build_sort_key(1, 0xFFFF, 0xFFFF, 0xFFFF);
    assert!(high_sig > low_sig_high_other);
}

// ============================================================================
// distance_to_u16
// ============================================================================

#[test]
fn test_distance_to_u16_zero_is_neutral() {
    let z = distance_to_u16(0.0);
    let neg_small = distance_to_u16(-0.0001);
    let pos_small = distance_to_u16(0.0001);
    assert!(neg_small < z);
    assert!(pos_small > z);
}

#[test]
fn test_distance_to_u16_preserves_ordering_positive() {
    assert!(distance_to_u16(1.0) < distance_to_u16(2.0));
    assert!(distance_to_u16(2.0) < distance_to_u16(100.0));
    assert!(distance_to_u16(100.0) < distance_to_u16(10_000.0));
}

#[test]
fn test_distance_to_u16_preserves_ordering_negative() {
    assert!(distance_to_u16(-100.0) < distance_to_u16(-1.0));
    assert!(distance_to_u16(-1.0) < distance_to_u16(0.0));
}

#[test]
fn test_distance_to_u16_negative_lt_positive() {
    assert!(distance_to_u16(-5.0) < distance_to_u16(5.0));
    assert!(distance_to_u16(-1000.0) < distance_to_u16(0.001));
}

// ============================================================================
// RenderQueue
// ============================================================================

#[test]
fn test_with_capacity_is_empty() {
    let q = RenderQueue::with_capacity(64);
    assert_eq!(q.len(), 0);
    assert!(q.is_empty());
}

#[test]
fn test_push_appends_draw_call() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), 100);
    q.push(make_dc(1), 50);
    q.push(make_dc(2), 200);
    assert_eq!(q.len(), 3);
    assert!(!q.is_empty());
}

#[test]
fn test_clear_resets_length_only() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), 1);
    q.push(make_dc(1), 2);
    q.clear();
    assert_eq!(q.len(), 0);
    assert!(q.is_empty());
}

#[test]
fn test_iter_sorted_unsorted_initial_order() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), 30);
    q.push(make_dc(1), 10);
    q.push(make_dc(2), 20);
    // Without sort, iter_sorted reflects insertion order.
    let slots: Vec<u32> = q.iter_sorted().map(|dc| dc.draw_slot).collect();
    assert_eq!(slots, vec![0, 1, 2]);
}

#[test]
fn test_sort_orders_ascending_by_key() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), 30);
    q.push(make_dc(1), 10);
    q.push(make_dc(2), 20);
    q.sort();
    let slots: Vec<u32> = q.iter_sorted().map(|dc| dc.draw_slot).collect();
    assert_eq!(slots, vec![1, 2, 0]);
}

#[test]
fn test_sort_with_full_packed_keys() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), build_sort_key(2, 0, 0, 0));
    q.push(make_dc(1), build_sort_key(1, 0xFFFF, 0xFFFF, 0xFFFF));
    q.push(make_dc(2), build_sort_key(0, 0, 0, 0));
    q.sort();
    let slots: Vec<u32> = q.iter_sorted().map(|dc| dc.draw_slot).collect();
    assert_eq!(slots, vec![2, 1, 0]);
}

#[test]
fn test_clear_then_reuse() {
    let mut q = RenderQueue::with_capacity(4);
    q.push(make_dc(0), 1);
    q.clear();
    q.push(make_dc(7), 99);
    assert_eq!(q.len(), 1);
    let slots: Vec<u32> = q.iter_sorted().map(|dc| dc.draw_slot).collect();
    assert_eq!(slots, vec![7]);
}
