/// Galaxy3D Engine - Singleton manager for engine subsystems
///
/// This module provides global singleton management for the renderer and other
/// engine subsystems. It uses thread-safe static storage with RwLock for safe
/// concurrent access.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock, Arc, Mutex};
use std::time::SystemTime;
use crate::renderer::Renderer;
use crate::resource::ResourceManager;
use crate::scene::SceneManager;
use crate::target::TargetManager;
use crate::error::{Result, Error};
use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};

// ===== INTERNAL STATE =====

/// Global engine state storage
static ENGINE_STATE: OnceLock<EngineState> = OnceLock::new();

/// Global logger (initialized with DefaultLogger)
static LOGGER: OnceLock<RwLock<Box<dyn Logger>>> = OnceLock::new();

/// Internal state structure holding all engine singletons
struct EngineState {
    /// Named renderers (multiple renderers supported, keyed by name)
    renderers: RwLock<HashMap<String, Arc<Mutex<dyn Renderer>>>>,
    /// Resource manager singleton
    resource_manager: RwLock<Option<Arc<Mutex<ResourceManager>>>>,
    /// Scene manager singleton
    scene_manager: RwLock<Option<Arc<Mutex<SceneManager>>>>,
    /// Target manager singleton
    target_manager: RwLock<Option<Arc<Mutex<TargetManager>>>>,
}

impl EngineState {
    /// Create a new empty engine state
    fn new() -> Self {
        Self {
            renderers: RwLock::new(HashMap::new()),
            resource_manager: RwLock::new(None),
            scene_manager: RwLock::new(None),
            target_manager: RwLock::new(None),
        }
    }
}

// ===== PUBLIC API =====

/// Main engine singleton manager
///
/// Manages the lifecycle of all engine subsystems (renderer, resource manager, etc.)
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
            // Clear target manager BEFORE scene manager (targets reference scenes)
            if let Ok(mut tm) = state.target_manager.write() {
                *tm = None;
            }
            // Clear scene manager BEFORE resource manager (scenes reference resources)
            if let Ok(mut sm) = state.scene_manager.write() {
                *sm = None;
            }
            // Clear resource manager BEFORE renderers (resources reference GPU objects)
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            // Clear all renderers
            if let Ok(mut renderers) = state.renderers.write() {
                renderers.clear();
            }
        }
    }

    /// Create and register a named renderer
    ///
    /// Wraps the renderer in Arc and registers it in the global renderer map.
    /// Returns the created renderer for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this renderer
    /// * `renderer` - Any type implementing the Renderer trait
    ///
    /// # Returns
    ///
    /// The created renderer wrapped in `Arc<Mutex<dyn Renderer>>`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A renderer with the same name already exists
    /// - The renderers lock is poisoned
    ///
    pub fn create_renderer<R: Renderer + 'static>(name: &str, renderer: R) -> Result<Arc<Mutex<dyn Renderer>>> {
        let arc_renderer: Arc<Mutex<dyn Renderer>> = Arc::new(Mutex::new(renderer));

        Self::register_renderer(name, Arc::clone(&arc_renderer))?;

        crate::engine_info!("galaxy3d::Engine", "Renderer '{}' created successfully", name);

        Ok(arc_renderer)
    }

    /// Register a renderer by name (internal use)
    pub(crate) fn register_renderer(name: &str, renderer: Arc<Mutex<dyn Renderer>>) -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.renderers.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderers lock poisoned".to_string())
            ))?;

        if lock.contains_key(name) {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed(format!("Renderer '{}' already exists. Call Engine::destroy_renderer() first.", name))
            ));
        }

        lock.insert(name.to_string(), renderer);
        Ok(())
    }

    /// Get a renderer by name
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the renderer to retrieve
    ///
    /// # Returns
    ///
    /// A shared pointer to the renderer wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - No renderer with the given name exists
    ///
    pub fn renderer(name: &str) -> Result<Arc<Mutex<dyn Renderer>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.renderers.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderers lock poisoned".to_string())
            ))?;

        lock.get(name)
            .cloned()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed(format!("Renderer '{}' not found. Call Engine::create_renderer() first.", name))
            ))
    }

    /// Destroy a renderer by name
    ///
    /// Removes the renderer from the map. Existing references remain valid until dropped.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the renderer to destroy
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    pub fn destroy_renderer(name: &str) -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.renderers.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("Renderers lock poisoned".to_string())
            ))?;

        lock.remove(name);

        crate::engine_info!("galaxy3d::Engine", "Renderer '{}' destroyed", name);

        Ok(())
    }

    /// Get the names of all registered renderers
    pub fn renderer_names() -> Vec<String> {
        ENGINE_STATE.get()
            .and_then(|state| state.renderers.read().ok())
            .map(|lock| lock.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of registered renderers
    pub fn renderer_count() -> usize {
        ENGINE_STATE.get()
            .and_then(|state| state.renderers.read().ok())
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

    // ===== TARGET MANAGER API =====

    /// Create and register the target manager singleton
    ///
    /// Creates a new TargetManager and registers it as a global singleton.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - A target manager already exists
    ///
    pub fn create_target_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let mut lock = state.target_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("TargetManager lock poisoned".to_string())
            ))?;

        if lock.is_some() {
            return Err(Self::log_and_return_error(
                Error::InitializationFailed("TargetManager already exists. Call Engine::destroy_target_manager() first.".to_string())
            ));
        }

        *lock = Some(Arc::new(Mutex::new(TargetManager::new())));

        crate::engine_info!("galaxy3d::Engine", "TargetManager singleton created successfully");

        Ok(())
    }

    /// Get the target manager singleton
    ///
    /// Provides global access to the target manager after it has been created.
    ///
    /// # Returns
    ///
    /// A shared pointer to the TargetManager wrapped in a Mutex for thread-safe access
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The engine is not initialized
    /// - The target manager has not been created
    ///
    pub fn target_manager() -> Result<Arc<Mutex<TargetManager>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized. Call Engine::initialize() first.".to_string())
            ))?;

        let lock = state.target_manager.read()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("TargetManager lock poisoned".to_string())
            ))?;

        lock.clone()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("TargetManager not created. Call Engine::create_target_manager() first.".to_string())
            ))
    }

    /// Destroy the target manager singleton
    ///
    /// Removes the target manager singleton, allowing a new one to be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the engine is not initialized
    ///
    pub fn destroy_target_manager() -> Result<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Self::log_and_return_error(
                Error::InitializationFailed("Engine not initialized".to_string())
            ))?;

        let mut lock = state.target_manager.write()
            .map_err(|_| Self::log_and_return_error(
                Error::BackendError("TargetManager lock poisoned".to_string())
            ))?;

        *lock = None;

        crate::engine_info!("galaxy3d::Engine", "TargetManager singleton destroyed");

        Ok(())
    }

    /// Reset all singletons for testing (only available in test builds)
    #[cfg(test)]
    pub fn reset_for_testing() {
        if let Some(state) = ENGINE_STATE.get() {
            if let Ok(mut tm) = state.target_manager.write() {
                *tm = None;
            }
            if let Ok(mut sm) = state.scene_manager.write() {
                *sm = None;
            }
            if let Ok(mut rm) = state.resource_manager.write() {
                *rm = None;
            }
            if let Ok(mut renderers) = state.renderers.write() {
                renderers.clear();
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
