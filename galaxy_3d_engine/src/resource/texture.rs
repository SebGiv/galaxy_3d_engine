/// Unified resource-level texture type.
///
/// Replaces the old SimpleTexture, AtlasTexture, and ArrayTexture with a single
/// unified type that supports layers and atlas regions.
///
/// Architecture:
/// - Simple texture: 1 layer (array_layers=1), optional atlas regions
/// - Indexed texture: N layers (array_layers>1), each with optional atlas regions
///
/// A layer can be an atlas texture if it has regions defined.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::{Error, Result};
use crate::renderer::{
    Texture as RendererTexture,
    DescriptorSet,
    Renderer,
    TextureData,
    TextureLayerData,
    TextureDesc as RenderTextureDesc,
    TextureInfo,
    TextureFormat,
    MipmapMode,
    ManualMipmapData,
};

// ===== TEXTURE =====

/// Unified texture resource
///
/// Supports both simple and indexed textures, with optional atlas regions per layer.
pub struct Texture {
    renderer_texture: Arc<dyn RendererTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    layers: Vec<TextureLayer>,
    layer_names: HashMap<String, usize>,
}

/// A single layer in a texture
///
/// Can optionally contain atlas regions for sprite/tile mapping.
pub struct TextureLayer {
    name: String,
    layer_index: u32,
    regions: Vec<AtlasRegion>,
    region_names: HashMap<String, usize>,
}

/// Atlas region definition
///
/// Defines a rectangular sub-region within a texture layer.
#[derive(Debug, Clone)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

// ===== DESCRIPTORS =====

/// Texture creation descriptor
pub struct TextureDesc {
    pub renderer: Arc<Mutex<dyn Renderer>>,
    pub texture: RenderTextureDesc,
    pub layers: Vec<LayerDesc>,
}

/// Layer descriptor
pub struct LayerDesc {
    pub name: String,
    pub layer_index: u32,
    pub data: Option<Vec<u8>>,
    pub regions: Vec<AtlasRegionDesc>,
}

/// Atlas region descriptor
pub struct AtlasRegionDesc {
    pub name: String,
    pub region: AtlasRegion,
}

// ===== TEXTURE IMPLEMENTATION =====

impl Texture {
    /// Create texture from descriptor (internal use by ResourceManager)
    pub(crate) fn from_desc(desc: TextureDesc) -> Result<Self> {
        let array_layers = desc.texture.array_layers;
        let is_simple = array_layers == 1;
        let is_indexed = array_layers > 1;

        // ========== VALIDATION 1: Simple texture constraints ==========
        if is_simple {
            // Simple texture MUST have exactly 1 layer
            if desc.layers.len() != 1 {
                return Err(Error::BackendError(format!(
                    "Simple texture (array_layers=1) must have exactly 1 layer, got {}",
                    desc.layers.len()
                )));
            }

            // Layer index must be 0
            if desc.layers[0].layer_index != 0 {
                return Err(Error::BackendError(format!(
                    "Simple texture layer must have index 0, got {}",
                    desc.layers[0].layer_index
                )));
            }
        }

        // ========== VALIDATION 2: Indexed texture constraints ==========
        if is_indexed && desc.layers.is_empty() {
            // OK: indexed texture can be created empty and layers added later
        }

        // ========== VALIDATION 3: Layer index bounds ==========
        for layer_desc in &desc.layers {
            if layer_desc.layer_index >= array_layers {
                return Err(Error::BackendError(format!(
                    "Layer '{}' has index {} but array_layers = {}",
                    layer_desc.name, layer_desc.layer_index, array_layers
                )));
            }
        }

        // ========== VALIDATION 4: No duplicate layer names ==========
        let mut seen_names = std::collections::HashSet::new();
        for layer_desc in &desc.layers {
            if !seen_names.insert(&layer_desc.name) {
                return Err(Error::BackendError(format!(
                    "Duplicate layer name '{}'", layer_desc.name
                )));
            }
        }

        // ========== VALIDATION 5: No duplicate layer indices ==========
        let mut seen_indices = std::collections::HashSet::new();
        for layer_desc in &desc.layers {
            if !seen_indices.insert(layer_desc.layer_index) {
                return Err(Error::BackendError(format!(
                    "Duplicate layer index {}", layer_desc.layer_index
                )));
            }
        }

        // ========== VALIDATION 6: Atlas region bounds ==========
        let texture_width = desc.texture.width;
        let texture_height = desc.texture.height;

        for layer_desc in &desc.layers {
            for region_desc in &layer_desc.regions {
                let region = &region_desc.region;

                // Check if region is within texture bounds
                if region.x + region.width > texture_width {
                    return Err(Error::BackendError(format!(
                        "Region '{}' in layer '{}' exceeds texture width: {}+{} > {}",
                        region_desc.name, layer_desc.name,
                        region.x, region.width, texture_width
                    )));
                }

                if region.y + region.height > texture_height {
                    return Err(Error::BackendError(format!(
                        "Region '{}' in layer '{}' exceeds texture height: {}+{} > {}",
                        region_desc.name, layer_desc.name,
                        region.y, region.height, texture_height
                    )));
                }

                // Check non-zero dimensions
                if region.width == 0 || region.height == 0 {
                    return Err(Error::BackendError(format!(
                        "Region '{}' in layer '{}' has zero dimension: {}x{}",
                        region_desc.name, layer_desc.name,
                        region.width, region.height
                    )));
                }
            }
        }

        // ========== VALIDATION 7: No duplicate region names within layer ==========
        for layer_desc in &desc.layers {
            let mut seen_region_names = std::collections::HashSet::new();
            for region_desc in &layer_desc.regions {
                if !seen_region_names.insert(&region_desc.name) {
                    return Err(Error::BackendError(format!(
                        "Duplicate region name '{}' in layer '{}'",
                        region_desc.name, layer_desc.name
                    )));
                }
            }
        }

        // ========== VALIDATION 8: Mipmap data consistency ==========
        // If manual mipmaps with layers, validate layer indices
        if let MipmapMode::Manual(ref manual_data) = desc.texture.mipmap {
            if let ManualMipmapData::Layers(ref mipmap_layers) = manual_data {
                for mipmap_layer in mipmap_layers {
                    if mipmap_layer.layer >= array_layers {
                        return Err(Error::BackendError(format!(
                            "Manual mipmap references layer {} but array_layers = {}",
                            mipmap_layer.layer, array_layers
                        )));
                    }
                }
            }
        }

        // ========== VALIDATION 9: Layer data validation ==========
        for layer_desc in &desc.layers {
            if let Some(ref data) = layer_desc.data {
                // Validate data size matches texture dimensions
                let expected_size = Self::calculate_layer_data_size(
                    texture_width,
                    texture_height,
                    desc.texture.format
                );

                if data.len() != expected_size {
                    return Err(Error::BackendError(format!(
                        "Layer '{}' data size mismatch: expected {} bytes, got {}",
                        layer_desc.name, expected_size, data.len()
                    )));
                }
            }
        }

        // ========== CREATE RENDER TEXTURE ==========

        // Prepare texture data for render::Texture
        let mut render_texture_desc = desc.texture.clone();

        // Collect layer data for upload
        if !desc.layers.is_empty() {
            let layer_data: Vec<TextureLayerData> = desc.layers
                .iter()
                .filter_map(|ld| {
                    ld.data.as_ref().map(|d| TextureLayerData {
                        layer: ld.layer_index,
                        data: d.clone(),
                    })
                })
                .collect();

            if !layer_data.is_empty() {
                render_texture_desc.data = Some(TextureData::Layers(layer_data));
            }
        }

        // Create the GPU texture
        let renderer_texture = desc.renderer.lock().unwrap().create_texture(render_texture_desc)?;

        // Create descriptor set
        let descriptor_set = desc.renderer.lock().unwrap().create_descriptor_set_for_texture(&renderer_texture)?;

        // ========== BUILD LAYERS ==========

        let mut layers = Vec::new();
        let mut layer_names = HashMap::new();

        for (vec_index, layer_desc) in desc.layers.into_iter().enumerate() {
            // Build regions for this layer
            let mut regions = Vec::new();
            let mut region_names = HashMap::new();

            for (region_index, region_desc) in layer_desc.regions.into_iter().enumerate() {
                regions.push(region_desc.region);
                region_names.insert(region_desc.name, region_index);
            }

            let layer = TextureLayer {
                name: layer_desc.name.clone(),
                layer_index: layer_desc.layer_index,
                regions,
                region_names,
            };

            layers.push(layer);
            layer_names.insert(layer_desc.name, vec_index);
        }

        Ok(Self {
            renderer_texture,
            descriptor_set,
            layers,
            layer_names,
        })
    }

    /// Calculate expected data size for a layer
    fn calculate_layer_data_size(width: u32, height: u32, format: TextureFormat) -> usize {
        let bytes_per_pixel = match format {
            TextureFormat::R8G8B8A8_SRGB => 4,
            TextureFormat::R8G8B8A8_UNORM => 4,
            TextureFormat::B8G8R8A8_SRGB => 4,
            TextureFormat::B8G8R8A8_UNORM => 4,
            _ => {
                // For unsupported formats (depth, vertex attributes), return 0 to skip validation
                return 0;
            }
        };

        (width * height) as usize * bytes_per_pixel
    }

    // ===== LAYER ACCESS =====

    /// Get layer by index (primary access method)
    pub fn layer(&self, index: u32) -> Option<&TextureLayer> {
        self.layers.get(index as usize)
    }

    /// Get layer by name
    pub fn layer_by_name(&self, name: &str) -> Option<&TextureLayer> {
        let index = self.layer_names.get(name)?;
        self.layers.get(*index)
    }

    /// Get layer index from name
    pub fn layer_index_by_name(&self, name: &str) -> Option<u32> {
        self.layer_names.get(name).map(|&idx| idx as u32)
    }

    /// Get total number of layers
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    // ===== REGION ACCESS (convenience) =====

    /// Get region from a specific layer by names
    pub fn region(&self, layer_name: &str, region_name: &str) -> Option<&AtlasRegion> {
        self.layer_by_name(layer_name)?.region_by_name(region_name)
    }

    // ===== MODIFICATION =====

    /// Add a new layer (indexed textures only)
    pub fn add_layer(&mut self, desc: LayerDesc) -> Result<u32> {
        if !self.is_indexed() {
            return Err(Error::BackendError(
                "Cannot add layer to simple texture (array_layers=1)".to_string()
            ));
        }

        let array_layers = self.renderer_texture.info().array_layers;

        // Validate layer index
        if desc.layer_index >= array_layers {
            return Err(Error::BackendError(format!(
                "Layer index {} >= array_layers {}",
                desc.layer_index, array_layers
            )));
        }

        // Check for duplicate name
        if self.layer_names.contains_key(&desc.name) {
            return Err(Error::BackendError(format!(
                "Layer '{}' already exists", desc.name
            )));
        }

        // Check for duplicate index
        for existing_layer in &self.layers {
            if existing_layer.layer_index == desc.layer_index {
                return Err(Error::BackendError(format!(
                    "Layer index {} already used by layer '{}'",
                    desc.layer_index, existing_layer.name
                )));
            }
        }

        // Upload layer data if provided
        if let Some(ref data) = desc.data {
            self.renderer_texture.update(desc.layer_index, 0, data)?;
        }

        // Build regions
        let mut regions = Vec::new();
        let mut region_names = HashMap::new();

        for (region_index, region_desc) in desc.regions.into_iter().enumerate() {
            // Validate region bounds
            let info = self.renderer_texture.info();
            if region_desc.region.x + region_desc.region.width > info.width ||
               region_desc.region.y + region_desc.region.height > info.height {
                return Err(Error::BackendError(format!(
                    "Region '{}' exceeds texture bounds", region_desc.name
                )));
            }

            regions.push(region_desc.region);
            region_names.insert(region_desc.name, region_index);
        }

        let layer = TextureLayer {
            name: desc.name.clone(),
            layer_index: desc.layer_index,
            regions,
            region_names,
        };

        let vec_index = self.layers.len();
        self.layers.push(layer);
        self.layer_names.insert(desc.name, vec_index);

        Ok(vec_index as u32)
    }

    /// Add region to an existing layer
    pub fn add_region(&mut self, layer_name: &str, desc: AtlasRegionDesc) -> Result<u32> {
        let layer_vec_index = *self.layer_names.get(layer_name)
            .ok_or_else(|| Error::BackendError(format!("Layer '{}' not found", layer_name)))?;

        let layer = &mut self.layers[layer_vec_index];
        layer.add_region(desc, &self.renderer_texture.info())
    }

    // ===== INFO =====

    /// Check if this is a simple texture (array_layers == 1)
    pub fn is_simple(&self) -> bool {
        self.renderer_texture.info().array_layers == 1
    }

    /// Check if this is an indexed texture (array_layers > 1)
    pub fn is_indexed(&self) -> bool {
        self.renderer_texture.info().array_layers > 1
    }

    /// Get the underlying renderer texture
    pub fn renderer_texture(&self) -> &Arc<dyn RendererTexture> {
        &self.renderer_texture
    }

    /// Get the descriptor set for shader binding
    pub fn descriptor_set(&self) -> &Arc<dyn DescriptorSet> {
        &self.descriptor_set
    }
}

// ===== TEXTURE LAYER IMPLEMENTATION =====

impl TextureLayer {
    // ===== REGION ACCESS =====

    /// Get region by index
    pub fn region(&self, index: u32) -> Option<&AtlasRegion> {
        self.regions.get(index as usize)
    }

    /// Get region by name
    pub fn region_by_name(&self, name: &str) -> Option<&AtlasRegion> {
        let index = self.region_names.get(name)?;
        self.regions.get(*index)
    }

    /// Get region index from name
    pub fn region_index_by_name(&self, name: &str) -> Option<u32> {
        self.region_names.get(name).map(|&idx| idx as u32)
    }

    /// Get region count
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    // ===== INFO =====

    /// Check if this layer has atlas regions
    pub fn is_atlas(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Get layer name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get layer index in render texture
    pub fn layer_index(&self) -> u32 {
        self.layer_index
    }

    // ===== MODIFICATION (internal) =====

    pub(crate) fn add_region(&mut self, desc: AtlasRegionDesc, texture_info: &TextureInfo) -> Result<u32> {
        // Check duplicate name
        if self.region_names.contains_key(&desc.name) {
            return Err(Error::BackendError(format!(
                "Region '{}' already exists in layer '{}'", desc.name, self.name
            )));
        }

        // Validate bounds
        if desc.region.x + desc.region.width > texture_info.width {
            return Err(Error::BackendError(format!(
                "Region '{}' exceeds texture width: {}+{} > {}",
                desc.name, desc.region.x, desc.region.width, texture_info.width
            )));
        }

        if desc.region.y + desc.region.height > texture_info.height {
            return Err(Error::BackendError(format!(
                "Region '{}' exceeds texture height: {}+{} > {}",
                desc.name, desc.region.y, desc.region.height, texture_info.height
            )));
        }

        // Check non-zero dimensions
        if desc.region.width == 0 || desc.region.height == 0 {
            return Err(Error::BackendError(format!(
                "Region '{}' has zero dimension", desc.name
            )));
        }

        let index = self.regions.len();
        self.regions.push(desc.region);
        self.region_names.insert(desc.name, index);
        Ok(index as u32)
    }
}

#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;
