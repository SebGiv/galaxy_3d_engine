/// RendererDevice trait - main device interface for creating resources and submitting commands

use std::sync::Arc;
use winit::window::Window;

use crate::renderer::{
    RenderResult, RendererCommandList, RendererRenderTarget, RendererRenderPass, RendererSwapchain,
    RendererTexture, RendererBuffer, RendererShader, RendererPipeline,
    TextureDesc, BufferDesc, ShaderDesc, PipelineDesc,
    RendererRenderTargetDesc, RendererRenderPassDesc,
};

/// Main renderer device trait
///
/// This is the central factory interface for creating GPU resources and submitting commands.
/// Implemented by backend-specific devices (e.g., VulkanRendererDevice).
pub trait RendererDevice: Send + Sync {
    /// Create a command list for recording rendering commands
    ///
    /// # Returns
    ///
    /// A boxed command list
    fn create_command_list(&self) -> RenderResult<Box<dyn RendererCommandList>>;

    /// Create a render target (texture that can be rendered to)
    ///
    /// # Arguments
    ///
    /// * `desc` - Render target descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created render target
    fn create_render_target(&self, desc: &RendererRenderTargetDesc) -> RenderResult<Arc<dyn RendererRenderTarget>>;

    /// Create a render pass
    ///
    /// # Arguments
    ///
    /// * `desc` - Render pass descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created render pass
    fn create_render_pass(&self, desc: &RendererRenderPassDesc) -> RenderResult<Arc<dyn RendererRenderPass>>;

    /// Create a swapchain for window presentation
    ///
    /// # Arguments
    ///
    /// * `window` - Window to create swapchain for
    ///
    /// # Returns
    ///
    /// A boxed swapchain
    fn create_swapchain(&self, window: &Window) -> RenderResult<Box<dyn RendererSwapchain>>;

    // Existing resource creation methods (from Renderer trait)

    /// Create a texture
    ///
    /// # Arguments
    ///
    /// * `desc` - Texture descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created texture
    fn create_texture(&mut self, desc: TextureDesc) -> RenderResult<Arc<dyn RendererTexture>>;

    /// Create a buffer
    ///
    /// # Arguments
    ///
    /// * `desc` - Buffer descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created buffer
    fn create_buffer(&mut self, desc: BufferDesc) -> RenderResult<Arc<dyn RendererBuffer>>;

    /// Create a shader
    ///
    /// # Arguments
    ///
    /// * `desc` - Shader descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created shader
    fn create_shader(&mut self, desc: ShaderDesc) -> RenderResult<Arc<dyn RendererShader>>;

    /// Create a graphics pipeline
    ///
    /// # Arguments
    ///
    /// * `desc` - Pipeline descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created pipeline
    fn create_pipeline(&mut self, desc: PipelineDesc) -> RenderResult<Arc<dyn RendererPipeline>>;

    /// Submit command lists for execution on the GPU
    ///
    /// # Arguments
    ///
    /// * `commands` - Slice of command lists to submit
    fn submit(&self, commands: &[&dyn RendererCommandList]) -> RenderResult<()>;

    /// Wait for all GPU operations to complete
    fn wait_idle(&self) -> RenderResult<()>;
}
