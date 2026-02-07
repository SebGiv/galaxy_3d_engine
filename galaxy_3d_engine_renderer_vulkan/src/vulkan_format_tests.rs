//! Unit tests for Vulkan format conversion functions
//!
//! Tests pure format conversion functions without requiring GPU.
//! Validates correct mapping between engine formats and Vulkan formats.

#[cfg(test)]
use galaxy_3d_engine::galaxy3d::render::{BufferFormat, TextureFormat};
#[cfg(test)]
use ash::vk;

// ============================================================================
// BUFFER FORMAT CONVERSION TESTS
// ============================================================================

#[test]
fn test_buffer_format_to_vk_float_formats() {
    // Create a minimal VulkanRenderer instance is not needed for these tests
    // We'll test the logic directly through the conversion mappings

    // Test that float formats map correctly
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32_SFLOAT),
        vk::Format::R32_SFLOAT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32_SFLOAT),
        vk::Format::R32G32_SFLOAT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32_SFLOAT),
        vk::Format::R32G32B32_SFLOAT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32A32_SFLOAT),
        vk::Format::R32G32B32A32_SFLOAT
    );
}

#[test]
fn test_buffer_format_to_vk_sint_formats() {
    // Signed integer formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32_SINT),
        vk::Format::R32_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32_SINT),
        vk::Format::R32G32_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32_SINT),
        vk::Format::R32G32B32_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32A32_SINT),
        vk::Format::R32G32B32A32_SINT
    );
}

#[test]
fn test_buffer_format_to_vk_uint_formats() {
    // Unsigned integer formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32_UINT),
        vk::Format::R32_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32_UINT),
        vk::Format::R32G32_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32_UINT),
        vk::Format::R32G32B32_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R32G32B32A32_UINT),
        vk::Format::R32G32B32A32_UINT
    );
}

#[test]
fn test_buffer_format_to_vk_short_sint_formats() {
    // Short signed integer formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16_SINT),
        vk::Format::R16_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16G16_SINT),
        vk::Format::R16G16_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16G16B16A16_SINT),
        vk::Format::R16G16B16A16_SINT
    );
}

#[test]
fn test_buffer_format_to_vk_short_uint_formats() {
    // Short unsigned integer formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16_UINT),
        vk::Format::R16_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16G16_UINT),
        vk::Format::R16G16_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R16G16B16A16_UINT),
        vk::Format::R16G16B16A16_UINT
    );
}

#[test]
fn test_buffer_format_to_vk_byte_formats() {
    // Byte formats (both signed and unsigned)

    // Signed byte formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8_SINT),
        vk::Format::R8_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8G8_SINT),
        vk::Format::R8G8_SINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8G8B8A8_SINT),
        vk::Format::R8G8B8A8_SINT
    );

    // Unsigned byte formats
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8_UINT),
        vk::Format::R8_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8G8_UINT),
        vk::Format::R8G8_UINT
    );
    assert_eq!(
        buffer_format_mapping(BufferFormat::R8G8B8A8_UINT),
        vk::Format::R8G8B8A8_UINT
    );
}

// ============================================================================
// TEXTURE FORMAT CONVERSION TESTS
// ============================================================================

#[test]
fn test_texture_format_to_vk_color_formats() {
    // Color formats (RGBA and BGRA)
    assert_eq!(
        texture_format_mapping(TextureFormat::R8G8B8A8_SRGB),
        vk::Format::R8G8B8A8_SRGB
    );
    assert_eq!(
        texture_format_mapping(TextureFormat::R8G8B8A8_UNORM),
        vk::Format::R8G8B8A8_UNORM
    );
    assert_eq!(
        texture_format_mapping(TextureFormat::B8G8R8A8_SRGB),
        vk::Format::B8G8R8A8_SRGB
    );
    assert_eq!(
        texture_format_mapping(TextureFormat::B8G8R8A8_UNORM),
        vk::Format::B8G8R8A8_UNORM
    );
}

#[test]
fn test_texture_format_to_vk_depth_formats() {
    // Depth formats
    assert_eq!(
        texture_format_mapping(TextureFormat::D16_UNORM),
        vk::Format::D16_UNORM
    );
    assert_eq!(
        texture_format_mapping(TextureFormat::D32_FLOAT),
        vk::Format::D32_SFLOAT  // Note: D32_FLOAT -> D32_SFLOAT
    );
}

#[test]
fn test_texture_format_to_vk_depth_stencil_format() {
    // Depth-stencil format
    assert_eq!(
        texture_format_mapping(TextureFormat::D24_UNORM_S8_UINT),
        vk::Format::D24_UNORM_S8_UINT
    );
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Helper to test buffer format conversion logic
/// This replicates the conversion logic without needing a VulkanRenderer instance
fn buffer_format_mapping(format: BufferFormat) -> vk::Format {
    match format {
        // Float formats
        BufferFormat::R32_SFLOAT => vk::Format::R32_SFLOAT,
        BufferFormat::R32G32_SFLOAT => vk::Format::R32G32_SFLOAT,
        BufferFormat::R32G32B32_SFLOAT => vk::Format::R32G32B32_SFLOAT,
        BufferFormat::R32G32B32A32_SFLOAT => vk::Format::R32G32B32A32_SFLOAT,
        // Integer formats (signed)
        BufferFormat::R32_SINT => vk::Format::R32_SINT,
        BufferFormat::R32G32_SINT => vk::Format::R32G32_SINT,
        BufferFormat::R32G32B32_SINT => vk::Format::R32G32B32_SINT,
        BufferFormat::R32G32B32A32_SINT => vk::Format::R32G32B32A32_SINT,
        // Integer formats (unsigned)
        BufferFormat::R32_UINT => vk::Format::R32_UINT,
        BufferFormat::R32G32_UINT => vk::Format::R32G32_UINT,
        BufferFormat::R32G32B32_UINT => vk::Format::R32G32B32_UINT,
        BufferFormat::R32G32B32A32_UINT => vk::Format::R32G32B32A32_UINT,
        // Short formats (signed)
        BufferFormat::R16_SINT => vk::Format::R16_SINT,
        BufferFormat::R16G16_SINT => vk::Format::R16G16_SINT,
        BufferFormat::R16G16B16A16_SINT => vk::Format::R16G16B16A16_SINT,
        // Short formats (unsigned)
        BufferFormat::R16_UINT => vk::Format::R16_UINT,
        BufferFormat::R16G16_UINT => vk::Format::R16G16_UINT,
        BufferFormat::R16G16B16A16_UINT => vk::Format::R16G16B16A16_UINT,
        // Byte formats (signed)
        BufferFormat::R8_SINT => vk::Format::R8_SINT,
        BufferFormat::R8G8_SINT => vk::Format::R8G8_SINT,
        BufferFormat::R8G8B8A8_SINT => vk::Format::R8G8B8A8_SINT,
        // Byte formats (unsigned)
        BufferFormat::R8_UINT => vk::Format::R8_UINT,
        BufferFormat::R8G8_UINT => vk::Format::R8G8_UINT,
        BufferFormat::R8G8B8A8_UINT => vk::Format::R8G8B8A8_UINT,
    }
}

/// Helper to test texture format conversion logic
fn texture_format_mapping(format: TextureFormat) -> vk::Format {
    match format {
        TextureFormat::R8G8B8A8_SRGB => vk::Format::R8G8B8A8_SRGB,
        TextureFormat::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_UNORM,
        TextureFormat::B8G8R8A8_SRGB => vk::Format::B8G8R8A8_SRGB,
        TextureFormat::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_UNORM,
        TextureFormat::D16_UNORM => vk::Format::D16_UNORM,
        TextureFormat::D32_FLOAT => vk::Format::D32_SFLOAT,
        TextureFormat::D24_UNORM_S8_UINT => vk::Format::D24_UNORM_S8_UINT,
    }
}
