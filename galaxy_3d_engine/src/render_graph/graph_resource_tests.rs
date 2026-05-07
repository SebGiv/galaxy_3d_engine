use super::*;
use crate::resource::resource_manager::{TextureKey, BufferKey};
use std::collections::HashSet;

fn make_texture(mip: u32, layer: u32, count: u32) -> GraphResource {
    GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: mip,
        mip_count: 1,
        base_array_layer: layer,
        layer_count: count,
    }
}

fn make_buffer() -> GraphResource {
    GraphResource::Buffer {
        buffer_key: BufferKey::default(),
        offset: 0,
        size: WHOLE_SIZE,
    }
}

#[test]
fn test_is_texture_for_texture_variant() {
    let res = make_texture(0, 0, 1);
    assert!(res.is_texture());
}

#[test]
fn test_is_texture_for_buffer_variant() {
    let res = make_buffer();
    assert!(!res.is_texture());
}

#[test]
fn test_is_buffer_for_buffer_variant() {
    let res = make_buffer();
    assert!(res.is_buffer());
}

#[test]
fn test_is_buffer_for_texture_variant() {
    let res = make_texture(0, 0, 1);
    assert!(!res.is_buffer());
}

#[test]
fn test_equality_same_texture_view() {
    assert_eq!(make_texture(0, 0, 1), make_texture(0, 0, 1));
}

#[test]
fn test_inequality_different_mip_level() {
    assert_ne!(make_texture(0, 0, 1), make_texture(1, 0, 1));
}

#[test]
fn test_inequality_different_array_layer() {
    assert_ne!(make_texture(0, 0, 1), make_texture(0, 2, 1));
}

#[test]
fn test_inequality_different_layer_count() {
    assert_ne!(make_texture(0, 0, 1), make_texture(0, 0, 6));
}

#[test]
fn test_inequality_different_mip_count() {
    let a = GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0,
        mip_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    };
    let b = GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: 0,
        mip_count: 4,
        base_array_layer: 0,
        layer_count: 1,
    };
    assert_ne!(a, b);
}

#[test]
fn test_inequality_buffer_vs_texture() {
    let tex = make_texture(0, 0, 1);
    let buf = make_buffer();
    assert_ne!(tex, buf);
}

#[test]
fn test_inequality_different_buffer_offset() {
    let a = GraphResource::Buffer {
        buffer_key: BufferKey::default(),
        offset: 0,
        size: 64,
    };
    let b = GraphResource::Buffer {
        buffer_key: BufferKey::default(),
        offset: 32,
        size: 64,
    };
    assert_ne!(a, b);
}

#[test]
fn test_inequality_different_buffer_size() {
    let a = GraphResource::Buffer {
        buffer_key: BufferKey::default(),
        offset: 0,
        size: 64,
    };
    let b = GraphResource::Buffer {
        buffer_key: BufferKey::default(),
        offset: 0,
        size: 128,
    };
    assert_ne!(a, b);
}

#[test]
fn test_hashable_in_hash_set() {
    let mut set = HashSet::new();
    set.insert(make_texture(0, 0, 1));
    set.insert(make_texture(0, 0, 1));
    set.insert(make_texture(1, 0, 1));
    set.insert(make_buffer());
    assert_eq!(set.len(), 3);
}

#[test]
fn test_clone_and_copy_preserve_value() {
    let original = make_texture(2, 3, 4);
    let copied = original;
    let cloned = original.clone();
    assert_eq!(original, copied);
    assert_eq!(original, cloned);
}

#[test]
fn test_texture_attachment_helper() {
    let tk = TextureKey::default();
    let r = GraphResource::texture_attachment(tk, 0, 0, 1);
    match r {
        GraphResource::Texture { texture_key, base_mip_level, mip_count, base_array_layer, layer_count } => {
            assert_eq!(texture_key, tk);
            assert_eq!(base_mip_level, 0);
            assert_eq!(mip_count, 1);
            assert_eq!(base_array_layer, 0);
            assert_eq!(layer_count, 1);
        }
        _ => panic!("expected Texture variant"),
    }
}

#[test]
fn test_texture_full_helper_uses_remaining_sentinels() {
    let r = GraphResource::texture_full(TextureKey::default());
    match r {
        GraphResource::Texture { mip_count, layer_count, .. } => {
            assert_eq!(mip_count, REMAINING_MIP_LEVELS);
            assert_eq!(layer_count, REMAINING_ARRAY_LAYERS);
        }
        _ => panic!("expected Texture variant"),
    }
}

#[test]
fn test_buffer_full_helper_uses_whole_size() {
    let bk = BufferKey::default();
    let r = GraphResource::buffer_full(bk);
    match r {
        GraphResource::Buffer { buffer_key, offset, size } => {
            assert_eq!(buffer_key, bk);
            assert_eq!(offset, 0);
            assert_eq!(size, WHOLE_SIZE);
        }
        _ => panic!("expected Buffer variant"),
    }
}

#[test]
fn test_buffer_range_helper() {
    let bk = BufferKey::default();
    let r = GraphResource::buffer_range(bk, 32, 256);
    match r {
        GraphResource::Buffer { buffer_key, offset, size } => {
            assert_eq!(buffer_key, bk);
            assert_eq!(offset, 32);
            assert_eq!(size, 256);
        }
        _ => panic!("expected Buffer variant"),
    }
}

// ============================================================================
// ImageSubRange / BufferSubRange overlap helpers
// ============================================================================

fn img_range(base_mip: u32, mip_count: u32, base_layer: u32, layer_count: u32)
    -> ImageSubRange
{
    ImageSubRange {
        base_mip_level: base_mip,
        mip_count,
        base_array_layer: base_layer,
        layer_count,
    }
}

#[test]
fn image_subranges_overlap_identical() {
    let a = img_range(0, 1, 0, 1);
    assert!(a.overlaps(&a));
    let b = img_range(2, 3, 4, 6);
    assert!(b.overlaps(&b));
}

#[test]
fn image_subranges_overlap_disjoint_mips() {
    let a = img_range(0, 1, 0, 1);
    let b = img_range(1, 1, 0, 1);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

#[test]
fn image_subranges_overlap_disjoint_layers() {
    let a = img_range(0, 1, 0, 2);
    let b = img_range(0, 1, 2, 2);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

#[test]
fn image_subranges_overlap_partial_mips() {
    // mips 0..3 vs mips 2..5 → overlap on mip 2
    let a = img_range(0, 3, 0, 1);
    let b = img_range(2, 3, 0, 1);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn image_subranges_overlap_disjoint_one_axis_kills_overlap() {
    // mips identical, layers disjoint → no overlap
    let a = img_range(0, 1, 0, 1);
    let b = img_range(0, 1, 1, 1);
    assert!(!a.overlaps(&b));
}

#[test]
fn image_subranges_overlap_with_remaining_mip_levels() {
    // a covers all mips from level 0 → overlaps anything mip-wise.
    let a = img_range(0, REMAINING_MIP_LEVELS, 0, 1);
    let b = img_range(5, 1, 0, 1);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn image_subranges_overlap_with_remaining_array_layers() {
    let a = img_range(0, 1, 0, REMAINING_ARRAY_LAYERS);
    let b = img_range(0, 1, 7, 1);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn image_subranges_overlap_disjoint_with_remaining_mips_starting_higher() {
    // a covers all mips from level 3 → does NOT cover mips 0..2.
    let a = img_range(3, REMAINING_MIP_LEVELS, 0, 1);
    let b = img_range(0, 2, 0, 1);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

fn buf_range(offset: u64, size: u64) -> BufferSubRange {
    BufferSubRange { offset, size }
}

#[test]
fn buffer_subranges_overlap_identical() {
    let a = buf_range(0, 64);
    assert!(a.overlaps(&a));
}

#[test]
fn buffer_subranges_overlap_disjoint() {
    let a = buf_range(0, 64);
    let b = buf_range(64, 64);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}

#[test]
fn buffer_subranges_overlap_partial() {
    let a = buf_range(0, 64);
    let b = buf_range(32, 64);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn buffer_subranges_overlap_with_whole_size() {
    let a = buf_range(0, WHOLE_SIZE);
    let b = buf_range(1024, 256);
    assert!(a.overlaps(&b));
    assert!(b.overlaps(&a));
}

#[test]
fn buffer_subranges_overlap_disjoint_with_whole_size_starting_higher() {
    // a covers from offset 1024 onward; b is below 1024.
    let a = buf_range(1024, WHOLE_SIZE);
    let b = buf_range(0, 1024);
    assert!(!a.overlaps(&b));
    assert!(!b.overlaps(&a));
}
