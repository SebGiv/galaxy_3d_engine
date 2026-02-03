/// Resource-level texture types.
///
/// These types wrap low-level `render::Texture` (GPU) objects with additional
/// metadata for the resource system. Three concrete types are provided:
///
/// - **SimpleTexture**: A single texture with no sub-regions
/// - **AtlasTexture**: A texture atlas with named UV regions
/// - **ArrayTexture**: A texture array with named layers
///
/// All types implement the `Texture` trait for uniform access via trait objects.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::{Error, Result};
use crate::renderer::{
    Texture as RenderTexture,
    DescriptorSet,
    Renderer,
};

// ===== TRAIT =====

/// Resource-level texture trait.
///
/// Provides uniform access to any texture resource regardless of its concrete
/// type (simple, atlas, or array). Downcast methods allow safe access to
/// type-specific functionality without using `Any`.
pub trait Texture: Send + Sync {
    /// Get the underlying GPU texture
    fn render_texture(&self) -> &Arc<dyn RenderTexture>;

    /// Get the descriptor set for shader binding
    fn descriptor_set(&self) -> &Arc<dyn DescriptorSet>;

    /// Get all region/layer names (empty for SimpleTexture)
    fn region_names(&self) -> Vec<&str>;

    /// Downcast to SimpleTexture (returns None for other types)
    fn as_simple(&self) -> Option<&SimpleTexture> { None }

    /// Downcast to AtlasTexture (returns None for other types)
    fn as_atlas(&self) -> Option<&AtlasTexture> { None }

    /// Downcast to mutable AtlasTexture (returns None for other types)
    fn as_atlas_mut(&mut self) -> Option<&mut AtlasTexture> { None }

    /// Downcast to ArrayTexture (returns None for other types)
    fn as_array(&self) -> Option<&ArrayTexture> { None }

    /// Downcast to mutable ArrayTexture (returns None for other types)
    fn as_array_mut(&mut self) -> Option<&mut ArrayTexture> { None }

    /// Add a region to this texture (atlas textures only)
    ///
    /// Default implementation returns an error. Override in AtlasTexture.
    fn add_atlas_region(&mut self, _name: String, _region: AtlasRegion) -> Result<()> {
        Err(Error::BackendError(
            "This texture type does not support atlas regions".to_string()
        ))
    }

    /// Add a layer mapping to this texture (array textures only)
    ///
    /// Default implementation returns an error. Override in ArrayTexture.
    fn add_array_layer(&mut self, _name: String, _layer: u32) -> Result<()> {
        Err(Error::BackendError(
            "This texture type does not support array layers".to_string()
        ))
    }
}

// ===== DATA TYPES =====

/// UV region within a texture atlas
#[derive(Debug, Clone)]
pub struct AtlasRegion {
    /// U coordinate (left edge, 0.0..1.0)
    pub u: f32,
    /// V coordinate (top edge, 0.0..1.0)
    pub v: f32,
    /// Width in UV space (0.0..1.0)
    pub width: f32,
    /// Height in UV space (0.0..1.0)
    pub height: f32,
}

/// Descriptor for batch-creating atlas regions
#[derive(Debug, Clone)]
pub struct AtlasRegionDesc {
    /// Region name (used as key in the atlas)
    pub name: String,
    /// UV region data
    pub region: AtlasRegion,
}

/// Descriptor for batch-creating array layers
#[derive(Debug, Clone)]
pub struct ArrayLayerDesc {
    /// Layer name (used as key in the array)
    pub name: String,
    /// Layer index in the texture array
    pub layer: u32,
}

// ===== SIMPLE TEXTURE =====

/// A single texture with no sub-regions.
///
/// The simplest resource texture type — wraps a GPU texture and its
/// descriptor set with no additional metadata.
pub struct SimpleTexture {
    #[allow(dead_code)]
    renderer: Arc<Mutex<dyn Renderer>>,
    render_texture: Arc<dyn RenderTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
}

impl SimpleTexture {
    /// Create a new simple texture resource
    pub fn new(
        renderer: Arc<Mutex<dyn Renderer>>,
        render_texture: Arc<dyn RenderTexture>,
        descriptor_set: Arc<dyn DescriptorSet>,
    ) -> Self {
        Self {
            renderer,
            render_texture,
            descriptor_set,
        }
    }
}

impl Texture for SimpleTexture {
    fn render_texture(&self) -> &Arc<dyn RenderTexture> {
        &self.render_texture
    }

    fn descriptor_set(&self) -> &Arc<dyn DescriptorSet> {
        &self.descriptor_set
    }

    fn region_names(&self) -> Vec<&str> {
        Vec::new()
    }

    fn as_simple(&self) -> Option<&SimpleTexture> {
        Some(self)
    }
}

// ===== ATLAS TEXTURE =====

/// A texture atlas with named UV regions.
///
/// Wraps a single GPU texture that contains multiple sub-images arranged
/// spatially. Each sub-image is identified by name and described by UV
/// coordinates.
///
/// Regions can be provided at creation time and/or added later.
pub struct AtlasTexture {
    #[allow(dead_code)]
    renderer: Arc<Mutex<dyn Renderer>>,
    render_texture: Arc<dyn RenderTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    regions: HashMap<String, AtlasRegion>,
}

impl AtlasTexture {
    /// Create a new atlas texture resource
    ///
    /// Pass `&[]` for `regions` to create an empty atlas and add regions later.
    pub fn new(
        renderer: Arc<Mutex<dyn Renderer>>,
        render_texture: Arc<dyn RenderTexture>,
        descriptor_set: Arc<dyn DescriptorSet>,
        regions: &[AtlasRegionDesc],
    ) -> Self {
        let mut map = HashMap::with_capacity(regions.len());
        for desc in regions {
            map.insert(desc.name.clone(), desc.region.clone());
        }
        Self {
            renderer,
            render_texture,
            descriptor_set,
            regions: map,
        }
    }

    /// Add or update a region in the atlas (internal method)
    fn add_region_internal(&mut self, name: String, region: AtlasRegion) {
        self.regions.insert(name, region);
    }

    /// Get a region by name
    pub fn get_region(&self, name: &str) -> Option<&AtlasRegion> {
        self.regions.get(name)
    }

    /// Get the number of regions
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }
}

impl Texture for AtlasTexture {
    fn render_texture(&self) -> &Arc<dyn RenderTexture> {
        &self.render_texture
    }

    fn descriptor_set(&self) -> &Arc<dyn DescriptorSet> {
        &self.descriptor_set
    }

    fn region_names(&self) -> Vec<&str> {
        self.regions.keys().map(|k| k.as_str()).collect()
    }

    fn as_atlas(&self) -> Option<&AtlasTexture> {
        Some(self)
    }

    fn as_atlas_mut(&mut self) -> Option<&mut AtlasTexture> {
        Some(self)
    }

    fn add_atlas_region(&mut self, name: String, region: AtlasRegion) -> Result<()> {
        self.add_region_internal(name, region);
        Ok(())
    }
}

// ===== ARRAY TEXTURE =====

/// A texture array with named layers.
///
/// Wraps a GPU texture array where each layer is identified by name
/// and mapped to a layer index.
///
/// Layers can be provided at creation time and/or added later.
pub struct ArrayTexture {
    #[allow(dead_code)]
    renderer: Arc<Mutex<dyn Renderer>>,
    render_texture: Arc<dyn RenderTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    layers: HashMap<String, u32>,
}

impl ArrayTexture {
    /// Create a new array texture resource
    ///
    /// Pass `&[]` for `layers` to create an empty array texture and add layers later.
    pub fn new(
        renderer: Arc<Mutex<dyn Renderer>>,
        render_texture: Arc<dyn RenderTexture>,
        descriptor_set: Arc<dyn DescriptorSet>,
        layers: &[ArrayLayerDesc],
    ) -> Self {
        let mut map = HashMap::with_capacity(layers.len());
        for desc in layers {
            map.insert(desc.name.clone(), desc.layer);
        }
        Self {
            renderer,
            render_texture,
            descriptor_set,
            layers: map,
        }
    }

    /// Add or update a layer mapping (internal method)
    fn add_layer_internal(&mut self, name: String, layer: u32) {
        self.layers.insert(name, layer);
    }

    /// Get a layer index by name
    pub fn get_layer(&self, name: &str) -> Option<u32> {
        self.layers.get(name).copied()
    }

    /// Get the number of named layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

impl Texture for ArrayTexture {
    fn render_texture(&self) -> &Arc<dyn RenderTexture> {
        &self.render_texture
    }

    fn descriptor_set(&self) -> &Arc<dyn DescriptorSet> {
        &self.descriptor_set
    }

    fn region_names(&self) -> Vec<&str> {
        self.layers.keys().map(|k| k.as_str()).collect()
    }

    fn as_array(&self) -> Option<&ArrayTexture> {
        Some(self)
    }

    fn as_array_mut(&mut self) -> Option<&mut ArrayTexture> {
        Some(self)
    }

    fn add_array_layer(&mut self, name: String, layer: u32) -> Result<()> {
        self.add_layer_internal(name, layer);
        Ok(())
    }
}
