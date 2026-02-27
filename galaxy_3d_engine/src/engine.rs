/// Galaxy3D Engine - Singleton manager for engine subsystems
///
/// This module provides global singleton management for the graphics device and other
/// engine subsystems. It uses thread-safe static storage with RwLock for safe
/// concurrent access.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock, Arc, Mutex};
use std::time::SystemTime;
use crate::graphics_device::GraphicsDevice;
use crate::resource::ResourceManager;
use crate::scene::SceneManager;
use crate::render_graph::RenderGraphManager;
use crate::error::{Result, Error};
use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};

// ===== INTERNAL STATE =====

/// Global engine state storage
static ENGINE_STATE: OnceLock<EngineState> = OnceLock::new();

/// Global logger (initialized with DefaultLogger)
static LOGGER: OnceLock<RwLock<Box<dyn Logger>>> = OnceLock::new();

/// Internal state structure holding all engine singletons
struct EngineState {
    /// Named graphics devices (multiple devices supported, keyed by name)
    graphics_devices: RwLock<HashMap<String, Arc<Mutex<dyn GraphicsDevice>>>>,
    /// Resource manager singleton
    resource_manager: RwLock<Option<Arc<Mutex<ResourceManager>>>>,
    /// Scene manager singleton
    scene_manager: RwLock<Option<Arc<Mutex<SceneManager>>>>,
    /// Render graph manager singleton
    render_graph_manager: RwLock<Option<Arc<Mutex<RenderGraphManager>>>>,
}

impl EngineState {
    /// Create a new empty engine state
    fn new() -> Self {
        Self {
            graphics_devices: RwLock::new(HashMap::new()),
            resource_manager: RwLock::new(None),
            scene_manager: RwLock::new(None),
            render_graph_manager: RwLock::new(None),
        }
    }
}

// ===== PUBLIC API =====

/// Main engine singleton manager
///
/// Manages the lifecycle of all engine subsystems (graphics_device, resource manager, etc.)
/// using a singleton pattern with thread-safe access.
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
            // Clear render graph manager BEFORE scene manager
            if let Ok(mut rgm) = state.render_graph_manager.write() {
                *rgm = None;
            }
            // Clear scene manager BEFORE resource manager (scenes reference resources)
            if let Ok(mut sm) = state.scene_manager.write() {
                *sm = None;
            }
            // Clear resource manager BEFORE graphics_devices (resources reference GPU objects)
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            // Clear all graphics devices
            if let Ok(mut graphics_devices) = state.graphics_devices.write() {
                graphics_devices.clear();
            }
        }
    }

    /// Create and register a named graphics device
    ///
    /// Wraps the graphics device in Arc and registers it in the global graphics_device map.
    /// Returns the created graphics device for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this graphics device
    /// * `graphics_device` - Any type implementing the GraphicsDevice trait
    ///
    /// # Returns
    ///
    /// The created graphics device wrapped in `Arc<Mutex<dyn GraphicsDevice>>`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A graphics device with the same name already exists
    /// - The graphics devices lock is poisoned
    ///
    pub fn create_graphics_device<R: GraphicsDevice + 'static>(name: &str, graphics_device: R) -> Result<Arc<Mutex<dyn GraphicsDevice>>> {
        let arc_device: Arc<Mutex<dyn GraphicsDevice>> = Arc::new(Mutex::new(graphics_device));

        Self::register_graphics_device(name, Arc::clone(&arc_device))?;

        crate::engine_info!("galaxy3d::Engine", "GraphicsDevice '{}' created successfully", name);

        Ok(arc_device)
    }

    /// Register a graphics device by name (internal use)
    pub(crate) fn register_graphics_device(name: &str, graphics_device: Arc<Mutex<dyn GraphicsDevice>>) -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.graphics_devices.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("GraphicsDevices lock poisoned".to_string())
            ))?;

        if lock.contains_key(name) {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed(format!("GraphicsDevice '{}' already exists. Call Engine::destroy_graphics_device() first.", name))
            ));
        }

        lock.insert(name.to_string(), graphics_device);
        Ok(())
    }

    /// Get a graphics device by name
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the graphics device to retrieve
    ///
    /// # Returns
    ///
    /// A shared pointer to the graphics device wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - No graphics device with the given name exists
    ///
    pub fn graphics_device(name: &str) -> Result<Arc<Mutex<dyn GraphicsDevice>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.graphics_devices.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("GraphicsDevices lock poisoned".to_string())
            ))?;

        lock.get(name)
            .cloned()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed(format!("GraphicsDevice '{}' not found. Call Engine::create_graphics_device() first.", name))
            ))
    }

    /// Destroy a graphics device by name
    ///
    /// Removes the graphics device from the map. Existing references remain valid until dropped.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the graphics device to destroy
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    pub fn destroy_graphics_device(name: &str) -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.graphics_devices.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("GraphicsDevices lock poisoned".to_string())
            ))?;

        lock.remove(name);

        crate::engine_info!("galaxy3d::Engine", "GraphicsDevice '{}' destroyed", name);

        Ok(())
    }

    /// Get the names of all registered graphics devices
    pub fn graphics_device_names() -> Vec<String> {
        ENGINE_STATE.get()
            .and_then(|state| state.graphics_devices.read().ok())
            .map(|lock| lock.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of registered graphics devices
    pub fn graphics_device_count() -> usize {
        ENGINE_STATE.get()
            .and_then(|state| state.graphics_devices.read().ok())
            .map(|lock| lock.len())
            .unwrap_or(0)
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

    // ===== SCENE MANAGER API =====

    /// Create and register the scene manager singleton
    ///
    /// Creates a new SceneManager and registers it as a global singleton.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A scene manager already exists
    ///
    pub fn create_scene_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.scene_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("SceneManager lock poisoned".to_string())
            ))?;

        if lock.is_some() {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed("SceneManager already exists. Call Engine::destroy_scene_manager() first.".to_string())
            ));
        }

        *lock = Some(Arc::new(Mutex::new(SceneManager::new())));

        crate::engine_info!("galaxy3d::Engine", "SceneManager singleton created successfully");

        Ok(())
    }

    /// Get the scene manager singleton
    ///
    /// Provides global access to the scene manager after it has been created.
    ///
    /// # Returns
    ///
    /// A shared pointer to the SceneManager wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - The scene manager has not been created
    ///
    pub fn scene_manager() -> Result<Arc<Mutex<SceneManager>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.scene_manager.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("SceneManager lock poisoned".to_string())
            ))?;

        lock.clone()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("SceneManager not created. Call Engine::create_scene_manager() first.".to_string())
            ))
    }

    /// Destroy the scene manager singleton
    ///
    /// Removes the scene manager singleton, allowing a new one to be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    pub fn destroy_scene_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.scene_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("SceneManager lock poisoned".to_string())
            ))?;

        *lock = None;

        crate::engine_info!("galaxy3d::Engine", "SceneManager singleton destroyed");

        Ok(())
    }

    // ===== RENDER GRAPH MANAGER API =====

    /// Create and register the render graph manager singleton
    ///
    /// Creates a new RenderGraphManager and registers it as a global singleton.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A render graph manager already exists
    ///
    pub fn create_render_graph_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.render_graph_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("RenderGraphManager lock poisoned".to_string())
            ))?;

        if lock.is_some() {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed("RenderGraphManager already exists. Call Engine::destroy_render_graph_manager() first.".to_string())
            ));
        }

        *lock = Some(Arc::new(Mutex::new(RenderGraphManager::new())));

        crate::engine_info!("galaxy3d::Engine", "RenderGraphManager singleton created successfully");

        Ok(())
    }

    /// Get the render graph manager singleton
    ///
    /// Provides global access to the render graph manager after it has been created.
    ///
    /// # Returns
    ///
    /// A shared pointer to the RenderGraphManager wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - The render graph manager has not been created
    ///
    pub fn render_graph_manager() -> Result<Arc<Mutex<RenderGraphManager>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.render_graph_manager.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("RenderGraphManager lock poisoned".to_string())
            ))?;

        lock.clone()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("RenderGraphManager not created. Call Engine::create_render_graph_manager() first.".to_string())
            ))
    }

    /// Destroy the render graph manager singleton
    ///
    /// Removes the render graph manager singleton, allowing a new one to be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    pub fn destroy_render_graph_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.render_graph_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("RenderGraphManager lock poisoned".to_string())
            ))?;

        *lock = None;

        crate::engine_info!("galaxy3d::Engine", "RenderGraphManager singleton destroyed");

        Ok(())
    }

    /// Reset all singletons for testing (only available in test builds)
    #[cfg(test)]
    pub fn reset_for_testing() {
        if let Some(state) = ENGINE_STATE.get() {
            if let Ok(mut rgm) = state.render_graph_manager.write() {
                *rgm = None;
            }
            if let Ok(mut sm) = state.scene_manager.write() {
                *sm = None;
            }
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            if let Ok(mut graphics_devices) = state.graphics_devices.write() {
                graphics_devices.clear();
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
    pub fn set_logger<L: Logger + 'static>(logger: L) {
        let logger_lock = LOGGER.get_or_init(|| RwLock::new(Box::new(DefaultLogger)));
        if let Ok(mut lock) = logger_lock.write() {
            *lock = Box::new(logger);
        }
    }

    /// Reset logger to default (DefaultLogger)
    ///
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

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
