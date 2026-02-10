/// Texture - Vulkan implementation of RendererTexture trait

use galaxy_3d_engine::galaxy3d::{Result, Error};
use galaxy_3d_engine::galaxy3d::render::Texture as RendererTexture;
use galaxy_3d_engine::galaxy3d::render::TextureInfo;
use galaxy_3d_engine::{engine_error, engine_bail, engine_err, engine_warn_err};
use ash::vk;
use gpu_allocator::vulkan::Allocation;
use std::sync::Arc;

use crate::vulkan_context::GpuContext;

/// Vulkan texture implementation
pub struct Texture {
    /// Shared GPU context (device, allocator, queue, command pool)
    ctx: Arc<GpuContext>,
    /// Vulkan image
    pub(crate) image: vk::Image,
    /// Vulkan image view
    pub(crate) view: vk::ImageView,
    /// GPU memory allocation
    pub(crate) allocation: Option<Allocation>,
    /// Read-only texture properties
    pub(crate) info: TextureInfo,
}

impl Texture {
    /// Create a new Vulkan texture
    pub fn new(
        ctx: Arc<GpuContext>,
        image: vk::Image,
        view: vk::ImageView,
        allocation: Allocation,
        info: TextureInfo,
    ) -> Self {
        Self {
            ctx,
            image,
            view,
            allocation: Some(allocation),
            info,
        }
    }
}

impl RendererTexture for Texture {
    fn info(&self) -> &TextureInfo {
        &self.info
    }

    fn update(&self, layer: u32, mip_level: u32, data: &[u8]) -> Result<()> {
        // Validate layer index
        if layer >= self.info.array_layers {
            engine_bail!("galaxy3d::vulkan", "update: layer {} out of range (array_layers = {})", layer, self.info.array_layers);
        }

        // Validate mip level
        if mip_level >= self.info.mip_levels {
            engine_bail!("galaxy3d::vulkan", "update: mip_level {} out of range (mip_levels = {})", mip_level, self.info.mip_levels);
        }

        // Calculate expected size for this mip level
        let mip_width = (self.info.width >> mip_level).max(1);
        let mip_height = (self.info.height >> mip_level).max(1);
        let expected_size = (mip_width * mip_height * self.info.format.bytes_per_pixel()) as usize;

        if data.len() != expected_size {
            engine_bail!("galaxy3d::vulkan", "update: data size {} doesn't match expected {} for mip level {} ({}x{})",
                data.len(), expected_size, mip_level, mip_width, mip_height);
        }

        unsafe {
            let device = &self.ctx.device;
            let buffer_size = data.len() as u64;

            // Create staging buffer
            let staging_buffer_create_info = vk::BufferCreateInfo::default()
                .size(buffer_size)
                .usage(vk::BufferUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let staging_buffer = device.create_buffer(&staging_buffer_create_info, None)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to create staging buffer: {:?}", e))?;

            let staging_requirements = device.get_buffer_memory_requirements(staging_buffer);

            let staging_allocation = self.ctx.allocator.lock().unwrap()
                .allocate(&gpu_allocator::vulkan::AllocationCreateDesc {
                    name: "texture_layer_staging",
                    requirements: staging_requirements,
                    location: gpu_allocator::MemoryLocation::CpuToGpu,
                    linear: true,
                    allocation_scheme: gpu_allocator::vulkan::AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|_e| {
                    engine_error!("galaxy3d::vulkan", "update: out of GPU memory for staging buffer");
                    Error::OutOfMemory
                })?;

            device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to bind staging buffer memory: {:?}", e))?;

            // Copy data to staging buffer
            let mapped_ptr = staging_allocation.mapped_ptr()
                .ok_or_else(|| engine_err!("galaxy3d::vulkan", "Texture update: staging buffer is not mapped"))?
                .as_ptr() as *mut u8;
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped_ptr, data.len());

            // Get command pool and allocate command buffer
            let command_pool = *self.ctx.upload_command_pool.lock().unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let command_buffers = device.allocate_command_buffers(&command_buffer_allocate_info)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to allocate command buffer: {:?}", e))?;
            let command_buffer = command_buffers[0];

            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            device.begin_command_buffer(command_buffer, &begin_info)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to begin command buffer: {:?}", e))?;

            // Transition single layer/mip: SHADER_READ_ONLY → TRANSFER_DST
            let barrier_to_transfer = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(self.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: mip_level,
                    level_count: 1,
                    base_array_layer: layer,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

            device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_transfer],
            );

            // Copy buffer to image layer/mip
            let region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level,
                    base_array_layer: layer,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: mip_width,
                    height: mip_height,
                    depth: 1,
                });

            device.cmd_copy_buffer_to_image(
                command_buffer,
                staging_buffer,
                self.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );

            // Transition single layer/mip: TRANSFER_DST → SHADER_READ_ONLY
            let barrier_to_shader = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(self.image)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: mip_level,
                    level_count: 1,
                    base_array_layer: layer,
                    layer_count: 1,
                })
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ);

            device.cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier_to_shader],
            );

            // End recording, submit, and wait
            device.end_command_buffer(command_buffer)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to end command buffer: {:?}", e))?;

            let command_buffers_submit = [command_buffer];
            let submit_info = vk::SubmitInfo::default()
                .command_buffers(&command_buffers_submit);

            device.queue_submit(self.ctx.graphics_queue, &[submit_info], vk::Fence::null())
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to submit commands: {:?}", e))?;

            device.queue_wait_idle(self.ctx.graphics_queue)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "update: failed to wait for completion: {:?}", e))?;

            // Clean up staging buffer (command buffer will be reset automatically)
            device.free_command_buffers(command_pool, &[command_buffer]);
            device.destroy_buffer(staging_buffer, None);
            self.ctx.allocator.lock().unwrap().free(staging_allocation)
                .map_err(|_e| engine_warn_err!("galaxy3d::vulkan", "Texture update: failed to free staging allocation"))?;

            Ok(())
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            // Destroy image view
            self.ctx.device.destroy_image_view(self.view, None);

            // Free GPU memory
            if let Some(allocation) = self.allocation.take() {
                self.ctx.allocator.lock().unwrap().free(allocation).ok();
            }

            // Destroy image
            self.ctx.device.destroy_image(self.image, None);
        }
    }
}