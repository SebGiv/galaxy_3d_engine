/// VulkanRenderer - Vulkan implementation of Renderer trait

use galaxy_3d_engine::galaxy3d::{Renderer, Result, Error};
use galaxy_3d_engine::galaxy3d::render::{
    CommandList as RendererCommandList, RenderTarget as RendererRenderTarget,
    RenderPass as RendererRenderPass, Swapchain as RendererSwapchain,
    Texture as RendererTexture, Buffer as RendererBuffer,
    Shader as RendererShader, Pipeline as RendererPipeline,
    BindingGroup as RendererBindingGroup,
    Framebuffer as RendererFramebuffer, FramebufferDesc,
    RenderPassDesc,
    TextureDesc, TextureData, TextureInfo, BufferDesc, ShaderDesc, PipelineDesc,
    BindingResource, BindingType, ShaderStageFlags,
    ReflectedBinding, ReflectedPushConstant, ReflectedMember, ReflectedMemberType,
    ScalarKind, PipelineReflection,
    TextureFormat, BufferFormat, ShaderStage, BufferUsage, PrimitiveTopology,
    LoadOp, StoreOp, ImageLayout,
    RendererStats, VertexInputRate,
    Config, DebugSeverity, TextureUsage,
    MipmapMode, ManualMipmapData,
    CullMode, FrontFace, PolygonMode, CompareOp, StencilOp, StencilOpState,
    BlendFactor, BlendOp, ColorWriteMask, SampleCount,
};
use ash::vk;
use std::sync::{Arc, Mutex};
use std::ffi::CString;
use std::mem::ManuallyDrop;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;
use galaxy_3d_engine::{engine_trace, engine_debug, engine_info, engine_warn, engine_error, engine_bail, engine_bail_warn, engine_err, engine_warn_err};

use crate::vulkan_texture::Texture;
use crate::vulkan_buffer::Buffer;
use crate::vulkan_shader::Shader;
use crate::vulkan_pipeline::Pipeline;
use crate::vulkan_command_list::CommandList;
use crate::vulkan_render_target::RenderTarget;
use crate::vulkan_render_pass::RenderPass;
use crate::vulkan_swapchain::Swapchain;
use crate::vulkan_sampler::SamplerCache;
use crate::vulkan_binding_group::BindingGroup;
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

    /// Descriptor pools for binding group allocation (grows dynamically when exhausted)
    descriptor_pools: Mutex<Vec<vk::DescriptorPool>>,
    /// Internal sampler cache (creates VkSampler on first use, behind Mutex for &self access)
    sampler_cache: Mutex<SamplerCache>,

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait for submit fence: {:?}", e))?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to reset submit fence: {:?}", e))?;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to submit commands to GPU queue: {:?}", e))?;

            Ok(())
        }
    }

    /// Create a new Vulkan device
    ///
    /// # Arguments
    ///
    /// * `window` - Window for surface creation
    /// * `config` - Renderer configuration
    ///
    /// Create a descriptor pool with fixed capacity (1024 sets).
    /// Called during init and when the current pool is exhausted.
    fn create_descriptor_pool(device: &ash::Device) -> Result<vk::DescriptorPool> {
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 2048,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: 1024,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1024,
            },
        ];
        let info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(1024);

        unsafe {
            device.create_descriptor_pool(&info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create descriptor pool: {:?}", e);
                    Error::InitializationFailed(format!("Failed to create descriptor pool: {:?}", e))
                })
        }
    }

    pub fn new<W: HasDisplayHandle + HasWindowHandle>(
        window: &W,
        config: Config,
    ) -> Result<Self> {
        unsafe {
            // Create Vulkan Entry
            let entry = ash::Entry::load()
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to load Vulkan library: {:?}", e);
                    Error::InitializationFailed(format!("Failed to load Vulkan library: {:?}", e))
                })?;

            // Application Info
            let app_info = vk::ApplicationInfo::default()
                .application_name(c"Galaxy3D Application")
                .application_version(vk::make_api_version(0, 1, 0, 0))
                .engine_name(c"Galaxy3D")
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::API_VERSION_1_3);

            // Get required extensions
            let display_handle = window.display_handle()
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get display handle: {}", e);
                    Error::InitializationFailed(format!("Failed to get display handle: {}", e))
                })?;
            let mut extension_names = ash_window::enumerate_required_extensions(display_handle.as_raw())
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get required extensions: {}", e);
                    Error::InitializationFailed(format!("Failed to get required extensions: {}", e))
                })?
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
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create Vulkan instance: {:?}", e);
                    Error::InitializationFailed(format!("Failed to create instance: {:?}", e))
                })?;

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
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to create debug messenger: {:?}", e);
                        Error::InitializationFailed(format!("Failed to create debug messenger: {:?}", e))
                    })?;

                (Some(debug_utils), Some(messenger))
            } else {
                (None, None)
            };

            // Create Surface (temporary for queue selection)
            let window_handle = window.window_handle()
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to get window handle: {}", e);
                    Error::InitializationFailed(format!("Failed to get window handle: {}", e))
                })?;
            let surface = ash_window::create_surface(
                &entry,
                &instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to create surface: {:?}", e);
                Error::InitializationFailed(format!("Failed to create surface: {:?}", e))
            })?;

            let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

            // Pick Physical Device
            let physical_devices = instance
                .enumerate_physical_devices()
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to enumerate physical devices: {:?}", e);
                    Error::InitializationFailed(format!("Failed to enumerate physical devices: {:?}", e))
                })?;

            let physical_device = physical_devices
                .into_iter()
                .next()
                .ok_or_else(|| {
                    engine_error!("galaxy3d::vulkan", "No Vulkan-capable GPU found");
                    Error::InitializationFailed("No Vulkan-capable GPU found".to_string())
                })?;

            // Find Queue Families
            let queue_families = instance.get_physical_device_queue_family_properties(physical_device);

            let graphics_family_index = queue_families
                .iter()
                .enumerate()
                .find(|(_, qf)| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .map(|(i, _)| i as u32)
                .ok_or_else(|| {
                    engine_error!("galaxy3d::vulkan", "No graphics queue family found");
                    Error::InitializationFailed("No graphics queue family found".to_string())
                })?;

            let present_family_index = (0..queue_families.len() as u32)
                .find(|&i| {
                    surface_loader
                        .get_physical_device_surface_support(physical_device, i, surface)
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    engine_error!("galaxy3d::vulkan", "No present queue family found");
                    Error::InitializationFailed("No present queue family found".to_string())
                })?;

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

            let device_features = vk::PhysicalDeviceFeatures::default()
                .sampler_anisotropy(true);

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_create_infos)
                .enabled_extension_names(&device_extension_names)
                .enabled_features(&device_features);

            let device = Arc::new(
                instance
                    .create_device(physical_device, &device_create_info, None)
                    .map_err(|e| {
                        engine_error!("galaxy3d::vulkan", "Failed to create logical device: {:?}", e);
                        Error::InitializationFailed(format!("Failed to create device: {:?}", e))
                    })?,
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
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to create GPU allocator: {:?}", e);
                Error::InitializationFailed(format!("Failed to create allocator: {:?}", e))
            })?;

            // Create submit fences (2 for double buffering)
            const MAX_SUBMITS_IN_FLIGHT: usize = 2;
            let fence_create_info = vk::FenceCreateInfo::default()
                .flags(vk::FenceCreateFlags::SIGNALED);

            let mut submit_fences = Vec::with_capacity(MAX_SUBMITS_IN_FLIGHT);
            for _ in 0..MAX_SUBMITS_IN_FLIGHT {
                submit_fences.push(
                    device.create_fence(&fence_create_info, None)
                        .map_err(|e| {
                            engine_error!("galaxy3d::vulkan", "Failed to create submit fence: {:?}", e);
                            Error::InitializationFailed(format!("Failed to create fence: {:?}", e))
                        })?
                );
            }

            // Create initial descriptor pool for binding group allocation
            let descriptor_pool = Self::create_descriptor_pool(&device)?;

            // Create upload command pool (TRANSIENT + RESET for reusable one-shot uploads)
            let upload_pool_create_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_family_index)
                .flags(vk::CommandPoolCreateFlags::TRANSIENT | vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let upload_command_pool = device.create_command_pool(&upload_pool_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create upload command pool: {:?}", e);
                    Error::InitializationFailed(format!("Failed to create upload command pool: {:?}", e))
                })?;

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
                descriptor_pools: Mutex::new(vec![descriptor_pool]),
                sampler_cache: Mutex::new(SamplerCache::new(Arc::clone(&gpu_context))),
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
            TextureFormat::D32_FLOAT_S8_UINT => vk::Format::D32_SFLOAT_S8_UINT,
        }
    }

    /// Convert BufferFormat (vertex attributes) to Vulkan format
    fn buffer_format_to_vk(&self, format: BufferFormat) -> vk::Format {
        match format {
            // Float formats
            BufferFormat::R32_SFLOAT => vk::Format::R32_SFLOAT,
            BufferFormat::R32G32_SFLOAT => vk::Format::R32G32_SFLOAT,
            BufferFormat::R32G32B32_SFLOAT => vk::Format::R32G32B32_SFLOAT,
            BufferFormat::R32G32B32A32_SFLOAT => vk::Format::R32G32B32A32_SFLOAT,
            // Integer formats (signed)
            BufferFormat::R32_SINT => vk::Format::R32_SINT,
            BufferFormat::R32G32_SINT => vk::Format::R32G32_SINT,
            BufferFormat::R32G32B32_SINT => vk::Format::R32G32B32_SINT,
            BufferFormat::R32G32B32A32_SINT => vk::Format::R32G32B32A32_SINT,
            // Integer formats (unsigned)
            BufferFormat::R32_UINT => vk::Format::R32_UINT,
            BufferFormat::R32G32_UINT => vk::Format::R32G32_UINT,
            BufferFormat::R32G32B32_UINT => vk::Format::R32G32B32_UINT,
            BufferFormat::R32G32B32A32_UINT => vk::Format::R32G32B32A32_UINT,
            // Short formats (signed)
            BufferFormat::R16_SINT => vk::Format::R16_SINT,
            BufferFormat::R16G16_SINT => vk::Format::R16G16_SINT,
            BufferFormat::R16G16B16A16_SINT => vk::Format::R16G16B16A16_SINT,
            // Short formats (unsigned)
            BufferFormat::R16_UINT => vk::Format::R16_UINT,
            BufferFormat::R16G16_UINT => vk::Format::R16G16_UINT,
            BufferFormat::R16G16B16A16_UINT => vk::Format::R16G16B16A16_UINT,
            // Byte formats (signed)
            BufferFormat::R8_SINT => vk::Format::R8_SINT,
            BufferFormat::R8G8_SINT => vk::Format::R8G8_SINT,
            BufferFormat::R8G8B8A8_SINT => vk::Format::R8G8B8A8_SINT,
            // Byte formats (unsigned)
            BufferFormat::R8_UINT => vk::Format::R8_UINT,
            BufferFormat::R8G8_UINT => vk::Format::R8G8_UINT,
            BufferFormat::R8G8B8A8_UINT => vk::Format::R8G8B8A8_UINT,
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

    /// Convert renderer ShaderStage to ShaderStageFlags
    fn shader_stage_to_flags(stage: ShaderStage) -> ShaderStageFlags {
        match stage {
            ShaderStage::Vertex => ShaderStageFlags::VERTEX,
            ShaderStage::Fragment => ShaderStageFlags::FRAGMENT,
            ShaderStage::Compute => ShaderStageFlags::COMPUTE,
        }
    }

    /// Parse SPIR-V bytecode and extract reflected bindings and push constants using spirq
    fn reflect_shader(code: &[u32], stage_flags: ShaderStageFlags)
        -> Result<(Vec<ReflectedBinding>, Vec<ReflectedPushConstant>)>
    {
        let entry_points = spirq::ReflectConfig::new()
            .spv(code)
            .ref_all_rscs(true)
            .reflect()
            .map_err(|e| engine_err!("galaxy3d::vulkan",
                "SPIR-V reflection failed: {:?}", e))?;

        let mut bindings = Vec::new();
        let mut push_constants = Vec::new();

        for entry_point in &entry_points {
            for var in entry_point.vars.iter() {
                match var {
                    spirq::var::Variable::Descriptor {
                        name, desc_bind, desc_ty, ty, ..
                    } => {
                        let binding_type = Self::spirq_desc_type_to_binding_type(desc_ty.clone())?;
                        let members = Self::spirq_type_to_members(ty);
                        bindings.push(ReflectedBinding {
                            name: name.clone().unwrap_or_default(),
                            set: desc_bind.set(),
                            binding: desc_bind.bind(),
                            binding_type,
                            stage_flags,
                            members,
                        });
                    }
                    spirq::var::Variable::PushConstant { name, ty } => {
                        let members = Self::spirq_type_to_members(ty);
                        push_constants.push(ReflectedPushConstant {
                            name: name.clone().unwrap_or_default(),
                            stage_flags,
                            size: ty.nbyte().map(|s| s as u32),
                            members,
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok((bindings, push_constants))
    }

    /// Convert spirq descriptor type to renderer BindingType
    fn spirq_desc_type_to_binding_type(desc_ty: spirq::ty::DescriptorType) -> Result<BindingType> {
        use spirq::ty::DescriptorType;
        match desc_ty {
            DescriptorType::UniformBuffer() => Ok(BindingType::UniformBuffer),
            DescriptorType::StorageBuffer(..) => Ok(BindingType::StorageBuffer),
            DescriptorType::CombinedImageSampler() => Ok(BindingType::CombinedImageSampler),
            DescriptorType::SampledImage() => Ok(BindingType::CombinedImageSampler),
            DescriptorType::Sampler() => Ok(BindingType::CombinedImageSampler),
            other => {
                engine_bail!("galaxy3d::vulkan",
                    "Unsupported SPIR-V descriptor type: {:?}", other);
            }
        }
    }

    /// Convert a spirq ScalarType to our ScalarKind
    fn spirq_scalar_to_kind(scalar_ty: &spirq::ty::ScalarType) -> ScalarKind {
        use spirq::ty::ScalarType;
        match scalar_ty {
            ScalarType::Float { bits: 64 } => ScalarKind::Float64,
            ScalarType::Float { .. } => ScalarKind::Float32,
            ScalarType::Integer { is_signed: true, .. } => ScalarKind::Int32,
            ScalarType::Integer { is_signed: false, .. } => ScalarKind::UInt32,
            ScalarType::Boolean => ScalarKind::Bool,
            ScalarType::Void => ScalarKind::Float32,
        }
    }

    /// Recursively convert a spirq Type to our ReflectedMemberType
    fn spirq_type_to_reflected(ty: &spirq::ty::Type) -> ReflectedMemberType {
        use spirq::ty::Type;
        match ty {
            Type::Scalar(s) => ReflectedMemberType::Scalar(Self::spirq_scalar_to_kind(s)),
            Type::Vector(v) => ReflectedMemberType::Vector(
                Self::spirq_scalar_to_kind(&v.scalar_ty),
                v.nscalar,
            ),
            Type::Matrix(m) => ReflectedMemberType::Matrix(
                Self::spirq_scalar_to_kind(&m.vector_ty.scalar_ty),
                m.nvector,
                m.vector_ty.nscalar,
            ),
            Type::Array(a) => ReflectedMemberType::Array {
                element_type: Box::new(Self::spirq_type_to_reflected(&a.element_ty)),
                count: a.nelement,
                stride: a.stride.map(|s| s as u32),
            },
            Type::Struct(st) => {
                let members = st.members.iter().map(|m| {
                    ReflectedMember {
                        name: m.name.clone().unwrap_or_default(),
                        offset: m.offset.unwrap_or(0) as u32,
                        size: m.ty.nbyte().map(|s| s as u32),
                        member_type: Self::spirq_type_to_reflected(&m.ty),
                    }
                }).collect();
                ReflectedMemberType::Struct(members)
            }
            // Fallback for image/sampler types (shouldn't appear inside struct members)
            _ => ReflectedMemberType::Scalar(ScalarKind::Float32),
        }
    }

    /// Extract reflected members from a spirq Type (populated for UBO/SSBO structs)
    fn spirq_type_to_members(ty: &spirq::ty::Type) -> Vec<ReflectedMember> {
        if let spirq::ty::Type::Struct(st) = ty {
            st.members.iter().map(|m| {
                ReflectedMember {
                    name: m.name.clone().unwrap_or_default(),
                    offset: m.offset.unwrap_or(0) as u32,
                    size: m.ty.nbyte().map(|s| s as u32),
                    member_type: Self::spirq_type_to_reflected(&m.ty),
                }
            }).collect()
        } else {
            Vec::new()
        }
    }

    /// Merge reflected bindings from vertex + fragment shaders into a PipelineReflection
    fn merge_shader_reflections(desc: &PipelineDesc) -> Result<PipelineReflection> {
        let vk_vs = unsafe { &*(Arc::as_ptr(&desc.vertex_shader) as *const Shader) };
        let vk_fs = unsafe { &*(Arc::as_ptr(&desc.fragment_shader) as *const Shader) };

        // Merge bindings
        let mut merged: Vec<ReflectedBinding> = Vec::new();

        for binding in &vk_vs.reflected_bindings {
            merged.push(binding.clone());
        }

        for fs_binding in &vk_fs.reflected_bindings {
            if let Some(existing) = merged.iter_mut()
                .find(|b| b.set == fs_binding.set && b.binding == fs_binding.binding)
            {
                if existing.binding_type != fs_binding.binding_type {
                    engine_bail!("galaxy3d::vulkan",
                        "Binding '{}' (set={}, binding={}) has different types in vertex ({:?}) and fragment ({:?})",
                        existing.name, existing.set, existing.binding,
                        existing.binding_type, fs_binding.binding_type);
                }
                existing.stage_flags = ShaderStageFlags::from_bits(
                    existing.stage_flags.bits() | fs_binding.stage_flags.bits()
                );
            } else {
                merged.push(fs_binding.clone());
            }
        }

        // Merge push constants
        let mut push_constants: Vec<ReflectedPushConstant> = Vec::new();

        for pc in &vk_vs.reflected_push_constants {
            push_constants.push(pc.clone());
        }

        for fs_pc in &vk_fs.reflected_push_constants {
            if let Some(existing) = push_constants.iter_mut()
                .find(|p| p.name == fs_pc.name)
            {
                // Same push constant block in both stages: merge stage_flags
                existing.stage_flags = ShaderStageFlags::from_bits(
                    existing.stage_flags.bits() | fs_pc.stage_flags.bits()
                );
            } else {
                push_constants.push(fs_pc.clone());
            }
        }

        Ok(PipelineReflection::new(merged, push_constants))
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

    // ===== Pipeline state conversions =====

    fn cull_mode_to_vk(&self, mode: CullMode) -> vk::CullModeFlags {
        match mode {
            CullMode::None => vk::CullModeFlags::NONE,
            CullMode::Front => vk::CullModeFlags::FRONT,
            CullMode::Back => vk::CullModeFlags::BACK,
        }
    }

    fn front_face_to_vk(&self, face: FrontFace) -> vk::FrontFace {
        match face {
            FrontFace::CounterClockwise => vk::FrontFace::COUNTER_CLOCKWISE,
            FrontFace::Clockwise => vk::FrontFace::CLOCKWISE,
        }
    }

    fn polygon_mode_to_vk(&self, mode: PolygonMode) -> vk::PolygonMode {
        match mode {
            PolygonMode::Fill => vk::PolygonMode::FILL,
            PolygonMode::Line => vk::PolygonMode::LINE,
            PolygonMode::Point => vk::PolygonMode::POINT,
        }
    }

    fn compare_op_to_vk(&self, op: CompareOp) -> vk::CompareOp {
        match op {
            CompareOp::Never => vk::CompareOp::NEVER,
            CompareOp::Less => vk::CompareOp::LESS,
            CompareOp::Equal => vk::CompareOp::EQUAL,
            CompareOp::LessOrEqual => vk::CompareOp::LESS_OR_EQUAL,
            CompareOp::Greater => vk::CompareOp::GREATER,
            CompareOp::NotEqual => vk::CompareOp::NOT_EQUAL,
            CompareOp::GreaterOrEqual => vk::CompareOp::GREATER_OR_EQUAL,
            CompareOp::Always => vk::CompareOp::ALWAYS,
        }
    }

    fn stencil_op_to_vk(&self, op: StencilOp) -> vk::StencilOp {
        match op {
            StencilOp::Keep => vk::StencilOp::KEEP,
            StencilOp::Zero => vk::StencilOp::ZERO,
            StencilOp::Replace => vk::StencilOp::REPLACE,
            StencilOp::IncrementAndClamp => vk::StencilOp::INCREMENT_AND_CLAMP,
            StencilOp::DecrementAndClamp => vk::StencilOp::DECREMENT_AND_CLAMP,
            StencilOp::Invert => vk::StencilOp::INVERT,
            StencilOp::IncrementAndWrap => vk::StencilOp::INCREMENT_AND_WRAP,
            StencilOp::DecrementAndWrap => vk::StencilOp::DECREMENT_AND_WRAP,
        }
    }

    fn stencil_op_state_to_vk(&self, state: &StencilOpState) -> vk::StencilOpState {
        vk::StencilOpState {
            fail_op: self.stencil_op_to_vk(state.fail_op),
            pass_op: self.stencil_op_to_vk(state.pass_op),
            depth_fail_op: self.stencil_op_to_vk(state.depth_fail_op),
            compare_op: self.compare_op_to_vk(state.compare_op),
            compare_mask: state.compare_mask,
            write_mask: state.write_mask,
            reference: state.reference,
        }
    }

    fn blend_factor_to_vk(&self, factor: BlendFactor) -> vk::BlendFactor {
        match factor {
            BlendFactor::Zero => vk::BlendFactor::ZERO,
            BlendFactor::One => vk::BlendFactor::ONE,
            BlendFactor::SrcColor => vk::BlendFactor::SRC_COLOR,
            BlendFactor::OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
            BlendFactor::DstColor => vk::BlendFactor::DST_COLOR,
            BlendFactor::OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
            BlendFactor::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
            BlendFactor::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            BlendFactor::DstAlpha => vk::BlendFactor::DST_ALPHA,
            BlendFactor::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
            BlendFactor::ConstantColor => vk::BlendFactor::CONSTANT_COLOR,
            BlendFactor::OneMinusConstantColor => vk::BlendFactor::ONE_MINUS_CONSTANT_COLOR,
            BlendFactor::SrcAlphaSaturate => vk::BlendFactor::SRC_ALPHA_SATURATE,
        }
    }

    fn blend_op_to_vk(&self, op: BlendOp) -> vk::BlendOp {
        match op {
            BlendOp::Add => vk::BlendOp::ADD,
            BlendOp::Subtract => vk::BlendOp::SUBTRACT,
            BlendOp::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
            BlendOp::Min => vk::BlendOp::MIN,
            BlendOp::Max => vk::BlendOp::MAX,
        }
    }

    fn sample_count_to_vk(&self, count: SampleCount) -> vk::SampleCountFlags {
        match count {
            SampleCount::S1 => vk::SampleCountFlags::TYPE_1,
            SampleCount::S2 => vk::SampleCountFlags::TYPE_2,
            SampleCount::S4 => vk::SampleCountFlags::TYPE_4,
            SampleCount::S8 => vk::SampleCountFlags::TYPE_8,
        }
    }

    fn color_write_mask_to_vk(&self, mask: &ColorWriteMask) -> vk::ColorComponentFlags {
        let mut flags = vk::ColorComponentFlags::empty();
        if mask.r { flags |= vk::ColorComponentFlags::R; }
        if mask.g { flags |= vk::ColorComponentFlags::G; }
        if mask.b { flags |= vk::ColorComponentFlags::B; }
        if mask.a { flags |= vk::ColorComponentFlags::A; }
        flags
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
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to get display handle for swapchain: {}", e);
                Error::InitializationFailed(format!("Failed to get display handle: {}", e))
            })?;
        let window_handle = window.window_handle()
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to get window handle for swapchain: {}", e);
                Error::InitializationFailed(format!("Failed to get window handle: {}", e))
            })?;

        let surface = unsafe {
            ash_window::create_surface(
                &self._entry,
                &self._instance,
                display_handle.as_raw(),
                window_handle.as_raw(),
                None,
            )
            .map_err(|e| {
                engine_error!("galaxy3d::vulkan", "Failed to create surface for swapchain: {:?}", e);
                Error::InitializationFailed(format!("Failed to create surface: {:?}", e))
            })?
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

    /// Convert BindingType to Vulkan descriptor type
    fn binding_type_to_vk(binding_type: BindingType) -> vk::DescriptorType {
        match binding_type {
            BindingType::UniformBuffer => vk::DescriptorType::UNIFORM_BUFFER,
            BindingType::CombinedImageSampler => vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            BindingType::StorageBuffer => vk::DescriptorType::STORAGE_BUFFER,
        }
    }

    /// Convert ShaderStageFlags to Vulkan shader stage flags
    fn stage_flags_to_vk(flags: ShaderStageFlags) -> vk::ShaderStageFlags {
        let mut vk_flags = vk::ShaderStageFlags::empty();
        if flags.contains_vertex() { vk_flags |= vk::ShaderStageFlags::VERTEX; }
        if flags.contains_fragment() { vk_flags |= vk::ShaderStageFlags::FRAGMENT; }
        if flags.contains_compute() { vk_flags |= vk::ShaderStageFlags::COMPUTE; }
        vk_flags
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

    fn create_render_target_texture(
        &self,
        texture: &dyn RendererTexture,
        layer: u32,
        mip_level: u32,
    ) -> Result<Arc<dyn RendererRenderTarget>> {
        let info = texture.info();

        // Validate usage
        match info.usage {
            TextureUsage::RenderTarget
            | TextureUsage::SampledAndRenderTarget
            | TextureUsage::DepthStencil => {}
            _ => {
                engine_bail!("galaxy3d::vulkan",
                    "create_render_target_texture: texture usage {:?} is not compatible \
                     with render target (expected RenderTarget, SampledAndRenderTarget, \
                     or DepthStencil)", info.usage);
            }
        }

        // Validate layer
        if layer >= info.array_layers {
            engine_bail!("galaxy3d::vulkan",
                "create_render_target_texture: layer {} out of range (array_layers = {})",
                layer, info.array_layers);
        }

        // Validate mip level
        if mip_level >= info.mip_levels {
            engine_bail!("galaxy3d::vulkan",
                "create_render_target_texture: mip_level {} out of range (mip_levels = {})",
                mip_level, info.mip_levels);
        }

        unsafe {
            // Downcast to Vulkan texture to access VkImage
            let vk_texture = texture as *const dyn RendererTexture as *const Texture;
            let vk_texture = &*vk_texture;

            let format = self.format_to_vk(info.format);
            let aspect_mask = if matches!(info.usage, TextureUsage::DepthStencil) {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            };

            // Create image view targeting specific layer/mip of the existing image
            let view_create_info = vk::ImageViewCreateInfo::default()
                .image(vk_texture.image)
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
                    base_mip_level: mip_level,
                    level_count: 1,
                    base_array_layer: layer,
                    layer_count: 1,
                });

            let view = self.device.create_image_view(&view_create_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan",
                    "Failed to create image view for render target view: {:?}", e))?;

            // Calculate dimensions for this mip level
            let mip_width = (info.width >> mip_level).max(1);
            let mip_height = (info.height >> mip_level).max(1);

            // RenderTarget owns the ImageView (cleaned up on Drop)
            // but NOT the VkImage (owned by the Texture)
            Ok(Arc::new(RenderTarget::new_texture_target(
                mip_width,
                mip_height,
                info.format,
                view,
                (*self.device).clone(),
            )))
        }
    }

    fn create_framebuffer(&self, desc: &FramebufferDesc) -> Result<Arc<dyn RendererFramebuffer>> {
        unsafe {
            // Downcast render pass to Vulkan type
            let vk_render_pass = desc.render_pass.as_ref()
                as *const dyn RendererRenderPass
                as *const crate::vulkan_render_pass::RenderPass;
            let vk_render_pass = &*vk_render_pass;

            // Collect image views from attachments
            let mut attachments = Vec::with_capacity(
                desc.color_attachments.len()
                    + if desc.depth_stencil_attachment.is_some() { 1 } else { 0 },
            );

            for color_rt in &desc.color_attachments {
                let vk_rt = color_rt.as_ref()
                    as *const dyn RendererRenderTarget
                    as *const crate::vulkan_render_target::RenderTarget;
                attachments.push((*vk_rt).image_view);
            }

            if let Some(depth_rt) = &desc.depth_stencil_attachment {
                let vk_rt = depth_rt.as_ref()
                    as *const dyn RendererRenderTarget
                    as *const crate::vulkan_render_target::RenderTarget;
                attachments.push((*vk_rt).image_view);
            }

            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(vk_render_pass.render_pass)
                .attachments(&attachments)
                .width(desc.width)
                .height(desc.height)
                .layers(1);

            let framebuffer = self.device.create_framebuffer(&framebuffer_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan",
                    "Failed to create framebuffer: {:?}", e))?;

            Ok(Arc::new(crate::vulkan_frame_buffer::Framebuffer::new(
                framebuffer, desc.width, desc.height, (*self.device).clone(),
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
                    .stencil_load_op(self.load_op_to_vk(color_attachment.stencil_load_op))
                    .stencil_store_op(self.store_op_to_vk(color_attachment.stencil_store_op))
                    .initial_layout(self.image_layout_to_vk(color_attachment.initial_layout))
                    .final_layout(self.image_layout_to_vk(color_attachment.final_layout)));

                color_attachment_refs.push(vk::AttachmentReference::default()
                    .attachment(i as u32)
                    .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL));
            }

            if let Some(depth_attachment) = &desc.depth_stencil_attachment {
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
                    .stencil_load_op(self.load_op_to_vk(depth_attachment.stencil_load_op))
                    .stencil_store_op(self.store_op_to_vk(depth_attachment.stencil_store_op))
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

            // Subpass dependency — include depth stages when depth attachment is present
            let has_depth = depth_attachment_ref.is_some();
            let (stage_mask, access_mask) = if has_depth {
                (
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                )
            } else {
                (
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                )
            };

            let dependency = vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(stage_mask)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_stage_mask(stage_mask)
                .dst_access_mask(access_mask);

            // Create render pass
            let render_pass_info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&dependency));

            let render_pass = self.device.create_render_pass(&render_pass_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create render pass: {:?}", e))?;

            Ok(Arc::new(RenderPass {
                render_pass,
                device: (*self.device).clone(),
            }))
        }
    }

    fn create_binding_group(
        &self,
        pipeline: &Arc<dyn RendererPipeline>,
        set_index: u32,
        resources: &[BindingResource],
    ) -> Result<Arc<dyn RendererBindingGroup>> {
        unsafe {
            // Downcast pipeline to access stored descriptor set layouts
            let vk_pipeline = pipeline.as_ref() as *const dyn RendererPipeline as *const Pipeline;
            let vk_pipeline = &*vk_pipeline;

            if set_index as usize >= vk_pipeline.descriptor_set_layouts.len() {
                engine_bail!("galaxy3d::vulkan",
                    "create_binding_group: set_index {} out of range (pipeline has {} layouts)",
                    set_index, vk_pipeline.descriptor_set_layouts.len());
            }

            let ds_layout = vk_pipeline.descriptor_set_layouts[set_index as usize];

            // Allocate descriptor set from pool (grow dynamically if exhausted)
            let layouts = [ds_layout];
            let descriptor_sets = {
                let mut pools = self.descriptor_pools.lock().unwrap();
                let current_pool = *pools.last().unwrap();
                let allocate_info = vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(current_pool)
                    .set_layouts(&layouts);

                match self.device.allocate_descriptor_sets(&allocate_info) {
                    Ok(sets) => sets,
                    Err(vk::Result::ERROR_OUT_OF_POOL_MEMORY) => {
                        let new_pool = Self::create_descriptor_pool(&self.device)?;
                        pools.push(new_pool);
                        engine_info!("galaxy3d::vulkan",
                            "Descriptor pool exhausted, created new pool (total: {})",
                            pools.len()
                        );
                        let retry_info = vk::DescriptorSetAllocateInfo::default()
                            .descriptor_pool(new_pool)
                            .set_layouts(&layouts);
                        self.device.allocate_descriptor_sets(&retry_info)
                            .map_err(|e| engine_err!("galaxy3d::vulkan",
                                "Failed to allocate descriptor set after pool growth: {:?}", e))?
                    }
                    Err(e) => return Err(engine_err!("galaxy3d::vulkan",
                        "Failed to allocate descriptor set: {:?}", e)),
                }
            };

            let descriptor_set = descriptor_sets[0];

            // Write resources into descriptor set
            // We need to keep buffer_infos and image_infos alive for the duration of the write
            let mut buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::new();
            let mut image_infos: Vec<vk::DescriptorImageInfo> = Vec::new();
            let mut writes: Vec<vk::WriteDescriptorSet> = Vec::new();

            for (binding_index, resource) in resources.iter().enumerate() {
                match resource {
                    BindingResource::UniformBuffer(buffer) => {
                        let vk_buffer = *buffer as *const dyn RendererBuffer as *const crate::vulkan_buffer::Buffer;
                        let vk_buffer = &*vk_buffer;

                        buffer_infos.push(
                            vk::DescriptorBufferInfo::default()
                                .buffer(vk_buffer.buffer)
                                .offset(0)
                                .range(vk::WHOLE_SIZE)
                        );
                    }
                    BindingResource::SampledTexture(texture, sampler_type) => {
                        let vk_texture = *texture as *const dyn RendererTexture as *const Texture;
                        let vk_texture = &*vk_texture;
                        let vk_sampler = self.sampler_cache.lock().unwrap().get(*sampler_type);

                        image_infos.push(
                            vk::DescriptorImageInfo::default()
                                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                .image_view(vk_texture.view)
                                .sampler(vk_sampler)
                        );
                    }
                    BindingResource::StorageBuffer(buffer) => {
                        let vk_buffer = *buffer as *const dyn RendererBuffer as *const crate::vulkan_buffer::Buffer;
                        let vk_buffer = &*vk_buffer;

                        buffer_infos.push(
                            vk::DescriptorBufferInfo::default()
                                .buffer(vk_buffer.buffer)
                                .offset(0)
                                .range(vk::WHOLE_SIZE)
                        );
                    }
                }
                // Track binding index for write construction
                let _ = binding_index;
            }

            // Build write descriptor sets with correct pointers
            let mut buffer_idx = 0usize;
            let mut image_idx = 0usize;

            for (binding_index, resource) in resources.iter().enumerate() {
                match resource {
                    BindingResource::UniformBuffer(_) => {
                        writes.push(
                            vk::WriteDescriptorSet::default()
                                .dst_set(descriptor_set)
                                .dst_binding(binding_index as u32)
                                .dst_array_element(0)
                                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                                .buffer_info(std::slice::from_ref(&buffer_infos[buffer_idx]))
                        );
                        buffer_idx += 1;
                    }
                    BindingResource::SampledTexture(_, _) => {
                        writes.push(
                            vk::WriteDescriptorSet::default()
                                .dst_set(descriptor_set)
                                .dst_binding(binding_index as u32)
                                .dst_array_element(0)
                                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                                .image_info(std::slice::from_ref(&image_infos[image_idx]))
                        );
                        image_idx += 1;
                    }
                    BindingResource::StorageBuffer(_) => {
                        writes.push(
                            vk::WriteDescriptorSet::default()
                                .dst_set(descriptor_set)
                                .dst_binding(binding_index as u32)
                                .dst_array_element(0)
                                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                                .buffer_info(std::slice::from_ref(&buffer_infos[buffer_idx]))
                        );
                        buffer_idx += 1;
                    }
                }
            }

            self.device.update_descriptor_sets(&writes, &[]);

            Ok(Arc::new(BindingGroup {
                descriptor_set,
                set_index,
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

            // Calculate mip levels from MipmapMode
            let mip_levels = desc.mipmap.mip_levels(desc.width, desc.height);

            // Determine image view type based on array layers or force_array flag
            let view_type = if array_layers > 1 || desc.force_array {
                vk::ImageViewType::TYPE_2D_ARRAY
            } else {
                vk::ImageViewType::TYPE_2D
            };

            // Image usage flags based on declared TextureUsage
            let mut usage_flags = match desc.usage {
                TextureUsage::Sampled => {
                    vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST
                }
                TextureUsage::RenderTarget => {
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST
                }
                TextureUsage::SampledAndRenderTarget => {
                    vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST
                }
                TextureUsage::DepthStencil => {
                    vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                        | vk::ImageUsageFlags::TRANSFER_DST
                }
            };
            if matches!(desc.mipmap, MipmapMode::Generate { .. }) && mip_levels > 1 {
                usage_flags |= vk::ImageUsageFlags::TRANSFER_SRC;
            }

            // Depth/stencil textures need DEPTH aspect, all others use COLOR
            let aspect_mask = if matches!(desc.usage, TextureUsage::DepthStencil) {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
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
                .mip_levels(mip_levels)
                .array_layers(array_layers)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage_flags)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = self.device.create_image(&image_create_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create texture image: {:?}", e))?;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to bind texture image memory: {:?}", e))?;

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
                    aspect_mask,
                    base_mip_level: 0,
                    level_count: mip_levels,
                    base_array_layer: 0,
                    layer_count: array_layers,
                });

            let view = self.device.create_image_view(&view_create_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create texture image view: {:?}", e))?;

            // Collect upload items: Vec<(layer_index, &[u8])>
            let upload_items: Vec<(u32, &[u8])> = match &desc.data {
                Some(TextureData::Single(data)) => {
                    vec![(0, data.as_slice())]
                }
                Some(TextureData::Layers(layers)) => {
                    // Validate layer indices
                    for layer_data in layers {
                        if layer_data.layer >= array_layers {
                            engine_bail!("galaxy3d::vulkan", "Layer index {} exceeds array_layers {}", layer_data.layer, array_layers);
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
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create command pool for texture upload: {:?}", e))?;

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);

                let command_buffers = self.device.allocate_command_buffers(&command_buffer_allocate_info)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to allocate command buffer for texture upload: {:?}", e))?;
                let command_buffer = command_buffers[0];

                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

                self.device.begin_command_buffer(command_buffer, &begin_info)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to begin command buffer for texture upload: {:?}", e))?;

                // Transition all layers: UNDEFINED → TRANSFER_DST_OPTIMAL
                let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask,
                        base_mip_level: 0,
                        level_count: mip_levels,
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
                        .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create staging buffer for layer {}: {:?}", layer_index, e))?;

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
                        .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to bind staging buffer memory for layer {}: {:?}", layer_index, e))?;

                    // Copy data to staging buffer
                    let mapped_ptr = staging_allocation.mapped_ptr()
                        .ok_or_else(|| engine_err!("galaxy3d::vulkan", "Staging buffer is not mapped for layer {}", layer_index))?
                        .as_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr, data.len());

                    // Record copy command for this layer
                    let region = vk::BufferImageCopy::default()
                        .buffer_offset(0)
                        .buffer_row_length(0)
                        .buffer_image_height(0)
                        .image_subresource(vk::ImageSubresourceLayers {
                            aspect_mask,
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

                // Generate or upload mipmaps (levels 1+)
                match &desc.mipmap {
                    MipmapMode::Generate { .. } if mip_levels > 1 => {
                        // GPU mipmap generation using vkCmdBlitImage
                        for mip in 1..mip_levels {
                            let src_mip = mip - 1;
                            let src_width = (desc.width >> src_mip).max(1);
                            let src_height = (desc.height >> src_mip).max(1);
                            let dst_width = (desc.width >> mip).max(1);
                            let dst_height = (desc.height >> mip).max(1);

                            // Transition src mip to TRANSFER_SRC_OPTIMAL
                            let barrier_src = vk::ImageMemoryBarrier::default()
                                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                                .image(image)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask,
                                    base_mip_level: src_mip,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: array_layers,
                                })
                                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                                .dst_access_mask(vk::AccessFlags::TRANSFER_READ);

                            self.device.cmd_pipeline_barrier(
                                command_buffer,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::DependencyFlags::empty(),
                                &[],
                                &[],
                                &[barrier_src],
                            );

                            // Blit from src_mip to dst_mip
                            let blit = vk::ImageBlit::default()
                                .src_subresource(vk::ImageSubresourceLayers {
                                    aspect_mask,
                                    mip_level: src_mip,
                                    base_array_layer: 0,
                                    layer_count: array_layers,
                                })
                                .src_offsets([
                                    vk::Offset3D { x: 0, y: 0, z: 0 },
                                    vk::Offset3D {
                                        x: src_width as i32,
                                        y: src_height as i32,
                                        z: 1,
                                    },
                                ])
                                .dst_subresource(vk::ImageSubresourceLayers {
                                    aspect_mask,
                                    mip_level: mip,
                                    base_array_layer: 0,
                                    layer_count: array_layers,
                                })
                                .dst_offsets([
                                    vk::Offset3D { x: 0, y: 0, z: 0 },
                                    vk::Offset3D {
                                        x: dst_width as i32,
                                        y: dst_height as i32,
                                        z: 1,
                                    },
                                ]);

                            self.device.cmd_blit_image(
                                command_buffer,
                                image,
                                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                image,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                &[blit],
                                vk::Filter::LINEAR,
                            );

                            // Transition src mip to SHADER_READ_ONLY (done with this level)
                            let barrier_src_final = vk::ImageMemoryBarrier::default()
                                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                                .image(image)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask,
                                    base_mip_level: src_mip,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: array_layers,
                                })
                                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                                .dst_access_mask(vk::AccessFlags::SHADER_READ);

                            self.device.cmd_pipeline_barrier(
                                command_buffer,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::PipelineStageFlags::FRAGMENT_SHADER,
                                vk::DependencyFlags::empty(),
                                &[],
                                &[],
                                &[barrier_src_final],
                            );
                        }

                        // Transition last mip level to SHADER_READ_ONLY
                        let barrier_last_mip = vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .image(image)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask,
                                base_mip_level: mip_levels - 1,
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
                            &[barrier_last_mip],
                        );
                    }
                    MipmapMode::Manual(manual_data) if mip_levels > 1 => {
                        // Manual mipmap upload - user provides pixel data for each mip level
                        match manual_data {
                            ManualMipmapData::Single(mips) => {
                                // Upload each mip level (1+) for all array layers
                                for (mip_index, mip_data) in mips.iter().enumerate() {
                                    let mip_level = (mip_index + 1) as u32; // Level 0 already uploaded
                                    if mip_level >= mip_levels {
                                        break; // Don't exceed computed mip_levels
                                    }

                                    let mip_width = (desc.width >> mip_level).max(1);
                                    let mip_height = (desc.height >> mip_level).max(1);

                                    // Create staging buffer for this mip level
                                    let buffer_size = mip_data.len() as u64;
                                    let staging_buffer_create_info = vk::BufferCreateInfo::default()
                                        .size(buffer_size)
                                        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                                        .sharing_mode(vk::SharingMode::EXCLUSIVE);

                                    let staging_buffer = self.device.create_buffer(&staging_buffer_create_info, None)
                                        .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create staging buffer for mip level {}: {:?}", mip_level, e))?;

                                    let staging_requirements = self.device.get_buffer_memory_requirements(staging_buffer);

                                    let staging_allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                                        name: "mipmap_staging_buffer",
                                        requirements: staging_requirements,
                                        location: gpu_allocator::MemoryLocation::CpuToGpu,
                                        linear: true,
                                        allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                                    })
                                    .map_err(|_e| {
                                        let size_mb = staging_requirements.size as f64 / (1024.0 * 1024.0);
                                        engine_error!("galaxy3d::vulkan", "Out of GPU memory for mipmap staging buffer level {} ({:.2} MB)", mip_level, size_mb);
                                        Error::OutOfMemory
                                    })?;

                                    self.device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
                                        .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to bind staging buffer memory for mip level {}: {:?}", mip_level, e))?;

                                    // Copy data to staging buffer
                                    let mapped_ptr = staging_allocation.mapped_ptr()
                                        .ok_or_else(|| engine_err!("galaxy3d::vulkan", "Staging buffer is not mapped for mip level {}", mip_level))?
                                        .as_ptr() as *mut u8;
                                    std::ptr::copy_nonoverlapping(mip_data.as_ptr(), mapped_ptr, mip_data.len());

                                    // Copy to all array layers at this mip level
                                    let region = vk::BufferImageCopy::default()
                                        .buffer_offset(0)
                                        .buffer_row_length(0)
                                        .buffer_image_height(0)
                                        .image_subresource(vk::ImageSubresourceLayers {
                                            aspect_mask,
                                            mip_level,
                                            base_array_layer: 0,
                                            layer_count: array_layers,
                                        })
                                        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                                        .image_extent(vk::Extent3D {
                                            width: mip_width,
                                            height: mip_height,
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
                            }
                            ManualMipmapData::Layers(layers) => {
                                // Upload per-layer mip data
                                for layer_data in layers {
                                    if layer_data.layer >= array_layers {
                                        engine_error!("galaxy3d::vulkan",
                                            "LayerMipmapData references layer {} but array_layers = {}",
                                            layer_data.layer, array_layers);
                                        continue;
                                    }

                                    for (mip_index, mip_data) in layer_data.mips.iter().enumerate() {
                                        let mip_level = (mip_index + 1) as u32;
                                        if mip_level >= mip_levels {
                                            break;
                                        }

                                        let mip_width = (desc.width >> mip_level).max(1);
                                        let mip_height = (desc.height >> mip_level).max(1);

                                        // Create staging buffer for this layer's mip level
                                        let buffer_size = mip_data.len() as u64;
                                        let staging_buffer_create_info = vk::BufferCreateInfo::default()
                                            .size(buffer_size)
                                            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                                            .sharing_mode(vk::SharingMode::EXCLUSIVE);

                                        let staging_buffer = self.device.create_buffer(&staging_buffer_create_info, None)
                                            .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create staging buffer for layer {} mip {}: {:?}", layer_data.layer, mip_level, e))?;

                                        let staging_requirements = self.device.get_buffer_memory_requirements(staging_buffer);

                                        let staging_allocation = self.allocator.lock().unwrap().allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                                            name: "layer_mipmap_staging_buffer",
                                            requirements: staging_requirements,
                                            location: gpu_allocator::MemoryLocation::CpuToGpu,
                                            linear: true,
                                            allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                                        })
                                        .map_err(|_e| {
                                            let size_mb = staging_requirements.size as f64 / (1024.0 * 1024.0);
                                            engine_error!("galaxy3d::vulkan", "Out of GPU memory for layer {} mip {} staging buffer ({:.2} MB)", layer_data.layer, mip_level, size_mb);
                                            Error::OutOfMemory
                                        })?;

                                        self.device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
                                            .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to bind staging buffer memory for layer {} mip {}: {:?}", layer_data.layer, mip_level, e))?;

                                        // Copy data to staging buffer
                                        let mapped_ptr = staging_allocation.mapped_ptr()
                                            .ok_or_else(|| engine_err!("galaxy3d::vulkan", "Staging buffer is not mapped for layer {} mip {}", layer_data.layer, mip_level))?
                                            .as_ptr() as *mut u8;
                                        std::ptr::copy_nonoverlapping(mip_data.as_ptr(), mapped_ptr, mip_data.len());

                                        // Copy to specific layer at this mip level
                                        let region = vk::BufferImageCopy::default()
                                            .buffer_offset(0)
                                            .buffer_row_length(0)
                                            .buffer_image_height(0)
                                            .image_subresource(vk::ImageSubresourceLayers {
                                                aspect_mask,
                                                mip_level,
                                                base_array_layer: layer_data.layer,
                                                layer_count: 1,
                                            })
                                            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                                            .image_extent(vk::Extent3D {
                                                width: mip_width,
                                                height: mip_height,
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
                                }
                            }
                        }

                        // Transition all mip levels to SHADER_READ_ONLY
                        let barrier_all_mips = vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .image(image)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask,
                                base_mip_level: 0,
                                level_count: mip_levels,
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
                            &[barrier_all_mips],
                        );
                    }
                    _ => {
                        // No mipmaps or mip_levels == 1, transition all to SHADER_READ_ONLY
                        let barrier_to_shader = vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                            .image(image)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask,
                                base_mip_level: 0,
                                level_count: mip_levels,
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
                    }
                }

                // End recording, submit, and wait
                self.device.end_command_buffer(command_buffer)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to end command buffer for texture upload: {:?}", e))?;

                let command_buffers_submit = [command_buffer];
                let submit_info = vk::SubmitInfo::default()
                    .command_buffers(&command_buffers_submit);

                self.device.queue_submit(self.graphics_queue, &[submit_info], vk::Fence::null())
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to submit texture upload commands to GPU: {:?}", e))?;

                self.device.queue_wait_idle(self.graphics_queue)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait for texture upload completion: {:?}", e))?;

                // Clean up staging buffers and command pool
                self.device.destroy_command_pool(command_pool, None);
                for (staging_buf, staging_alloc) in staging_buffers {
                    self.device.destroy_buffer(staging_buf, None);
                    self.allocator.lock().unwrap().free(staging_alloc)
                        .map_err(|_e| engine_warn_err!("galaxy3d::vulkan", "Failed to free staging buffer allocation during texture upload"))?;
                }
            } else if matches!(desc.usage, TextureUsage::Sampled | TextureUsage::SampledAndRenderTarget) {
                // No data to upload — transition to SHADER_READ_ONLY_OPTIMAL
                // (only for sampled textures; RenderTarget/DepthStencil stay UNDEFINED
                // and the render pass handles the initial layout transition)
                let command_pool_create_info = vk::CommandPoolCreateInfo::default()
                    .queue_family_index(self.graphics_queue_family)
                    .flags(vk::CommandPoolCreateFlags::TRANSIENT);

                let command_pool = self.device.create_command_pool(&command_pool_create_info, None)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create command pool for layout transition: {:?}", e))?;

                let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1);

                let command_buffers = self.device.allocate_command_buffers(&command_buffer_allocate_info)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to allocate command buffer for layout transition: {:?}", e))?;
                let command_buffer = command_buffers[0];

                let begin_info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

                self.device.begin_command_buffer(command_buffer, &begin_info)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to begin command buffer for layout transition: {:?}", e))?;

                let barrier = vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask,
                        base_mip_level: 0,
                        level_count: mip_levels,
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
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to end command buffer for layout transition: {:?}", e))?;

                let command_buffers_submit = [command_buffer];
                let submit_info = vk::SubmitInfo::default()
                    .command_buffers(&command_buffers_submit);

                self.device.queue_submit(self.graphics_queue, &[submit_info], vk::Fence::null())
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to submit layout transition: {:?}", e))?;

                self.device.queue_wait_idle(self.graphics_queue)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait for layout transition: {:?}", e))?;

                self.device.destroy_command_pool(command_pool, None);
            }

            // Build TextureInfo from the descriptor
            let info = TextureInfo::new(
                desc.width,
                desc.height,
                desc.format,
                desc.usage,
                array_layers,
                mip_levels,
                desc.force_array,
            );

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create buffer of size {} bytes: {:?}", desc.size, e))?;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to bind buffer memory: {:?}", e))?;

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
                engine_bail_warn!("galaxy3d::vulkan",
                    "Shader code not 4-byte aligned (size: {} bytes)", desc.code.len());
            }

            // Convert to u32 slice for SPIR-V
            let code_u32 = std::slice::from_raw_parts(
                desc.code.as_ptr() as *const u32,
                desc.code.len() / 4,
            );

            let create_info = vk::ShaderModuleCreateInfo::default()
                .code(code_u32);

            let module = self.device.create_shader_module(&create_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create shader module: {:?}", e))?;

            // SPIR-V reflection via spirq
            let stage_flags = Self::shader_stage_to_flags(desc.stage);
            let (reflected_bindings, reflected_push_constants) =
                Self::reflect_shader(code_u32, stage_flags)?;

            Ok(Arc::new(Shader {
                module,
                stage: self.shader_stage_to_vk(desc.stage),
                entry_point: desc.entry_point.clone(),
                device: (*self.device).clone(),
                reflected_bindings,
                reflected_push_constants,
            }))
        }
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<Arc<dyn RendererPipeline>> {
        // NOTE: This implementation is temporary and creates a hardcoded render pass
        // In the new architecture, pipelines should be created with a specific render pass
        // For now, we create a simple render pass for compatibility

        unsafe {
            // Create a temporary render pass for pipeline creation
            // Must include depth attachment when depth/stencil testing is enabled
            let color_attachment = vk::AttachmentDescription::default()
                .format(vk::Format::B8G8R8A8_SRGB)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

            let color_attachment_ref = vk::AttachmentReference::default()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

            let needs_depth = desc.depth_stencil.depth_test_enable
                || desc.depth_stencil.stencil_test_enable;

            let mut attachments = vec![color_attachment];

            let depth_attachment_ref = vk::AttachmentReference::default()
                .attachment(1)
                .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

            if needs_depth {
                let depth_attachment = vk::AttachmentDescription::default()
                    .format(vk::Format::D32_SFLOAT)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
                attachments.push(depth_attachment);
            }

            let mut subpass = vk::SubpassDescription::default()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .color_attachments(std::slice::from_ref(&color_attachment_ref));

            if needs_depth {
                subpass = subpass.depth_stencil_attachment(&depth_attachment_ref);
            }

            let dst_stage = if needs_depth {
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
            } else {
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
            };
            let dst_access = if needs_depth {
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE
            } else {
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
            };

            let dependency = vk::SubpassDependency::default()
                .src_subpass(vk::SUBPASS_EXTERNAL)
                .dst_subpass(0)
                .src_stage_mask(dst_stage)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_stage_mask(dst_stage)
                .dst_access_mask(dst_access);

            let render_pass_info = vk::RenderPassCreateInfo::default()
                .attachments(&attachments)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(std::slice::from_ref(&dependency));

            let temp_render_pass = self.device.create_render_pass(&render_pass_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create temporary render pass for pipeline: {:?}", e))?;

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
                    format: self.buffer_format_to_vk(attribute.format),
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
            let rasterization_state = {
                let mut info = vk::PipelineRasterizationStateCreateInfo::default()
                    .depth_clamp_enable(false)
                    .rasterizer_discard_enable(false)
                    .polygon_mode(self.polygon_mode_to_vk(desc.rasterization.polygon_mode))
                    .line_width(1.0)
                    .cull_mode(self.cull_mode_to_vk(desc.rasterization.cull_mode))
                    .front_face(self.front_face_to_vk(desc.rasterization.front_face));
                if let Some(bias) = desc.rasterization.depth_bias {
                    info = info
                        .depth_bias_enable(true)
                        .depth_bias_constant_factor(bias.constant_factor)
                        .depth_bias_slope_factor(bias.slope_factor)
                        .depth_bias_clamp(bias.clamp);
                } else {
                    info = info.depth_bias_enable(false);
                }
                info
            };

            // Depth/stencil state
            let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(desc.depth_stencil.depth_test_enable)
                .depth_write_enable(desc.depth_stencil.depth_write_enable)
                .depth_compare_op(self.compare_op_to_vk(desc.depth_stencil.depth_compare_op))
                .depth_bounds_test_enable(false)
                .stencil_test_enable(desc.depth_stencil.stencil_test_enable)
                .front(self.stencil_op_state_to_vk(&desc.depth_stencil.front))
                .back(self.stencil_op_state_to_vk(&desc.depth_stencil.back));

            // Multisample state
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
                .sample_shading_enable(false)
                .rasterization_samples(self.sample_count_to_vk(desc.multisample.sample_count))
                .alpha_to_coverage_enable(desc.multisample.alpha_to_coverage);

            // Color blend state
            let color_blend_attachment = {
                let mut attachment = vk::PipelineColorBlendAttachmentState::default()
                    .color_write_mask(self.color_write_mask_to_vk(&desc.color_blend.color_write_mask))
                    .blend_enable(desc.color_blend.blend_enable);
                if desc.color_blend.blend_enable {
                    attachment = attachment
                        .src_color_blend_factor(self.blend_factor_to_vk(desc.color_blend.src_color_factor))
                        .dst_color_blend_factor(self.blend_factor_to_vk(desc.color_blend.dst_color_factor))
                        .color_blend_op(self.blend_op_to_vk(desc.color_blend.color_blend_op))
                        .src_alpha_blend_factor(self.blend_factor_to_vk(desc.color_blend.src_alpha_factor))
                        .dst_alpha_blend_factor(self.blend_factor_to_vk(desc.color_blend.dst_alpha_factor))
                        .alpha_blend_op(self.blend_op_to_vk(desc.color_blend.alpha_blend_op));
                }
                attachment
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

            // Create VkDescriptorSetLayouts from abstract BindingGroupLayoutDescs
            let mut descriptor_set_layouts: Vec<vk::DescriptorSetLayout> = Vec::new();
            for bg_layout_desc in &desc.binding_group_layouts {
                let bindings: Vec<vk::DescriptorSetLayoutBinding> = bg_layout_desc.entries
                    .iter()
                    .map(|entry| {
                        vk::DescriptorSetLayoutBinding::default()
                            .binding(entry.binding)
                            .descriptor_type(Self::binding_type_to_vk(entry.binding_type))
                            .descriptor_count(entry.count)
                            .stage_flags(Self::stage_flags_to_vk(entry.stage_flags))
                    })
                    .collect();

                let layout_create = vk::DescriptorSetLayoutCreateInfo::default()
                    .bindings(&bindings);

                let ds_layout = self.device.create_descriptor_set_layout(&layout_create, None)
                    .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create descriptor set layout: {:?}", e))?;

                descriptor_set_layouts.push(ds_layout);
            }

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create pipeline layout: {:?}", e))?;

            // Create pipeline
            let pipeline_create_info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input_state)
                .input_assembly_state(&input_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .depth_stencil_state(&depth_stencil_state)
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
            .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create graphics pipeline: {:?}", e.1))?;

            let pipeline = pipelines[0];

            // Destroy temporary render pass
            self.device.destroy_render_pass(temp_render_pass, None);

            // Merge SPIR-V reflections from vertex + fragment shaders
            let reflection = Self::merge_shader_reflections(&desc)?;

            Ok(Arc::new(Pipeline {
                pipeline,
                pipeline_layout: layout,
                descriptor_set_layouts,
                device: (*self.device).clone(),
                reflection,
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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "submit: failed to wait for fence: {:?}", e))?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| engine_err!("galaxy3d::vulkan", "submit: failed to reset fence: {:?}", e))?;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "submit: failed to submit queue: {:?}", e))?;

            Ok(())
        }
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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait for submit fence (swapchain): {:?}", e))?;

            // Reset fence
            self.device
                .reset_fences(&[self.submit_fences[self.current_submit_fence]])
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to reset submit fence (swapchain): {:?}", e))?;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to submit commands to GPU queue (swapchain): {:?}", e))?;

            Ok(())
        }
    }

    fn wait_idle(&self) -> Result<()> {
        unsafe {
            self.device
                .device_wait_idle()
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to wait idle: {:?}", e))
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

            // 1. Shutdown sampler cache: destroy VkSamplers + release Arc<GpuContext>
            //    Must happen first while device is alive.
            //    After this, self.gpu_context is the sole Arc<GpuContext> owner.
            self.sampler_cache.get_mut().unwrap().shutdown();

            // 2. Destroy VulkanRenderer-owned Vulkan objects
            for &fence in &self.submit_fences {
                self.device.destroy_fence(fence, None);
            }
            for &pool in self.descriptor_pools.get_mut().unwrap().iter() {
                self.device.destroy_descriptor_pool(pool, None);
            }

            // 3. Destroy upload command pool from GpuContext
            {
                let mut pool = self.gpu_context.upload_command_pool.lock().unwrap();
                if *pool != vk::CommandPool::null() {
                    self.device.destroy_command_pool(*pool, None);
                    *pool = vk::CommandPool::null();
                }
            }

            // 4. Drop allocator: free VkDeviceMemory pages BEFORE destroying device.
            //    First drop VulkanRenderer's Arc, then GpuContext's ManuallyDrop Arc.
            ManuallyDrop::drop(&mut self.allocator);
            if let Some(ctx) = Arc::get_mut(&mut self.gpu_context) {
                ManuallyDrop::drop(&mut ctx.allocator);
            }

            // 5. Cleanup debug config to prevent callbacks during destruction
            crate::debug::cleanup_debug_config();

            // 6. Destroy debug messenger BEFORE device and instance
            if let (Some(debug_utils), Some(messenger)) = (
                &self.gpu_context.debug_utils_loader,
                &self.gpu_context.debug_messenger,
            ) {
                debug_utils.destroy_debug_utils_messenger(*messenger, None);
            }

            // 7. Destroy device and instance
            self.device.destroy_device(None);
            self._instance.destroy_instance(None);
        }
    }
}

#[cfg(test)]
#[path = "vulkan_format_tests.rs"]
mod tests;