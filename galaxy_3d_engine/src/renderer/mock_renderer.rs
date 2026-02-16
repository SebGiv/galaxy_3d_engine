/// Mock Renderer for unit tests (no GPU required)
///
/// This mock renderer allows testing ResourceManager and other components
/// without requiring a real GPU or graphics backend.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use winit::window::Window;

#[cfg(test)]
use crate::renderer::{
    Renderer, Buffer, Texture, Shader, Pipeline, CommandList,
    RenderPass, RenderTarget, Swapchain, BindingGroup, Framebuffer,
    BufferDesc, TextureDesc, ShaderDesc, ShaderStage, PipelineDesc,
    BindingResource,
    RenderPassDesc, FramebufferDesc, Viewport, Rect2D,
    ClearValue, IndexType, TextureInfo, TextureUsage,
    PipelineReflection,
};
#[cfg(test)]
use crate::error::Result;
#[cfg(test)]
use crate::engine_bail;

// ============================================================================
// Mock Buffer
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockBuffer {
    pub size: u64,
    pub name: String,
}

#[cfg(test)]
impl MockBuffer {
    pub fn new(size: u64, name: String) -> Self {
        Self { size, name }
    }
}

#[cfg(test)]
impl Buffer for MockBuffer {
    fn update(&self, _offset: u64, _data: &[u8]) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Mock Texture
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockTexture {
    pub info: TextureInfo,
    pub name: String,
}

#[cfg(test)]
impl MockTexture {
    pub fn new(width: u32, height: u32, array_layers: u32, name: String) -> Self {
        Self {
            info: TextureInfo {
                width,
                height,
                format: crate::renderer::TextureFormat::R8G8B8A8_UNORM,
                usage: crate::renderer::TextureUsage::Sampled,
                array_layers,
                mip_levels: 1,
            },
            name,
        }
    }
}

#[cfg(test)]
impl Texture for MockTexture {
    fn info(&self) -> &TextureInfo {
        &self.info
    }
}

// ============================================================================
// Mock Shader
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockShader {
    pub name: String,
}

#[cfg(test)]
impl MockShader {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[cfg(test)]
impl Shader for MockShader {}

// ============================================================================
// Mock Pipeline
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockPipeline {
    pub name: String,
}

#[cfg(test)]
impl MockPipeline {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[cfg(test)]
impl Pipeline for MockPipeline {
    fn binding_group_layout_count(&self) -> u32 { 0 }
    fn reflection(&self) -> &PipelineReflection {
        static EMPTY: std::sync::LazyLock<PipelineReflection> =
            std::sync::LazyLock::new(PipelineReflection::empty);
        &EMPTY
    }
}

// ============================================================================
// Mock CommandList
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockCommandList {
    pub commands: Vec<String>,
}

#[cfg(test)]
impl MockCommandList {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }
}

#[cfg(test)]
impl CommandList for MockCommandList {
    fn begin(&mut self) -> Result<()> {
        self.commands.push("begin".to_string());
        Ok(())
    }

    fn end(&mut self) -> Result<()> {
        self.commands.push("end".to_string());
        Ok(())
    }

    fn begin_render_pass(
        &mut self,
        _render_pass: &Arc<dyn RenderPass>,
        _framebuffer: &Arc<dyn Framebuffer>,
        _clear_values: &[ClearValue],
    ) -> Result<()> {
        self.commands.push("begin_render_pass".to_string());
        Ok(())
    }

    fn end_render_pass(&mut self) -> Result<()> {
        self.commands.push("end_render_pass".to_string());
        Ok(())
    }

    fn bind_pipeline(&mut self, _pipeline: &Arc<dyn Pipeline>) -> Result<()> {
        self.commands.push("bind_pipeline".to_string());
        Ok(())
    }

    fn bind_vertex_buffer(&mut self, _buffer: &Arc<dyn Buffer>, _offset: u64) -> Result<()> {
        self.commands.push("bind_vertex_buffer".to_string());
        Ok(())
    }

    fn bind_index_buffer(&mut self, _buffer: &Arc<dyn Buffer>, _offset: u64, _index_type: IndexType) -> Result<()> {
        self.commands.push("bind_index_buffer".to_string());
        Ok(())
    }

    fn bind_binding_group(
        &mut self,
        _pipeline: &Arc<dyn Pipeline>,
        _set_index: u32,
        _binding_group: &Arc<dyn BindingGroup>,
    ) -> Result<()> {
        self.commands.push("bind_binding_group".to_string());
        Ok(())
    }

    fn draw(&mut self, _vertex_count: u32, _first_vertex: u32) -> Result<()> {
        self.commands.push("draw".to_string());
        Ok(())
    }

    fn draw_indexed(&mut self, _index_count: u32, _first_index: u32, _vertex_offset: i32) -> Result<()> {
        self.commands.push("draw_indexed".to_string());
        Ok(())
    }

    fn set_viewport(&mut self, _viewport: Viewport) -> Result<()> {
        self.commands.push("set_viewport".to_string());
        Ok(())
    }

    fn set_scissor(&mut self, _scissor: Rect2D) -> Result<()> {
        self.commands.push("set_scissor".to_string());
        Ok(())
    }

    fn push_constants(&mut self, _stages: &[ShaderStage], _offset: u32, _data: &[u8]) -> Result<()> {
        self.commands.push("push_constants".to_string());
        Ok(())
    }
}

// ============================================================================
// Mock RenderPass
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockRenderPass;

#[cfg(test)]
impl MockRenderPass {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
impl RenderPass for MockRenderPass {}

// ============================================================================
// Mock RenderTarget
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockRenderTarget {
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
impl MockRenderTarget {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
impl RenderTarget for MockRenderTarget {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn format(&self) -> crate::renderer::TextureFormat {
        crate::renderer::TextureFormat::R8G8B8A8_UNORM
    }
}

// ============================================================================
// Mock Framebuffer
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockFramebuffer {
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
impl MockFramebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
impl Framebuffer for MockFramebuffer {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

// ============================================================================
// Mock Swapchain
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockSwapchain {
    pub image_count: u32,
}

#[cfg(test)]
impl MockSwapchain {
    pub fn new(image_count: u32) -> Self {
        Self { image_count }
    }
}

#[cfg(test)]
impl Swapchain for MockSwapchain {
    fn acquire_next_image(&mut self) -> Result<u32> {
        Ok(0)
    }

    fn record_present_blit(
        &self,
        _cmd: &mut dyn CommandList,
        _src: &dyn Texture,
        _image_index: u32,
    ) -> Result<()> {
        Ok(())
    }

    fn present(&mut self, _image_index: u32) -> Result<()> {
        Ok(())
    }

    fn image_count(&self) -> usize {
        self.image_count as usize
    }

    fn width(&self) -> u32 {
        800
    }

    fn height(&self) -> u32 {
        600
    }

    fn format(&self) -> crate::renderer::TextureFormat {
        crate::renderer::TextureFormat::B8G8R8A8_UNORM
    }

    fn recreate(&mut self, _width: u32, _height: u32) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Mock BindingGroup
// ============================================================================

#[cfg(test)]
#[derive(Debug)]
pub struct MockBindingGroup {
    pub name: String,
    pub set_index: u32,
}

#[cfg(test)]
impl MockBindingGroup {
    pub fn new(name: String, set_index: u32) -> Self {
        Self { name, set_index }
    }
}

#[cfg(test)]
impl BindingGroup for MockBindingGroup {
    fn set_index(&self) -> u32 { self.set_index }
}

// ============================================================================
// Mock Renderer
// ============================================================================

/// Mock Renderer that tracks created resources without GPU
#[cfg(test)]
#[derive(Debug)]
pub struct MockRenderer {
    /// Track created buffers
    pub created_buffers: Arc<Mutex<Vec<String>>>,
    /// Track created textures
    pub created_textures: Arc<Mutex<Vec<String>>>,
    /// Track created shaders
    pub created_shaders: Arc<Mutex<Vec<String>>>,
    /// Track created pipelines
    pub created_pipelines: Arc<Mutex<Vec<String>>>,
}

#[cfg(test)]
impl MockRenderer {
    /// Create a new mock renderer
    pub fn new() -> Self {
        Self {
            created_buffers: Arc::new(Mutex::new(Vec::new())),
            created_textures: Arc::new(Mutex::new(Vec::new())),
            created_shaders: Arc::new(Mutex::new(Vec::new())),
            created_pipelines: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get names of created buffers
    pub fn get_created_buffers(&self) -> Vec<String> {
        self.created_buffers.lock().unwrap().clone()
    }

    /// Get names of created textures
    pub fn get_created_textures(&self) -> Vec<String> {
        self.created_textures.lock().unwrap().clone()
    }

    /// Get names of created shaders
    pub fn get_created_shaders(&self) -> Vec<String> {
        self.created_shaders.lock().unwrap().clone()
    }

    /// Get names of created pipelines
    pub fn get_created_pipelines(&self) -> Vec<String> {
        self.created_pipelines.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl Renderer for MockRenderer {
    fn create_texture(&mut self, desc: TextureDesc) -> Result<Arc<dyn Texture>> {
        let name = format!("texture_{}x{}", desc.width, desc.height);
        self.created_textures.lock().unwrap().push(name.clone());
        Ok(Arc::new(MockTexture::new(desc.width, desc.height, desc.array_layers, name)))
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<Arc<dyn Buffer>> {
        let name = format!("buffer_{}", desc.size);
        self.created_buffers.lock().unwrap().push(name.clone());
        Ok(Arc::new(MockBuffer::new(desc.size, name)))
    }

    fn create_shader(&mut self, desc: ShaderDesc) -> Result<Arc<dyn Shader>> {
        let name = format!("shader_{:?}", desc.stage);
        self.created_shaders.lock().unwrap().push(name.clone());
        Ok(Arc::new(MockShader::new(name)))
    }

    fn create_pipeline(&mut self, _desc: PipelineDesc) -> Result<Arc<dyn Pipeline>> {
        let name = "pipeline".to_string();
        self.created_pipelines.lock().unwrap().push(name.clone());
        Ok(Arc::new(MockPipeline::new(name)))
    }

    fn create_command_list(&self) -> Result<Box<dyn CommandList>> {
        Ok(Box::new(MockCommandList::new()))
    }

    fn create_render_target_texture(
        &self,
        texture: &dyn Texture,
        layer: u32,
        mip_level: u32,
    ) -> Result<Arc<dyn RenderTarget>> {
        let info = texture.info();
        match info.usage {
            TextureUsage::RenderTarget
            | TextureUsage::SampledAndRenderTarget
            | TextureUsage::DepthStencil => {}
            _ => {
                engine_bail!("galaxy3d::mock",
                    "create_render_target_texture: incompatible texture usage {:?}",
                    info.usage);
            }
        }
        if layer >= info.array_layers {
            engine_bail!("galaxy3d::mock",
                "create_render_target_texture: layer {} out of range (array_layers = {})",
                layer, info.array_layers);
        }
        if mip_level >= info.mip_levels {
            engine_bail!("galaxy3d::mock",
                "create_render_target_texture: mip_level {} out of range (mip_levels = {})",
                mip_level, info.mip_levels);
        }
        let w = (info.width >> mip_level).max(1);
        let h = (info.height >> mip_level).max(1);
        Ok(Arc::new(MockRenderTarget::new(w, h)))
    }

    fn create_framebuffer(&self, desc: &FramebufferDesc) -> Result<Arc<dyn Framebuffer>> {
        Ok(Arc::new(MockFramebuffer::new(desc.width, desc.height)))
    }

    fn create_render_pass(&self, _desc: &RenderPassDesc) -> Result<Arc<dyn RenderPass>> {
        Ok(Arc::new(MockRenderPass::new()))
    }

    fn create_binding_group(
        &self,
        _pipeline: &Arc<dyn Pipeline>,
        set_index: u32,
        _resources: &[BindingResource],
    ) -> Result<Arc<dyn BindingGroup>> {
        Ok(Arc::new(MockBindingGroup::new(
            format!("binding_group_set{}", set_index),
            set_index,
        )))
    }

    fn create_swapchain(&self, _window: &Window) -> Result<Box<dyn Swapchain>> {
        Ok(Box::new(MockSwapchain::new(3)))
    }

    fn submit(&self, _commands: &[&dyn CommandList]) -> Result<()> {
        Ok(())
    }

    fn submit_with_swapchain(
        &self,
        _commands: &[&dyn CommandList],
        _swapchain: &dyn Swapchain,
        _image_index: u32,
    ) -> Result<()> {
        Ok(())
    }

    fn wait_idle(&self) -> Result<()> {
        Ok(())
    }

    fn stats(&self) -> crate::renderer::RendererStats {
        crate::renderer::RendererStats::default()
    }

    fn resize(&mut self, _width: u32, _height: u32) {
        // No-op for mock
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "mock_renderer_tests.rs"]
mod tests;
