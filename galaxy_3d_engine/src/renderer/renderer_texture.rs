/// RendererTexture trait and texture descriptor

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
    /// Optional initial data (raw pixel bytes)
    /// If provided, the data must match the size: width * height * bytes_per_pixel
    pub data: Option<Vec<u8>>,
}

/// Texture resource trait
///
/// Implemented by backend-specific texture types (e.g., VulkanRendererTexture).
/// The texture is automatically destroyed when dropped.
pub trait RendererTexture: Send + Sync {
    // No public methods for now, textures are created and used by the renderer
}
