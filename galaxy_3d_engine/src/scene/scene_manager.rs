/// Central scene manager for the engine.
///
/// Manages named scenes. Scenes are stored as Arc<Mutex<Scene>>
/// for thread-safe shared access.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::{engine_bail};
use crate::graphics_device;
use crate::resource::buffer::Buffer;
use super::scene::Scene;

/// Scene manager singleton (managed by Engine)
///
/// Stores named scenes. Multiple scenes can be active simultaneously
/// (main scene, UI overlay, minimap, etc.).
pub struct SceneManager {
    scenes: HashMap<String, Arc<Mutex<Scene>>>,
}

impl SceneManager {
    /// Create a new empty scene manager
    pub fn new() -> Self {
        Self {
            scenes: HashMap::new(),
        }
    }

    /// Create a new named scene
    ///
    /// Returns the created scene for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique scene name
    /// * `graphics_device` - GraphicsDevice for creating GPU resources
    /// * `frame_buffer` - Per-frame uniform buffer (camera, lighting, time)
    /// * `instance_buffer` - Per-instance storage buffer (world matrices, flags)
    /// * `material_buffer` - Material storage buffer (shared material parameters)
    ///
    /// # Errors
    ///
    /// Returns an error if a scene with the same name already exists.
    pub fn create_scene(
        &mut self,
        name: &str,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        frame_buffer: Arc<Buffer>,
        instance_buffer: Arc<Buffer>,
        material_buffer: Arc<Buffer>,
    ) -> Result<Arc<Mutex<Scene>>> {
        if self.scenes.contains_key(name) {
            engine_bail!("galaxy3d::SceneManager",
                "Scene '{}' already exists", name);
        }

        let scene = Arc::new(Mutex::new(Scene::new(
            graphics_device, frame_buffer, instance_buffer, material_buffer,
        )));
        self.scenes.insert(name.to_string(), Arc::clone(&scene));
        Ok(scene)
    }

    /// Get a scene by name
    pub fn scene(&self, name: &str) -> Option<Arc<Mutex<Scene>>> {
        self.scenes.get(name).cloned()
    }

    /// Remove a scene by name
    ///
    /// Returns the removed scene, or None if not found.
    pub fn remove_scene(&mut self, name: &str) -> Option<Arc<Mutex<Scene>>> {
        self.scenes.remove(name)
    }

    /// Get the number of scenes
    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }

    /// Get all scene names
    pub fn scene_names(&self) -> Vec<&str> {
        self.scenes.keys().map(|k| k.as_str()).collect()
    }

    /// Remove all scenes
    pub fn clear(&mut self) {
        self.scenes.clear();
    }
}

#[cfg(test)]
#[path = "scene_manager_tests.rs"]
mod tests;
