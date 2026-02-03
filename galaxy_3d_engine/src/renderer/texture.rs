/// Texture trait, texture descriptor, and texture info

/// Texture and vertex attribute format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum TextureFormat {
    // Texture formats
    R8G8B8A8_SRGB,
    R8G8B8A8_UNORM,
    B8G8R8A8_SRGB,
    B8G8R8A8_UNORM,
    D16_UNORM,
    D32_FLOAT,
    D24_UNORM_S8_UINT,

    // Vertex attribute formats
    R32_SFLOAT,
    R32G32_SFLOAT,
    R32G32B32_SFLOAT,
    R32G32B32A32_SFLOAT,
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
    /// Optional initial data to upload at creation time
    pub data: Option<TextureData>,
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
}

impl TextureInfo {
    /// Returns true if this texture is a texture array (array_layers > 1)
    pub fn is_array(&self) -> bool {
        self.array_layers > 1
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
}
