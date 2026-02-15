/// RenderTarget trait - represents a render target (texture or swapchain image)

use crate::renderer::TextureFormat;

/// Render target trait
///
/// Represents a surface that can be rendered to (either an offscreen texture or a swapchain image).
pub trait RenderTarget: Send + Sync {
    /// Get the width of the render target in pixels
    fn width(&self) -> u32;

    /// Get the height of the render target in pixels
    fn height(&self) -> u32;

    /// Get the pixel format of the render target
    fn format(&self) -> TextureFormat;
}
