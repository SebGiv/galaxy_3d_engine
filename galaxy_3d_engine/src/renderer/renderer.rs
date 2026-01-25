/// Renderer trait - main rendering factory interface

use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use std::fmt;
use winit::window::Window;

use crate::renderer::{
    RendererBuffer, RendererTexture, RendererShader, RendererPipeline, RendererFrame,
    BufferDesc, TextureDesc, ShaderDesc, PipelineDesc,
};

// ============================================================================
// Common types and error handling
// ============================================================================

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;

/// Rendering errors
#[derive(Debug, Clone)]
pub enum RenderError {
    /// Backend-specific error
    BackendError(String),
    /// Out of GPU memory
    OutOfMemory,
    /// Invalid resource
    InvalidResource(String),
    /// Initialization failed
    InitializationFailed(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::BackendError(msg) => write!(f, "Backend error: {}", msg),
            RenderError::OutOfMemory => write!(f, "Out of GPU memory"),
            RenderError::InvalidResource(msg) => write!(f, "Invalid resource: {}", msg),
            RenderError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
        }
    }
}

impl std::error::Error for RenderError {}

/// Renderer configuration
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Enable validation/debug layers
    pub enable_validation: bool,
    /// Application name
    pub app_name: String,
    /// Application version (major, minor, patch)
    pub app_version: (u32, u32, u32),
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            enable_validation: cfg!(debug_assertions),
            app_name: "Galaxy3D Application".to_string(),
            app_version: (1, 0, 0),
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

    /// Begin a new frame
    ///
    /// # Returns
    ///
    /// A frame object for recording rendering commands
    fn begin_frame(&mut self) -> RenderResult<Arc<dyn RendererFrame>>;

    /// End the current frame and present to screen
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to end
    fn end_frame(&mut self, frame: Arc<dyn RendererFrame>) -> RenderResult<()>;

    /// Wait for all GPU operations to complete
    fn wait_idle(&self) -> RenderResult<()>;

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
type RendererPluginFactory = Box<dyn Fn(&Window, RendererConfig) -> RenderResult<Arc<Mutex<dyn Renderer>>> + Send + Sync>;

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
        F: Fn(&Window, RendererConfig) -> RenderResult<Arc<Mutex<dyn Renderer>>> + Send + Sync + 'static,
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
    pub fn create_renderer(&self, plugin_name: &str, window: &Window, config: RendererConfig) -> RenderResult<Arc<Mutex<dyn Renderer>>> {
        self.plugins
            .get(plugin_name)
            .ok_or_else(|| RenderError::InitializationFailed(format!("Plugin '{}' not found", plugin_name)))?
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
    F: Fn(&Window, RendererConfig) -> RenderResult<Arc<Mutex<dyn Renderer>>> + Send + Sync + 'static,
{
    renderer_plugin_registry()
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .register_plugin(name, factory);
}
