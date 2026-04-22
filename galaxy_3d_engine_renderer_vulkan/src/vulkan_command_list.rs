/// CommandList - Vulkan implementation of RendererCommandList trait

use galaxy_3d_engine::galaxy3d::Result;
use galaxy_3d_engine::galaxy3d::render::{
    CommandList as RendererCommandList,
    RenderPass as RendererRenderPass,
    Framebuffer as RendererFramebuffer,
    Pipeline as RendererPipeline,
    Buffer as RendererBuffer,
    BindingGroup as RendererBindingGroup,
    Texture as RendererTexture,
    Viewport, Rect2D, ClearValue, IndexType, ShaderStageFlags,
    ImageAccess, AccessType, TextureFormat,
    DynamicRenderState, LoadOp, StoreOp,
    CullMode, FrontFace, CompareOp, StencilOp, ColorWriteMask,
};
use galaxy_3d_engine::{engine_bail, engine_err};
use ash::vk;
use std::sync::Arc;

use crate::vulkan_render_pass::RenderPass;
use crate::vulkan_frame_buffer::Framebuffer;
use crate::vulkan_pipeline::Pipeline;
use crate::vulkan_buffer::Buffer;
use crate::vulkan_binding_group::BindingGroup;
use crate::vulkan_texture::Texture as VulkanTexture;

impl CommandList {
    fn stage_flags_to_vk(flags: ShaderStageFlags) -> vk::ShaderStageFlags {
        let mut vk_flags = vk::ShaderStageFlags::empty();
        if flags.contains_vertex() { vk_flags |= vk::ShaderStageFlags::VERTEX; }
        if flags.contains_fragment() { vk_flags |= vk::ShaderStageFlags::FRAGMENT; }
        if flags.contains_compute() { vk_flags |= vk::ShaderStageFlags::COMPUTE; }
        vk_flags
    }
}

/// Initial capacity reserved in the `CommandList` scratch buffers. Chosen so
/// that the capacity usually covers the largest render pass the engine ever
/// builds — past that, the `Vec` will grow once and keep its new capacity
/// for all subsequent frames (no per-frame allocation in steady state).
const SCRATCH_CAPACITY: usize = 8;

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
    /// Bindless descriptor set (set 0) — bound on every bind_pipeline
    bindless_descriptor_set: vk::DescriptorSet,
    /// Scratch buffer reused every `begin_render_pass` to collect image
    /// barriers. Cleared before use; capacity grows to fit the largest
    /// render pass seen so far, then stays allocated — no heap
    /// allocation in steady state.
    barriers_scratch: Vec<vk::ImageMemoryBarrier2<'static>>,
    /// Scratch buffer reused every `begin_render_pass` to collect the
    /// per-color-attachment `VkRenderingAttachmentInfo`. Same policy as
    /// `barriers_scratch`.
    color_infos_scratch: Vec<vk::RenderingAttachmentInfo<'static>>,
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
        bindless_descriptor_set: vk::DescriptorSet,
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
                bindless_descriptor_set,
                barriers_scratch: Vec::with_capacity(SCRATCH_CAPACITY),
                color_infos_scratch: Vec::with_capacity(SCRATCH_CAPACITY),
            })
        }
    }

    /// Get the underlying Vulkan command buffer
    pub fn command_buffer(&self) -> vk::CommandBuffer {
        self.command_buffer
    }

    /// Map an AccessType to the corresponding Vulkan image layout.
    fn access_type_to_layout(access: AccessType) -> vk::ImageLayout {
        match access {
            AccessType::ColorAttachmentWrite | AccessType::ColorAttachmentRead
                => vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            AccessType::DepthStencilWrite
                => vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            AccessType::DepthStencilReadOnly
                => vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            AccessType::FragmentShaderRead | AccessType::VertexShaderRead
            | AccessType::ComputeRead | AccessType::RayTracingRead
                => vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessType::ComputeWrite
                => vk::ImageLayout::GENERAL,
            AccessType::TransferRead
                => vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            AccessType::TransferWrite
                => vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        }
    }

    /// Returns true if the texture format is a depth or depth/stencil format.
    fn is_depth_format(format: TextureFormat) -> bool {
        matches!(format,
            TextureFormat::D16_UNORM
            | TextureFormat::D32_FLOAT
            | TextureFormat::D24_UNORM_S8_UINT
            | TextureFormat::D32_FLOAT_S8_UINT
        )
    }

    /// Map an engine `LoadOp` to the Vulkan attachment load op.
    fn load_op_to_vk(op: LoadOp) -> vk::AttachmentLoadOp {
        match op {
            LoadOp::Load => vk::AttachmentLoadOp::LOAD,
            LoadOp::Clear => vk::AttachmentLoadOp::CLEAR,
            LoadOp::DontCare => vk::AttachmentLoadOp::DONT_CARE,
        }
    }

    /// Map an engine `StoreOp` to the Vulkan attachment store op.
    fn store_op_to_vk(op: StoreOp) -> vk::AttachmentStoreOp {
        match op {
            StoreOp::Store => vk::AttachmentStoreOp::STORE,
            StoreOp::DontCare => vk::AttachmentStoreOp::DONT_CARE,
        }
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
        accesses: &[ImageAccess],
    ) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "begin_render_pass: command list not recording");
        }

        if self.in_render_pass {
            engine_bail!("galaxy3d::vulkan", "begin_render_pass: already inside a render pass");
        }

        unsafe {
            // Dynamic rendering: with no VkRenderPass, layout transitions that
            // used to be carried by subpass dependencies / initialLayout must
            // now be emitted explicitly here, for ALL accesses (attachments
            // included). When `previous_access_type` is None (first use of
            // the texture in this frame) we transition from UNDEFINED.
            //
            // All barriers are batched into a single `vkCmdPipelineBarrier2`
            // call (synchronization2) so the driver can combine stages and
            // accesses optimally.
            // Reuse the persistent scratch buffer from `self`: `clear()`
            // resets the length to 0 while keeping the already-allocated
            // capacity, so no heap allocation happens per frame in steady
            // state.
            self.barriers_scratch.clear();

            for access in accesses {
                let new_layout = Self::access_type_to_layout(access.access_type);
                let (dst_stage, dst_access) =
                    crate::vulkan_sync::access_type_to_stage_access_2(access.access_type);

                let (old_layout, src_stage, src_access) = match access.previous_access_type {
                    Some(prev) => {
                        let layout = Self::access_type_to_layout(prev);
                        let (stage, acc) =
                            crate::vulkan_sync::access_type_to_stage_access_2(prev);
                        (layout, stage, acc)
                    }
                    None => (
                        vk::ImageLayout::UNDEFINED,
                        vk::PipelineStageFlags2::NONE,
                        vk::AccessFlags2::NONE,
                    ),
                };

                // Skip emission when neither a layout transition nor a
                // synchronization between two accesses is required.
                if old_layout == new_layout && access.previous_access_type.is_none() {
                    continue;
                }

                let vk_texture = access.texture.as_ref()
                    as *const dyn RendererTexture
                    as *const VulkanTexture;
                let vk_texture = &*vk_texture;

                let aspect_mask = if Self::is_depth_format(vk_texture.info.format) {
                    vk::ImageAspectFlags::DEPTH
                } else {
                    vk::ImageAspectFlags::COLOR
                };

                self.barriers_scratch.push(crate::vulkan_sync::image_barrier2(
                    vk_texture.image,
                    aspect_mask,
                    old_layout,
                    new_layout,
                    src_stage,
                    src_access,
                    dst_stage,
                    dst_access,
                ));
            }

            crate::vulkan_sync::emit_image_barriers2(
                &self.device,
                self.command_buffer,
                &self.barriers_scratch,
            );
            // Downcast to Vulkan types
            let vk_render_pass = render_pass.as_ref()
                as *const dyn RendererRenderPass
                as *const RenderPass;
            let vk_render_pass = &*vk_render_pass;

            let vk_framebuffer = framebuffer.as_ref()
                as *const dyn RendererFramebuffer
                as *const Framebuffer;
            let vk_framebuffer = &*vk_framebuffer;

            // Dynamic rendering: build `VkRenderingAttachmentInfo` per
            // attachment inline instead of a pre-baked VkRenderPass. Clear
            // values are expected in the same order as the render pass's
            // attachments: color_0, color_1, ..., then depth if present.
            let color_count = vk_render_pass.color_attachments.len();
            let has_depth = vk_render_pass.depth_stencil_attachment.is_some();
            let has_resolve = !vk_render_pass.color_resolve_attachments.is_empty();

            // Same zero-alloc pattern as `barriers_scratch` above.
            self.color_infos_scratch.clear();
            let _ = color_count; // kept for readability above, actual count = Vec len
            for (i, color_att) in vk_render_pass.color_attachments.iter().enumerate() {
                let clear_value = match clear_values.get(i) {
                    Some(ClearValue::Color(rgba)) => vk::ClearValue {
                        color: vk::ClearColorValue { float32: *rgba },
                    },
                    _ => vk::ClearValue::default(),
                };

                let mut info = vk::RenderingAttachmentInfo::default()
                    .image_view(vk_framebuffer.color_image_views[i])
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(Self::load_op_to_vk(color_att.load_op))
                    .store_op(Self::store_op_to_vk(color_att.store_op))
                    .clear_value(clear_value);

                if has_resolve {
                    info = info
                        .resolve_mode(vk::ResolveModeFlags::AVERAGE)
                        .resolve_image_view(vk_framebuffer.resolve_image_views[i])
                        .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
                }

                self.color_infos_scratch.push(info);
            }

            let depth_rendering_info = vk_render_pass.depth_stencil_attachment.as_ref().map(|d| {
                let clear_value = match clear_values.get(color_count) {
                    Some(ClearValue::DepthStencil { depth, stencil }) => vk::ClearValue {
                        depth_stencil: vk::ClearDepthStencilValue {
                            depth: *depth,
                            stencil: *stencil,
                        },
                    },
                    _ => vk::ClearValue::default(),
                };

                vk::RenderingAttachmentInfo::default()
                    .image_view(vk_framebuffer.depth_image_view.expect(
                        "Framebuffer has no depth image view but RenderPass declared one",
                    ))
                    .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .load_op(Self::load_op_to_vk(d.load_op))
                    .store_op(Self::store_op_to_vk(d.store_op))
                    .clear_value(clear_value)
            });

            let render_area = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: vk_framebuffer.width(),
                    height: vk_framebuffer.height(),
                },
            };

            let mut rendering_info = vk::RenderingInfo::default()
                .render_area(render_area)
                .layer_count(1)
                .color_attachments(&self.color_infos_scratch);

            if let Some(ref depth_info) = depth_rendering_info {
                rendering_info = rendering_info.depth_attachment(depth_info);
            }
            // Silence unused-var warning in the no-depth branch.
            let _ = has_depth;

            self.device.cmd_begin_rendering(self.command_buffer, &rendering_info);

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
            self.device.cmd_end_rendering(self.command_buffer);
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

    fn set_dynamic_state(&mut self, state: &DynamicRenderState) -> Result<()> {
        debug_assert!(self.is_recording, "set_dynamic_state: command list not recording");

        unsafe {
            let cb = self.command_buffer;

            // Rasterization
            self.device.cmd_set_cull_mode(cb, cull_mode_to_vk(state.cull_mode));
            self.device.cmd_set_front_face(cb, front_face_to_vk(state.front_face));

            // Depth
            self.device.cmd_set_depth_test_enable(cb, state.depth_test_enable);
            self.device.cmd_set_depth_write_enable(cb, state.depth_write_enable);
            self.device.cmd_set_depth_compare_op(cb, compare_op_to_vk(state.depth_compare_op));
            self.device.cmd_set_depth_bias_enable(cb, state.depth_bias_enable);
            self.device.cmd_set_depth_bias(
                cb,
                state.depth_bias.constant_factor,
                state.depth_bias.clamp,
                state.depth_bias.slope_factor,
            );
            self.device.cmd_set_depth_bounds_test_enable(cb, state.depth_bounds_test_enable);
            self.device.cmd_set_depth_bounds(cb, state.depth_bounds_min, state.depth_bounds_max);

            // Stencil
            self.device.cmd_set_stencil_test_enable(cb, state.stencil_test_enable);
            self.device.cmd_set_stencil_op(
                cb, vk::StencilFaceFlags::FRONT,
                stencil_op_to_vk(state.stencil_front.fail_op),
                stencil_op_to_vk(state.stencil_front.pass_op),
                stencil_op_to_vk(state.stencil_front.depth_fail_op),
                compare_op_to_vk(state.stencil_front.compare_op),
            );
            self.device.cmd_set_stencil_op(
                cb, vk::StencilFaceFlags::BACK,
                stencil_op_to_vk(state.stencil_back.fail_op),
                stencil_op_to_vk(state.stencil_back.pass_op),
                stencil_op_to_vk(state.stencil_back.depth_fail_op),
                compare_op_to_vk(state.stencil_back.compare_op),
            );
            self.device.cmd_set_stencil_compare_mask(cb, vk::StencilFaceFlags::FRONT, state.stencil_front.compare_mask);
            self.device.cmd_set_stencil_compare_mask(cb, vk::StencilFaceFlags::BACK, state.stencil_back.compare_mask);
            self.device.cmd_set_stencil_write_mask(cb, vk::StencilFaceFlags::FRONT, state.stencil_front.write_mask);
            self.device.cmd_set_stencil_write_mask(cb, vk::StencilFaceFlags::BACK, state.stencil_back.write_mask);
            self.device.cmd_set_stencil_reference(cb, vk::StencilFaceFlags::FRONT, state.stencil_front.reference);
            self.device.cmd_set_stencil_reference(cb, vk::StencilFaceFlags::BACK, state.stencil_back.reference);

            // Blend constants
            self.device.cmd_set_blend_constants(cb, &state.blend_constants);
        }

        Ok(())
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

            // Save pipeline layout for push constants and bind_textures
            self.bound_pipeline_layout = Some(vk_pipeline.pipeline_layout);

            Ok(())
        }
    }

    fn bind_textures(&mut self) -> Result<()> {
        unsafe {
            let pipeline_layout = self.bound_pipeline_layout
                .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                    "bind_textures: no pipeline bound (call bind_pipeline first)"))?;

            self.device.cmd_bind_descriptor_sets(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0, // firstSet = 0 (bindless textures)
                &[self.bindless_descriptor_set],
                &[],
            );

            Ok(())
        }
    }

    fn push_constants(&mut self, stage_flags: ShaderStageFlags, offset: u32, data: &[u8]) -> Result<()> {
        if !self.is_recording {
            engine_bail!("galaxy3d::vulkan", "push_constants: command list not recording");
        }

        let layout = self.bound_pipeline_layout.ok_or_else(|| engine_err!("galaxy3d::vulkan", "push_constants: no pipeline bound"))?;

        let vk_flags = Self::stage_flags_to_vk(stage_flags);

        unsafe {
            self.device.cmd_push_constants(
                self.command_buffer,
                layout,
                vk_flags,
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

// ===== Free conversion functions for DynamicRenderState =====
// Kept as free functions (no &self) for use from CommandList without
// needing a reference to VulkanGraphicsDevice.

#[inline(always)]
fn cull_mode_to_vk(mode: CullMode) -> vk::CullModeFlags {
    match mode {
        CullMode::None => vk::CullModeFlags::NONE,
        CullMode::Front => vk::CullModeFlags::FRONT,
        CullMode::Back => vk::CullModeFlags::BACK,
    }
}

#[inline(always)]
fn front_face_to_vk(face: FrontFace) -> vk::FrontFace {
    match face {
        FrontFace::CounterClockwise => vk::FrontFace::COUNTER_CLOCKWISE,
        FrontFace::Clockwise => vk::FrontFace::CLOCKWISE,
    }
}

#[inline(always)]
fn compare_op_to_vk(op: CompareOp) -> vk::CompareOp {
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

#[inline(always)]
fn stencil_op_to_vk(op: StencilOp) -> vk::StencilOp {
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

#[inline(always)]
pub(crate) fn color_write_mask_to_vk(mask: ColorWriteMask) -> vk::ColorComponentFlags {
    let mut flags = vk::ColorComponentFlags::empty();
    if mask.r { flags |= vk::ColorComponentFlags::R; }
    if mask.g { flags |= vk::ColorComponentFlags::G; }
    if mask.b { flags |= vk::ColorComponentFlags::B; }
    if mask.a { flags |= vk::ColorComponentFlags::A; }
    flags
}