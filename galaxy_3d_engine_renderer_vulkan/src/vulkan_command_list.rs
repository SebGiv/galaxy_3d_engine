/// CommandList - Vulkan implementation of RendererCommandList trait

use galaxy_3d_engine::galaxy3d::{Result, Error};
use galaxy_3d_engine::galaxy3d::render::{
    CommandList as RendererCommandList,
    RenderPass as RendererRenderPass,
    RenderTarget as RendererRenderTarget,
    Pipeline as RendererPipeline,
    Buffer as RendererBuffer,
    DescriptorSet as RendererDescriptorSet,
    Viewport, Rect2D, ClearValue,
};
use galaxy_3d_engine::engine_error;
use ash::vk;
use std::sync::Arc;

use crate::vulkan_render_target::RenderTarget;
use crate::vulkan_render_pass::RenderPass;
use crate::vulkan_pipeline::Pipeline;
use crate::vulkan_buffer::Buffer;
use crate::vulkan_descriptor_set::DescriptorSet;

/// Vulkan command list implementation
///
/// Records rendering commands for later submission to the GPU.
pub struct CommandList {
    /// Vulkan device
    device: Arc<ash::Device>,
    /// Command pool for allocating command buffers
    command_pool: vk::CommandPool,
    /// Command buffer for recording
    command_buffer: vk::CommandBuffer,
    /// Whether the command list is currently recording
    is_recording: bool,
    /// Whether we're inside a render pass
    in_render_pass: bool,
    /// Currently bound pipeline layout (for push constants)
    bound_pipeline_layout: Option<vk::PipelineLayout>,
    /// Framebuffers created during recording (destroyed after command buffer is done)
    framebuffers: Vec<vk::Framebuffer>,
}

impl CommandList {
    /// Create a new command list
    ///
    /// # Arguments
    ///
    /// * `device` - Vulkan logical device
    /// * `graphics_queue_family` - Graphics queue family index
    pub fn new(
        device: Arc<ash::Device>,
        graphics_queue_family: u32,
    ) -> Result<Self> {
        unsafe {
            // Create command pool
            let command_pool_create_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_queue_family)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

            let command_pool = device.create_command_pool(&command_pool_create_info, None)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to create command pool: {:?}", e);
                    Error::BackendError(format!("Failed to create command pool: {:?}", e))
                })?;

            // Allocate command buffer
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let command_buffers = device.allocate_command_buffers(&command_buffer_allocate_info)
                .map_err(|e| {
                    engine_error!("galaxy3d::vulkan", "Failed to allocate command buffer: {:?}", e);
                    Error::BackendError(format!("Failed to allocate command buffers: {:?}", e))
                })?;

            Ok(Self {
                device,
                command_pool,
                command_buffer: command_buffers[0],
                is_recording: false,
                in_render_pass: false,
                bound_pipeline_layout: None,
                framebuffers: Vec::new(),
            })
        }
    }

    /// Get the underlying Vulkan command buffer
    pub fn command_buffer(&self) -> vk::CommandBuffer {
        self.command_buffer
    }

    /// Bind descriptor sets to the command buffer
    ///
    /// # Arguments
    ///
    /// * `descriptor_sets` - Array of descriptor sets to bind
    /// * `pipeline_layout` - Pipeline layout that the descriptor sets are compatible with
    pub fn bind_descriptor_sets(
        &mut self,
        descriptor_sets: &[vk::DescriptorSet],
        pipeline_layout: vk::PipelineLayout,
    ) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0, // first_set
                descriptor_sets,
                &[], // dynamic_offsets
            );

            Ok(())
        }
    }
}

impl RendererCommandList for CommandList {
    fn begin(&mut self) -> Result<()> {
        if self.is_recording {
            return Err(Error::BackendError("Command list already recording".to_string()));
        }

        unsafe {
            // Reset command buffer
            self.device
                .reset_command_buffer(
                    self.command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )
                .map_err(|e| Error::BackendError(format!("Failed to reset command buffer: {:?}", e)))?;

            // Begin command buffer
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .map_err(|e| Error::BackendError(format!("Failed to begin command buffer: {:?}", e)))?;

            self.is_recording = true;
            self.in_render_pass = false;
            self.bound_pipeline_layout = None;

            // Destroy old framebuffers before starting recording
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            Ok(())
        }
    }

    fn end(&mut self) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        if self.in_render_pass {
            return Err(Error::BackendError("Render pass not ended before ending command list".to_string()));
        }

        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|e| Error::BackendError(format!("Failed to end command buffer: {:?}", e)))?;

            self.is_recording = false;

            Ok(())
        }
    }

    fn begin_render_pass(
        &mut self,
        render_pass: &Arc<dyn RendererRenderPass>,
        render_target: &Arc<dyn RendererRenderTarget>,
        clear_values: &[ClearValue],
    ) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        if self.in_render_pass {
            return Err(Error::BackendError("Already inside a render pass".to_string()));
        }

        unsafe {
            // Downcast to Vulkan types
            let vk_render_pass = render_pass.as_ref() as *const dyn RendererRenderPass as *const RenderPass;
            let vk_render_pass = &*vk_render_pass;

            let vk_render_target = render_target.as_ref() as *const dyn RendererRenderTarget as *const RenderTarget;
            let vk_render_target = &*vk_render_target;

            // Convert clear values
            let vk_clear_values: Vec<vk::ClearValue> = clear_values
                .iter()
                .map(|cv| match cv {
                    ClearValue::Color(color) => vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: *color,
                        },
                    },
                    ClearValue::DepthStencil { depth, stencil } => vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: *depth,
                            stencil: *stencil,
                        },
                    },
                })
                .collect();

            // Create framebuffer on the fly
            // TODO: Cache framebuffers for performance
            let attachments = [vk_render_target.image_view];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(vk_render_pass.render_pass)
                .attachments(&attachments)
                .width(render_target.width())
                .height(render_target.height())
                .layers(1);

            let framebuffer = self.device.create_framebuffer(&framebuffer_info, None)
                .map_err(|e| Error::BackendError(format!("Failed to create framebuffer: {:?}", e)))?;

            // Store framebuffer for destruction in begin() or Drop
            self.framebuffers.push(framebuffer);

            // Begin render pass
            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(vk_render_pass.render_pass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: render_target.width(),
                        height: render_target.height(),
                    },
                })
                .clear_values(&vk_clear_values);

            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );

            self.in_render_pass = true;

            Ok(())
        }
    }

    fn end_render_pass(&mut self) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        if !self.in_render_pass {
            return Err(Error::BackendError("Not inside a render pass".to_string()));
        }

        unsafe {
            self.device.cmd_end_render_pass(self.command_buffer);
            self.in_render_pass = false;

            // Framebuffers will be destroyed in begin() or Drop
            Ok(())
        }
    }

    fn set_viewport(&mut self, viewport: Viewport) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            let vk_viewport = vk::Viewport::default()
                .x(viewport.x)
                .y(viewport.y)
                .width(viewport.width)
                .height(viewport.height)
                .min_depth(viewport.min_depth)
                .max_depth(viewport.max_depth);

            self.device.cmd_set_viewport(self.command_buffer, 0, &[vk_viewport]);

            Ok(())
        }
    }

    fn set_scissor(&mut self, scissor: Rect2D) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            let vk_scissor = vk::Rect2D::default()
                .offset(vk::Offset2D { x: scissor.x, y: scissor.y })
                .extent(vk::Extent2D { width: scissor.width, height: scissor.height });

            self.device.cmd_set_scissor(self.command_buffer, 0, &[vk_scissor]);

            Ok(())
        }
    }

    fn bind_pipeline(&mut self, pipeline: &Arc<dyn RendererPipeline>) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            // Downcast to Vulkan type
            let vk_pipeline = pipeline.as_ref() as *const dyn RendererPipeline as *const Pipeline;
            let vk_pipeline = &*vk_pipeline;

            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                vk_pipeline.pipeline,
            );

            // Save pipeline layout for push constants
            self.bound_pipeline_layout = Some(vk_pipeline.pipeline_layout);

            Ok(())
        }
    }

    fn push_constants(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        let layout = self.bound_pipeline_layout.ok_or_else(|| {
            Error::BackendError("No pipeline bound for push constants".to_string())
        })?;

        unsafe {
            // TODO: Use proper stage flags from pipeline layout
            // For now, use VERTEX only (matching our push constant range)
            self.device.cmd_push_constants(
                self.command_buffer,
                layout,
                vk::ShaderStageFlags::VERTEX,
                offset,
                data,
            );

            Ok(())
        }
    }

    fn bind_vertex_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            // Downcast to Vulkan type
            let vk_buffer = buffer.as_ref() as *const dyn RendererBuffer as *const Buffer;
            let vk_buffer = &*vk_buffer;

            self.device.cmd_bind_vertex_buffers(
                self.command_buffer,
                0,
                &[vk_buffer.buffer],
                &[offset],
            );

            Ok(())
        }
    }

    fn bind_index_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            // Downcast to Vulkan type
            let vk_buffer = buffer.as_ref() as *const dyn RendererBuffer as *const Buffer;
            let vk_buffer = &*vk_buffer;

            self.device.cmd_bind_index_buffer(
                self.command_buffer,
                vk_buffer.buffer,
                offset,
                vk::IndexType::UINT32, // TODO: Support UINT16
            );

            Ok(())
        }
    }

    fn draw(&mut self, vertex_count: u32, first_vertex: u32) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        if !self.in_render_pass {
            return Err(Error::BackendError("Not inside a render pass".to_string()));
        }

        unsafe {
            self.device.cmd_draw(
                self.command_buffer,
                vertex_count,
                1, // instance_count
                first_vertex,
                0, // first_instance
            );

            Ok(())
        }
    }

    fn draw_indexed(&mut self, index_count: u32, first_index: u32, vertex_offset: i32) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        if !self.in_render_pass {
            return Err(Error::BackendError("Not inside a render pass".to_string()));
        }

        unsafe {
            self.device.cmd_draw_indexed(
                self.command_buffer,
                index_count,
                1, // instance_count
                first_index,
                vertex_offset,
                0, // first_instance
            );

            Ok(())
        }
    }

    fn bind_descriptor_sets(
        &mut self,
        pipeline: &Arc<dyn RendererPipeline>,
        descriptor_sets: &[&Arc<dyn RendererDescriptorSet>],
    ) -> Result<()> {
        if !self.is_recording {
            return Err(Error::BackendError("Command list not recording".to_string()));
        }

        unsafe {
            // Downcast pipeline to extract pipeline_layout (now private)
            let vk_pipeline = pipeline.as_ref() as *const dyn RendererPipeline as *const Pipeline;
            let vk_pipeline = &*vk_pipeline;
            let pipeline_layout = vk_pipeline.pipeline_layout;

            // Downcast descriptor sets from abstract types to Vulkan types
            let vk_descriptor_sets: Vec<vk::DescriptorSet> = descriptor_sets
                .iter()
                .map(|ds| {
                    let vk_ds = ds.as_ref() as *const dyn RendererDescriptorSet as *const DescriptorSet;
                    (*vk_ds).descriptor_set
                })
                .collect();

            // Bind Vulkan descriptor sets
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0, // first_set
                &vk_descriptor_sets,
                &[], // dynamic_offsets
            );

            Ok(())
        }
    }
}

impl Drop for CommandList {
    fn drop(&mut self) {
        unsafe {
            // Destroy all remaining framebuffers
            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            // Free command buffer (automatically freed when pool is destroyed)
            // Destroy command pool
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}
