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
    /// Returns the region id (index) on success.
    /// Default implementation returns an error. Override in AtlasTexture.
    fn add_atlas_region(&mut self, _name: String, _region: AtlasRegion) -> Result<usize> {
        Err(Error::BackendError(
            "This texture type does not support atlas regions".to_string()
        ))
    }

    /// Get atlas region id by name (atlas textures only)
    fn get_atlas_region_id(&self, _name: &str) -> Option<usize> { None }

    /// Get atlas region by id (atlas textures only)
    fn get_atlas_region(&self, _id: usize) -> Option<&AtlasRegion> { None }

    /// Add a layer mapping to this texture (array textures only)
    ///
    /// Returns the region id (index) on success.
    /// If `data` is provided, uploads pixel data to the specified layer.
    /// Default implementation returns an error. Override in ArrayTexture.
    fn add_array_layer(&mut self, _name: String, _layer: u32, _data: Option<&[u8]>) -> Result<usize> {
        Err(Error::BackendError(
            "This texture type does not support array layers".to_string()
        ))
    }

    /// Get array layer region id by name (array textures only)
    fn get_array_layer_id(&self, _name: &str) -> Option<usize> { None }

    /// Get array layer region by id (array textures only)
    fn get_array_layer(&self, _id: usize) -> Option<&LayerRegion> { None }
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

/// Layer region within a texture array
#[derive(Debug, Clone)]
pub struct LayerRegion {
    /// Layer index in the GPU texture array
    pub layer: u32,
}

/// Descriptor for batch-creating array layers
#[derive(Debug, Clone)]
pub struct ArrayLayerDesc {
    /// Layer name (used as key in the array)
    pub name: String,
    /// Layer index in the texture array
    pub layer: u32,
    /// Optional pixel data to upload for this layer
    pub data: Option<Vec<u8>>,
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
/// coordinates. Regions are stored in a Vec and accessible by id (index).
///
/// Regions can be provided at creation time and/or added later.
pub struct AtlasTexture {
    #[allow(dead_code)]
    renderer: Arc<Mutex<dyn Renderer>>,
    render_texture: Arc<dyn RenderTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    /// Regions stored by index (id)
    regions: Vec<AtlasRegion>,
    /// Name to id (index) mapping
    region_names: HashMap<String, usize>,
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
        let mut region_vec = Vec::with_capacity(regions.len());
        let mut name_map = HashMap::with_capacity(regions.len());
        for desc in regions {
            let id = region_vec.len();
            region_vec.push(desc.region.clone());
            name_map.insert(desc.name.clone(), id);
        }
        Self {
            renderer,
            render_texture,
            descriptor_set,
            regions: region_vec,
            region_names: name_map,
        }
    }

    /// Add a region, returns its id (index)
    fn add_region_internal(&mut self, name: String, region: AtlasRegion) -> usize {
        let id = self.regions.len();
        self.regions.push(region);
        self.region_names.insert(name, id);
        id
    }

    /// Get a region by id (index)
    pub fn get_region(&self, id: usize) -> Option<&AtlasRegion> {
        self.regions.get(id)
    }

    /// Get region id by name
    pub fn get_region_id(&self, name: &str) -> Option<usize> {
        self.region_names.get(name).copied()
    }

    /// Get a region by name (convenience: name -> id -> region)
    pub fn get_region_by_name(&self, name: &str) -> Option<&AtlasRegion> {
        self.region_names.get(name).and_then(|&id| self.regions.get(id))
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
        self.region_names.keys().map(|k| k.as_str()).collect()
    }

    fn as_atlas(&self) -> Option<&AtlasTexture> {
        Some(self)
    }

    fn as_atlas_mut(&mut self) -> Option<&mut AtlasTexture> {
        Some(self)
    }

    fn add_atlas_region(&mut self, name: String, region: AtlasRegion) -> Result<usize> {
        let id = self.add_region_internal(name, region);
        Ok(id)
    }

    fn get_atlas_region_id(&self, name: &str) -> Option<usize> {
        self.get_region_id(name)
    }

    fn get_atlas_region(&self, id: usize) -> Option<&AtlasRegion> {
        self.get_region(id)
    }
}

// ===== ARRAY TEXTURE =====

/// A texture array with named layers.
///
/// Wraps a GPU texture array where each layer is identified by name
/// and mapped to a layer index. Regions are stored in a Vec and accessible by id (index).
///
/// Layers can be provided at creation time and/or added later.
pub struct ArrayTexture {
    #[allow(dead_code)]
    renderer: Arc<Mutex<dyn Renderer>>,
    render_texture: Arc<dyn RenderTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    /// Regions stored by index (id)
    regions: Vec<LayerRegion>,
    /// Name to id (index) mapping
    region_names: HashMap<String, usize>,
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
        let mut region_vec = Vec::with_capacity(layers.len());
        let mut name_map = HashMap::with_capacity(layers.len());
        for desc in layers {
            let id = region_vec.len();
            region_vec.push(LayerRegion { layer: desc.layer });
            name_map.insert(desc.name.clone(), id);
        }
        Self {
            renderer,
            render_texture,
            descriptor_set,
            regions: region_vec,
            region_names: name_map,
        }
    }

    /// Add a region, returns its id (index)
    fn add_region_internal(&mut self, name: String, region: LayerRegion) -> usize {
        let id = self.regions.len();
        self.regions.push(region);
        self.region_names.insert(name, id);
        id
    }

    /// Get a region by id (index)
    pub fn get_region(&self, id: usize) -> Option<&LayerRegion> {
        self.regions.get(id)
    }

    /// Get region id by name
    pub fn get_region_id(&self, name: &str) -> Option<usize> {
        self.region_names.get(name).copied()
    }

    /// Get a region by name (convenience: name -> id -> region)
    pub fn get_region_by_name(&self, name: &str) -> Option<&LayerRegion> {
        self.region_names.get(name).and_then(|&id| self.regions.get(id))
    }

    /// Get the number of regions
    pub fn region_count(&self) -> usize {
        self.regions.len()
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
        self.region_names.keys().map(|k| k.as_str()).collect()
    }

    fn as_array(&self) -> Option<&ArrayTexture> {
        Some(self)
    }

    fn as_array_mut(&mut self) -> Option<&mut ArrayTexture> {
        Some(self)
    }

    fn add_array_layer(&mut self, name: String, layer: u32, data: Option<&[u8]>) -> Result<usize> {
        // If pixel data is provided, upload it to the GPU via the render texture
        if let Some(pixel_data) = data {
            self.render_texture.update_layer(layer, pixel_data)?;
        }

        let id = self.add_region_internal(name, LayerRegion { layer });
        Ok(id)
    }

    fn get_array_layer_id(&self, name: &str) -> Option<usize> {
        self.get_region_id(name)
    }

    fn get_array_layer(&self, id: usize) -> Option<&LayerRegion> {
        self.get_region(id)
    }
}

// ===== RESOURCE DESCRIPTORS =====

use crate::renderer::TextureDesc;

/// Descriptor for creating a SimpleTexture resource
pub struct SimpleTextureDesc {
    /// Renderer to use for GPU texture creation
    pub renderer: Arc<Mutex<dyn Renderer>>,
    /// GPU texture description (format, size, data, etc.)
    pub texture: TextureDesc,
}

/// Descriptor for creating an AtlasTexture resource
pub struct AtlasTextureDesc {
    /// Renderer to use for GPU texture creation
    pub renderer: Arc<Mutex<dyn Renderer>>,
    /// GPU texture description (format, size, data, etc.)
    pub texture: TextureDesc,
    /// Initial atlas regions (can be empty, add later via add_atlas_region)
    pub regions: Vec<AtlasRegionDesc>,
}

/// Descriptor for creating an ArrayTexture resource
pub struct ArrayTextureDesc {
    /// Renderer to use for GPU texture creation
    pub renderer: Arc<Mutex<dyn Renderer>>,
    /// GPU texture description (format, size, array_layers, etc.)
    pub texture: TextureDesc,
    /// Initial layer mappings with optional pixel data (can be empty)
    pub layers: Vec<ArrayLayerDesc>,
}
