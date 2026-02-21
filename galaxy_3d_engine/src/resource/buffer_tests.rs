use super::*;
use crate::renderer;
use std::sync::{Arc, Mutex};

// ============================================================================
// Helpers
// ============================================================================

fn create_mock_renderer() -> Arc<Mutex<dyn renderer::Renderer>> {
    Arc::new(Mutex::new(renderer::mock_renderer::MockRenderer::new()))
}

fn make_fields(specs: &[(&str, FieldType)]) -> Vec<FieldDesc> {
    specs.iter().map(|(name, ft)| FieldDesc {
        name: name.to_string(),
        field_type: *ft,
    }).collect()
}

fn create_test_buffer(kind: BufferKind, fields: &[(&str, FieldType)], count: u32) -> Buffer {
    let renderer = create_mock_renderer();
    Buffer::from_desc(BufferDesc {
        renderer,
        kind,
        fields: make_fields(fields),
        count,
    }).unwrap()
}

// ============================================================================
// FieldType tests
// ============================================================================

#[test]
fn test_field_type_size_bytes() {
    assert_eq!(FieldType::Float.size_bytes(), 4);
    assert_eq!(FieldType::Vec2.size_bytes(), 8);
    assert_eq!(FieldType::Vec3.size_bytes(), 16); // std140 padding
    assert_eq!(FieldType::Vec4.size_bytes(), 16);
    assert_eq!(FieldType::Mat3.size_bytes(), 48);
    assert_eq!(FieldType::Mat4.size_bytes(), 64);
    assert_eq!(FieldType::Int.size_bytes(), 4);
    assert_eq!(FieldType::UInt.size_bytes(), 4);
}

#[test]
fn test_field_type_alignment() {
    assert_eq!(FieldType::Float.alignment(), 4);
    assert_eq!(FieldType::Vec2.alignment(), 8);
    assert_eq!(FieldType::Vec3.alignment(), 16);
    assert_eq!(FieldType::Vec4.alignment(), 16);
    assert_eq!(FieldType::Mat3.alignment(), 16);
    assert_eq!(FieldType::Mat4.alignment(), 16);
    assert_eq!(FieldType::Int.alignment(), 4);
    assert_eq!(FieldType::UInt.alignment(), 4);
}

// ============================================================================
// Validation tests
// ============================================================================

#[test]
fn test_empty_fields_fails() {
    let renderer = create_mock_renderer();
    let result = Buffer::from_desc(BufferDesc {
        renderer,
        kind: BufferKind::Storage,
        fields: vec![],
        count: 10,
    });
    assert!(result.is_err());
}

#[test]
fn test_zero_count_fails() {
    let renderer = create_mock_renderer();
    let result = Buffer::from_desc(BufferDesc {
        renderer,
        kind: BufferKind::Storage,
        fields: make_fields(&[("world", FieldType::Mat4)]),
        count: 0,
    });
    assert!(result.is_err());
}

#[test]
fn test_duplicate_field_names_fails() {
    let renderer = create_mock_renderer();
    let result = Buffer::from_desc(BufferDesc {
        renderer,
        kind: BufferKind::Storage,
        fields: make_fields(&[
            ("world", FieldType::Mat4),
            ("world", FieldType::Mat4),
        ]),
        count: 10,
    });
    assert!(result.is_err());
}

// ============================================================================
// Layout std140 tests
// ============================================================================

#[test]
fn test_layout_single_mat4() {
    // Mat4 = 64 bytes, alignment 16 → stride = 64
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 100);

    assert_eq!(buf.stride(), 64);
    assert_eq!(buf.size(), 64 * 100);
    assert_eq!(buf.field_offset(0), Some(0));
}

#[test]
fn test_layout_mat4_mat4_uint() {
    // world: offset 0, size 64
    // world_inv: offset 64, size 64
    // material_index: offset 128, size 4
    // current_offset = 132, aligned to 16 → stride = 144
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
        ("world_inv", FieldType::Mat4),
        ("material_index", FieldType::UInt),
    ], 1000);

    assert_eq!(buf.field_offset(0), Some(0));
    assert_eq!(buf.field_offset(1), Some(64));
    assert_eq!(buf.field_offset(2), Some(128));
    assert_eq!(buf.stride(), 144);
    assert_eq!(buf.size(), 144 * 1000);
}

#[test]
fn test_layout_float_vec3_alignment() {
    // float: offset 0, size 4
    // vec3: alignment 16 → offset 16, size 16
    // current_offset = 32, aligned to 16 → stride = 32
    let buf = create_test_buffer(BufferKind::Uniform, &[
        ("intensity", FieldType::Float),
        ("position", FieldType::Vec3),
    ], 1);

    assert_eq!(buf.field_offset(0), Some(0));
    assert_eq!(buf.field_offset(1), Some(16));
    assert_eq!(buf.stride(), 32);
}

#[test]
fn test_layout_vec2_float() {
    // vec2: offset 0, size 8
    // float: alignment 4 → offset 8, size 4
    // current_offset = 12, aligned to 16 → stride = 16
    let buf = create_test_buffer(BufferKind::Uniform, &[
        ("uv", FieldType::Vec2),
        ("alpha", FieldType::Float),
    ], 1);

    assert_eq!(buf.field_offset(0), Some(0));
    assert_eq!(buf.field_offset(1), Some(8));
    assert_eq!(buf.stride(), 16);
}

#[test]
fn test_layout_mixed_complex() {
    // float: offset 0, size 4
    // vec2: alignment 8 → offset 8, size 8
    // vec4: alignment 16 → offset 16, size 16
    // mat4: alignment 16 → offset 32, size 64
    // uint: alignment 4 → offset 96, size 4
    // current_offset = 100, aligned to 16 → stride = 112
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("roughness", FieldType::Float),
        ("uv_scale", FieldType::Vec2),
        ("color", FieldType::Vec4),
        ("transform", FieldType::Mat4),
        ("flags", FieldType::UInt),
    ], 50);

    assert_eq!(buf.field_offset(0), Some(0));
    assert_eq!(buf.field_offset(1), Some(8));
    assert_eq!(buf.field_offset(2), Some(16));
    assert_eq!(buf.field_offset(3), Some(32));
    assert_eq!(buf.field_offset(4), Some(96));
    assert_eq!(buf.stride(), 112);
    assert_eq!(buf.size(), 112 * 50);
}

// ============================================================================
// Accessor tests
// ============================================================================

#[test]
fn test_accessors() {
    let buf = create_test_buffer(BufferKind::Uniform, &[
        ("view", FieldType::Mat4),
        ("proj", FieldType::Mat4),
    ], 4);

    assert_eq!(buf.kind(), BufferKind::Uniform);
    assert_eq!(buf.count(), 4);
    assert_eq!(buf.fields().len(), 2);
    assert_eq!(buf.stride(), 128);
    assert_eq!(buf.size(), 128 * 4);
}

#[test]
fn test_field_id() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
        ("world_inv", FieldType::Mat4),
        ("material_index", FieldType::UInt),
    ], 1);

    assert_eq!(buf.field_id("world"), Some(0));
    assert_eq!(buf.field_id("world_inv"), Some(1));
    assert_eq!(buf.field_id("material_index"), Some(2));
}

#[test]
fn test_field_id_unknown() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 1);

    assert_eq!(buf.field_id("nonexistent"), None);
}

#[test]
fn test_field_offset_out_of_bounds() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 1);

    assert_eq!(buf.field_offset(0), Some(0));
    assert_eq!(buf.field_offset(1), None);
    assert_eq!(buf.field_offset(999), None);
}

// ============================================================================
// update_element tests
// ============================================================================

#[test]
fn test_update_element_success() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 64]; // Mat4 = 64 bytes
    assert!(buf.update_element(0, &data).is_ok());
    assert!(buf.update_element(9, &data).is_ok());
}

#[test]
fn test_update_element_out_of_bounds() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 64];
    assert!(buf.update_element(10, &data).is_err());
    assert!(buf.update_element(100, &data).is_err());
}

#[test]
fn test_update_element_data_too_large() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    // stride = 64, data = 65 → trop grand
    let data = vec![0u8; 65];
    assert!(buf.update_element(0, &data).is_err());
}

// ============================================================================
// update_field tests
// ============================================================================

#[test]
fn test_update_field_success() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
        ("material_index", FieldType::UInt),
    ], 10);

    let mat4_data = vec![0u8; 64];
    let uint_data = vec![0u8; 4];
    assert!(buf.update_field(0, 0, &mat4_data).is_ok()); // world
    assert!(buf.update_field(9, 1, &uint_data).is_ok()); // material_index
}

#[test]
fn test_update_field_element_out_of_bounds() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 64];
    assert!(buf.update_field(10, 0, &data).is_err());
}

#[test]
fn test_update_field_field_out_of_bounds() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 64];
    assert!(buf.update_field(0, 1, &data).is_err());
    assert!(buf.update_field(0, 999, &data).is_err());
}

#[test]
fn test_update_field_wrong_data_size() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    // Mat4 expects 64 bytes, give 32
    let data = vec![0u8; 32];
    assert!(buf.update_field(0, 0, &data).is_err());
}

// ============================================================================
// update_raw tests
// ============================================================================

#[test]
fn test_update_raw_success() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 16];
    assert!(buf.update_raw(0, &data).is_ok());
    assert!(buf.update_raw(buf.size() - 16, &data).is_ok());
}

#[test]
fn test_update_raw_exceeds_size() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 10);

    let data = vec![0u8; 16];
    // Offset + data dépasse la taille du buffer
    assert!(buf.update_raw(buf.size() - 15, &data).is_err());
    assert!(buf.update_raw(buf.size(), &data).is_err());
}

// ============================================================================
// Buffer kind tests
// ============================================================================

#[test]
fn test_buffer_kind_uniform() {
    let buf = create_test_buffer(BufferKind::Uniform, &[
        ("view", FieldType::Mat4),
    ], 1);
    assert_eq!(buf.kind(), BufferKind::Uniform);
}

#[test]
fn test_buffer_kind_storage() {
    let buf = create_test_buffer(BufferKind::Storage, &[
        ("world", FieldType::Mat4),
    ], 1);
    assert_eq!(buf.kind(), BufferKind::Storage);
}
