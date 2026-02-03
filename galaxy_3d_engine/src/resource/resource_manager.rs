/// Central resource manager for the engine.
///
/// Stores and provides access to all engine resources (textures, meshes, etc.).
/// Resources will be added incrementally as the engine evolves.

use std::collections::HashMap;
use std::sync::Arc;
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::renderer::TextureDesc;
use crate::resource::texture::{
    Texture, SimpleTexture, AtlasTexture, ArrayTexture,
    AtlasRegion, AtlasRegionDesc, ArrayLayerDesc,
};

pub struct ResourceManager {
    textures: HashMap<String, Arc<dyn Texture>>,
}

impl ResourceManager {
    /// Create a new empty resource manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    // ===== TEXTURE CREATION =====

    /// Create a simple texture (no sub-regions) and register it
    ///
    /// Internally creates the GPU texture and descriptor set via the renderer.
    /// Returns the created texture for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this texture resource
    /// * `desc` - GPU texture description (format, size, data, etc.)
    pub fn create_simple_texture(&mut self, name: String, desc: TextureDesc) -> Result<Arc<dyn Texture>> {
        if self.textures.contains_key(&name) {
            return Err(Error::BackendError(format!(
                "Texture '{}' already exists in ResourceManager", name
            )));
        }

        // Simple textures must have array_layers == 1
        if desc.array_layers != 1 {
            return Err(Error::BackendError(format!(
                "SimpleTexture requires array_layers = 1, got {}", desc.array_layers
            )));
        }

        let renderer_arc = Engine::renderer()?;
        let render_texture;
        let descriptor_set;
        {
            let mut renderer = renderer_arc.lock()
                .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

            render_texture = renderer.create_texture(desc)?;
            descriptor_set = renderer.create_descriptor_set_for_texture(&render_texture)?;
        }

        let texture: Arc<dyn Texture> = Arc::new(SimpleTexture::new(
            renderer_arc,
            render_texture,
            descriptor_set,
        ));
        self.textures.insert(name.clone(), Arc::clone(&texture));
        crate::engine_info!("galaxy3d::ResourceManager", "Created SimpleTexture resource '{}'", name);
        Ok(texture)
    }

    /// Create an atlas texture and register it
    ///
    /// Pass `&[]` for `regions` to create an empty atlas and add regions later
    /// via `add_atlas_region()`.
    /// Returns the created texture for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this texture resource
    /// * `desc` - GPU texture description
    /// * `regions` - Initial atlas regions (can be empty)
    pub fn create_atlas_texture(
        &mut self,
        name: String,
        desc: TextureDesc,
        regions: &[AtlasRegionDesc],
    ) -> Result<Arc<dyn Texture>> {
        if self.textures.contains_key(&name) {
            return Err(Error::BackendError(format!(
                "Texture '{}' already exists in ResourceManager", name
            )));
        }

        // Atlas textures must have array_layers == 1 (single image with UV sub-regions)
        if desc.array_layers != 1 {
            return Err(Error::BackendError(format!(
                "AtlasTexture requires array_layers = 1, got {}", desc.array_layers
            )));
        }

        let renderer_arc = Engine::renderer()?;
        let render_texture;
        let descriptor_set;
        {
            let mut renderer = renderer_arc.lock()
                .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

            render_texture = renderer.create_texture(desc)?;
            descriptor_set = renderer.create_descriptor_set_for_texture(&render_texture)?;
        }

        let texture: Arc<dyn Texture> = Arc::new(AtlasTexture::new(
            renderer_arc,
            render_texture,
            descriptor_set,
            regions,
        ));
        self.textures.insert(name.clone(), Arc::clone(&texture));
        crate::engine_info!("galaxy3d::ResourceManager", "Created AtlasTexture resource '{}' with {} initial regions", name, regions.len());
        Ok(texture)
    }

    /// Create an array texture and register it
    ///
    /// Pass `&[]` for `layers` to create an empty array texture and add layers
    /// later via `add_array_layer()`.
    /// Returns the created texture for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this texture resource
    /// * `desc` - GPU texture description
    /// * `layers` - Initial layer mappings (can be empty)
    pub fn create_array_texture(
        &mut self,
        name: String,
        desc: TextureDesc,
        layers: &[ArrayLayerDesc],
    ) -> Result<Arc<dyn Texture>> {
        if self.textures.contains_key(&name) {
            return Err(Error::BackendError(format!(
                "Texture '{}' already exists in ResourceManager", name
            )));
        }

        let array_layers = desc.array_layers;

        // Array textures must have array_layers > 1
        if array_layers <= 1 {
            return Err(Error::BackendError(format!(
                "ArrayTexture requires array_layers > 1, got {}", array_layers
            )));
        }

        // Validate that layer name mappings don't exceed array_layers
        for layer_desc in layers {
            if layer_desc.layer >= array_layers {
                return Err(Error::BackendError(format!(
                    "ArrayLayerDesc '{}' references layer {} but array_layers = {}",
                    layer_desc.name, layer_desc.layer, array_layers
                )));
            }
        }

        let renderer_arc = Engine::renderer()?;
        let render_texture;
        let descriptor_set;
        {
            let mut renderer = renderer_arc.lock()
                .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

            render_texture = renderer.create_texture(desc)?;
            descriptor_set = renderer.create_descriptor_set_for_texture(&render_texture)?;
        }

        let texture: Arc<dyn Texture> = Arc::new(ArrayTexture::new(
            renderer_arc,
            render_texture,
            descriptor_set,
            layers,
        ));
        self.textures.insert(name.clone(), Arc::clone(&texture));
        crate::engine_info!("galaxy3d::ResourceManager", "Created ArrayTexture resource '{}' with {} array layers and {} named layers",
            name, array_layers, layers.len());
        Ok(texture)
    }

    // ===== TEXTURE ACCESS =====

    /// Get a texture by name
    pub fn texture(&self, name: &str) -> Option<&Arc<dyn Texture>> {
        self.textures.get(name)
    }

    /// Remove a texture by name
    ///
    /// Returns `true` if the texture was found and removed.
    pub fn remove_texture(&mut self, name: &str) -> bool {
        self.textures.remove(name).is_some()
    }

    /// Get the number of registered textures
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    // ===== TEXTURE MODIFICATION =====

    /// Add a region to an existing atlas texture
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the texture Arc exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The texture does not exist
    /// - The texture is not an AtlasTexture
    /// - Other Arc references prevent mutable access
    pub fn add_atlas_region(
        &mut self,
        texture_name: &str,
        region_name: String,
        region: AtlasRegion,
    ) -> Result<()> {
        let arc = self.textures.get_mut(texture_name)
            .ok_or_else(|| Error::BackendError(format!(
                "Texture '{}' not found in ResourceManager", texture_name
            )))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| Error::BackendError(format!(
                "Cannot mutate texture '{}': other references exist", texture_name
            )))?;

        // Delegate to the trait method (will return error if not an AtlasTexture)
        texture.add_atlas_region(region_name, region)
    }

    /// Add a layer mapping to an existing array texture
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the texture Arc exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The texture does not exist
    /// - The texture is not an ArrayTexture
    /// - Other Arc references prevent mutable access
    pub fn add_array_layer(
        &mut self,
        texture_name: &str,
        layer_name: String,
        layer: u32,
    ) -> Result<()> {
        let arc = self.textures.get_mut(texture_name)
            .ok_or_else(|| Error::BackendError(format!(
                "Texture '{}' not found in ResourceManager", texture_name
            )))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| Error::BackendError(format!(
                "Cannot mutate texture '{}': other references exist", texture_name
            )))?;

        // Delegate to the trait method (will return error if not an ArrayTexture)
        texture.add_array_layer(layer_name, layer)
    }
}
