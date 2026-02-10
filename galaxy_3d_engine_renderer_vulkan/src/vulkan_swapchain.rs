/// Swapchain - Vulkan implementation of RendererSwapchain trait

use galaxy_3d_engine::galaxy3d::{Result, Error};
use galaxy_3d_engine::galaxy3d::render::{Swapchain as RendererSwapchain, RenderTarget as RendererRenderTarget, TextureFormat};
use galaxy_3d_engine::{engine_error, engine_err};
use ash::vk;
use std::sync::Arc;

use crate::vulkan_render_target::RenderTarget;

/// Vulkan swapchain implementation
///
/// Manages presentation to the window, completely separated from rendering logic.
/// Handles image acquisition, presentation, and swapchain recreation on resize.
pub struct Swapchain {
    /// Vulkan device
    device: Arc<ash::Device>,
    /// Physical device for capabilities queries
    physical_device: vk::PhysicalDevice,

    /// Present queue
    present_queue: vk::Queue,

    /// Surface
    surface: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,

    /// Swapchain
    swapchain: vk::SwapchainKHR,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,

    /// Render targets (one per swapchain image)
    render_targets: Vec<Arc<dyn RendererRenderTarget>>,

    /// Synchronization primitives
    /// One semaphore per frame in flight (for acquire)
    image_available_semaphores: Vec<vk::Semaphore>,
    /// One semaphore per swapchain image (for present)
    render_finished_semaphores: Vec<vk::Semaphore>,

    /// Current frame in flight (0 or 1 for double buffering)
    current_frame: usize,

    /// Number of frames that can be processed concurrently
    max_frames_in_flight: usize,
}

impl Swapchain {
    /// Create a new swapchain
    ///
    /// # Arguments
    ///
    /// * `device` - Vulkan logical device
    /// * `physical_device` - Vulkan physical device
    /// * `instance` - Vulkan instance (for surface loader)
    /// * `surface` - Window surface
    /// * `surface_loader` - Surface loader
    /// * `present_queue` - Queue for presenting
    /// * `width` - Initial width
    /// * `height` - Initial height
    pub fn new(
        device: Arc<ash::Device>,
        physical_device: vk::PhysicalDevice,
        instance: &ash::Instance,
        surface: vk::SurfaceKHR,
        surface_loader: ash::khr::surface::Instance,
        present_queue: vk::Queue,
        _width: u32,
        _height: u32,
    ) -> Result<Self> {
        unsafe {
            // Query surface capabilities
            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get surface capabilities: {:?}", e);
                    Error::InitializationFailed(format!("Failed to get surface capabilities: {:?}", e))
                })?;

            // Choose surface format
            let surface_formats = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to query surface formats: {:?}", e);
                    Error::InitializationFailed(format!("Failed to get surface formats: {:?}", e))
                })?;

            let surface_format = surface_formats
                .iter()
                .find(|f| f.format == vk::Format::B8G8R8A8_SRGB || f.format == vk::Format::R8G8B8A8_SRGB)
                .unwrap_or(&surface_formats[0]);

            let swapchain_extent = surface_capabilities.current_extent;

            // Create swapchain
            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(surface)
                .min_image_count(3.min(surface_capabilities.max_image_count))
                .image_format(surface_format.format)
                .image_color_space(surface_format.color_space)
                .image_extent(swapchain_extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(surface_capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO);

            let swapchain_loader = ash::khr::swapchain::Device::new(instance, &device);
            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create swapchain: {:?}", e);
                    Error::InitializationFailed(format!("Failed to create swapchain: {:?}", e))
                })?;

            // Get swapchain images
            let swapchain_images = swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get swapchain images: {:?}", e);
                    Error::InitializationFailed(format!("Failed to get swapchain images: {:?}", e))
                })?;

            // Create swapchain image views
            let swapchain_image_views: Vec<vk::ImageView> = swapchain_images
                .iter()
                .map(|&image| {
                    let create_info = vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        });
                    device.create_image_view(&create_info, None)
                })
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create swapchain image views: {:?}", e);
                    Error::InitializationFailed(format!("Failed to create image views: {:?}", e))
                })?;

            // Create render targets
            let render_targets: Vec<Arc<dyn RendererRenderTarget>> = swapchain_image_views
                .iter()
                .map(|&image_view| {
                    Arc::new(RenderTarget::new_swapchain_target(
                        swapchain_extent.width,
                        swapchain_extent.height,
                        vk_format_to_format(surface_format.format),
                        image_view,
                    )) as Arc<dyn RendererRenderTarget>
                })
                .collect();

            // Create synchronization primitives
            let image_count = swapchain_images.len();
            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            const MAX_FRAMES_IN_FLIGHT: usize = 2;

            let mut image_available_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
            let mut render_finished_semaphores = Vec::with_capacity(image_count);

            for _ in 0..MAX_FRAMES_IN_FLIGHT {
                image_available_semaphores.push(
                    device.create_semaphore(&semaphore_create_info, None)
                        .map_err(|e| {
                            engine_error!("galaxy3d::vulkan", "Failed to create image-available semaphore: {:?}", e);
                            Error::InitializationFailed(format!("Failed to create semaphore: {:?}", e))
                        })?
                );
            }

            for _ in 0..image_count {
                render_finished_semaphores.push(
                    device.create_semaphore(&semaphore_create_info, None)
                        .map_err(|e| {
                            engine_error!("galaxy3d::vulkan", "Failed to create render-finished semaphore: {:?}", e);
                            Error::InitializationFailed(format!("Failed to create semaphore: {:?}", e))
                        })?
                );
            }

            Ok(Self {
                device,
                physical_device,
                present_queue,
                surface,
                surface_loader,
                swapchain,
                swapchain_loader,
                swapchain_images,
                swapchain_image_views,
                swapchain_format: surface_format.format,
                swapchain_extent,
                render_targets,
                image_available_semaphores,
                render_finished_semaphores,
                current_frame: 0,
                max_frames_in_flight: MAX_FRAMES_IN_FLIGHT,
            })
        }
    }

    /// Get the current frame index for synchronization
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Get the image available semaphore for the current frame (to wait on in submit)
    pub fn image_available_semaphore(&self) -> vk::Semaphore {
        self.image_available_semaphores[self.current_frame]
    }

    /// Get the render finished semaphore for a specific image (to signal in submit)
    pub fn render_finished_semaphore(&self, image_index: u32) -> vk::Semaphore {
        self.render_finished_semaphores[image_index as usize]
    }

    /// Get synchronization info for submitting with this swapchain (crate-private)
    ///
    /// Returns (wait_semaphore, signal_semaphore) for the current frame and image.
    /// This is used internally by VulkanRenderer::submit_with_swapchain().
    pub(crate) fn sync_info(&self, image_index: u32) -> (vk::Semaphore, vk::Semaphore) {
        (
            self.image_available_semaphores[self.current_frame],
            self.render_finished_semaphores[image_index as usize],
        )
    }
}

impl RendererSwapchain for Swapchain {
    fn acquire_next_image(&mut self) -> Result<(u32, Arc<dyn RendererRenderTarget>)> {
        unsafe {
            // Acquire next image
            let (image_index, _is_suboptimal) = self
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.image_available_semaphores[self.current_frame],
                    vk::Fence::null(),
                )
                .map_err(|e| {
                    if e == vk::Result::ERROR_OUT_OF_DATE_KHR {
                        engine_error!("galaxy3d::vulkan", "Swapchain out of date during acquire");
                        Error::BackendError("Swapchain out of date".to_string())
                    } else {
                        engine_error!("galaxy3d::vulkan", "Failed to acquire next swapchain image: {:?}", e);
                        Error::BackendError(format!("Failed to acquire next image: {:?}", e))
                    }
                })?;

            Ok((image_index, self.render_targets[image_index as usize].clone()))
        }
    }

    fn present(&mut self, image_index: u32) -> Result<()> {
        unsafe {
            let swapchains = [self.swapchain];
            let image_indices = [image_index];
            let wait_semaphores = [self.render_finished_semaphores[image_index as usize]];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            match self.swapchain_loader
                .queue_present(self.present_queue, &present_info) {
                    Ok(_) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        // Move to next frame
                        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
                        Ok(())
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
                        Err(engine_err!("galaxy3d::vulkan", "Swapchain out of date during present"))
                    }
                    Err(e) => {
                        Err(engine_err!("galaxy3d::vulkan", "Failed to present swapchain image: {:?}", e))
                    }
                }
        }
    }

    fn recreate(&mut self, width: u32, height: u32) -> Result<()> {
        unsafe {
            // Wait for device to be idle
            self.device.device_wait_idle()
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait idle before swapchain recreate: {:?}", e))?;

            // Destroy old image views
            for image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(*image_view, None);
            }
            self.swapchain_image_views.clear();
            self.render_targets.clear();

            // Query surface capabilities with new window size
            let surface_capabilities = self.surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get surface capabilities during swapchain recreate: {:?}", e);
                    Error::InitializationFailed(format!("Failed to get surface capabilities: {:?}", e))
                })?;

            // Choose extent
            let extent = if surface_capabilities.current_extent.width != u32::MAX {
                surface_capabilities.current_extent
            } else {
                vk::Extent2D {
                    width: width.clamp(
                        surface_capabilities.min_image_extent.width,
                        surface_capabilities.max_image_extent.width,
                    ),
                    height: height.clamp(
                        surface_capabilities.min_image_extent.height,
                        surface_capabilities.max_image_extent.height,
                    ),
                }
            };

            let image_count = surface_capabilities.min_image_count + 1;
            let image_count = if surface_capabilities.max_image_count > 0 {
                image_count.min(surface_capabilities.max_image_count)
            } else {
                image_count
            };

            // Recreate swapchain
            let old_swapchain = self.swapchain;
            let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
                .surface(self.surface)
                .min_image_count(image_count as u32)
                .image_format(self.swapchain_format)
                .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(surface_capabilities.current_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(vk::PresentModeKHR::FIFO)
                .clipped(true)
                .old_swapchain(old_swapchain);

            let swapchain = self.swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to recreate swapchain: {:?}", e);
                    Error::InitializationFailed(format!("Failed to recreate swapchain: {:?}", e))
                })?;

            // Destroy old swapchain
            self.swapchain_loader.destroy_swapchain(old_swapchain, None);
            self.swapchain = swapchain;
            self.swapchain_extent = extent;

            // Get new swapchain images
            self.swapchain_images = self.swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get swapchain images during recreate: {:?}", e);
                    Error::InitializationFailed(format!("Failed to get swapchain images: {:?}", e))
                })?;

            // Recreate image views
            for &image in &self.swapchain_images {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.swapchain_format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });

                let image_view = self.device.create_image_view(&create_info, None)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to create image view during swapchain recreate: {:?}", e);
                        Error::InitializationFailed(format!("Failed to create image view: {:?}", e))
                    })?;
                self.swapchain_image_views.push(image_view);
            }

            // Recreate render targets
            for &image_view in &self.swapchain_image_views {
                self.render_targets.push(Arc::new(RenderTarget::new_swapchain_target(
                    extent.width,
                    extent.height,
                    vk_format_to_format(self.swapchain_format),
                    image_view,
                )) as Arc<dyn RendererRenderTarget>);
            }

            Ok(())
        }
    }

    fn image_count(&self) -> usize {
        self.swapchain_images.len()
    }

    fn width(&self) -> u32 {
        self.swapchain_extent.width
    }

    fn height(&self) -> u32 {
        self.swapchain_extent.height
    }

    fn format(&self) -> TextureFormat {
        vk_format_to_format(self.swapchain_format)
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            // Wait for device to finish
            self.device.device_wait_idle().ok();

            // Destroy synchronization primitives
            for &semaphore in &self.image_available_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &semaphore in &self.render_finished_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }

            // Destroy image views
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }

            // Destroy swapchain
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);

            // Destroy surface
            self.surface_loader.destroy_surface(self.surface, None);
        }
    }
}

/// Convert Vulkan format to engine TextureFormat
fn vk_format_to_format(vk_format: vk::Format) -> TextureFormat {
    match vk_format {
        vk::Format::R8G8B8A8_SRGB => TextureFormat::R8G8B8A8_SRGB,
        vk::Format::R8G8B8A8_UNORM => TextureFormat::R8G8B8A8_UNORM,
        vk::Format::B8G8R8A8_SRGB => TextureFormat::B8G8R8A8_SRGB,
        vk::Format::B8G8R8A8_UNORM => TextureFormat::B8G8R8A8_UNORM,
        _ => TextureFormat::R8G8B8A8_SRGB, // Fallback
    }
}