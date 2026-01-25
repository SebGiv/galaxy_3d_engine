/// VulkanRendererFrame - Vulkan implementation of RendererFrame trait

use galaxy_3d_engine::{RendererFrame, RendererPipeline, RendererBuffer, RenderResult};
use ash::vk;
use std::sync::Arc;
use crate::vulkan_renderer_pipeline::VulkanRendererPipeline;
use crate::vulkan_renderer_buffer::VulkanRendererBuffer;

/// Vulkan frame implementation
pub struct VulkanRendererFrame {
    /// Vulkan device
    pub(crate) device: ash::Device,
    /// Command buffer for this frame
    pub(crate) command_buffer: vk::CommandBuffer,
    /// Image index in swapchain
    pub(crate) image_index: u32,
}

impl RendererFrame for VulkanRendererFrame {
    fn bind_pipeline(&self, pipeline: &Arc<dyn RendererPipeline>) -> RenderResult<()> {
        // Downcast to VulkanRendererPipeline
        let vulkan_pipeline = pipeline
            .as_ref() as *const dyn RendererPipeline as *const VulkanRendererPipeline;

        unsafe {
            let vulkan_pipeline = &*vulkan_pipeline;

            // Bind pipeline
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                vulkan_pipeline.pipeline,
            );

            Ok(())
        }
    }

    fn bind_vertex_buffer(&self, buffer: &Arc<dyn RendererBuffer>, offset: u64) -> RenderResult<()> {
        // Downcast to VulkanRendererBuffer
        let vulkan_buffer = buffer
            .as_ref() as *const dyn RendererBuffer as *const VulkanRendererBuffer;

        unsafe {
            let vulkan_buffer = &*vulkan_buffer;

            // Bind vertex buffer
            self.device.cmd_bind_vertex_buffers(
                self.command_buffer,
                0,
                &[vulkan_buffer.buffer],
                &[offset],
            );

            Ok(())
        }
    }

    fn draw(&self, vertex_count: u32, first_vertex: u32) -> RenderResult<()> {
        unsafe {
            // Draw command
            self.device.cmd_draw(
                self.command_buffer,
                vertex_count,
                1,
                first_vertex,
                0,
            );

            Ok(())
        }
    }
}
