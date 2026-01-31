/// Galaxy3D Engine - Singleton manager for engine subsystems
///
/// This module provides global singleton management for the renderer and other
/// engine subsystems. It uses thread-safe static storage with RwLock for safe
/// concurrent access.

use std::sync::{OnceLock, RwLock, Arc, Mutex};
use crate::{Renderer, Galaxy3dResult, Galaxy3dError};

// ===== INTERNAL STATE =====

/// Global engine state storage
static ENGINE_STATE: OnceLock<EngineState> = OnceLock::new();

/// Internal state structure holding all engine singletons
struct EngineState {
    /// Renderer singleton (wrapped in Mutex for thread-safe mutable access)
    renderer: RwLock<Option<Arc<Mutex<dyn Renderer>>>>,
}

impl EngineState {
    /// Create a new empty engine state
    fn new() -> Self {
        Self {
            renderer: RwLock::new(None),
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
/// use galaxy_3d_engine::{Galaxy3dEngine, Renderer};
/// use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;
///
/// // Initialize engine
/// Galaxy3dEngine::initialize()?;
///
/// // Create renderer singleton
/// Galaxy3dEngine::create_renderer(VulkanRenderer::new(&window, config)?)?;
///
/// // Access renderer globally
/// let renderer = Renderer::instance()?;
///
/// // Cleanup
/// Galaxy3dEngine::shutdown();
/// # Ok::<(), galaxy_3d_engine::Galaxy3dError>(())
/// ```
pub struct Galaxy3dEngine;

impl Galaxy3dEngine {
    /// Initialize the engine
    ///
    /// This must be called once at application startup before creating any subsystems.
    ///
    /// # Errors
    ///
    /// Currently always succeeds, but returns Result for future extensibility.
    pub fn initialize() -> Galaxy3dResult<()> {
        ENGINE_STATE.get_or_init(|| EngineState::new());
        Ok(())
    }

    /// Shutdown the entire engine and destroy all singletons
    ///
    /// This should be called at application shutdown to properly cleanup all subsystems.
    /// After calling this, you must call `initialize()` again before creating new subsystems.
    pub fn shutdown() {
        if let Some(state) = ENGINE_STATE.get() {
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
    /// use galaxy_3d_engine::Galaxy3dEngine;
    /// use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;
    ///
    /// Galaxy3dEngine::initialize()?;
    /// Galaxy3dEngine::create_renderer(VulkanRenderer::new(&window, config)?)?;
    /// # Ok::<(), galaxy_3d_engine::Galaxy3dError>(())
    /// ```
    pub fn create_renderer<R: Renderer + 'static>(renderer: R) -> Galaxy3dResult<()> {
        // Wrap in Arc<Mutex<dyn Renderer>>
        let arc_renderer: Arc<Mutex<dyn Renderer>> = Arc::new(Mutex::new(renderer));

        // Register as singleton
        Self::register_renderer(arc_renderer)
    }

    /// Register a renderer singleton (internal use)
    ///
    /// This is called internally by create_renderer(). Marked pub(crate) to allow
    /// access from other modules if needed.
    pub(crate) fn register_renderer(renderer: Arc<Mutex<dyn Renderer>>) -> Galaxy3dResult<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Galaxy3dError::InitializationFailed("Engine not initialized. Call Galaxy3dEngine::initialize() first.".to_string()))?;

        let mut lock = state.renderer.write()
            .map_err(|_| Galaxy3dError::BackendError("Renderer lock poisoned".to_string()))?;

        if lock.is_some() {
            return Err(Galaxy3dError::InitializationFailed("Renderer already exists. Call Renderer::destroy_singleton() first.".to_string()));
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
    /// use galaxy_3d_engine::Galaxy3dEngine;
    ///
    /// let renderer = Galaxy3dEngine::renderer()?;
    /// let renderer_guard = renderer.lock().unwrap();
    /// // Use renderer_guard...
    /// # Ok::<(), galaxy_3d_engine::Galaxy3dError>(())
    /// ```
    pub fn renderer() -> Galaxy3dResult<Arc<Mutex<dyn Renderer>>> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Galaxy3dError::InitializationFailed("Engine not initialized. Call Galaxy3dEngine::initialize() first.".to_string()))?;

        let lock = state.renderer.read()
            .map_err(|_| Galaxy3dError::BackendError("Renderer lock poisoned".to_string()))?;

        lock.clone()
            .ok_or_else(|| Galaxy3dError::InitializationFailed("Renderer not created. Call Galaxy3dEngine::create_renderer() first.".to_string()))
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
    /// use galaxy_3d_engine::Galaxy3dEngine;
    ///
    /// Galaxy3dEngine::destroy_renderer()?;
    /// # Ok::<(), galaxy_3d_engine::Galaxy3dError>(())
    /// ```
    pub fn destroy_renderer() -> Galaxy3dResult<()> {
        let state = ENGINE_STATE.get()
            .ok_or_else(|| Galaxy3dError::InitializationFailed("Engine not initialized".to_string()))?;

        let mut lock = state.renderer.write()
            .map_err(|_| Galaxy3dError::BackendError("Renderer lock poisoned".to_string()))?;

        *lock = None;
        Ok(())
    }

    /// Reset all singletons for testing (only available in test builds)
    #[cfg(test)]
    pub fn reset_for_testing() {
        if let Some(state) = ENGINE_STATE.get() {
            if let Ok(mut renderer) = state.renderer.write() {
                *renderer = None;
            }
        }
    }
}
