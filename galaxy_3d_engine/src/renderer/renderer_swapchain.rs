/// RendererSwapchain trait - for window presentation

use std::sync::Arc;
use crate::renderer::{RenderResult, RendererRenderTarget, TextureFormat};

/// Swapchain for presenting rendered images to a window
///
/// Manages a set of images that are presented to the screen in sequence.
/// Completely separated from rendering logic.
pub trait RendererSwapchain: Send + Sync {
    /// Acquire the next image from the swapchain
    ///
    /// # Returns
    ///
    /// A tuple of (image_index, render_target) where:
    /// - image_index: Index of the acquired image (used for present)
    /// - render_target: The render target to render into
    fn acquire_next_image(&mut self) -> RenderResult<(u32, Arc<dyn RendererRenderTarget>)>;

    /// Present the rendered image to the screen
    ///
    /// # Arguments
    ///
    /// * `image_index` - Index of the image to present (from acquire_next_image)
    fn present(&mut self, image_index: u32) -> RenderResult<()>;

    /// Recreate the swapchain (e.g., after window resize)
    ///
    /// # Arguments
    ///
    /// * `width` - New width in pixels
    /// * `height` - New height in pixels
    fn recreate(&mut self, width: u32, height: u32) -> RenderResult<()>;

    /// Get the number of images in the swapchain
    fn image_count(&self) -> usize;

    /// Get the width of the swapchain images in pixels
    fn width(&self) -> u32;

    /// Get the height of the swapchain images in pixels
    fn height(&self) -> u32;

    /// Get the pixel format of the swapchain images
    fn format(&self) -> TextureFormat;
}
