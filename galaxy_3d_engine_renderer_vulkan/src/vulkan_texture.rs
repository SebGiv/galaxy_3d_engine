/// Texture - Vulkan implementation of RendererTexture trait

use galaxy_3d_engine::galaxy3d::render::Texture as RendererTexture;
use galaxy_3d_engine::galaxy3d::render::TextureInfo;
use ash::vk;
use gpu_allocator::vulkan::Allocation;

/// Vulkan texture implementation
pub struct Texture {
    /// Vulkan image
    pub(crate) image: vk::Image,
    /// Vulkan image view
    pub(crate) view: vk::ImageView,
    /// GPU memory allocation
    pub(crate) allocation: Option<Allocation>,
    /// Vulkan device (for cleanup)
    pub(crate) device: ash::Device,
    /// GPU allocator (for cleanup)
    pub(crate) allocator: std::sync::Arc<std::sync::Mutex<gpu_allocator::vulkan::Allocator>>,
    /// Read-only texture properties
    pub(crate) info: TextureInfo,
}

impl RendererTexture for Texture {
    fn info(&self) -> &TextureInfo {
        &self.info
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            // Destroy image view
            self.device.destroy_image_view(self.view, None);

            // Free GPU memory
            if let Some(allocation) = self.allocation.take() {
                self.allocator.lock().unwrap().free(allocation).ok();
            }

            // Destroy image
            self.device.destroy_image(self.image, None);
        }
    }
}
