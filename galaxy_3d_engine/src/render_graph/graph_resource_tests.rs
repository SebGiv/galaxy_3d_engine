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
