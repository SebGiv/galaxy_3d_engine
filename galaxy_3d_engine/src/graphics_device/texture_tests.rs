//! Unit tests for Texture module
//!
//! Tests TextureFormat::bytes_per_pixel() method to ensure correct size calculations
//! for all texture formats (color and depth/stencil).

#[cfg(test)]
use crate::graphics_device::TextureFormat;

// ============================================================================
// COLOR FORMATS
// ============================================================================

#[test]
fn test_texture_format_bytes_per_pixel_color_formats() {
    // All color formats are 4 bytes per pixel (RGBA8 or BGRA8)
    assert_eq!(TextureFormat::R8G8B8A8_SRGB.bytes_per_pixel(), 4);
    assert_eq!(TextureFormat::R8G8B8A8_UNORM.bytes_per_pixel(), 4);
    assert_eq!(TextureFormat::B8G8R8A8_SRGB.bytes_per_pixel(), 4);
    assert_eq!(TextureFormat::B8G8R8A8_UNORM.bytes_per_pixel(), 4);
}

// ============================================================================
// DEPTH/STENCIL FORMATS
// ============================================================================

#[test]
fn test_texture_format_bytes_per_pixel_depth_formats() {
    // D16 = 2 bytes (16-bit depth)
    assert_eq!(TextureFormat::D16_UNORM.bytes_per_pixel(), 2);

    // D32 = 4 bytes (32-bit float depth)
    assert_eq!(TextureFormat::D32_FLOAT.bytes_per_pixel(), 4);

    // D24S8 = 4 bytes (24-bit depth + 8-bit stencil)
    assert_eq!(TextureFormat::D24_UNORM_S8_UINT.bytes_per_pixel(), 4);
}

// ============================================================================
// COMPREHENSIVE TEST
// ============================================================================

#[test]
fn test_texture_format_bytes_per_pixel_all_variants() {
    // Verify all 7 variants have correct sizes
    // This test ensures no variant was missed or misconfigured

    let formats_with_sizes = [
        // Color formats (4 bytes each)
        (TextureFormat::R8G8B8A8_SRGB, 4),
        (TextureFormat::R8G8B8A8_UNORM, 4),
        (TextureFormat::B8G8R8A8_SRGB, 4),
        (TextureFormat::B8G8R8A8_UNORM, 4),

        // Depth/stencil formats
        (TextureFormat::D16_UNORM, 2),
        (TextureFormat::D32_FLOAT, 4),
        (TextureFormat::D24_UNORM_S8_UINT, 4),
    ];

    for (format, expected_size) in formats_with_sizes {
        assert_eq!(format.bytes_per_pixel(), expected_size,
                   "Texture format size mismatch for {:?}", format);
    }
}

// ============================================================================
// TEXTURE SIZE CALCULATIONS
// ============================================================================

#[test]
fn test_texture_format_total_size_calculations() {
    // Verify that bytes_per_pixel() can be used for size calculations

    // Example: 256x256 RGBA8 texture
    let width = 256u32;
    let height = 256u32;
    let format = TextureFormat::R8G8B8A8_UNORM;
    let total_bytes = width * height * format.bytes_per_pixel();
    assert_eq!(total_bytes, 262_144); // 256 * 256 * 4 = 262,144 bytes

    // Example: 512x512 D32 depth texture
    let width = 512u32;
    let height = 512u32;
    let format = TextureFormat::D32_FLOAT;
    let total_bytes = width * height * format.bytes_per_pixel();
    assert_eq!(total_bytes, 1_048_576); // 512 * 512 * 4 = 1,048,576 bytes

    // Example: 1024x768 D16 depth texture
    let width = 1024u32;
    let height = 768u32;
    let format = TextureFormat::D16_UNORM;
    let total_bytes = width * height * format.bytes_per_pixel();
    assert_eq!(total_bytes, 1_572_864); // 1024 * 768 * 2 = 1,572,864 bytes
}

// ============================================================================
// MIPMAP MODE TESTS
// ============================================================================

use crate::graphics_device::{MipmapMode, ManualMipmapData, LayerMipmapData};

#[test]
fn test_mipmap_mode_none() {
    let mode = MipmapMode::None;
    assert_eq!(mode.mip_levels(256, 256), 1);
    assert_eq!(mode.mip_levels(1024, 1024), 1);
}

#[test]
fn test_mipmap_mode_generate_full_chain() {
    let mode = MipmapMode::Generate { max_levels: None };

    // 256x256 -> 9 levels (256, 128, 64, 32, 16, 8, 4, 2, 1)
    assert_eq!(mode.mip_levels(256, 256), 9);

    // 1024x1024 -> 11 levels
    assert_eq!(mode.mip_levels(1024, 1024), 11);

    // 512x512 -> 10 levels
    assert_eq!(mode.mip_levels(512, 512), 10);

    // 1x1 -> 1 level
    assert_eq!(mode.mip_levels(1, 1), 1);

    // Non-square: 512x256 -> 10 levels (max dimension is 512)
    assert_eq!(mode.mip_levels(512, 256), 10);
}

#[test]
fn test_mipmap_mode_generate_with_max_levels() {
    // Limit to 4 levels
    let mode = MipmapMode::Generate { max_levels: Some(4) };

    // Full chain would be 9, but limited to 4
    assert_eq!(mode.mip_levels(256, 256), 4);

    // Full chain would be 11, but limited to 4
    assert_eq!(mode.mip_levels(1024, 1024), 4);

    // Full chain would be 3, not limited
    let mode = MipmapMode::Generate { max_levels: Some(10) };
    assert_eq!(mode.mip_levels(4, 4), 3);
}

#[test]
fn test_mipmap_mode_manual_single() {
    // Provide 3 manual mip levels (levels 1, 2, 3)
    let mode = MipmapMode::Manual(ManualMipmapData::Single(vec![
        vec![1, 2, 3], // level 1
        vec![4, 5],    // level 2
        vec![6],       // level 3
    ]));

    // Total = 1 (base) + 3 (manual) = 4 levels
    assert_eq!(mode.mip_levels(256, 256), 4);
}

#[test]
fn test_mipmap_mode_manual_layers() {
    // Provide manual mips for 2 layers
    let mode = MipmapMode::Manual(ManualMipmapData::Layers(vec![
        LayerMipmapData {
            layer: 0,
            mips: vec![vec![1], vec![2]], // 2 mip levels for layer 0
        },
        LayerMipmapData {
            layer: 1,
            mips: vec![vec![3], vec![4], vec![5]], // 3 mip levels for layer 1
        },
    ]));

    // Max is 3, so total = 1 + 3 = 4
    assert_eq!(mode.mip_levels(256, 256), 4);
}

#[test]
fn test_mipmap_mode_manual_empty() {
    // No manual mips
    let mode = MipmapMode::Manual(ManualMipmapData::Single(vec![]));
    assert_eq!(mode.mip_levels(256, 256), 1); // Just the base level
}

#[test]
fn test_mipmap_mode_max_mip_levels() {
    // 256x256 -> floor(log2(256)) + 1 = 8 + 1 = 9
    assert_eq!(MipmapMode::max_mip_levels(256, 256), 9);

    // 1024x1024 -> floor(log2(1024)) + 1 = 10 + 1 = 11
    assert_eq!(MipmapMode::max_mip_levels(1024, 1024), 11);

    // 512x512 -> 10
    assert_eq!(MipmapMode::max_mip_levels(512, 512), 10);

    // 1x1 -> 1
    assert_eq!(MipmapMode::max_mip_levels(1, 1), 1);

    // Non-square: use max dimension
    assert_eq!(MipmapMode::max_mip_levels(512, 256), 10); // max(512, 256) = 512
    assert_eq!(MipmapMode::max_mip_levels(1024, 512), 11); // max(1024, 512) = 1024
}

#[test]
fn test_mipmap_mode_default() {
    let mode = MipmapMode::default();
    assert_eq!(mode.mip_levels(256, 256), 1); // Default is None (1 level)
}

// ============================================================================
// TEXTURE INFO TESTS
// ============================================================================

use crate::graphics_device::{TextureInfo, TextureType, TextureUsage, SampleCount};

#[test]
fn test_texture_info_texture_type() {
    let info_2d = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 1,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };
    assert_eq!(info_2d.texture_type, TextureType::Tex2D);

    let info_array = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 4,
        mip_levels: 1,
        texture_type: TextureType::Array2D,
        sample_count: SampleCount::S1,
    };
    assert_eq!(info_array.texture_type, TextureType::Array2D);
}

#[test]
fn test_texture_info_has_mipmaps() {
    let info_no_mips = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 1,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };
    assert!(!info_no_mips.has_mipmaps());

    let info_with_mips = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 5,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };
    assert!(info_with_mips.has_mipmaps());
}

#[test]
fn test_texture_info_mip_dimensions() {
    let info = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 5,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };

    // Level 0: 256x256
    assert_eq!(info.mip_dimensions(0), Some((256, 256)));

    // Level 1: 128x128
    assert_eq!(info.mip_dimensions(1), Some((128, 128)));

    // Level 2: 64x64
    assert_eq!(info.mip_dimensions(2), Some((64, 64)));

    // Level 3: 32x32
    assert_eq!(info.mip_dimensions(3), Some((32, 32)));

    // Level 4: 16x16
    assert_eq!(info.mip_dimensions(4), Some((16, 16)));

    // Level 5: out of bounds
    assert_eq!(info.mip_dimensions(5), None);
}

#[test]
fn test_texture_info_mip_dimensions_non_square() {
    let info = TextureInfo {
        width: 512,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 4,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };

    assert_eq!(info.mip_dimensions(0), Some((512, 256)));
    assert_eq!(info.mip_dimensions(1), Some((256, 128)));
    assert_eq!(info.mip_dimensions(2), Some((128, 64)));
    assert_eq!(info.mip_dimensions(3), Some((64, 32)));
}

#[test]
fn test_texture_info_mip_dimensions_min_size() {
    let info = TextureInfo {
        width: 4,
        height: 4,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 5,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };

    // Should clamp to 1x1 minimum
    assert_eq!(info.mip_dimensions(0), Some((4, 4)));
    assert_eq!(info.mip_dimensions(1), Some((2, 2)));
    assert_eq!(info.mip_dimensions(2), Some((1, 1)));
    assert_eq!(info.mip_dimensions(3), Some((1, 1)));
    assert_eq!(info.mip_dimensions(4), Some((1, 1)));
}

#[test]
fn test_texture_info_mip_byte_size() {
    let info = TextureInfo {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM, // 4 bytes per pixel
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mip_levels: 4,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };

    // Level 0: 256x256x4 = 262,144 bytes
    assert_eq!(info.mip_byte_size(0), Some(262_144));

    // Level 1: 128x128x4 = 65,536 bytes
    assert_eq!(info.mip_byte_size(1), Some(65_536));

    // Level 2: 64x64x4 = 16,384 bytes
    assert_eq!(info.mip_byte_size(2), Some(16_384));

    // Level 3: 32x32x4 = 4,096 bytes
    assert_eq!(info.mip_byte_size(3), Some(4_096));

    // Level 4: out of bounds
    assert_eq!(info.mip_byte_size(4), None);
}

#[test]
fn test_texture_info_mip_byte_size_depth_format() {
    let info = TextureInfo {
        width: 512,
        height: 512,
        format: TextureFormat::D16_UNORM, // 2 bytes per pixel
        usage: TextureUsage::DepthStencil,
        array_layers: 1,
        mip_levels: 3,
        texture_type: TextureType::Tex2D,
        sample_count: SampleCount::S1,
    };

    // Level 0: 512x512x2 = 524,288 bytes
    assert_eq!(info.mip_byte_size(0), Some(524_288));

    // Level 1: 256x256x2 = 131,072 bytes
    assert_eq!(info.mip_byte_size(1), Some(131_072));

    // Level 2: 128x128x2 = 32,768 bytes
    assert_eq!(info.mip_byte_size(2), Some(32_768));
}

// ============================================================================
// SamplerType / TextureType / TextureUsage / TextureInfo extras
// ============================================================================

#[test]
fn test_sampler_type_equality_for_each_variant() {
    use crate::graphics_device::SamplerType;
    assert_eq!(SamplerType::LinearRepeat, SamplerType::LinearRepeat);
    assert_ne!(SamplerType::LinearRepeat, SamplerType::LinearClamp);
    assert_ne!(SamplerType::LinearClamp, SamplerType::NearestRepeat);
    assert_ne!(SamplerType::NearestRepeat, SamplerType::NearestClamp);
    assert_ne!(SamplerType::Shadow, SamplerType::Anisotropic);
}

#[test]
fn test_sampler_type_clone_copy_hash() {
    use std::collections::HashSet;
    use crate::graphics_device::SamplerType;
    let s = SamplerType::LinearRepeat;
    let t = s;
    let _u = s.clone();
    let mut set = HashSet::new();
    set.insert(s);
    set.insert(t);
    set.insert(SamplerType::Shadow);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_texture_type_clone_copy_eq() {
    use crate::graphics_device::TextureType;
    let a = TextureType::Tex2D;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(TextureType::Tex2D, TextureType::Array2D);
}

#[test]
fn test_texture_usage_clone_copy_eq() {
    use crate::graphics_device::TextureUsage;
    assert_eq!(TextureUsage::Sampled, TextureUsage::Sampled);
    assert_ne!(TextureUsage::Sampled, TextureUsage::RenderTarget);
    assert_ne!(TextureUsage::RenderTarget, TextureUsage::DepthStencil);
    assert_ne!(TextureUsage::SampledAndRenderTarget, TextureUsage::DepthStencil);
}

#[test]
fn test_texture_info_new_constructor() {
    let info = TextureInfo::new(
        128, 64,
        TextureFormat::R8G8B8A8_UNORM,
        TextureUsage::SampledAndRenderTarget,
        2, 3,
        TextureType::Array2D,
        SampleCount::S4,
    );
    assert_eq!(info.width, 128);
    assert_eq!(info.height, 64);
    assert_eq!(info.format, TextureFormat::R8G8B8A8_UNORM);
    assert_eq!(info.usage, TextureUsage::SampledAndRenderTarget);
    assert_eq!(info.array_layers, 2);
    assert_eq!(info.mip_levels, 3);
    assert_eq!(info.texture_type, TextureType::Array2D);
    assert_eq!(info.sample_count, SampleCount::S4);
}

#[test]
fn test_texture_info_clone() {
    let info = TextureInfo::new(
        16, 16, TextureFormat::R8G8B8A8_UNORM,
        TextureUsage::Sampled, 1, 1, TextureType::Tex2D, SampleCount::S1,
    );
    let cloned = info.clone();
    assert_eq!(cloned.width, info.width);
    assert_eq!(cloned.format, info.format);
}

#[test]
fn test_manual_mipmap_data_layers_with_max_chain_length() {
    use crate::graphics_device::{ManualMipmapData, LayerMipmapData, MipmapMode};
    // Layers with different mip chain lengths — max is taken.
    let mode = MipmapMode::Manual(ManualMipmapData::Layers(vec![
        LayerMipmapData { layer: 0, mips: vec![vec![0u8; 4], vec![0u8; 4]] }, // 2 mips
        LayerMipmapData { layer: 1, mips: vec![vec![0u8; 4]] }, // 1 mip
        LayerMipmapData { layer: 2, mips: vec![vec![0u8; 4], vec![0u8; 4], vec![0u8; 4]] }, // 3 mips
    ]));
    // Total = level 0 + max(2, 1, 3) = 1 + 3 = 4
    assert_eq!(mode.mip_levels(64, 64), 4);
}

#[test]
fn test_manual_mipmap_data_layers_empty() {
    use crate::graphics_device::{ManualMipmapData, LayerMipmapData, MipmapMode};
    let mode = MipmapMode::Manual(ManualMipmapData::Layers(vec![
        LayerMipmapData { layer: 0, mips: vec![] },
    ]));
    assert_eq!(mode.mip_levels(64, 64), 1);
}
