/// VulkanRenderer - Vulkan implementation of the Renderer trait
///
/// This is the main Vulkan backend that implements the galaxy_3d_engine Renderer trait.
/// It manages all Vulkan resources and provides factory methods for creating GPU resources.

use galaxy_3d_engine::{
    Renderer, RendererTexture, RendererBuffer, RendererShader, RendererPipeline,
    TextureDesc, BufferDesc, ShaderDesc, PipelineDesc,
    RenderResult, RenderError, RendererConfig, RendererStats,
    BufferUsage, ShaderStage, PrimitiveTopology, Format,
};
use ash::vk;
use std::ffi::CString;
use std::sync::{Arc, Mutex};
use std::mem::ManuallyDrop;
use winit::window::Window;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};

use crate::vulkan_renderer_texture::VulkanRendererTexture;
use crate::vulkan_renderer_buffer::VulkanRendererBuffer;
use crate::vulkan_renderer_shader::VulkanRendererShader;
use crate::vulkan_renderer_pipeline::VulkanRendererPipeline;

/// Vulkan renderer implementation
pub struct VulkanRenderer {
    // Core Vulkan objects
    _entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: Arc<ash::Device>,

    // Queues
    graphics_queue: vk::Queue,
    graphics_queue_family: u32,
    present_queue: vk::Queue,

    // Surface and swapchain
    surface: vk::SurfaceKHR,
    surface_loader: ash::khr::surface::Instance,
    swapchain: vk::SwapchainKHR,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    swapchain_format: vk::Format,
    swapchain_extent: vk::Extent2D,

    // Memory allocator (ManuallyDrop to control destruction order)
    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,

    // Synchronization
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    current_frame: usize,

    // Command buffers
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,

    // Rendering
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,

    // Window state
    window_width: u32,
    window_height: u32,
    framebuffer_resized: bool,
}

impl VulkanRenderer {
    /// Create a new Vulkan renderer
    ///
    /// # Arguments
    ///
    /// * `window` - Window to render to
    /// * `config` - Renderer configuration
    ///
    /// # Returns
    ///
    /// A new VulkanRenderer instance
    pub fn new(window: &Window, config: RendererConfig) -> RenderResult<Self> {
        unsafe {
            // Create Entry
            let entry = ash::Entry::load()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to load Vulkan: {}", e)))?;

            // Create Instance
            let app_name = CString::new(config.app_name.as_str())
                .map_err(|e| RenderError::InitializationFailed(format!("Invalid app name: {}", e)))?;

            let app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(vk::make_api_version(
                    0,
                    config.app_version.0,
                    config.app_version.1,
                    config.app_version.2,
                ))
                .engine_name(c"Galaxy3D")
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::API_VERSION_1_3);

            // Get required extensions
            let display_handle = window.display_handle()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get display handle: {}", e)))?;
            let extension_names = ash_window::enumerate_required_extensions(display_handle.as_raw())
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get required extensions: {}", e)))?
                .to_vec();

            // Validation layers
            let layer_names = if config.enable_validation {
                vec![c"VK_LAYER_KHRONOS_validation".as_ptr()]
            } else {
                vec![]
            };

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_layer_names(&layer_names)
                .enabled_extension_names(&extension_names);

            let instance = entry
                .create_instance(&create_info, None)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create instance: {:?}", e)))?;

            // Create Surface
            let window_handle = window.window_handle()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get window handle: {}", e)))?;
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(|e| RenderError::InitializationFailed(format!("Failed to create surface: {:?}", e)))?;

            let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

            // Pick Physical Device
            let physical_devices = instance
                .enumerate_physical_devices()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to enumerate physical devices: {:?}", e)))?;

            let physical_device = physical_devices
                .into_iter()
                .next()
                .ok_or_else(|| RenderError::InitializationFailed("No Vulkan-capable GPU found".to_string()))?;

            // Find Queue Families
            let queue_families = instance.get_physical_device_queue_family_properties(physical_device);

            let graphics_family_index = queue_families
                .iter()
                .enumerate()
                .find(|(_, qf)| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .map(|(i, _)| i as u32)
                .ok_or_else(|| RenderError::InitializationFailed("No graphics queue family found".to_string()))?;

            let present_family_index = (0..queue_families.len() as u32)
                .find(|&i| {
                    surface_loader
                        .get_physical_device_surface_support(physical_device, i, surface)
                        .unwrap_or(false)
                })
                .ok_or_else(|| RenderError::InitializationFailed("No present queue family found".to_string()))?;

            // Create Logical Device
            let queue_priorities = [1.0];
            let queue_create_infos = [
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(graphics_family_index)
                    .queue_priorities(&queue_priorities),
            ];

            let device_extension_names = vec![ash::khr::swapchain::NAME.as_ptr()];

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_extension_names);

            let device = Arc::new(
                instance
                    .create_device(physical_device, &device_create_info, None)
                    .map_err(|e| RenderError::InitializationFailed(format!("Failed to create device: {:?}", e)))?,
            );

            let graphics_queue = device.get_device_queue(graphics_family_index, 0);
            let present_queue = device.get_device_queue(present_family_index, 0);

            // Create Swapchain
            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(physical_device, surface)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get surface capabilities: {:?}", e)))?;

            let surface_formats = surface_loader
                .get_physical_device_surface_formats(physical_device, surface)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get surface formats: {:?}", e)))?;

            let surface_format = surface_formats
                .iter()
                .find(|f| f.format == vk::Format::B8G8R8A8_SRGB || f.format == vk::Format::R8G8B8A8_SRGB)
                .unwrap_or(&surface_formats[0]);

            let swapchain_extent = surface_capabilities.current_extent;

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

            let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create swapchain: {:?}", e)))?;

            let swapchain_images = swapchain_loader
                .get_swapchain_images(swapchain)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to get swapchain images: {:?}", e)))?;

            // Create GPU allocator
            let allocator = Allocator::new(&AllocatorCreateDesc {
                instance: instance.clone(),
                device: (*device).clone(),
                physical_device,
                debug_settings: Default::default(),
                buffer_device_address: false,
                allocation_sizes: Default::default(),
            })
            .map_err(|e| RenderError::InitializationFailed(format!("Failed to create allocator: {:?}", e)))?;

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
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create image views: {:?}", e)))?;

            // Create synchronization primitives
            let image_count = swapchain_images.len();
            let semaphore_create_info = vk::SemaphoreCreateInfo::default();
            let fence_create_info = vk::FenceCreateInfo::default()
                .flags(vk::FenceCreateFlags::SIGNALED);

            // Use a fixed number of frames in flight for consistent synchronization
            const MAX_FRAMES_IN_FLIGHT: usize = 2;

            let mut image_available_semaphores = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);
            let mut render_finished_semaphores = Vec::with_capacity(image_count);
            let mut in_flight_fences = Vec::with_capacity(MAX_FRAMES_IN_FLIGHT);

            for _ in 0..MAX_FRAMES_IN_FLIGHT {
                image_available_semaphores.push(
                    device.create_semaphore(&semaphore_create_info, None)
                        .map_err(|e| RenderError::InitializationFailed(format!("Failed to create semaphore: {:?}", e)))?
                );
                in_flight_fences.push(
                    device.create_fence(&fence_create_info, None)
                        .map_err(|e| RenderError::InitializationFailed(format!("Failed to create fence: {:?}", e)))?
                );
            }

            for _ in 0..image_count {
                render_finished_semaphores.push(
                    device.create_semaphore(&semaphore_create_info, None)
                        .map_err(|e| RenderError::InitializationFailed(format!("Failed to create semaphore: {:?}", e)))?
                );
            }

            // Create command pool
            let command_pool_create_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let command_pool = device.create_command_pool(&command_pool_create_info, None)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create command pool: {:?}", e)))?;

            // Create command buffers (one per frame in flight)
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);

            let command_buffers = device.allocate_command_buffers(&command_buffer_allocate_info)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to allocate command buffers: {:?}", e)))?;

            // Create render pass
            let color_attachment = vk::AttachmentDescription::default()
                .format(surface_format.format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

            let color_attachment_ref = vk::AttachmentReference::default()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

            let subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(std::slice::from_ref(&color_attachment_ref));

            let dependency = vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

            let render_pass_info = vk::RenderPassCreateInfo::default()
                .attachments(std::slice::from_ref(&color_attachment))
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&dependency));

            let render_pass = device.create_render_pass(&render_pass_info, None)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create render pass: {:?}", e)))?;

            // Create framebuffers
            let framebuffers: Vec<vk::Framebuffer> = swapchain_image_views
                .iter()
                .map(|&image_view| {
                    let attachments = [image_view];
                    let framebuffer_info = vk::FramebufferCreateInfo::default()
                        .render_pass(render_pass)
                        .attachments(&attachments)
                        .width(swapchain_extent.width)
                        .height(swapchain_extent.height)
                        .layers(1);

                    device.create_framebuffer(&framebuffer_info, None)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create framebuffers: {:?}", e)))?;

            Ok(Self {
                _entry: entry,
                instance,
                physical_device,
                device,
                graphics_queue,
                graphics_queue_family: graphics_family_index,
                present_queue,
                surface,
                surface_loader,
                swapchain,
                swapchain_loader,
                swapchain_images,
                swapchain_image_views,
                swapchain_format: surface_format.format,
                swapchain_extent,
                allocator: ManuallyDrop::new(Arc::new(Mutex::new(allocator))),
                image_available_semaphores,
                render_finished_semaphores,
                in_flight_fences,
                current_frame: 0,
                command_pool,
                command_buffers,
                render_pass,
                framebuffers,
                window_width: swapchain_extent.width,
                window_height: swapchain_extent.height,
                framebuffer_resized: false,
            })
        }
    }

    /// Convert Format to Vulkan format
    fn format_to_vk(&self, format: Format) -> vk::Format {
        match format {
            Format::R8G8B8A8_SRGB => vk::Format::R8G8B8A8_SRGB,
            Format::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_UNORM,
            Format::B8G8R8A8_SRGB => vk::Format::B8G8R8A8_SRGB,
            Format::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_UNORM,
            Format::D16_UNORM => vk::Format::D16_UNORM,
            Format::D32_FLOAT => vk::Format::D32_SFLOAT,
            Format::D24_UNORM_S8_UINT => vk::Format::D24_UNORM_S8_UINT,
            Format::R32_SFLOAT => vk::Format::R32_SFLOAT,
            Format::R32G32_SFLOAT => vk::Format::R32G32_SFLOAT,
            Format::R32G32B32_SFLOAT => vk::Format::R32G32B32_SFLOAT,
            Format::R32G32B32A32_SFLOAT => vk::Format::R32G32B32A32_SFLOAT,
        }
    }

    /// Convert ShaderStage to Vulkan shader stage flags
    fn shader_stage_to_vk(&self, stage: ShaderStage) -> vk::ShaderStageFlags {
        match stage {
            ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
            ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
            ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
        }
    }

    /// Convert PrimitiveTopology to Vulkan topology
    fn topology_to_vk(&self, topology: PrimitiveTopology) -> vk::PrimitiveTopology {
        match topology {
            PrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveTopology::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
            PrimitiveTopology::LineList => vk::PrimitiveTopology::LINE_LIST,
            PrimitiveTopology::PointList => vk::PrimitiveTopology::POINT_LIST,
        }
    }
}

impl Renderer for VulkanRenderer {
    fn create_texture(&mut self, desc: TextureDesc) -> RenderResult<Arc<dyn RendererTexture>> {
        unsafe {
            let format = self.format_to_vk(desc.format);

            // Create image
            let image_create_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D {
                    width: desc.width,
                    height: desc.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = self.device.create_image(&image_create_info, None)
                .map_err(|e| RenderError::BackendError(format!("Failed to create image: {:?}", e)))?;

            // Allocate memory
            let requirements = self.device.get_image_memory_requirements(image);

            let allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "texture",
                requirements,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|_e| RenderError::OutOfMemory)?;

            // Bind memory
            self.device.bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| RenderError::BackendError(format!("Failed to bind image memory: {:?}", e)))?;

            // Create image view
            let view_create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
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

            let view = self.device.create_image_view(&view_create_info, None)
                .map_err(|e| RenderError::BackendError(format!("Failed to create image view: {:?}", e)))?;

            Ok(Arc::new(VulkanRendererTexture {
                image,
                view,
                allocation: Some(allocation),
                device: (*self.device).clone(),
                allocator: (*self.allocator).clone(),
            }))
        }
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> RenderResult<Arc<dyn RendererBuffer>> {
        unsafe {
            let usage = match desc.usage {
                BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
                BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER,
                BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
                BufferUsage::Storage => vk::BufferUsageFlags::STORAGE_BUFFER,
            };

            // Create buffer
            let buffer_create_info = vk::BufferCreateInfo::default()
                .size(desc.size)
                .usage(usage | vk::BufferUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let buffer = self.device.create_buffer(&buffer_create_info, None)
                .map_err(|e| RenderError::BackendError(format!("Failed to create buffer: {:?}", e)))?;

            // Allocate memory
            let requirements = self.device.get_buffer_memory_requirements(buffer);

            let allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "buffer",
                requirements,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|_e| RenderError::OutOfMemory)?;

            // Bind memory
            self.device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|e| RenderError::BackendError(format!("Failed to bind buffer memory: {:?}", e)))?;

            Ok(Arc::new(VulkanRendererBuffer {
                buffer,
                allocation: Some(allocation),
                size: desc.size,
                device: (*self.device).clone(),
                allocator: (*self.allocator).clone(),
            }))
        }
    }

    fn create_shader(&mut self, desc: ShaderDesc) -> RenderResult<Arc<dyn RendererShader>> {
        unsafe {
            // Ensure code is properly aligned for u32
            if desc.code.len() % 4 != 0 {
                return Err(RenderError::InvalidResource("Shader code must be aligned to 4 bytes".to_string()));
            }

            // Convert to u32 slice for SPIR-V
            let code_u32 = std::slice::from_raw_parts(
                desc.code.as_ptr() as *const u32,
                desc.code.len() / 4,
            );

            let create_info = vk::ShaderModuleCreateInfo::default()
                .code(code_u32);

            let module = self.device.create_shader_module(&create_info, None)
                .map_err(|e| RenderError::BackendError(format!("Failed to create shader module: {:?}", e)))?;

            Ok(Arc::new(VulkanRendererShader {
                module,
                stage: self.shader_stage_to_vk(desc.stage),
                entry_point: desc.entry_point.clone(),
                device: (*self.device).clone(),
            }))
        }
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> RenderResult<Arc<dyn RendererPipeline>> {
        unsafe {
            // Downcast shaders to Vulkan types
            let vertex_shader = desc.vertex_shader
                .as_ref() as *const dyn RendererShader as *const VulkanRendererShader;
            let vertex_shader = &*vertex_shader;

            let fragment_shader = desc.fragment_shader
                .as_ref() as *const dyn RendererShader as *const VulkanRendererShader;
            let fragment_shader = &*fragment_shader;

            // Create shader stage infos
            let entry_point_vert = CString::new(vertex_shader.entry_point.as_str()).unwrap();
            let entry_point_frag = CString::new(fragment_shader.entry_point.as_str()).unwrap();

            let shader_stages = [
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(vertex_shader.stage)
                    .module(vertex_shader.module)
                    .name(&entry_point_vert),
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(fragment_shader.stage)
                    .module(fragment_shader.module)
                    .name(&entry_point_frag),
            ];

            // Vertex input state
            let vertex_bindings: Vec<vk::VertexInputBindingDescription> = desc.vertex_layout.bindings
                .iter()
                .map(|binding| vk::VertexInputBindingDescription {
                    binding: binding.binding,
                    stride: binding.stride,
                    input_rate: match binding.input_rate {
                        galaxy_3d_engine::VertexInputRate::Vertex => vk::VertexInputRate::VERTEX,
                        galaxy_3d_engine::VertexInputRate::Instance => vk::VertexInputRate::INSTANCE,
                    },
                })
                .collect();

            let vertex_attributes: Vec<vk::VertexInputAttributeDescription> = desc.vertex_layout.attributes
                .iter()
                .map(|attribute| vk::VertexInputAttributeDescription {
                    location: attribute.location,
                    binding: attribute.binding,
                    format: self.format_to_vk(attribute.format),
                    offset: attribute.offset,
                })
                .collect();

            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&vertex_bindings)
                .vertex_attribute_descriptions(&vertex_attributes);

            // Input assembly state
            let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(self.topology_to_vk(desc.topology))
                .primitive_restart_enable(false);

            // Viewport state (dynamic)
            let viewports = [vk::Viewport::default()];
            let scissors = [vk::Rect2D::default()];
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewports(&viewports)
                .scissors(&scissors);

            // Rasterization state
            let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
                .depth_clamp_enable(false)
                .rasterizer_discard_enable(false)
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .depth_bias_enable(false);

            // Multisample state
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
                .sample_shading_enable(false)
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);

            // Color blend state
            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
                .color_write_mask(vk::ColorComponentFlags::RGBA)
                .blend_enable(false);

            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
                .logic_op_enable(false)
                .attachments(std::slice::from_ref(&color_blend_attachment));

            // Dynamic state
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
                .dynamic_states(&dynamic_states);

            // Pipeline layout
            let layout_create_info = vk::PipelineLayoutCreateInfo::default();
            let layout = self.device.create_pipeline_layout(&layout_create_info, None)
                .map_err(|e| RenderError::BackendError(format!("Failed to create pipeline layout: {:?}", e)))?;

            // Create pipeline
            let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state)
                .layout(layout)
                .render_pass(self.render_pass)
                .subpass(0);

            let pipelines = self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_create_info],
                None,
            )
            .map_err(|e| RenderError::BackendError(format!("Failed to create graphics pipeline: {:?}", e.1)))?;

            let pipeline = pipelines[0];

            Ok(Arc::new(VulkanRendererPipeline {
                pipeline,
                layout,
                device: (*self.device).clone(),
            }))
        }
    }

    fn begin_frame(&mut self) -> RenderResult<Arc<dyn RendererFrame>> {
        unsafe {
            // Check if framebuffer was resized - recreate swapchain
            if self.framebuffer_resized {
                self.framebuffer_resized = false;
                self.recreate_swapchain()?;
                return Err(RenderError::BackendError("Swapchain recreated, retry frame".to_string()));
            }

            // Wait for fence from previous frame
            self.device
                .wait_for_fences(
                    &[self.in_flight_fences[self.current_frame]],
                    true,
                    u64::MAX,
                )
                .map_err(|e| RenderError::BackendError(format!("Failed to wait for fence: {:?}", e)))?;

            // Reset fence
            self.device
                .reset_fences(&[self.in_flight_fences[self.current_frame]])
                .map_err(|e| RenderError::BackendError(format!("Failed to reset fence: {:?}", e)))?;

            // Acquire next image
            let (image_index, _is_suboptimal) = match self
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.image_available_semaphores[self.current_frame],
                    vk::Fence::null(),
                ) {
                    Ok(result) => result,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.framebuffer_resized = true;
                        return Err(RenderError::BackendError("Swapchain out of date".to_string()));
                    }
                    Err(e) => return Err(RenderError::BackendError(format!("Failed to acquire next image: {:?}", e))),
                };

            let image_idx = image_index as usize;

            // Reset command buffer (use current_frame, not image_idx)
            self.device
                .reset_command_buffer(
                    self.command_buffers[self.current_frame],
                    vk::CommandBufferResetFlags::empty(),
                )
                .map_err(|e| RenderError::BackendError(format!("Failed to reset command buffer: {:?}", e)))?;

            // Begin command buffer
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            self.device
                .begin_command_buffer(self.command_buffers[self.current_frame], &begin_info)
                .map_err(|e| RenderError::BackendError(format!("Failed to begin command buffer: {:?}", e)))?;

            // Begin render pass
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            }];

            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[image_idx])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain_extent,
                })
                .clear_values(&clear_values);

            self.device.cmd_begin_render_pass(
                self.command_buffers[self.current_frame],
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );

            // Set viewport and scissor
            let viewport = vk::Viewport::default()
                .x(0.0)
                .y(0.0)
                .width(self.swapchain_extent.width as f32)
                .height(self.swapchain_extent.height as f32)
                .min_depth(0.0)
                .max_depth(1.0);

            self.device.cmd_set_viewport(self.command_buffers[self.current_frame], 0, &[viewport]);

            let scissor = vk::Rect2D::default()
                .offset(vk::Offset2D { x: 0, y: 0 })
                .extent(self.swapchain_extent);

            self.device.cmd_set_scissor(self.command_buffers[self.current_frame], 0, &[scissor]);

            Ok(Arc::new(VulkanRendererFrame {
                device: (*self.device).clone(),
                command_buffer: self.command_buffers[self.current_frame],
                image_index,
            }))
        }
    }

    fn end_frame(&mut self, frame: Arc<dyn RendererFrame>) -> RenderResult<()> {
        unsafe {
            // Downcast to VulkanRendererFrame
            let vulkan_frame = frame.as_ref() as *const dyn RendererFrame as *const VulkanRendererFrame;
            let vulkan_frame = &*vulkan_frame;

            let image_idx = vulkan_frame.image_index as usize;

            // End render pass (use current_frame for command buffer)
            self.device.cmd_end_render_pass(self.command_buffers[self.current_frame]);

            // End command buffer
            self.device
                .end_command_buffer(self.command_buffers[self.current_frame])
                .map_err(|e| RenderError::BackendError(format!("Failed to end command buffer: {:?}", e)))?;

            // Submit command buffer
            let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal_semaphores = [self.render_finished_semaphores[image_idx]];
            let command_buffers = [self.command_buffers[self.current_frame]];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit_info],
                    self.in_flight_fences[self.current_frame],
                )
                .map_err(|e| RenderError::BackendError(format!("Failed to submit queue: {:?}", e)))?;

            // Present
            let swapchains = [self.swapchain];
            let image_indices = [vulkan_frame.image_index];

            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            match self.swapchain_loader
                .queue_present(self.present_queue, &present_info) {
                    Ok(_) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                        // Move to next frame
                        self.current_frame = (self.current_frame + 1) % self.image_available_semaphores.len();
                        Ok(())
                    }
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                        self.framebuffer_resized = true;
                        self.current_frame = (self.current_frame + 1) % self.image_available_semaphores.len();
                        Ok(())
                    }
                    Err(e) => Err(RenderError::BackendError(format!("Failed to present: {:?}", e))),
                }
        }
    }

    fn wait_idle(&self) -> RenderResult<()> {
        unsafe {
            self.device
                .device_wait_idle()
                .map_err(|e| RenderError::BackendError(format!("Failed to wait idle: {:?}", e)))
        }
    }

    fn stats(&self) -> RendererStats {
        RendererStats::default()
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.window_width = width;
            self.window_height = height;
            self.framebuffer_resized = true;
        }
    }
}

impl VulkanRenderer {
    /// Recreate the swapchain when window is resized
    unsafe fn recreate_swapchain(&mut self) -> RenderResult<()> {
        // Wait for device to be idle
        self.device.device_wait_idle()
            .map_err(|e| RenderError::BackendError(format!("Failed to wait idle: {:?}", e)))?;

        // Destroy old framebuffers
        for framebuffer in &self.framebuffers {
            self.device.destroy_framebuffer(*framebuffer, None);
        }
        self.framebuffers.clear();

        // Destroy old image views
        for image_view in &self.swapchain_image_views {
            self.device.destroy_image_view(*image_view, None);
        }
        self.swapchain_image_views.clear();

        // Query surface capabilities with new window size
        let surface_capabilities = self.surface_loader
            .get_physical_device_surface_capabilities(self.physical_device, self.surface)
            .map_err(|e| RenderError::InitializationFailed(format!("Failed to get surface capabilities: {:?}", e)))?;

        // Choose extent
        let extent = if surface_capabilities.current_extent.width != u32::MAX {
            surface_capabilities.current_extent
        } else {
            vk::Extent2D {
                width: self.window_width.clamp(
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.max_image_extent.width,
                ),
                height: self.window_height.clamp(
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
            .map_err(|e| RenderError::InitializationFailed(format!("Failed to recreate swapchain: {:?}", e)))?;

        // Destroy old swapchain
        self.swapchain_loader.destroy_swapchain(old_swapchain, None);
        self.swapchain = swapchain;
        self.swapchain_extent = extent;

        // Get new swapchain images
        self.swapchain_images = self.swapchain_loader
            .get_swapchain_images(swapchain)
            .map_err(|e| RenderError::InitializationFailed(format!("Failed to get swapchain images: {:?}", e)))?;

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
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create image view: {:?}", e)))?;
            self.swapchain_image_views.push(image_view);
        }

        // Recreate framebuffers
        for &image_view in &self.swapchain_image_views {
            let attachments = [image_view];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(self.render_pass)
                .attachments(&attachments)
                .width(extent.width)
                .height(extent.height)
                .layers(1);

            let framebuffer = self.device.create_framebuffer(&framebuffer_info, None)
                .map_err(|e| RenderError::InitializationFailed(format!("Failed to create framebuffer: {:?}", e)))?;
            self.framebuffers.push(framebuffer);
        }

        Ok(())
    }
}

impl Drop for VulkanRenderer {
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
            for &fence in &self.in_flight_fences {
                self.device.destroy_fence(fence, None);
            }

            // Destroy command pool (this also frees command buffers)
            self.device.destroy_command_pool(self.command_pool, None);

            // Destroy framebuffers
            for &framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            // Destroy render pass
            self.device.destroy_render_pass(self.render_pass, None);

            // Destroy image views
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }

            // Destroy swapchain
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);

            // Destroy surface
            self.surface_loader.destroy_surface(self.surface, None);

            // Drop allocator explicitly before destroying device
            // This ensures all GPU memory is freed while the device is still valid
            ManuallyDrop::drop(&mut self.allocator);

            // Destroy device
            self.device.destroy_device(None);

            // Destroy instance
            self.instance.destroy_instance(None);
        }
    }
}
