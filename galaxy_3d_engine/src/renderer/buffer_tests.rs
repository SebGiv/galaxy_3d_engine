//! Unit tests for Buffer module
//!
//! Tests BufferFormat::size_bytes() method to ensure correct size calculations
//! for all vertex attribute and index buffer formats.

#[cfg(test)]
use crate::renderer::BufferFormat;

// ============================================================================
// FLOAT FORMATS
// ============================================================================

#[test]
fn test_buffer_format_size_bytes_float_formats() {
    // 32-bit float formats
    assert_eq!(BufferFormat::R32_SFLOAT.size_bytes(), 4);
    assert_eq!(BufferFormat::R32G32_SFLOAT.size_bytes(), 8);
    assert_eq!(BufferFormat::R32G32B32_SFLOAT.size_bytes(), 12);
    assert_eq!(BufferFormat::R32G32B32A32_SFLOAT.size_bytes(), 16);
}

// ============================================================================
// INTEGER FORMATS (SIGNED)
// ============================================================================

#[test]
fn test_buffer_format_size_bytes_signed_int_formats() {
    // 32-bit signed integers
    assert_eq!(BufferFormat::R32_SINT.size_bytes(), 4);
    assert_eq!(BufferFormat::R32G32_SINT.size_bytes(), 8);
    assert_eq!(BufferFormat::R32G32B32_SINT.size_bytes(), 12);
    assert_eq!(BufferFormat::R32G32B32A32_SINT.size_bytes(), 16);

    // 16-bit signed integers
    assert_eq!(BufferFormat::R16_SINT.size_bytes(), 2);
    assert_eq!(BufferFormat::R16G16_SINT.size_bytes(), 4);
    assert_eq!(BufferFormat::R16G16B16A16_SINT.size_bytes(), 8);

    // 8-bit signed integers
    assert_eq!(BufferFormat::R8_SINT.size_bytes(), 1);
    assert_eq!(BufferFormat::R8G8_SINT.size_bytes(), 2);
    assert_eq!(BufferFormat::R8G8B8A8_SINT.size_bytes(), 4);
}

// ============================================================================
// INTEGER FORMATS (UNSIGNED)
// ============================================================================

#[test]
fn test_buffer_format_size_bytes_unsigned_int_formats() {
    // 32-bit unsigned integers
    assert_eq!(BufferFormat::R32_UINT.size_bytes(), 4);
    assert_eq!(BufferFormat::R32G32_UINT.size_bytes(), 8);
    assert_eq!(BufferFormat::R32G32B32_UINT.size_bytes(), 12);
    assert_eq!(BufferFormat::R32G32B32A32_UINT.size_bytes(), 16);

    // 16-bit unsigned integers
    assert_eq!(BufferFormat::R16_UINT.size_bytes(), 2);
    assert_eq!(BufferFormat::R16G16_UINT.size_bytes(), 4);
    assert_eq!(BufferFormat::R16G16B16A16_UINT.size_bytes(), 8);

    // 8-bit unsigned integers
    assert_eq!(BufferFormat::R8_UINT.size_bytes(), 1);
    assert_eq!(BufferFormat::R8G8_UINT.size_bytes(), 2);
    assert_eq!(BufferFormat::R8G8B8A8_UINT.size_bytes(), 4);
}

// ============================================================================
// COMPREHENSIVE TEST
// ============================================================================

#[test]
fn test_buffer_format_size_bytes_all_variants() {
    // Verify all 24 variants have correct sizes
    // This test ensures no variant was missed or misconfigured

    // Float formats (4 variants)
    let float_formats = [
        (BufferFormat::R32_SFLOAT, 4),
        (BufferFormat::R32G32_SFLOAT, 8),
        (BufferFormat::R32G32B32_SFLOAT, 12),
        (BufferFormat::R32G32B32A32_SFLOAT, 16),
    ];
    for (format, expected_size) in float_formats {
        assert_eq!(format.size_bytes(), expected_size,
                   "Float format size mismatch for {:?}", format);
    }

    // Signed int formats (10 variants)
    let signed_int_formats = [
        (BufferFormat::R32_SINT, 4),
        (BufferFormat::R32G32_SINT, 8),
        (BufferFormat::R32G32B32_SINT, 12),
        (BufferFormat::R32G32B32A32_SINT, 16),
        (BufferFormat::R16_SINT, 2),
        (BufferFormat::R16G16_SINT, 4),
        (BufferFormat::R16G16B16A16_SINT, 8),
        (BufferFormat::R8_SINT, 1),
        (BufferFormat::R8G8_SINT, 2),
        (BufferFormat::R8G8B8A8_SINT, 4),
    ];
    for (format, expected_size) in signed_int_formats {
        assert_eq!(format.size_bytes(), expected_size,
                   "Signed int format size mismatch for {:?}", format);
    }

    // Unsigned int formats (10 variants)
    let unsigned_int_formats = [
        (BufferFormat::R32_UINT, 4),
        (BufferFormat::R32G32_UINT, 8),
        (BufferFormat::R32G32B32_UINT, 12),
        (BufferFormat::R32G32B32A32_UINT, 16),
        (BufferFormat::R16_UINT, 2),
        (BufferFormat::R16G16_UINT, 4),
        (BufferFormat::R16G16B16A16_UINT, 8),
        (BufferFormat::R8_UINT, 1),
        (BufferFormat::R8G8_UINT, 2),
        (BufferFormat::R8G8B8A8_UINT, 4),
    ];
    for (format, expected_size) in unsigned_int_formats {
        assert_eq!(format.size_bytes(), expected_size,
                   "Unsigned int format size mismatch for {:?}", format);
    }
}
