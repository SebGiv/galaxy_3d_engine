/// Resource-level material type.
///
/// A Material is a pure data description of a surface's visual properties.
/// It references a Pipeline (shader family) and provides textures and parameters.
///
/// No GPU resources are created at this level. The Scene/Renderer layer
/// will create optimized GPU objects (descriptor sets, UBOs) from Materials.
///
/// Architecture:
/// - Pipeline reference: which shader family to use (variant selected at render time)
/// - Texture slots: named texture bindings with optional layer/region targeting
/// - Parameters: named scalar/vector values (roughness, base_color, etc.)

use std::collections::HashMap;
use std::sync::Arc;
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::resource::texture::Texture;
use crate::resource::pipeline::Pipeline;

// ===== REFERENCE TYPES =====

/// Reference to a texture layer by name or index
///
/// Used in descriptors to let the user choose the most convenient way
/// to reference a layer. Resolved to a u32 index at creation time.
pub enum LayerRef {
    Index(u32),
    Name(String),
}

/// Reference to an atlas region by name or index
///
/// Used in descriptors to let the user choose the most convenient way
/// to reference a region. Resolved to a u32 index at creation time.
pub enum RegionRef {
    Index(u32),
    Name(String),
}

// ===== PARAMETER VALUES =====

/// A typed parameter value for the material
#[derive(Debug, Clone)]
pub enum ParamValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    UInt(u32),
}

// ===== MATERIAL TEXTURE SLOT =====

/// A texture bound to a named slot in the material (resolved indices)
///
/// After creation, layer and region references are resolved to u32 indices.
pub struct MaterialTextureSlot {
    name: String,
    texture: Arc<Texture>,
    layer: Option<u32>,
    region: Option<u32>,
}

// ===== MATERIAL =====

/// Material resource: visual description of a surface
///
/// Pure data — no GPU resources. References a Pipeline and provides
/// textures (with optional layer/region targeting) and named parameters.
pub struct Material {
    pipeline: Arc<Pipeline>,
    textures: Vec<MaterialTextureSlot>,
    texture_names: HashMap<String, usize>,
    params: Vec<(String, ParamValue)>,
    param_names: HashMap<String, usize>,
}

// ===== DESCRIPTORS =====

/// Material creation descriptor
pub struct MaterialDesc {
    pub pipeline: Arc<Pipeline>,
    pub textures: Vec<MaterialTextureSlotDesc>,
    pub params: Vec<(String, ParamValue)>,
}

/// Texture slot descriptor (user-facing, accepts names or indices)
pub struct MaterialTextureSlotDesc {
    pub name: String,
    pub texture: Arc<Texture>,
    pub layer: Option<LayerRef>,
    pub region: Option<RegionRef>,
}

// ===== MATERIAL IMPLEMENTATION =====

impl Material {
    /// Create material from descriptor (internal use by ResourceManager)
    pub(crate) fn from_desc(desc: MaterialDesc) -> Result<Self> {

        // ========== VALIDATION 1: No duplicate texture slot names ==========
        let mut seen_names = std::collections::HashSet::new();
        for slot_desc in &desc.textures {
            if !seen_names.insert(&slot_desc.name) {
                engine_bail!("galaxy3d::Material",
                    "Duplicate texture slot name '{}'", slot_desc.name);
            }
        }

        // ========== VALIDATION 2: No duplicate param names ==========
        let mut seen_param_names = std::collections::HashSet::new();
        for (param_name, _) in &desc.params {
            if !seen_param_names.insert(param_name) {
                engine_bail!("galaxy3d::Material",
                    "Duplicate parameter name '{}'", param_name);
            }
        }

        // ========== RESOLVE TEXTURE SLOTS ==========
        let mut textures = Vec::with_capacity(desc.textures.len());
        let mut texture_names = HashMap::new();

        for (vec_index, slot_desc) in desc.textures.into_iter().enumerate() {
            // Resolve layer reference
            let resolved_layer = match slot_desc.layer {
                None => None,
                Some(LayerRef::Index(i)) => {
                    if slot_desc.texture.layer(i).is_none() {
                        engine_bail!("galaxy3d::Material",
                            "Texture slot '{}': layer index {} does not exist",
                            slot_desc.name, i);
                    }
                    Some(i)
                }
                Some(LayerRef::Name(ref name)) => {
                    let idx = slot_desc.texture.layer_index_by_name(name)
                        .ok_or_else(|| engine_err!("galaxy3d::Material",
                            "Texture slot '{}': layer '{}' not found",
                            slot_desc.name, name))?;
                    Some(idx)
                }
            };

            // Resolve region reference (requires a resolved layer)
            let resolved_region = match slot_desc.region {
                None => None,
                Some(region_ref) => {
                    let layer_idx = resolved_layer
                        .ok_or_else(|| engine_err!("galaxy3d::Material",
                            "Texture slot '{}': region specified without a layer",
                            slot_desc.name))?;

                    let layer = slot_desc.texture.layer(layer_idx)
                        .ok_or_else(|| engine_err!("galaxy3d::Material",
                            "Texture slot '{}': layer {} not found during region resolution",
                            slot_desc.name, layer_idx))?;

                    match region_ref {
                        RegionRef::Index(i) => {
                            if layer.region(i).is_none() {
                                engine_bail!("galaxy3d::Material",
                                    "Texture slot '{}': region index {} does not exist in layer {}",
                                    slot_desc.name, i, layer_idx);
                            }
                            Some(i)
                        }
                        RegionRef::Name(ref name) => {
                            let idx = layer.region_index_by_name(name)
                                .ok_or_else(|| engine_err!("galaxy3d::Material",
                                    "Texture slot '{}': region '{}' not found in layer {}",
                                    slot_desc.name, name, layer_idx))?;
                            Some(idx)
                        }
                    }
                }
            };

            texture_names.insert(slot_desc.name.clone(), vec_index);
            textures.push(MaterialTextureSlot {
                name: slot_desc.name,
                texture: slot_desc.texture,
                layer: resolved_layer,
                region: resolved_region,
            });
        }

        // ========== BUILD PARAMS ==========
        let mut params = Vec::with_capacity(desc.params.len());
        let mut param_names = HashMap::new();

        for (vec_index, (name, value)) in desc.params.into_iter().enumerate() {
            param_names.insert(name.clone(), vec_index);
            params.push((name, value));
        }

        Ok(Self {
            pipeline: desc.pipeline,
            textures,
            texture_names,
            params,
            param_names,
        })
    }

    // ===== PIPELINE ACCESS =====

    /// Get the referenced pipeline
    pub fn pipeline(&self) -> &Arc<Pipeline> {
        &self.pipeline
    }

    // ===== TEXTURE ACCESS =====

    /// Get texture slot by name
    pub fn texture_slot(&self, name: &str) -> Option<&MaterialTextureSlot> {
        let idx = self.texture_names.get(name)?;
        self.textures.get(*idx)
    }

    /// Get texture slot by index
    pub fn texture_slot_at(&self, index: usize) -> Option<&MaterialTextureSlot> {
        self.textures.get(index)
    }

    /// Get number of texture slots
    pub fn texture_slot_count(&self) -> usize {
        self.textures.len()
    }

    // ===== PARAM ACCESS =====

    /// Get parameter value by name
    pub fn param(&self, name: &str) -> Option<&ParamValue> {
        let idx = self.param_names.get(name)?;
        self.params.get(*idx).map(|(_, v)| v)
    }

    /// Get parameter name and value by index
    pub fn param_at(&self, index: usize) -> Option<(&str, &ParamValue)> {
        self.params.get(index).map(|(n, v)| (n.as_str(), v))
    }

    /// Get number of parameters
    pub fn param_count(&self) -> usize {
        self.params.len()
    }
}

// ===== MATERIAL TEXTURE SLOT ACCESSORS =====

impl MaterialTextureSlot {
    /// Get slot name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the texture resource
    pub fn texture(&self) -> &Arc<Texture> {
        &self.texture
    }

    /// Get the resolved layer index (None = whole texture / layer 0)
    pub fn layer(&self) -> Option<u32> {
        self.layer
    }

    /// Get the resolved region index (None = whole layer)
    pub fn region(&self) -> Option<u32> {
        self.region
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
