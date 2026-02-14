/// Renderer trait - main rendering factory interface

use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use winit::window::Window;

use crate::renderer::{
    Buffer, Texture, Shader, Pipeline, BindingGroup,
    BufferDesc, TextureDesc, ShaderDesc, PipelineDesc,
    BindingResource,
    CommandList, RenderPass, RenderTarget, Swapchain,
    RenderPassDesc, RenderTargetDesc,
    Framebuffer, FramebufferDesc,
};

// Import error types from crate root
use crate::error::{Error, Result};

/// Debug severity level for validation messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugSeverity {
    /// Only critical errors
    ErrorsOnly,
    /// Errors and warnings (recommended for development)
    ErrorsAndWarnings,
    /// All messages including info and verbose (very detailed)
    All,
}

impl Default for DebugSeverity {
    fn default() -> Self {
        if cfg!(debug_assertions) {
            DebugSeverity::ErrorsAndWarnings
        } else {
            DebugSeverity::ErrorsOnly
        }
    }
}

/// Debug output destination
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugOutput {
    /// Console output only
    Console,
    /// File output only
    File(String),
    /// Both console and file
    Both(String),
}

impl Default for DebugOutput {
    fn default() -> Self {
        DebugOutput::Console
    }
}

/// Debug message category filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebugMessageFilter {
    /// Show general messages (device creation, etc.)
    pub show_general: bool,
    /// Show validation messages (API usage errors)
    pub show_validation: bool,
    /// Show performance warnings (suboptimal usage)
    pub show_performance: bool,
}

impl Default for DebugMessageFilter {
    fn default() -> Self {
        Self {
            show_general: true,
            show_validation: true,
            show_performance: true,
        }
    }
}

/// Validation statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidationStats {
    /// Number of errors
    pub errors: u32,
    /// Number of warnings
    pub warnings: u32,
    /// Number of info messages
    pub info: u32,
    /// Number of verbose messages
    pub verbose: u32,
}

impl ValidationStats {
    /// Get total message count
    pub fn total(&self) -> u32 {
        self.errors + self.warnings + self.info + self.verbose
    }

    /// Check if any errors were recorded
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Check if any warnings were recorded
    pub fn has_warnings(&self) -> bool {
        self.warnings > 0
    }
}

/// Renderer configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Enable validation/debug layers
    pub enable_validation: bool,
    /// Application name
    pub app_name: String,
    /// Application version (major, minor, patch)
    pub app_version: (u32, u32, u32),
    /// Debug message severity filter
    pub debug_severity: DebugSeverity,
    /// Debug output destination
    pub debug_output: DebugOutput,
    /// Debug message category filter
    pub debug_message_filter: DebugMessageFilter,
    /// Break on validation error (useful for debugging)
    pub break_on_validation_error: bool,
    /// Panic on any error (strict mode for development)
    pub panic_on_error: bool,
    /// Track and display validation statistics
    pub enable_validation_stats: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_validation: cfg!(debug_assertions),
            app_name: "Galaxy3D Application".to_string(),
            app_version: (1, 0, 0),
            debug_severity: DebugSeverity::default(),
            debug_output: DebugOutput::default(),
            debug_message_filter: DebugMessageFilter::default(),
            break_on_validation_error: false,
            panic_on_error: false,
            enable_validation_stats: cfg!(debug_assertions),
        }
    }
}

/// Renderer statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct RendererStats {
    /// Number of draw calls this frame
    pub draw_calls: u32,
    /// Number of triangles drawn this frame
    pub triangles: u32,
    /// GPU memory used (bytes)
    pub gpu_memory_used: u64,
}

// ============================================================================
// Renderer trait
// ============================================================================

/// Main renderer trait
///
/// This is the central factory interface for creating GPU resources.
/// Implemented by backend-specific renderers (e.g., VulkanRenderer).
pub trait Renderer: Send + Sync {
    /// Create a texture
    ///
    /// # Arguments
    ///
    /// * `desc` - Texture descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created texture
    fn create_texture(&mut self, desc: TextureDesc) -> Result<Arc<dyn Texture>>;

    /// Create a buffer
    ///
    /// # Arguments
    ///
    /// * `desc` - Buffer descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created buffer
    fn create_buffer(&mut self, desc: BufferDesc) -> Result<Arc<dyn Buffer>>;

    /// Create a shader
    ///
    /// # Arguments
    ///
    /// * `desc` - Shader descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created shader
    fn create_shader(&mut self, desc: ShaderDesc) -> Result<Arc<dyn Shader>>;

    /// Create a graphics pipeline
    ///
    /// # Arguments
    ///
    /// * `desc` - Pipeline descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created pipeline
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<Arc<dyn Pipeline>>;

    /// Create a command list for recording rendering commands
    ///
    /// # Returns
    ///
    /// A boxed command list
    fn create_command_list(&self) -> Result<Box<dyn CommandList>>;

    /// Create a render target (texture that can be rendered to)
    ///
    /// # Arguments
    ///
    /// * `desc` - Render target descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created render target
    fn create_render_target(&self, desc: &RenderTargetDesc) -> Result<Arc<dyn RenderTarget>>;

    /// Create a render target view from an existing texture
    ///
    /// Creates an image view suitable for framebuffer attachment,
    /// targeting a specific layer and mip level of the texture.
    /// The texture must have been created with a compatible usage
    /// (RenderTarget, SampledAndRenderTarget, or DepthStencil).
    ///
    /// # Arguments
    ///
    /// * `texture` - The texture to create a render target view from
    /// * `layer` - Array layer index (0 for simple textures)
    /// * `mip_level` - Mip level (0 for full resolution)
    ///
    /// # Errors
    ///
    /// Returns an error if the texture usage is incompatible,
    /// or if layer/mip_level are out of bounds.
    fn create_render_target_view(
        &self,
        texture: &dyn Texture,
        layer: u32,
        mip_level: u32,
    ) -> Result<Arc<dyn RenderTarget>>;

    /// Create a framebuffer grouping color and depth/stencil attachments
    ///
    /// # Arguments
    ///
    /// * `desc` - Framebuffer descriptor (render pass, attachments, dimensions)
    ///
    /// # Returns
    ///
    /// A shared pointer to the created framebuffer
    fn create_framebuffer(&self, desc: &FramebufferDesc) -> Result<Arc<dyn Framebuffer>>;

    /// Create a render pass
    ///
    /// # Arguments
    ///
    /// * `desc` - Render pass descriptor
    ///
    /// # Returns
    ///
    /// A shared pointer to the created render pass
    fn create_render_pass(&self, desc: &RenderPassDesc) -> Result<Arc<dyn RenderPass>>;

    /// Create a swapchain for window presentation
    ///
    /// # Arguments
    ///
    /// * `window` - Window to create swapchain for
    ///
    /// # Returns
    ///
    /// A boxed swapchain
    fn create_swapchain(&self, window: &Window) -> Result<Box<dyn Swapchain>>;

    /// Submit command lists for execution on the GPU
    ///
    /// # Arguments
    ///
    /// * `commands` - Slice of command lists to submit
    fn submit(&self, commands: &[&dyn CommandList]) -> Result<()>;

    /// Submit command lists with swapchain synchronization
    ///
    /// This method automatically handles synchronization with the swapchain
    /// (wait for image available, signal render finished).
    ///
    /// # Arguments
    ///
    /// * `commands` - Slice of command lists to submit
    /// * `swapchain` - Swapchain to synchronize with
    /// * `image_index` - Index of the swapchain image being rendered to
    fn submit_with_swapchain(
        &self,
        commands: &[&dyn CommandList],
        swapchain: &dyn Swapchain,
        image_index: u32,
    ) -> Result<()>;

    /// Create an immutable binding group (descriptor set) for a pipeline
    ///
    /// The binding group layout is deduced from the pipeline at the given set index.
    /// The pool is managed internally by the renderer.
    ///
    /// # Arguments
    ///
    /// * `pipeline` - Pipeline that defines the expected layout
    /// * `set_index` - Set index (0 = per-frame, 1 = per-material, etc.)
    /// * `resources` - Concrete resources to bind (must match the layout)
    ///
    /// # Returns
    ///
    /// A shared pointer to the created binding group
    fn create_binding_group(
        &self,
        pipeline: &Arc<dyn Pipeline>,
        set_index: u32,
        resources: &[BindingResource],
    ) -> Result<Arc<dyn BindingGroup>>;

    /// Wait for all GPU operations to complete
    fn wait_idle(&self) -> Result<()>;

    /// Get statistics about the renderer
    fn stats(&self) -> RendererStats;

    /// Notify renderer that the window has been resized
    ///
    /// # Arguments
    ///
    /// * `width` - New window width
    /// * `height` - New window height
    fn resize(&mut self, width: u32, height: u32);
}

// ============================================================================
// Plugin system for registering renderer backends
// ============================================================================

/// Renderer plugin factory function type
type RendererPluginFactory = Box<dyn Fn(&Window, Config) -> Result<Arc<Mutex<dyn Renderer>>> + Send + Sync>;

/// Plugin registry for renderer backends
pub struct RendererPluginRegistry {
    plugins: HashMap<&'static str, RendererPluginFactory>,
}

impl RendererPluginRegistry {
    /// Create a new plugin registry
    fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a plugin
    ///
    /// # Arguments
    ///
    /// * `name` - Plugin name (e.g., "vulkan")
    /// * `factory` - Factory function to create the plugin
    pub fn register_plugin<F>(&mut self, name: &'static str, factory: F)
    where
        F: Fn(&Window, Config) -> Result<Arc<Mutex<dyn Renderer>>> + Send + Sync + 'static,
    {
        self.plugins.insert(name, Box::new(factory));
    }

    /// Create a renderer using a registered plugin
    ///
    /// # Arguments
    ///
    /// * `plugin_name` - Name of the plugin to use
    /// * `window` - Window to render to
    /// * `config` - Renderer configuration
    ///
    /// # Returns
    ///
    /// A shared, thread-safe renderer instance
    pub fn create_renderer(&self, plugin_name: &str, window: &Window, config: Config) -> Result<Arc<Mutex<dyn Renderer>>> {
        self.plugins
            .get(plugin_name)
            .ok_or_else(|| Error::InitializationFailed(format!("Plugin '{}' not found", plugin_name)))?
            (window, config)
    }
}

static RENDERER_REGISTRY: Mutex<Option<RendererPluginRegistry>> = Mutex::new(None);

/// Get the global renderer plugin registry
pub fn renderer_plugin_registry() -> &'static Mutex<Option<RendererPluginRegistry>> {
    // Initialize on first access
    let mut registry = RENDERER_REGISTRY.lock().unwrap();
    if registry.is_none() {
        *registry = Some(RendererPluginRegistry::new());
    }
    drop(registry);
    &RENDERER_REGISTRY
}

/// Register a renderer plugin in the global registry
///
/// # Arguments
///
/// * `name` - Plugin name
/// * `factory` - Factory function
pub fn register_renderer_plugin<F>(name: &'static str, factory: F)
where
    F: Fn(&Window, Config) -> Result<Arc<Mutex<dyn Renderer>>> + Send + Sync + 'static,
{
    renderer_plugin_registry()
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .register_plugin(name, factory);
}
