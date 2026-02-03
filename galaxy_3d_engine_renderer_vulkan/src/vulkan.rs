/// VulkanRenderer - Vulkan implementation of Renderer trait

use galaxy_3d_engine::galaxy3d::{self, Renderer, Result, Error};
use galaxy_3d_engine::galaxy3d::render::{
    self as render_trait,
    CommandList as RendererCommandList, RenderTarget as RendererRenderTarget,
    RenderPass as RendererRenderPass, Swapchain as RendererSwapchain,
    Texture as RendererTexture, Buffer as RendererBuffer,
    Shader as RendererShader, Pipeline as RendererPipeline,
    DescriptorSet as RendererDescriptorSet,
    RenderTargetDesc, RenderPassDesc,
    TextureDesc, TextureData, TextureInfo, BufferDesc, ShaderDesc, PipelineDesc,
    TextureFormat, ShaderStage, BufferUsage, PrimitiveTopology,
    LoadOp, StoreOp, ImageLayout,
    RendererStats, VertexInputRate,
    Config, DebugSeverity, DebugOutput, DebugMessageFilter, ValidationStats, TextureUsage,
};
use ash::vk;
use ash::vk::Handle;
use std::sync::{Arc, Mutex};
use std::ffi::CString;
use std::mem::ManuallyDrop;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;
use galaxy_3d_engine::{engine_error, engine_warn};

use crate::vulkan_texture::Texture;
use crate::vulkan_buffer::Buffer;
use crate::vulkan_shader::Shader;
use crate::vulkan_pipeline::Pipeline;
use crate::vulkan_command_list::CommandList;
use crate::vulkan_render_target::RenderTarget;
use crate::vulkan_render_pass::RenderPass;
use crate::vulkan_swapchain::Swapchain;
use crate::vulkan_descriptor_set::DescriptorSet;
use crate::vulkan_context::GpuContext;

/// Vulkan device implementation
///
/// Central object for creating resources and submitting commands.
/// Completely separated from swapchain and presentation logic.
pub struct VulkanRenderer {
    /// Vulkan entry (needed for swapchain surface creation)
    _entry: ash::Entry,
    /// Vulkan instance reference (stored in GpuContext, kept here for swapchain creation)
    _instance: ash::Instance,
    /// Physical device
    physical_device: vk::PhysicalDevice,
    /// Logical device reference (stored in GpuContext, kept here for convenience)
    device: Arc<ash::Device>,

    /// Graphics queue
    graphics_queue: vk::Queue,
    graphics_queue_family: u32,
    /// Present queue (may be same as graphics)
    present_queue: vk::Queue,
    #[allow(dead_code)]
    present_queue_family: u32,

    /// GPU memory allocator reference (stored in GpuContext)
    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,

    /// Fences for submit synchronization
    submit_fences: Vec<vk::Fence>,
    current_submit_fence: usize,

    /// Descriptor pool for texture sampling
    descriptor_pool: vk::DescriptorPool,
    /// Texture sampler
    texture_sampler: vk::Sampler,
    /// Descriptor set layout
    descriptor_set_layout: vk::DescriptorSetLayout,

    /// Shared GPU context for all resources (textures, buffers)
    /// Owns device, instance, and debug messenger destruction
    gpu_context: Arc<GpuContext>,
}

impl VulkanRenderer {
    /// Submit command lists with synchronization for swapchain presentation
    ///
    /// # Arguments
    ///
    /// * `commands` - Slice of command lists to submit
    /// * `wait_semaphore` - Semaphore to wait on before execution (from swapchain)
    /// * `signal_semaphore` - Semaphore to signal after execution (for present)
    pub fn submit_with_sync(
        &self,
        commands: &[&dyn RendererCommandList],
        wait_semaphore: vk::Semaphore,
        signal_semaphore: vk::Semaphore,
    ) -> Result<()> {
        unsafe {
            // Wait for previous submit with this fence
            self.device
                .wait_for_fences(
                    &[self.submit_fences[self.current_submit_fence]],
                    true,
                    u64::MAX,
                )
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to wait for submit fence: {:?}", e);
                    Error::BackendError(format!("Failed to wait for fence: {:?}", e))
                })?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to reset submit fence: {:?}", e);
                    Error::BackendError(format!("Failed to reset fence: {:?}", e))
                })?;

            // Collect command buffers
            let command_buffers: Vec<vk::CommandBuffer> = commands
                .iter()
                .map(|cmd| {
                    let vk_cmd = *cmd as *const dyn RendererCommandList as *const CommandList;
                    (&*vk_cmd).command_buffer()
                })
                .collect();

            // Submit with synchronization
            let wait_semaphores = [wait_semaphore];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal_semaphores = [signal_semaphore];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit_info],
                    self.submit_fences[self.current_submit_fence],
                )
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to submit commands to GPU queue: {:?}", e);
                    Error::BackendError(format!("Failed to submit queue: {:?}", e))
                })?;

            Ok(())
        }
    }

    /// Create a new Vulkan device
    ///
    /// # Arguments
    ///
    /// * `window` - Window for surface creation
    /// * `config` - Renderer configuration
    pub fn new<W: HasDisplayHandle + HasWindowHandle>(
        window: &W,
        config: Config,
    ) -> Result<Self> {
        unsafe {
            // Create Vulkan Entry
            let entry = ash::Entry::load()
                .map_err(|e| Error::InitializationFailed(format!("Failed to load Vulkan library: {:?}", e)))?;

            // Application Info
            let app_info = vk::ApplicationInfo::default()
                .application_name(c"Galaxy3D Application")
                .application_version(vk::make_api_version(0, 1, 0, 0))
                .engine_name(c"Galaxy3D")
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::API_VERSION_1_3);

            // Get required extensions
            let display_handle = window.display_handle()
                .map_err(|e| Error::InitializationFailed(format!("Failed to get display handle: {}", e)))?;
            let mut extension_names = ash_window::enumerate_required_extensions(display_handle.as_raw())
                .map_err(|e| Error::InitializationFailed(format!("Failed to get required extensions: {}", e)))?
                .to_vec();

            // Add debug utils extension if validation is enabled
            if config.enable_validation {
                extension_names.push(ash::ext::debug_utils::NAME.as_ptr());
            }

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
                .map_err(|e| Error::InitializationFailed(format!("Failed to create instance: {:?}", e)))?;

            // Setup debug messenger if validation is enabled
            let (debug_utils_loader, debug_messenger) = if config.enable_validation {
                let debug_utils = ash::ext::debug_utils::Instance::new(&entry, &instance);

                // Initialize debug config
                crate::debug::init_debug_config(crate::debug::Config {
                    severity: config.debug_severity,
                    output: config.debug_output.clone(),
                    message_filter: config.debug_message_filter,
                    break_on_error: config.break_on_validation_error,
                    panic_on_error: config.panic_on_error,
                    enable_stats: config.enable_validation_stats,
                });

                // Determine severity flags based on config
                let severity_flags = match config.debug_severity {
                    DebugSeverity::ErrorsOnly => {
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    }
                    DebugSeverity::ErrorsAndWarnings => {
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    }
                    DebugSeverity::All => {
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                            | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    }
                };

                // Create debug messenger
                let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                    .message_severity(severity_flags)
                    .message_type(
                        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                            | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                            | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                    )
                    .pfn_user_callback(Some(crate::debug::vulkan_debug_callback));

                let messenger = debug_utils
                    .create_debug_utils_messenger(&debug_info, None)
                    .map_err(|e| Error::InitializationFailed(format!("Failed to create debug messenger: {:?}", e)))?;

                (Some(debug_utils), Some(messenger))
            } else {
                (None, None)
            };

            // Create Surface (temporary for queue selection)
            let window_handle = window.window_handle()
                .map_err(|e| Error::InitializationFailed(format!("Failed to get window handle: {}", e)))?;
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(|e| Error::InitializationFailed(format!("Failed to create surface: {:?}", e)))?;

            let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

            // Pick Physical Device
            let physical_devices = instance
                .enumerate_physical_devices()
                .map_err(|e| Error::InitializationFailed(format!("Failed to enumerate physical devices: {:?}", e)))?;

            let physical_device = physical_devices
                .into_iter()
                .next()
                .ok_or_else(|| Error::InitializationFailed("No Vulkan-capable GPU found".to_string()))?;

            // Find Queue Families
            let queue_families = instance.get_physical_device_queue_family_properties(physical_device);

            let graphics_family_index = queue_families
                .iter()
                .enumerate()
                .find(|(_, qf)| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .map(|(i, _)| i as u32)
                .ok_or_else(|| Error::InitializationFailed("No graphics queue family found".to_string()))?;

            let present_family_index = (0..queue_families.len() as u32)
                .find(|&i| {
                    surface_loader
                        .get_physical_device_surface_support(physical_device, i, surface)
                        .unwrap_or(false)
                })
                .ok_or_else(|| Error::InitializationFailed("No present queue family found".to_string()))?;

            // Destroy temporary surface
            surface_loader.destroy_surface(surface, None);

            // Create Logical Device
            let queue_priorities = [1.0];
            let queue_create_infos = if graphics_family_index == present_family_index {
                vec![
                    vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(graphics_family_index)
                        .queue_priorities(&queue_priorities),
                ]
            } else {
                vec![
                    vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(graphics_family_index)
                        .queue_priorities(&queue_priorities),
                    vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(present_family_index)
                        .queue_priorities(&queue_priorities),
                ]
            };

            let device_extension_names = vec![ash::khr::swapchain::NAME.as_ptr()];

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_extension_names);

            let device = Arc::new(
                instance
                    .create_device(physical_device, &device_create_info, None)
                    .map_err(|e| Error::InitializationFailed(format!("Failed to create device: {:?}", e)))?,
            );

            let graphics_queue = device.get_device_queue(graphics_family_index, 0);
            let present_queue = device.get_device_queue(present_family_index, 0);

            // Create GPU allocator
            let allocator = Allocator::new(&AllocatorCreateDesc {
                instance: instance.clone(),
                device: (*device).clone(),
                physical_device,
                debug_settings: Default::default(),
                buffer_device_address: false,
                allocation_sizes: Default::default(),
            })
            .map_err(|e| Error::InitializationFailed(format!("Failed to create allocator: {:?}", e)))?;

            // Create submit fences (2 for double buffering)
            const MAX_SUBMITS_IN_FLIGHT: usize = 2;
            let fence_create_info = vk::FenceCreateInfo::default()
                .flags(vk::FenceCreateFlags::SIGNALED);

            let mut submit_fences = Vec::with_capacity(MAX_SUBMITS_IN_FLIGHT);
            for _ in 0..MAX_SUBMITS_IN_FLIGHT {
                submit_fences.push(
                    device.create_fence(&fence_create_info, None)
                        .map_err(|e| Error::InitializationFailed(format!("Failed to create fence: {:?}", e)))?
                );
            }

            // Create descriptor pool for texture sampling (enough for 1000 sets)
            let pool_size = vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1000,
            };

            let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::default()
                .pool_sizes(std::slice::from_ref(&pool_size))
                .max_sets(1000);

            let descriptor_pool = device.create_descriptor_pool(&descriptor_pool_create_info, None)
                .map_err(|e| Error::InitializationFailed(format!("Failed to create descriptor pool: {:?}", e)))?;

            // Create texture sampler with linear filtering and repeat addressing
            let sampler_create_info = vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(false)
                .max_anisotropy(1.0)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .compare_op(vk::CompareOp::ALWAYS)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .mip_lod_bias(0.0)
                .min_lod(0.0)
                .max_lod(0.0);

            let texture_sampler = device.create_sampler(&sampler_create_info, None)
                .map_err(|e| Error::InitializationFailed(format!("Failed to create sampler: {:?}", e)))?;

            // Create descriptor set layout with binding 0 for COMBINED_IMAGE_SAMPLER in fragment shader
            let sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(std::slice::from_ref(&sampler_binding));

            let descriptor_set_layout = device.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .map_err(|e| Error::InitializationFailed(format!("Failed to create descriptor set layout: {:?}", e)))?;

            // Create upload command pool (TRANSIENT + RESET for reusable one-shot uploads)
            let upload_pool_create_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_family_index)
                .flags(vk::CommandPoolCreateFlags::TRANSIENT | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let upload_command_pool = device.create_command_pool(&upload_pool_create_info, None)
                .map_err(|e| Error::InitializationFailed(format!("Failed to create upload command pool: {:?}", e)))?;

            // Create shared GPU context for all resources
            // GpuContext owns device, instance, and debug messenger destruction
            let allocator_arc = Arc::new(Mutex::new(allocator));
            let gpu_context = Arc::new(GpuContext::new(
                (*device).clone(),
                Arc::clone(&allocator_arc),
                graphics_queue,
                graphics_family_index,
                upload_command_pool,
                instance.clone(),
                debug_utils_loader,
                debug_messenger,
            ));

            Ok(Self {
                _entry: entry,
                _instance: instance,
                physical_device,
                device,
                graphics_queue,
                graphics_queue_family: graphics_family_index,
                present_queue,
                present_queue_family: present_family_index,
                allocator: ManuallyDrop::new(allocator_arc),
                submit_fences,
                current_submit_fence: 0,
                descriptor_pool,
                texture_sampler,
                descriptor_set_layout,
                gpu_context,
            })
        }
    }

    /// Convert TextureFormat to Vulkan format
    fn format_to_vk(&self, format: TextureFormat) -> vk::Format {
        match format {
            TextureFormat::R8G8B8A8_SRGB => vk::Format::R8G8B8A8_SRGB,
            TextureFormat::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_UNORM,
            TextureFormat::B8G8R8A8_SRGB => vk::Format::B8G8R8A8_SRGB,
            TextureFormat::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_UNORM,
            TextureFormat::D16_UNORM => vk::Format::D16_UNORM,
            TextureFormat::D32_FLOAT => vk::Format::D32_SFLOAT,
            TextureFormat::D24_UNORM_S8_UINT => vk::Format::D24_UNORM_S8_UINT,
            TextureFormat::R32_SFLOAT => vk::Format::R32_SFLOAT,
            TextureFormat::R32G32_SFLOAT => vk::Format::R32G32_SFLOAT,
            TextureFormat::R32G32B32_SFLOAT => vk::Format::R32G32B32_SFLOAT,
            TextureFormat::R32G32B32A32_SFLOAT => vk::Format::R32G32B32A32_SFLOAT,
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

    /// Convert LoadOp to Vulkan
    fn load_op_to_vk(&self, load_op: LoadOp) -> vk::AttachmentLoadOp {
        match load_op {
            LoadOp::Load => vk::AttachmentLoadOp::LOAD,
            LoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
            LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
        }
    }

    /// Convert StoreOp to Vulkan
    fn store_op_to_vk(&self, store_op: StoreOp) -> vk::AttachmentStoreOp {
        match store_op {
            StoreOp::Store => vk::AttachmentStoreOp::STORE,
            StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
        }
    }

    /// Convert ImageLayout to Vulkan
    fn image_layout_to_vk(&self, layout: ImageLayout) -> vk::ImageLayout {
        match layout {
            ImageLayout::Undefined => vk::ImageLayout::UNDEFINED,
            ImageLayout::ColorAttachment => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ImageLayout::DepthStencilAttachment => vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ImageLayout::ShaderReadOnly => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageLayout::TransferSrc => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            ImageLayout::TransferDst => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ImageLayout::PresentSrc => vk::ImageLayout::PRESENT_SRC_KHR,
        }
    }

    /// Create a Vulkan swapchain (returns concrete type for Vulkan-specific methods)
    ///
    /// # Arguments
    ///
    /// * `window` - Window to create swapchain for
    pub fn create_vulkan_swapchain(&self, window: &Window) -> Result<Swapchain> {
        // Get window size
        // TODO: Pass width/height as parameters
        let width = 800; // Temporary default
        let height = 600;

        // Create surface
        let display_handle = window.display_handle()
            .map_err(|e| Error::InitializationFailed(format!("Failed to get display handle: {}", e)))?;
        let window_handle = window.window_handle()
            .map_err(|e| Error::InitializationFailed(format!("Failed to get window handle: {}", e)))?;

        let surface = unsafe {
            ash_window::create_surface(
                &self._entry,
                &self._instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(|e| Error::InitializationFailed(format!("Failed to create surface: {:?}", e)))?
        };

        let surface_loader = ash::khr::surface::Instance::new(&self._entry, &self._instance);

        Swapchain::new(
            self.device.clone(),
            self.physical_device,
            &self._instance,
            surface,
            surface_loader,
            self.present_queue,
            width,
            height,
        )
    }

    /// Get the descriptor set layout for texture sampling
    ///
    /// # Returns
    ///
    /// Get descriptor set layout (crate-private)
    ///
    /// The descriptor set layout with binding 0 for COMBINED_IMAGE_SAMPLER in fragment shader stage.
    /// This is used internally for pipeline creation and descriptor set allocation.
    #[allow(dead_code)]
    pub(crate) fn get_descriptor_set_layout(&self) -> vk::DescriptorSetLayout {
        self.descriptor_set_layout
    }

    /// Get descriptor set layout as u64 for pipeline creation (public API)
    ///
    /// Returns the descriptor set layout handle as a u64, which can be passed to PipelineDesc.
    /// This avoids exposing Vulkan types in the public API.
    pub fn get_descriptor_set_layout_handle(&self) -> u64 {
        Handle::as_raw(self.descriptor_set_layout)
    }

    /// Create a descriptor set for a texture
    ///
    /// Allocates a descriptor set from the pool and updates it to point to the given texture.
    ///
    /// # Arguments
    ///
    /// * `texture` - The texture to bind to the descriptor set
    ///
    /// # Returns
    ///
    /// A descriptor set that can be used in shaders to sample the texture
    pub fn create_texture_descriptor_set(&self, texture: &Texture) -> Result<vk::DescriptorSet> {
        unsafe {
            // Allocate descriptor set
            let layouts = [self.descriptor_set_layout];
            let allocate_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(&layouts);

            let descriptor_sets = self.device.allocate_descriptor_sets(&allocate_info)
                .map_err(|e| Error::BackendError(format!("Failed to allocate descriptor set: {:?}", e)))?;

            let descriptor_set = descriptor_sets[0];

            // Update descriptor set to point to the texture
            let image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(texture.view)
                .sampler(self.texture_sampler);

            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info));

            self.device.update_descriptor_sets(std::slice::from_ref(&descriptor_write), &[]);

            Ok(descriptor_set)
        }
    }
}

impl Renderer for VulkanRenderer {
    fn create_command_list(&self) -> Result<Box<dyn RendererCommandList>> {
        let cmd_list = CommandList::new(
            self.device.clone(),
            self.graphics_queue_family,
        )?;
        Ok(Box::new(cmd_list))
    }

    fn create_render_target(&self, desc: &RenderTargetDesc) -> Result<Arc<dyn RendererRenderTarget>> {
        unsafe {
            let format = self.format_to_vk(desc.format);

            // Determine usage flags
            let mut usage_flags = vk::ImageUsageFlags::empty();
            match desc.usage {
                TextureUsage::Sampled => {
                    usage_flags |= vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
                }
                TextureUsage::RenderTarget => {
                    usage_flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
                }
                TextureUsage::SampledAndRenderTarget => {
                    usage_flags |= vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST;
                }
                TextureUsage::DepthStencil => {
                    usage_flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
                }
            }

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
                .samples(match desc.samples {
                    1 => vk::SampleCountFlags::TYPE_1,
                    2 => vk::SampleCountFlags::TYPE_2,
                    4 => vk::SampleCountFlags::TYPE_4,
                    8 => vk::SampleCountFlags::TYPE_8,
                    _ => vk::SampleCountFlags::TYPE_1,
                })
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage_flags)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = self.device.create_image(&image_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create image for render target: {:?}", e);
                    Error::BackendError(format!("Failed to create image: {:?}", e))
                })?;

            // Allocate memory
            let requirements = self.device.get_image_memory_requirements(image);

            let allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "render_target",
                requirements,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|_e| {
                let size_mb = requirements.size as f64 / (1024.0 * 1024.0);
                engine_error!("galaxy3d::vulkan", "Out of GPU memory for render target (required: {:.2} MB)", size_mb);
                Error::OutOfMemory
            })?;

            // Bind memory
            self.device.bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to bind image memory for render target: {:?}", e);
                    Error::BackendError(format!("Failed to bind image memory: {:?}", e))
                })?;

            // Create image view
            let aspect_mask = if matches!(desc.usage, TextureUsage::DepthStencil) {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            };

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
                    aspect_mask,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let view = self.device.create_image_view(&view_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create image view for render target: {:?}", e);
                    Error::BackendError(format!("Failed to create image view: {:?}", e))
                })?;

            Ok(Arc::new(RenderTarget::new_texture_target(
                desc.width,
                desc.height,
                desc.format,
                view,
                (*self.device).clone(),
            )))
        }
    }

    fn create_render_pass(&self, desc: &RenderPassDesc) -> Result<Arc<dyn RendererRenderPass>> {
        unsafe {
            // Convert attachment descriptions
            let mut attachments = Vec::new();
            let mut color_attachment_refs = Vec::new();
            let mut depth_attachment_ref: Option<vk::AttachmentReference> = None;

            for (i, color_attachment) in desc.color_attachments.iter().enumerate() {
                attachments.push(vk::AttachmentDescription::default()
                    .format(self.format_to_vk(color_attachment.format))
                    .samples(match color_attachment.samples {
                        1 => vk::SampleCountFlags::TYPE_1,
                        2 => vk::SampleCountFlags::TYPE_2,
                        4 => vk::SampleCountFlags::TYPE_4,
                        8 => vk::SampleCountFlags::TYPE_8,
                        _ => vk::SampleCountFlags::TYPE_1,
                    })
                    .load_op(self.load_op_to_vk(color_attachment.load_op))
                    .store_op(self.store_op_to_vk(color_attachment.store_op))
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(self.image_layout_to_vk(color_attachment.initial_layout))
                    .final_layout(self.image_layout_to_vk(color_attachment.final_layout)));

                color_attachment_refs.push(vk::AttachmentReference::default()
                    .attachment(i as u32)
                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL));
            }

            if let Some(depth_attachment) = &desc.depth_attachment {
                let depth_index = attachments.len() as u32;
                attachments.push(vk::AttachmentDescription::default()
                    .format(self.format_to_vk(depth_attachment.format))
                    .samples(match depth_attachment.samples {
                        1 => vk::SampleCountFlags::TYPE_1,
                        2 => vk::SampleCountFlags::TYPE_2,
                        4 => vk::SampleCountFlags::TYPE_4,
                        8 => vk::SampleCountFlags::TYPE_8,
                        _ => vk::SampleCountFlags::TYPE_1,
                    })
                    .load_op(self.load_op_to_vk(depth_attachment.load_op))
                    .store_op(self.store_op_to_vk(depth_attachment.store_op))
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(self.image_layout_to_vk(depth_attachment.initial_layout))
                    .final_layout(self.image_layout_to_vk(depth_attachment.final_layout)));

                depth_attachment_ref = Some(vk::AttachmentReference::default()
                    .attachment(depth_index)
                    .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL));
            }

            // Create subpass
            let mut subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(&color_attachment_refs);

            if let Some(ref depth_ref) = depth_attachment_ref {
                subpass = subpass.depth_stencil_attachment(depth_ref);
            }

            // Subpass dependency
            let dependency = vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

            // Create render pass
            let render_pass_info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&dependency));

            let render_pass = self.device.create_render_pass(&render_pass_info, None)
                .map_err(|e| Error::BackendError(format!("Failed to create render pass: {:?}", e)))?;

            Ok(Arc::new(RenderPass {
                render_pass,
                device: (*self.device).clone(),
            }))
        }
    }

    fn create_swapchain(&self, window: &Window) -> Result<Box<dyn RendererSwapchain>> {
        let swapchain = self.create_vulkan_swapchain(window)?;
        Ok(Box::new(swapchain))
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<Arc<dyn RendererTexture>> {
        unsafe {
            let format = self.format_to_vk(desc.format);
            let array_layers = desc.array_layers.max(1);

            // Determine image view type based on array layers
            let view_type = if array_layers > 1 {
                vk::ImageViewType::TYPE_2D_ARRAY
            } else {
                vk::ImageViewType::TYPE_2D
            };

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
                .array_layers(array_layers)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = self.device.create_image(&image_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create texture image: {:?}", e);
                    Error::BackendError(format!("Failed to create image: {:?}", e))
                })?;

            // Allocate memory
            let requirements = self.device.get_image_memory_requirements(image);

            let allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "texture",
                requirements,
                location: gpu_allocator::MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|_e| {
                let size_mb = requirements.size as f64 / (1024.0 * 1024.0);
                engine_error!("galaxy3d::vulkan", "Out of GPU memory for texture (size: {}x{}, layers: {}, {:.2} MB)", desc.width, desc.height, array_layers, size_mb);
                Error::OutOfMemory
            })?;

            // Bind memory
            self.device.bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to bind texture image memory: {:?}", e);
                    Error::BackendError(format!("Failed to bind image memory: {:?}", e))
                })?;

            // Create image view
            let view_create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(view_type)
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
                    layer_count: array_layers,
                });

            let view = self.device.create_image_view(&view_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create texture image view: {:?}", e);
                    Error::BackendError(format!("Failed to create image view: {:?}", e))
                })?;

            // Collect upload items: Vec<(layer_index, &[u8])>
            let upload_items: Vec<(u32, &[u8])> = match &desc.data {
                Some(TextureData::Single(data)) => {
                    vec![(0, data.as_slice())]
                }
                Some(TextureData::Layers(layers)) => {
                    // Validate layer indices
                    for layer_data in layers {
                        if layer_data.layer >= array_layers {
                            engine_error!("galaxy3d::vulkan", "Layer index {} exceeds array_layers {}", layer_data.layer, array_layers);
                            return Err(Error::BackendError(format!(
                                "Layer index {} out of range (array_layers = {})", layer_data.layer, array_layers
                            )));
                        }
                    }
                    layers.iter().map(|ld| (ld.layer, ld.data.as_slice())).collect()
                }
                None => {
                    vec![]
                }
            };

            let has_data = !upload_items.is_empty();

            if has_data {
                // Create a one-time command buffer for upload
                let command_pool_create_info = vk::CommandPoolCreateInfo::default()
                    .queue_family_index(self.graphics_queue_family)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT);

                let command_pool = self.device.create_command_pool(&command_pool_create_info, None)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to create command pool for texture upload: {:?}", e);
                        Error::BackendError(format!("Failed to create command pool: {:?}", e))
                    })?;

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);

                let command_buffers = self.device.allocate_command_buffers(&command_buffer_allocate_info)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to allocate command buffer for texture upload: {:?}", e);
                        Error::BackendError(format!("Failed to allocate command buffers: {:?}", e))
                    })?;
                let command_buffer = command_buffers[0];

                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

                self.device.begin_command_buffer(command_buffer, &begin_info)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to begin command buffer for texture upload: {:?}", e);
                        Error::BackendError(format!("Failed to begin command buffer: {:?}", e))
                    })?;

                // Transition all layers: UNDEFINED → TRANSFER_DST_OPTIMAL
                let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: array_layers,
                    })
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

                self.device.cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_to_transfer],
                );

                // Upload each layer with its own staging buffer
                let mut staging_buffers: Vec<(vk::Buffer, gpu_allocator::vulkan::Allocation)> = Vec::new();

                for (layer_index, data) in &upload_items {
                    let buffer_size = data.len() as u64;

                    // Create staging buffer
                    let staging_buffer_create_info = vk::BufferCreateInfo::default()
                        .size(buffer_size)
                        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE);

                    let staging_buffer = self.device.create_buffer(&staging_buffer_create_info, None)
                        .map_err(|e| {
                            engine_error!("galaxy3d::vulkan", "Failed to create staging buffer for layer {}: {:?}", layer_index, e);
                            Error::BackendError(format!("Failed to create staging buffer: {:?}", e))
                        })?;

                    let staging_requirements = self.device.get_buffer_memory_requirements(staging_buffer);

                    let staging_allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                        name: "texture_staging_buffer",
                        requirements: staging_requirements,
                        location: gpu_allocator::MemoryLocation::CpuToGpu,
                        linear: true,
                        allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                    })
                    .map_err(|_e| {
                        let size_mb = staging_requirements.size as f64 / (1024.0 * 1024.0);
                        engine_error!("galaxy3d::vulkan", "Out of GPU memory for texture staging buffer layer {} ({:.2} MB)", layer_index, size_mb);
                        Error::OutOfMemory
                    })?;

                    self.device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
                        .map_err(|e| {
                            engine_error!("galaxy3d::vulkan", "Failed to bind staging buffer memory for layer {}: {:?}", layer_index, e);
                            Error::BackendError(format!("Failed to bind staging buffer memory: {:?}", e))
                        })?;

                    // Copy data to staging buffer
                    let mapped_ptr = staging_allocation.mapped_ptr()
                        .ok_or_else(|| Error::BackendError("Staging buffer is not mapped".to_string()))?
                        .as_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr, data.len());

                    // Record copy command for this layer
                    let region = vk::BufferImageCopy::default()
                        .buffer_offset(0)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(vk::ImageSubresourceLayers {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            mip_level: 0,
                            base_array_layer: *layer_index,
                            layer_count: 1,
                        })
                        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                        .image_extent(vk::Extent3D {
                            width: desc.width,
                            height: desc.height,
                            depth: 1,
                        });

                    self.device.cmd_copy_buffer_to_image(
                        command_buffer,
                        staging_buffer,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[region],
                    );

                    staging_buffers.push((staging_buffer, staging_allocation));
                }

                // Transition all layers: TRANSFER_DST_OPTIMAL → SHADER_READ_ONLY_OPTIMAL
                let barrier_to_shader = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: array_layers,
                    })
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ);

                self.device.cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier_to_shader],
                );

                // End recording, submit, and wait
                self.device.end_command_buffer(command_buffer)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to end command buffer for texture upload: {:?}", e);
                        Error::BackendError(format!("Failed to end command buffer: {:?}", e))
                    })?;

                let command_buffers_submit = [command_buffer];
                let submit_info = vk::SubmitInfo::default()
                    .command_buffers(&command_buffers_submit);

                self.device.queue_submit(self.graphics_queue, &[submit_info], vk::Fence::null())
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to submit texture upload commands to GPU: {:?}", e);
                        Error::BackendError(format!("Failed to submit command buffer: {:?}", e))
                    })?;

                self.device.queue_wait_idle(self.graphics_queue)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to wait for texture upload completion: {:?}", e);
                        Error::BackendError(format!("Failed to wait for queue idle: {:?}", e))
                    })?;

                // Clean up staging buffers and command pool
                self.device.destroy_command_pool(command_pool, None);
                for (staging_buf, staging_alloc) in staging_buffers {
                    self.device.destroy_buffer(staging_buf, None);
                    self.allocator.lock().unwrap().free(staging_alloc)
                        .map_err(|_e| Error::BackendError("Failed to free staging buffer allocation".to_string()))?;
                }
            } else {
                // No data to upload — transition directly to SHADER_READ_ONLY_OPTIMAL
                let command_pool_create_info = vk::CommandPoolCreateInfo::default()
                    .queue_family_index(self.graphics_queue_family)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT);

                let command_pool = self.device.create_command_pool(&command_pool_create_info, None)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to create command pool for layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to create command pool: {:?}", e))
                    })?;

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);

                let command_buffers = self.device.allocate_command_buffers(&command_buffer_allocate_info)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to allocate command buffer for layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to allocate command buffers: {:?}", e))
                    })?;
                let command_buffer = command_buffers[0];

                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

                self.device.begin_command_buffer(command_buffer, &begin_info)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to begin command buffer for layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to begin command buffer: {:?}", e))
                    })?;

                let barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: array_layers,
                    })
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::SHADER_READ);

                self.device.cmd_pipeline_barrier(
                    command_buffer,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[barrier],
                );

                self.device.end_command_buffer(command_buffer)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to end command buffer for layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to end command buffer: {:?}", e))
                    })?;

                let command_buffers_submit = [command_buffer];
                let submit_info = vk::SubmitInfo::default()
                    .command_buffers(&command_buffers_submit);

                self.device.queue_submit(self.graphics_queue, &[submit_info], vk::Fence::null())
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to submit layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to submit command buffer: {:?}", e))
                    })?;

                self.device.queue_wait_idle(self.graphics_queue)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to wait for layout transition: {:?}", e);
                        Error::BackendError(format!("Failed to wait for queue idle: {:?}", e))
                    })?;

                self.device.destroy_command_pool(command_pool, None);
            }

            // Build TextureInfo from the descriptor
            let info = TextureInfo {
                width: desc.width,
                height: desc.height,
                format: desc.format,
                usage: desc.usage,
                array_layers,
            };

            Ok(Arc::new(Texture::new(
                Arc::clone(&self.gpu_context),
                image,
                view,
                allocation,
                info,
            )))
        }
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<Arc<dyn RendererBuffer>> {
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
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create buffer of size {} bytes: {:?}", desc.size, e);
                    Error::BackendError(format!("Failed to create buffer: {:?}", e))
                })?;

            // Allocate memory
            let requirements = self.device.get_buffer_memory_requirements(buffer);

            let allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                name: "buffer",
                requirements,
                location: gpu_allocator::MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|_e| {
                let size_mb = requirements.size as f64 / (1024.0 * 1024.0);
                engine_error!("galaxy3d::vulkan", "Out of GPU memory for buffer (required: {:.2} MB)", size_mb);
                Error::OutOfMemory
            })?;

            // Bind memory
            self.device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to bind buffer memory: {:?}", e);
                    Error::BackendError(format!("Failed to bind buffer memory: {:?}", e))
                })?;

            Ok(Arc::new(Buffer::new(
                Arc::clone(&self.gpu_context),
                buffer,
                allocation,
                desc.size,
            )))
        }
    }

    fn create_shader(&mut self, desc: ShaderDesc) -> Result<Arc<dyn RendererShader>> {
        unsafe {
            // Ensure code is properly aligned for u32
            if desc.code.len() % 4 != 0 {
                engine_warn!("galaxy3d::vulkan", "Shader code not 4-byte aligned (size: {} bytes)", desc.code.len());
                return Err(Error::InvalidResource("Shader code must be aligned to 4 bytes".to_string()));
            }

            // Convert to u32 slice for SPIR-V
            let code_u32 = std::slice::from_raw_parts(
                desc.code.as_ptr() as *const u32,
                desc.code.len() / 4,
            );

            let create_info = vk::ShaderModuleCreateInfo::default()
                .code(code_u32);

            let module = self.device.create_shader_module(&create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create shader module: {:?}", e);
                    Error::BackendError(format!("Failed to create shader module: {:?}", e))
                })?;

            Ok(Arc::new(Shader {
                module,
                stage: self.shader_stage_to_vk(desc.stage),
                entry_point: desc.entry_point.clone(),
                device: (*self.device).clone(),
            }))
        }
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<Arc<dyn RendererPipeline>> {
        // NOTE: This implementation is temporary and creates a hardcoded render pass
        // In the new architecture, pipelines should be created with a specific render pass
        // For now, we create a simple render pass for compatibility

        unsafe {
            // Create a temporary render pass for pipeline creation
            // TODO: Pipeline should be created with an explicit render pass
            let color_attachment = vk::AttachmentDescription::default()
                .format(vk::Format::B8G8R8A8_SRGB) // Hardcoded for now
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

            let temp_render_pass = self.device.create_render_pass(&render_pass_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create temporary render pass for pipeline: {:?}", e);
                    Error::BackendError(format!("Failed to create render pass: {:?}", e))
                })?;

            // Downcast shaders to Vulkan types
            let vertex_shader = desc.vertex_shader
                .as_ref() as *const dyn RendererShader as *const Shader;
            let vertex_shader = &*vertex_shader;

            let fragment_shader = desc.fragment_shader
                .as_ref() as *const dyn RendererShader as *const Shader;
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
                        VertexInputRate::Vertex => vk::VertexInputRate::VERTEX,
                        VertexInputRate::Instance => vk::VertexInputRate::INSTANCE,
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
            let color_blend_attachment = if desc.enable_blending {
                // Alpha blending: source * src_alpha + destination * (1 - src_alpha)
                vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA)
                    .blend_enable(true)
                    .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                    .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                    .color_blend_op(vk::BlendOp::ADD)
                    .src_alpha_blend_factor(vk::BlendFactor::ONE)
                    .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                    .alpha_blend_op(vk::BlendOp::ADD)
            } else {
                vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(vk::ColorComponentFlags::RGBA)
                    .blend_enable(false)
            };

            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
                .logic_op_enable(false)
                .attachments(std::slice::from_ref(&color_blend_attachment));

            // Dynamic state
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
                .dynamic_states(&dynamic_states);

            // Pipeline layout with push constants
            let push_constant_ranges: Vec<vk::PushConstantRange> = desc.push_constant_ranges
                .iter()
                .map(|range| {
                    let mut stage_flags = vk::ShaderStageFlags::empty();
                    for stage in &range.stages {
                        stage_flags |= match stage {
                            ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
                            ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
                            ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
                        };
                    }
                    vk::PushConstantRange {
                        stage_flags,
                        offset: range.offset,
                        size: range.size,
                    }
                })
                .collect();

            // Convert u64 descriptor set layouts back to vk::DescriptorSetLayout
            let descriptor_set_layouts: Vec<vk::DescriptorSetLayout> = desc.descriptor_set_layouts
                .iter()
                .map(|&layout| vk::DescriptorSetLayout::from_raw(layout))
                .collect();

            let mut layout_create_info = vk::PipelineLayoutCreateInfo::default();

            // Add descriptor set layouts if present
            if !descriptor_set_layouts.is_empty() {
                layout_create_info = layout_create_info.set_layouts(&descriptor_set_layouts);
            }

            // Add push constant ranges if present
            if !push_constant_ranges.is_empty() {
                layout_create_info = layout_create_info.push_constant_ranges(&push_constant_ranges);
            }

            let layout = self.device.create_pipeline_layout(&layout_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create pipeline layout: {:?}", e);
                    Error::BackendError(format!("Failed to create pipeline layout: {:?}", e))
                })?;

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
                .render_pass(temp_render_pass) // Using temporary render pass
                .subpass(0);

            let pipelines = self.device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[pipeline_create_info],
                None,
            )
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to create graphics pipeline: {:?}", e.1);
                Error::BackendError(format!("Failed to create graphics pipeline: {:?}", e.1))
            })?;

            let pipeline = pipelines[0];

            // Destroy temporary render pass
            self.device.destroy_render_pass(temp_render_pass, None);

            Ok(Arc::new(Pipeline {
                pipeline,
                pipeline_layout: layout,
                device: (*self.device).clone(),
            }))
        }
    }

    fn submit(&self, commands: &[&dyn RendererCommandList]) -> Result<()> {
        unsafe {
            // Wait for previous submit with this fence
            self.device
                .wait_for_fences(
                    &[self.submit_fences[self.current_submit_fence]],
                    true,
                    u64::MAX,
                )
                .map_err(|e| Error::BackendError(format!("Failed to wait for fence: {:?}", e)))?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| Error::BackendError(format!("Failed to reset fence: {:?}", e)))?;

            // Collect command buffers
            let command_buffers: Vec<vk::CommandBuffer> = commands
                .iter()
                .map(|cmd| {
                    let vk_cmd = *cmd as *const dyn RendererCommandList as *const CommandList;
                    (&*vk_cmd).command_buffer()
                })
                .collect();

            // Submit
            let submit_info = vk::SubmitInfo::default()
                .command_buffers(&command_buffers);

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit_info],
                    self.submit_fences[self.current_submit_fence],
                )
                .map_err(|e| Error::BackendError(format!("Failed to submit queue: {:?}", e)))?;

            Ok(())
        }
    }

    fn create_descriptor_set_for_texture(
        &self,
        texture: &Arc<dyn RendererTexture>,
    ) -> Result<Arc<dyn RendererDescriptorSet>> {
        // Downcast to Texture internally (not in demo)
        let vk_texture = texture.as_ref() as *const dyn RendererTexture as *const Texture;
        let vk_texture = unsafe { &*vk_texture };

        unsafe {
            // Allocate descriptor set from pool
            let layouts = [self.descriptor_set_layout];
            let allocate_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(&layouts);

            let descriptor_sets = self.device.allocate_descriptor_sets(&allocate_info)
                .map_err(|e| Error::BackendError(format!("Failed to allocate descriptor set: {:?}", e)))?;

            let descriptor_set = descriptor_sets[0];

            // Update descriptor set to point to the texture
            let image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(vk_texture.view)
                .sampler(self.texture_sampler);

            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info));

            self.device.update_descriptor_sets(std::slice::from_ref(&descriptor_write), &[]);

            // Wrap in abstract type
            Ok(Arc::new(DescriptorSet {
                descriptor_set,
                device: (*self.device).clone(),
            }))
        }
    }

    fn get_descriptor_set_layout_handle(&self) -> u64 {
        Handle::as_raw(self.descriptor_set_layout)
    }

    fn submit_with_swapchain(
        &self,
        commands: &[&dyn RendererCommandList],
        swapchain: &dyn RendererSwapchain,
        image_index: u32,
    ) -> Result<()> {
        // Downcast swapchain to Swapchain internally (not in demo)
        let vk_swapchain = swapchain as *const dyn RendererSwapchain as *const Swapchain;
        let vk_swapchain = unsafe { &*vk_swapchain };

        // Get synchronization primitives from swapchain (now private)
        let (wait_semaphore, signal_semaphore) = vk_swapchain.sync_info(image_index);

        unsafe {
            // Wait for previous submit with this fence
            self.device
                .wait_for_fences(
                    &[self.submit_fences[self.current_submit_fence]],
                    true,
                    u64::MAX,
                )
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to wait for submit fence (swapchain): {:?}", e);
                    Error::BackendError(format!("Failed to wait for fence: {:?}", e))
                })?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to reset submit fence (swapchain): {:?}", e);
                    Error::BackendError(format!("Failed to reset fence: {:?}", e))
                })?;

            // Collect command buffers
            let command_buffers: Vec<vk::CommandBuffer> = commands
                .iter()
                .map(|cmd| {
                    let vk_cmd = *cmd as *const dyn RendererCommandList as *const CommandList;
                    (&*vk_cmd).command_buffer()
                })
                .collect();

            // Submit with synchronization
            let wait_semaphores = [wait_semaphore];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let signal_semaphores = [signal_semaphore];

            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);

            self.device
                .queue_submit(
                    self.graphics_queue,
                    &[submit_info],
                    self.submit_fences[self.current_submit_fence],
                )
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to submit commands to GPU queue (swapchain): {:?}", e);
                    Error::BackendError(format!("Failed to submit queue: {:?}", e))
                })?;

            Ok(())
        }
    }

    fn wait_idle(&self) -> Result<()> {
        unsafe {
            self.device
                .device_wait_idle()
                .map_err(|e| Error::BackendError(format!("Failed to wait idle: {:?}", e)))
        }
    }

    fn stats(&self) -> RendererStats {
        RendererStats::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // Swapchain recreation is handled by the swapchain itself
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            // Wait for device to finish
            self.device.device_wait_idle().ok();

            // Destroy submit fences
            for &fence in &self.submit_fences {
                self.device.destroy_fence(fence, None);
            }

            // Destroy descriptor set layout
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);

            // Destroy texture sampler
            self.device.destroy_sampler(self.texture_sampler, None);

            // Destroy descriptor pool
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);

            // Destroy upload command pool from GpuContext
            {
                let mut pool = self.gpu_context.upload_command_pool.lock().unwrap();
                if *pool != vk::CommandPool::null() {
                    self.device.destroy_command_pool(*pool, None);
                    *pool = vk::CommandPool::null();
                }
            }

            // Drop allocator explicitly BEFORE destroying device
            ManuallyDrop::drop(&mut self.allocator);

            // Cleanup debug config to prevent callbacks during device destruction
            crate::debug::cleanup_debug_config();

            // Destroy device and instance directly (not via GpuContext::drop)
            // This avoids potential issues with drop ordering and callback exceptions on Windows
            self.device.destroy_device(None);
            self._instance.destroy_instance(None);
        }
    }
}
