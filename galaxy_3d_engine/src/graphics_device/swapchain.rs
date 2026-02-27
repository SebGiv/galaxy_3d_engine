/// Swapchain trait - for window presentation

use crate::error::Result;
use crate::graphics_device::{CommandList, Texture, TextureFormat};

/// Swapchain for presenting rendered images to a window
///
/// Manages a set of images that are presented to the screen in sequence.
/// Completely separated from rendering logic.
pub trait Swapchain: Send + Sync {
    /// Acquire the next available swapchain image index
    fn acquire_next_image(&mut self) -> Result<u32>;

    /// Record a blit from the final rendered texture to a swapchain image
    ///
    /// Copies the source texture into the swapchain image at the given index,
    /// handling layout transitions and format conversion.
    /// Must be called while the command list is recording and outside a render pass.
    ///
    /// # Arguments
    ///
    /// * `cmd` - Command list to record the blit into
    /// * `src` - Source texture (the final rendered output)
    /// * `image_index` - Swapchain image index (from acquire_next_image)
    fn record_present_blit(
        &self,
        cmd: &mut dyn CommandList,
        src: &dyn Texture,
        image_index: u32,
    ) -> Result<()>;

    /// Present the rendered image to the screen
    ///
    /// # Arguments
    ///
    /// * `image_index` - Index of the image to present (from acquire_next_image)
    fn present(&mut self, image_index: u32) -> Result<()>;

    /// Recreate the swapchain (e.g., after window resize)
    ///
    /// # Arguments
    ///
    /// * `width` - New width in pixels
    /// * `height` - New height in pixels
    fn recreate(&mut self, width: u32, height: u32) -> Result<()>;

    /// Get the number of images in the swapchain
    fn image_count(&self) -> usize;

    /// Get the width of the swapchain images in pixels
    fn width(&self) -> u32;

    /// Get the height of the swapchain images in pixels
    fn height(&self) -> u32;

    /// Get the pixel format of the swapchain images
    fn format(&self) -> TextureFormat;
}
