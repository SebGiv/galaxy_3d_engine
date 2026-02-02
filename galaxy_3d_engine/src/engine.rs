/// Galaxy3D Engine - Singleton manager for engine subsystems
///
/// This module provides global singleton management for the renderer and other
/// engine subsystems. It uses thread-safe static storage with RwLock for safe
/// concurrent access.

use std::sync::{OnceLock, RwLock, Arc, Mutex};
use std::time::SystemTime;
use crate::renderer::Renderer;
use crate::resource::ResourceManager;
use crate::error::{Result, Error};
use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};

// ===== INTERNAL STATE =====

/// Global engine state storage
static ENGINE_STATE: OnceLock<EngineState> = OnceLock::new();

/// Global logger (initialized with DefaultLogger)
static LOGGER: OnceLock<RwLock<Box<dyn Logger>>> = OnceLock::new();

/// Internal state structure holding all engine singletons
struct EngineState {
    /// Renderer singleton (wrapped in Mutex for thread-safe mutable access)
    renderer: RwLock<Option<Arc<Mutex<dyn Renderer>>>>,
    /// Resource manager singleton
    resource_manager: RwLock<Option<Arc<Mutex<ResourceManager>>>>,
}

impl EngineState {
    /// Create a new empty engine state
    fn new() -> Self {
        Self {
            renderer: RwLock::new(None),
            resource_manager: RwLock::new(None),
        }
    }
}

// ===== PUBLIC API =====

/// Main engine singleton manager
///
/// Manages the lifecycle of all engine subsystems (renderer, resource manager, etc.)
/// using a singleton pattern with thread-safe access.
///
/// # Example
///
/// ```no_run
/// use galaxy_3d_engine::{Engine, Renderer};
/// use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;
///
/// // Initialize engine
/// Engine::initialize()?;
///
/// // Create renderer singleton
/// Engine::create_renderer(VulkanRenderer::new(&window, config)?)?;
///
/// // Access renderer globally
/// let renderer = Renderer::instance()?;
///
/// // Cleanup
/// Engine::shutdown();
/// # Ok::<(), galaxy_3d_engine::Error>(())
/// ```
pub struct Engine;

impl Engine {
    /// Helper to log errors before returning them (internal use)
    ///
    /// This ensures all Engine errors are automatically logged with proper severity
    /// and source information, enabling better debugging and monitoring.
    fn log_and_return_error(error: Error) -> Error {
        match &error {
            Error::InitializationFailed(msg) => {
                crate::engine_error!("galaxy3d::Engine", "Initialization failed: {}", msg);
            }
            Error::BackendError(msg) => {
                crate::engine_error!("galaxy3d::Engine", "Backend error: {}", msg);
            }
            _ => {
                crate::engine_error!("galaxy3d::Engine", "Engine error: {}", error);
            }
        }
        error
    }

    /// Initialize the engine
    ///
    /// This must be called once at application startup before creating any subsystems.
    ///
    /// # Errors
    ///
    /// Currently always succeeds, but returns Result for future extensibility.
    pub fn initialize() -> Result<()> {
        ENGINE_STATE.get_or_init(|| EngineState::new());
        Ok(())
    }

    /// Shutdown the entire engine and destroy all singletons
    ///
    /// This should be called at application shutdown to properly cleanup all subsystems.
    /// After calling this, you must call `initialize()` again before creating new subsystems.
    pub fn shutdown() {
        if let Some(state) = ENGINE_STATE.get() {
            // Clear resource manager BEFORE renderer (resources reference GPU objects)
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            // Clear renderer
            if let Ok(mut renderer) = state.renderer.write() {
                *renderer = None;
            }
        }
    }

    /// Create and register the renderer singleton
    ///
    /// This is a simplified API that automatically wraps the renderer in Arc
    /// and registers it as a global singleton.
    ///
    /// # Arguments
    ///
    /// * `renderer` - Any type implementing the Renderer trait
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A renderer already exists
    /// - The renderer lock is poisoned
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::Engine;
    /// use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;
    ///
    /// Engine::initialize()?;
    /// Engine::create_renderer(VulkanRenderer::new(&window, config)?)?;
    /// # Ok::<(), galaxy_3d_engine::Error>(())
    /// ```
    pub fn create_renderer<R: Renderer + 'static>(renderer: R) -> Result<()> {
        // Wrap in Arc<Mutex<dyn Renderer>>
        let arc_renderer: Arc<Mutex<dyn Renderer>> = Arc::new(Mutex::new(renderer));

        // Register as singleton
        Self::register_renderer(arc_renderer)?;

        // Log successful creation
        crate::engine_info!("galaxy3d::Engine", "Renderer singleton created successfully");

        Ok(())
    }

    /// Register a renderer singleton (internal use)
    ///
    /// This is called internally by create_renderer(). Marked pub(crate) to allow
    /// access from other modules if needed.
    pub(crate) fn register_renderer(renderer: Arc<Mutex<dyn Renderer>>) -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.renderer.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderer lock poisoned".to_string())
            ))?;

        if lock.is_some() {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed("Renderer already exists. Call Renderer::destroy_singleton() first.".to_string())
            ));
        }

        *lock = Some(renderer);
        Ok(())
    }

    /// Get the renderer singleton
    ///
    /// This provides global access to the renderer after it has been created.
    ///
    /// # Returns
    ///
    /// A shared pointer to the renderer wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - The renderer has not been created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::Engine;
    ///
    /// let renderer = Engine::renderer()?;
    /// let renderer_guard = renderer.lock().unwrap();
    /// // Use renderer_guard...
    /// # Ok::<(), galaxy_3d_engine::Error>(())
    /// ```
    pub fn renderer() -> Result<Arc<Mutex<dyn Renderer>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.renderer.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderer lock poisoned".to_string())
            ))?;

        lock.clone()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Renderer not created. Call Engine::create_renderer() first.".to_string())
            ))
    }

    /// Destroy the renderer singleton
    ///
    /// Removes the renderer singleton, allowing a new one to be created.
    /// All existing renderer references will remain valid until dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::Engine;
    ///
    /// Engine::destroy_renderer()?;
    /// # Ok::<(), galaxy_3d_engine::Error>(())
    /// ```
    pub fn destroy_renderer() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.renderer.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderer lock poisoned".to_string())
            ))?;

        *lock = None;

        // Log successful destruction
        crate::engine_info!("galaxy3d::Engine", "Renderer singleton destroyed");

        Ok(())
    }

    // ===== RESOURCE MANAGER API =====

    /// Create and register the resource manager singleton
    ///
    /// Creates a new ResourceManager and registers it as a global singleton.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A resource manager already exists
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::galaxy3d::Engine;
    ///
    /// Engine::initialize()?;
    /// Engine::create_resource_manager()?;
    /// # Ok::<(), galaxy_3d_engine::galaxy3d::Error>(())
    /// ```
    pub fn create_resource_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.resource_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("ResourceManager lock poisoned".to_string())
            ))?;

        if lock.is_some() {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed("ResourceManager already exists. Call Engine::destroy_resource_manager() first.".to_string())
            ));
        }

        *lock = Some(Arc::new(Mutex::new(ResourceManager::new())));

        crate::engine_info!("galaxy3d::Engine", "ResourceManager singleton created successfully");

        Ok(())
    }

    /// Get the resource manager singleton
    ///
    /// Provides global access to the resource manager after it has been created.
    ///
    /// # Returns
    ///
    /// A shared pointer to the ResourceManager wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - The resource manager has not been created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::galaxy3d::Engine;
    ///
    /// let rm = Engine::resource_manager()?;
    /// let rm_guard = rm.lock().unwrap();
    /// // Use rm_guard...
    /// # Ok::<(), galaxy_3d_engine::galaxy3d::Error>(())
    /// ```
    pub fn resource_manager() -> Result<Arc<Mutex<ResourceManager>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.resource_manager.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("ResourceManager lock poisoned".to_string())
            ))?;

        lock.clone()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("ResourceManager not created. Call Engine::create_resource_manager() first.".to_string())
            ))
    }

    /// Destroy the resource manager singleton
    ///
    /// Removes the resource manager singleton, allowing a new one to be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::galaxy3d::Engine;
    ///
    /// Engine::destroy_resource_manager()?;
    /// # Ok::<(), galaxy_3d_engine::galaxy3d::Error>(())
    /// ```
    pub fn destroy_resource_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.resource_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("ResourceManager lock poisoned".to_string())
            ))?;

        *lock = None;

        crate::engine_info!("galaxy3d::Engine", "ResourceManager singleton destroyed");

        Ok(())
    }

    /// Reset all singletons for testing (only available in test builds)
    #[cfg(test)]
    pub fn reset_for_testing() {
        if let Some(state) = ENGINE_STATE.get() {
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            if let Ok(mut renderer) = state.renderer.write() {
                *renderer = None;
            }
        }
    }

    // ===== LOGGING API =====

    /// Set a custom logger
    ///
    /// Replace the default logger with a custom implementation (file logger, network logger, etc.)
    ///
    /// # Arguments
    ///
    /// * `logger` - Any type implementing the Logger trait
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::galaxy3d::{Engine, log::{Logger, LogEntry}};
    ///
    /// struct FileLogger;
    /// impl Logger for FileLogger {
    ///     fn log(&self, entry: &LogEntry) {
    ///         // Write to file...
    ///     }
    /// }
    ///
    /// Engine::set_logger(FileLogger);
    /// ```
    pub fn set_logger<L: Logger + 'static>(logger: L) {
        let logger_lock = LOGGER.get_or_init(|| RwLock::new(Box::new(DefaultLogger)));
        if let Ok(mut lock) = logger_lock.write() {
            *lock = Box::new(logger);
        }
    }

    /// Reset logger to default (DefaultLogger)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use galaxy_3d_engine::galaxy3d::Engine;
    ///
    /// Engine::reset_logger();
    /// ```
    pub fn reset_logger() {
        let logger_lock = LOGGER.get_or_init(|| RwLock::new(Box::new(DefaultLogger)));
        if let Ok(mut lock) = logger_lock.write() {
            *lock = Box::new(DefaultLogger);
        }
    }

    /// Internal logging method (for simple logs without file:line)
    ///
    /// Used by macros like engine_info!, engine_warn!, etc.
    ///
    /// # Arguments
    ///
    /// * `severity` - Log severity level
    /// * `source` - Source module (e.g., "galaxy3d::Engine")
    /// * `message` - Log message
    pub fn log(severity: LogSeverity, source: &str, message: String) {
        let logger_lock = LOGGER.get_or_init(|| RwLock::new(Box::new(DefaultLogger)));
        if let Ok(lock) = logger_lock.read() {
            lock.log(&LogEntry {
                severity,
                timestamp: SystemTime::now(),
                source: source.to_string(),
                message,
                file: None,
                line: None,
            });
        }
    }

    /// Internal logging method with file:line information (for ERROR logs)
    ///
    /// Used by engine_error! macro to include source location.
    ///
    /// # Arguments
    ///
    /// * `severity` - Log severity level (typically Error)
    /// * `source` - Source module (e.g., "galaxy3d::Engine")
    /// * `message` - Log message
    /// * `file` - Source file path
    /// * `line` - Source line number
    pub fn log_detailed(
        severity: LogSeverity,
        source: &str,
        message: String,
        file: &'static str,
        line: u32,
    ) {
        let logger_lock = LOGGER.get_or_init(|| RwLock::new(Box::new(DefaultLogger)));
        if let Ok(lock) = logger_lock.read() {
            lock.log(&LogEntry {
                severity,
                timestamp: SystemTime::now(),
                source: source.to_string(),
                message,
                file: Some(file),
                line: Some(line),
            });
        }
    }
}
