/// RendererRenderTarget trait - represents a render target (texture or swapchain image)

use crate::renderer::{Format, TextureUsage};

/// Render target trait
///
/// Represents a surface that can be rendered to (either an offscreen texture or a swapchain image).
pub trait RendererRenderTarget: Send + Sync {
    /// Get the width of the render target in pixels
    fn width(&self) -> u32;

    /// Get the height of the render target in pixels
    fn height(&self) -> u32;

    /// Get the pixel format of the render target
    fn format(&self) -> Format;
}

/// Descriptor for creating a render target
#[derive(Debug, Clone)]
pub struct RendererRenderTargetDesc {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format
    pub format: Format,
    /// Usage flags
    pub usage: TextureUsage,
    /// Number of samples (1 = no MSAA, 2/4/8/etc = MSAA)
    pub samples: u32,
}
