/// CommandList - Vulkan implementation of RendererCommandList trait

use galaxy_3d_engine::galaxy3d::Result;
use galaxy_3d_engine::galaxy3d::render::{
    CommandList as RendererCommandList,
    RenderPass as RendererRenderPass,
    Framebuffer as RendererFramebuffer,
    Pipeline as RendererPipeline,
    Buffer as RendererBuffer,
    BindingGroup as RendererBindingGroup,
    Viewport, Rect2D, ClearValue, IndexType, ShaderStage,
};
use galaxy_3d_engine::{engine_bail, engine_err};
use ash::vk;
use std::sync::Arc;

use crate::vulkan_render_pass::RenderPass;
use crate::vulkan_frame_buffer::Framebuffer;
use crate::vulkan_pipeline::Pipeline;
use crate::vulkan_buffer::Buffer;
use crate::vulkan_binding_group::BindingGroup;

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
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to create command pool: {:?}", e))?;

            // Allocate command buffer
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);

            let command_buffers = device.allocate_command_buffers(&command_buffer_allocate_info)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to allocate command buffer: {:?}", e))?;

            Ok(Self {
                device,
                command_pool,
                command_buffer: command_buffers[0],
                is_recording: false,
                in_render_pass: false,
                bound_pipeline_layout: None,
            })
        }
    }

    /// Get the underlying Vulkan command buffer
    pub fn command_buffer(&self) -> vk::CommandBuffer {
        self.command_buffer
    }

    /// Bind a single descriptor set to the command buffer at a given set index
    ///
    /// # Arguments
    ///
    /// * `descriptor_set` - The descriptor set to bind
    /// * `pipeline_layout` - Pipeline layout that the descriptor set is compatible with
    /// * `first_set` - The set index to bind at
    pub fn bind_descriptor_set_at(
        &mut self,
        descriptor_set: vk::DescriptorSet,
        pipeline_layout: vk::PipelineLayout,
        first_set: u32,
    ) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "bind_descriptor_set_at: command list not recording");
        }

        unsafe {
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                first_set,
                &[descriptor_set],
                &[], // dynamic_offsets
            );

            Ok(())
        }
    }
}

impl RendererCommandList for CommandList {
    fn begin(&mut self) -> Result<()> {
        if self.is_recording {
            engine_bail!("galaxy3d::vulkan", "begin: command list already recording");
        }

        unsafe {
            // Reset command buffer
            self.device
                .reset_command_buffer(
                    self.command_buffer,
                    vk::CommandBufferResetFlags::empty(),
                )
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to reset command buffer: {:?}", e))?;

            // Begin command buffer
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to begin command buffer: {:?}", e))?;

            self.is_recording = true;
            self.in_render_pass = false;
            self.bound_pipeline_layout = None;

            Ok(())
        }
    }

    fn end(&mut self) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "end: command list not recording");
        }

        if self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "end: render pass not ended before ending command list");
        }

        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|e| engine_err!("galaxy3d::vulkan", "Failed to end command buffer: {:?}", e))?;

            self.is_recording = false;

            Ok(())
        }
    }

    fn begin_render_pass(
        &mut self,
        render_pass: &Arc<dyn RendererRenderPass>,
        framebuffer: &Arc<dyn RendererFramebuffer>,
        clear_values: &[ClearValue],
    ) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "begin_render_pass: command list not recording");
        }

        if self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "begin_render_pass: already inside a render pass");
        }

        unsafe {
            // Downcast to Vulkan types
            let vk_render_pass = render_pass.as_ref()
                as *const dyn RendererRenderPass
                as *const RenderPass;
            let vk_render_pass = &*vk_render_pass;

            let vk_framebuffer = framebuffer.as_ref()
                as *const dyn RendererFramebuffer
                as *const Framebuffer;
            let vk_framebuffer = &*vk_framebuffer;

            // Convert clear values
            let vk_clear_values: Vec<vk::ClearValue> = clear_values
                .iter()
                .map(|cv| match cv {
                    ClearValue::Color(rgba) => vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: *rgba,
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

            let render_area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: vk_framebuffer.width(),
                    height: vk_framebuffer.height(),
                },
            };

            let render_pass_begin_info = vk::RenderPassBeginInfo::default()
                .render_pass(vk_render_pass.render_pass)
                .framebuffer(vk_framebuffer.framebuffer)
                .render_area(render_area)
                .clear_values(&vk_clear_values);

            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            self.in_render_pass = true;

            Ok(())
        }
    }

    fn end_render_pass(&mut self) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "end_render_pass: command list not recording");
        }

        if !self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "end_render_pass: not inside a render pass");
        }

        unsafe {
            self.device.cmd_end_render_pass(self.command_buffer);
            self.in_render_pass = false;

            Ok(())
        }
    }

    fn set_viewport(&mut self, viewport: Viewport) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "set_viewport: command list not recording");
        }

        unsafe {
            // Viewport Y-flip: Vulkan framebuffer is Y-down, but the engine uses Y-up
            let vk_viewport = vk::Viewport::default()
                .x(viewport.x)
                .y(viewport.y + viewport.height)
                .width(viewport.width)
                .height(-viewport.height)
                .min_depth(viewport.min_depth)
                .max_depth(viewport.max_depth);

            self.device.cmd_set_viewport(self.command_buffer, 0, &[vk_viewport]);

            Ok(())
        }
    }

    fn set_scissor(&mut self, scissor: Rect2D) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "set_scissor: command list not recording");
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
            engine_bail!("galaxy3d::vulkan", "bind_pipeline: command list not recording");
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

    fn push_constants(&mut self, stages: &[ShaderStage], offset: u32, data: &[u8]) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "push_constants: command list not recording");
        }

        let layout = self.bound_pipeline_layout.ok_or_else(|| engine_err!("galaxy3d::vulkan", "push_constants: no pipeline bound"))?;

        // Convert ShaderStage to vk::ShaderStageFlags
        let mut stage_flags = vk::ShaderStageFlags::empty();
        for stage in stages {
            stage_flags |= match stage {
                ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
                ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
                ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
            };
        }

        unsafe {
            self.device.cmd_push_constants(
                self.command_buffer,
                layout,
                stage_flags,
                offset,
                data,
            );

            Ok(())
        }
    }

    fn bind_vertex_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "bind_vertex_buffer: command list not recording");
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

    fn bind_index_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64, index_type: IndexType) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "bind_index_buffer: command list not recording");
        }

        unsafe {
            // Downcast to Vulkan type
            let vk_buffer = buffer.as_ref() as *const dyn RendererBuffer as *const Buffer;
            let vk_buffer = &*vk_buffer;

            // Convert engine IndexType to Vulkan IndexType
            let vk_index_type = match index_type {
                IndexType::U16 => vk::IndexType::UINT16,
                IndexType::U32 => vk::IndexType::UINT32,
            };

            self.device.cmd_bind_index_buffer(
                self.command_buffer,
                vk_buffer.buffer,
                offset,
                vk_index_type,
            );

            Ok(())
        }
    }

    fn draw(&mut self, vertex_count: u32, first_vertex: u32) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "draw: command list not recording");
        }

        if !self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "draw: not inside a render pass");
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
            engine_bail!("galaxy3d::vulkan", "draw_indexed: command list not recording");
        }

        if !self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "draw_indexed: not inside a render pass");
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

    fn bind_binding_group(
        &mut self,
        pipeline: &Arc<dyn RendererPipeline>,
        set_index: u32,
        binding_group: &Arc<dyn RendererBindingGroup>,
    ) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "bind_binding_group: command list not recording");
        }

        unsafe {
            // Downcast pipeline to extract pipeline_layout
            let vk_pipeline = pipeline.as_ref() as *const dyn RendererPipeline as *const Pipeline;
            let vk_pipeline = &*vk_pipeline;
            let pipeline_layout = vk_pipeline.pipeline_layout;

            // Downcast binding group to extract descriptor set
            let vk_bg = binding_group.as_ref() as *const dyn RendererBindingGroup as *const BindingGroup;
            let vk_bg = &*vk_bg;

            // Bind single descriptor set at the given set index
            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                set_index,
                &[vk_bg.descriptor_set],
                &[], // dynamic_offsets
            );

            Ok(())
        }
    }
}

impl Drop for CommandList {
    fn drop(&mut self) {
        unsafe {
            // Free command buffer (automatically freed when pool is destroyed)
            // Destroy command pool
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}