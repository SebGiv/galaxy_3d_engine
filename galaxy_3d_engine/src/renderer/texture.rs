/// Texture trait, texture descriptor, texture info, and mipmap types

use crate::error::Result;
use crate::engine_err;

/// Texture format enumeration
///
/// Defines pixel formats for textures, render targets, and depth buffers.
/// For vertex attribute formats, see `BufferFormat` in buffer.rs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum TextureFormat {
    // Color texture formats
    R8G8B8A8_SRGB,
    R8G8B8A8_UNORM,
    B8G8R8A8_SRGB,
    B8G8R8A8_UNORM,

    // Depth/stencil formats
    D16_UNORM,
    D32_FLOAT,
    D24_UNORM_S8_UINT,
    D32_FLOAT_S8_UINT,
}

impl TextureFormat {
    /// Returns bytes per pixel for this format
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            // Color formats (4 bytes per pixel)
            TextureFormat::R8G8B8A8_SRGB | TextureFormat::R8G8B8A8_UNORM |
            TextureFormat::B8G8R8A8_SRGB | TextureFormat::B8G8R8A8_UNORM => 4,

            // Depth/stencil formats
            TextureFormat::D16_UNORM => 2,
            TextureFormat::D32_FLOAT => 4,
            TextureFormat::D24_UNORM_S8_UINT => 4,
            TextureFormat::D32_FLOAT_S8_UINT => 8,
        }
    }
}

/// Texture usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureUsage {
    /// Texture can be sampled in shaders
    Sampled,
    /// Texture can be used as render target
    RenderTarget,
    /// Texture can be used for both
    SampledAndRenderTarget,
    /// Texture can be used as depth/stencil attachment
    DepthStencil,
}

// ===== TEXTURE DATA =====

/// Data for a single layer of a texture array
#[derive(Debug, Clone)]
pub struct TextureLayerData {
    /// Target layer index (0-based)
    pub layer: u32,
    /// Raw pixel bytes for this layer
    pub data: Vec<u8>,
}

/// Data to upload to a texture at creation time
#[derive(Debug, Clone)]
pub enum TextureData {
    /// Single image data (for simple textures, or layer 0 of an array)
    Single(Vec<u8>),

    /// Per-layer data for array textures.
    /// Only the layers listed are uploaded; others remain uninitialized.
    /// Allows full, partial, or empty upload.
    Layers(Vec<TextureLayerData>),
}

// ===== MIPMAP MODE =====

/// Mipmap generation mode for textures
#[derive(Debug, Clone)]
pub enum MipmapMode {
    /// No mipmaps - only base level (mip_levels = 1)
    /// Use for: UI textures, render targets, procedural textures
    None,

    /// Generate mipmaps automatically on GPU using hardware blit
    /// - Generates full chain down to 1x1 by default
    /// - Optional max_levels limits the chain length
    /// - Quality: bilinear/box filter (fast but average quality)
    Generate {
        /// Maximum mip levels to generate. None = full chain to 1x1
        max_levels: Option<u32>,
    },

    /// Manually provided mipmap data (levels 1+)
    /// Level 0 comes from TextureData
    /// Use for: pre-processed assets with high-quality mipmaps (Lanczos, Kaiser)
    Manual(ManualMipmapData),
}

/// Manual mipmap data (levels 1, 2, 3, ...)
/// Level 0 is provided via TextureData
#[derive(Debug, Clone)]
pub enum ManualMipmapData {
    /// For single/simple textures
    /// mips[0] = level 1 (half resolution)
    /// mips[1] = level 2 (quarter resolution), etc.
    Single(Vec<Vec<u8>>),

    /// For array textures - per-layer mip data
    Layers(Vec<LayerMipmapData>),
}

/// Per-layer mipmap data for array textures
#[derive(Debug, Clone)]
pub struct LayerMipmapData {
    /// Target layer index (0-based)
    pub layer: u32,
    /// Mip levels for this layer
    /// mips[0] = level 1, mips[1] = level 2, etc.
    pub mips: Vec<Vec<u8>>,
}

impl MipmapMode {
    /// Calculate actual mip levels for given dimensions
    pub fn mip_levels(&self, width: u32, height: u32) -> u32 {
        match self {
            MipmapMode::None => 1,
            MipmapMode::Generate { max_levels } => {
                let full_chain = Self::max_mip_levels(width, height);
                max_levels.map(|m| m.min(full_chain)).unwrap_or(full_chain)
            }
            MipmapMode::Manual(data) => {
                let manual_levels = match data {
                    ManualMipmapData::Single(mips) => mips.len(),
                    ManualMipmapData::Layers(layers) => {
                        layers.iter().map(|l| l.mips.len()).max().unwrap_or(0)
                    }
                };
                1 + manual_levels as u32 // Level 0 + manual levels
            }
        }
    }

    /// Calculate max possible mip levels for dimensions
    /// Returns floor(log2(max(width, height))) + 1
    pub fn max_mip_levels(width: u32, height: u32) -> u32 {
        (width.max(height) as f32).log2().floor() as u32 + 1
    }
}

impl Default for MipmapMode {
    fn default() -> Self {
        MipmapMode::None
    }
}

// ===== TEXTURE DESC =====

/// Descriptor for creating a texture
#[derive(Debug, Clone)]
pub struct TextureDesc {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format
    pub format: TextureFormat,
    /// Usage flags
    pub usage: TextureUsage,
    /// Number of array layers (1 = simple 2D texture, >1 = texture array)
    pub array_layers: u32,
    /// Optional initial data to upload at creation time (level 0)
    pub data: Option<TextureData>,
    /// Mipmap mode (default: None)
    pub mipmap: MipmapMode,
}

// ===== TEXTURE INFO =====

/// Read-only properties of a created texture.
///
/// Returned by `Texture::info()` to query texture properties
/// without exposing backend-specific details.
#[derive(Debug, Clone)]
pub struct TextureInfo {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format
    pub format: TextureFormat,
    /// Usage flags
    pub usage: TextureUsage,
    /// Number of array layers (1 = simple 2D texture, >1 = texture array)
    pub array_layers: u32,
    /// Number of mip levels (1 = no mipmaps, >1 = has mipmaps)
    pub mip_levels: u32,
}

impl TextureInfo {
    /// Returns true if this texture is a texture array (array_layers > 1)
    pub fn is_array(&self) -> bool {
        self.array_layers > 1
    }

    /// Returns true if this texture has mipmaps (mip_levels > 1)
    pub fn has_mipmaps(&self) -> bool {
        self.mip_levels > 1
    }

    /// Calculate dimensions for a specific mip level
    /// Returns None if mip_level >= mip_levels
    pub fn mip_dimensions(&self, mip_level: u32) -> Option<(u32, u32)> {
        if mip_level >= self.mip_levels {
            return None;
        }
        let w = (self.width >> mip_level).max(1);
        let h = (self.height >> mip_level).max(1);
        Some((w, h))
    }

    /// Calculate expected byte size for a specific mip level
    /// Returns None if mip_level >= mip_levels
    pub fn mip_byte_size(&self, mip_level: u32) -> Option<usize> {
        self.mip_dimensions(mip_level).map(|(w, h)| {
            (w * h * self.format.bytes_per_pixel()) as usize
        })
    }
}

// ===== TEXTURE TRAIT =====

/// Texture resource trait
///
/// Implemented by backend-specific texture types (e.g., VulkanTexture).
/// The texture is automatically destroyed when dropped.
pub trait Texture: Send + Sync {
    /// Get the read-only properties of this texture
    fn info(&self) -> &TextureInfo;

    /// Update texture data at a specific layer and mip level
    ///
    /// Uploads pixel data to a specific layer and mip level of an existing texture.
    ///
    /// Default implementation returns an error. Override in backend implementations.
    ///
    /// # Arguments
    ///
    /// * `layer` - Target layer index (0-based). Must be 0 for non-array textures.
    /// * `mip_level` - Target mip level (0 = base, 1 = half resolution, etc.)
    /// * `data` - Raw pixel data to upload (must match expected size for this mip level)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - layer >= array_layers
    /// - mip_level >= mip_levels
    /// - data size doesn't match expected size for the mip level
    fn update(&self, _layer: u32, _mip_level: u32, _data: &[u8]) -> Result<()> {
        Err(engine_err!("galaxy3d::render",
            "update not supported by this backend"))
    }
}

#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;
