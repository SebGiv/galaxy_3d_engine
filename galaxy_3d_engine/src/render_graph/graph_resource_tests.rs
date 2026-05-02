use super::*;
use crate::resource::resource_manager::{TextureKey, BufferKey};
use std::collections::HashSet;

fn make_texture(mip: u32, layer: u32, count: u32) -> GraphResource {
    GraphResource::Texture {
        texture_key: TextureKey::default(),
        base_mip_level: mip,
        base_array_layer: layer,
        layer_count: count,
    }
}

#[test]
fn test_is_texture_for_texture_variant() {
    let res = make_texture(0, 0, 1);
    assert!(res.is_texture());
}

#[test]
fn test_is_texture_for_buffer_variant() {
    let res = GraphResource::Buffer(BufferKey::default());
    assert!(!res.is_texture());
}

#[test]
fn test_is_buffer_for_buffer_variant() {
    let res = GraphResource::Buffer(BufferKey::default());
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
fn test_inequality_buffer_vs_texture() {
    let tex = make_texture(0, 0, 1);
    let buf = GraphResource::Buffer(BufferKey::default());
    assert_ne!(tex, buf);
}

#[test]
fn test_hashable_in_hash_set() {
    let mut set = HashSet::new();
    set.insert(make_texture(0, 0, 1));
    set.insert(make_texture(0, 0, 1));
    set.insert(make_texture(1, 0, 1));
    set.insert(GraphResource::Buffer(BufferKey::default()));
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
