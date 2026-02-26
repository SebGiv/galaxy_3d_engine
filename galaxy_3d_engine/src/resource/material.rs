/// Resource-level material type.
///
/// A Material describes a surface's visual properties and owns its GPU texture bindings.
/// It references a Pipeline (shader family) and provides textures and parameters.
///
/// At creation time, the Material builds BindingGroups for its textures against every
/// variant/pass of the referenced Pipeline. This avoids duplicating descriptor sets
/// when multiple RenderInstances share the same Material.
///
/// Architecture:
/// - Pipeline reference: which shader family to use (variant selected at render time)
/// - Texture slots: named texture bindings with optional layer/region targeting
/// - Texture bindings: pre-built BindingGroups organized by [variant][pass][set]
/// - Parameters: named scalar/vector/matrix values (roughness, base_color, etc.)

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::resource::texture::Texture;
use crate::resource::pipeline::Pipeline;
use crate::renderer::{SamplerType, BindingGroup, BindingResource, BindingType};

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
    Bool(bool),
    Mat3([[f32; 3]; 3]),
    Mat4([[f32; 4]; 4]),
}

impl ParamValue {
    /// Convert to raw bytes for GPU push constants
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            ParamValue::Float(v) => v.to_ne_bytes().to_vec(),
            ParamValue::Vec2(v) => v.iter().flat_map(|f| f.to_ne_bytes()).collect(),
            ParamValue::Vec3(v) => v.iter().flat_map(|f| f.to_ne_bytes()).collect(),
            ParamValue::Vec4(v) => v.iter().flat_map(|f| f.to_ne_bytes()).collect(),
            ParamValue::Int(v) => v.to_ne_bytes().to_vec(),
            ParamValue::UInt(v) => v.to_ne_bytes().to_vec(),
            ParamValue::Bool(v) => (if *v { 1u32 } else { 0u32 }).to_ne_bytes().to_vec(),
            ParamValue::Mat3(v) => v.iter().flat_map(|row| row.iter().flat_map(|f| f.to_ne_bytes())).collect(),
            ParamValue::Mat4(v) => v.iter().flat_map(|col| col.iter().flat_map(|f| f.to_ne_bytes())).collect(),
        }
    }
}

// ===== MATERIAL PARAMETER =====

/// A named parameter in the material
#[derive(Debug, Clone)]
pub struct MaterialParam {
    name: String,
    value: ParamValue,
}

// ===== TEXTURE BINDING GROUPS =====

/// Texture binding groups for a single rendering pass.
///
/// Contains one BindingGroup per descriptor set (set 1, set 2, ...) that holds
/// texture/sampler bindings. Empty if the pass shader uses no textures.
struct MaterialPassBindings {
    binding_groups: Vec<Arc<dyn BindingGroup>>,
}

/// Texture binding groups for a single pipeline variant.
///
/// Contains one entry per pass in the variant.
struct MaterialVariantBindings {
    passes: Vec<MaterialPassBindings>,
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
    sampler_type: SamplerType,
}

// ===== MATERIAL =====

/// Material resource: visual description of a surface
///
/// Pure data — no GPU resources. References a Pipeline and provides
/// textures (with optional layer/region targeting) and named parameters.
pub struct Material {
    slot_id: u32,
    pipeline: Arc<Pipeline>,
    textures: Vec<MaterialTextureSlot>,
    texture_names: HashMap<String, usize>,
    params: Vec<MaterialParam>,
    param_names: HashMap<String, usize>,
    /// Pre-built texture BindingGroups organized by [variant][pass][set].
    /// Built at creation time from pipeline reflection data.
    texture_bindings: Vec<MaterialVariantBindings>,
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
    pub sampler_type: SamplerType,
}

// ===== MATERIAL IMPLEMENTATION =====

impl Material {
    /// Create material from descriptor (internal use by ResourceManager)
    pub(crate) fn from_desc(slot_id: u32, desc: MaterialDesc) -> Result<Self> {

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
                sampler_type: slot_desc.sampler_type,
            });
        }

        // ========== BUILD PARAMS ==========
        let mut params = Vec::with_capacity(desc.params.len());
        let mut param_names = HashMap::new();

        for (vec_index, (name, value)) in desc.params.into_iter().enumerate() {
            param_names.insert(name.clone(), vec_index);
            params.push(MaterialParam { name, value });
        }

        // ========== BUILD TEXTURE BINDING GROUPS ==========
        // For each variant/pass, match texture slot names against shader reflection
        // to create pre-built BindingGroups (descriptor sets for textures).
        let renderer_lock = desc.pipeline.renderer().lock().unwrap();
        let mut texture_bindings = Vec::with_capacity(desc.pipeline.variant_count());

        for variant_idx in 0..desc.pipeline.variant_count() {
            let variant = desc.pipeline.variant(variant_idx as u32).unwrap();
            let mut pass_bindings = Vec::with_capacity(variant.pass_count());

            for pass_idx in 0..variant.pass_count() {
                let pass = variant.pass(pass_idx as u32).unwrap();
                let renderer_pipeline = pass.renderer_pipeline();
                let reflection = renderer_pipeline.reflection();

                // Group CombinedImageSampler bindings by set index
                let mut sets: BTreeMap<u32, Vec<(u32, BindingResource)>> = BTreeMap::new();

                for binding_idx in 0..reflection.binding_count() {
                    let binding = reflection.binding(binding_idx).unwrap();

                    if binding.binding_type == BindingType::CombinedImageSampler {
                        if let Some(&tex_idx) = texture_names.get(&binding.name) {
                            let slot = &textures[tex_idx];
                            let renderer_texture = slot.texture().renderer_texture();
                            sets.entry(binding.set)
                                .or_default()
                                .push((binding.binding, BindingResource::SampledTexture(
                                    renderer_texture.as_ref(),
                                    slot.sampler_type(),
                                )));
                        }
                    }
                }

                // Create one BindingGroup per set
                let mut binding_groups = Vec::new();
                for (set_index, mut resources) in sets {
                    resources.sort_by_key(|(binding, _)| *binding);
                    let resource_refs: Vec<BindingResource> = resources.into_iter()
                        .map(|(_, r)| r)
                        .collect();
                    let bg = renderer_lock.create_binding_group(
                        renderer_pipeline,
                        set_index,
                        &resource_refs,
                    )?;
                    binding_groups.push(bg);
                }

                pass_bindings.push(MaterialPassBindings { binding_groups });
            }

            texture_bindings.push(MaterialVariantBindings { passes: pass_bindings });
        }
        drop(renderer_lock);

        Ok(Self {
            slot_id,
            pipeline: desc.pipeline,
            textures,
            texture_names,
            params,
            param_names,
            texture_bindings,
        })
    }

    // ===== SLOT ID =====

    /// Get the unique material slot ID (for GPU buffer indexing)
    pub fn slot_id(&self) -> u32 {
        self.slot_id
    }

    // ===== PIPELINE ACCESS =====

    /// Get the referenced pipeline
    pub fn pipeline(&self) -> &Arc<Pipeline> {
        &self.pipeline
    }

    // ===== TEXTURE SLOT ACCESS =====

    /// Get texture slot by index
    pub fn texture_slot(&self, index: usize) -> Option<&MaterialTextureSlot> {
        self.textures.get(index)
    }

    /// Get texture slot by name
    pub fn texture_slot_by_name(&self, name: &str) -> Option<&MaterialTextureSlot> {
        let idx = self.texture_names.get(name)?;
        self.textures.get(*idx)
    }

    /// Get texture slot index from name
    pub fn texture_slot_id(&self, name: &str) -> Option<usize> {
        self.texture_names.get(name).copied()
    }

    /// Get all texture slots as a slice
    pub fn texture_slots(&self) -> &[MaterialTextureSlot] {
        &self.textures
    }

    /// Get number of texture slots
    pub fn texture_slot_count(&self) -> usize {
        self.textures.len()
    }

    // ===== TEXTURE BINDING GROUP ACCESS =====

    /// Get pre-built texture BindingGroups for a specific variant and pass.
    ///
    /// Returns a slice of BindingGroups (one per descriptor set: set 1, set 2, ...).
    /// Returns an empty slice if the pass has no texture bindings.
    /// Returns None if variant or pass index is out of range.
    pub fn texture_binding_groups(&self, variant: u32, pass: u32) -> Option<&[Arc<dyn BindingGroup>]> {
        let variant_bindings = self.texture_bindings.get(variant as usize)?;
        let pass_bindings = variant_bindings.passes.get(pass as usize)?;
        Some(&pass_bindings.binding_groups)
    }

    // ===== PARAM ACCESS =====

    /// Get parameter by index
    pub fn param(&self, index: usize) -> Option<&MaterialParam> {
        self.params.get(index)
    }

    /// Get parameter by name
    pub fn param_by_name(&self, name: &str) -> Option<&MaterialParam> {
        let idx = self.param_names.get(name)?;
        self.params.get(*idx)
    }

    /// Get parameter index from name
    pub fn param_id(&self, name: &str) -> Option<usize> {
        self.param_names.get(name).copied()
    }

    /// Get all parameters as a slice
    pub fn params(&self) -> &[MaterialParam] {
        &self.params
    }

    /// Get number of parameters
    pub fn param_count(&self) -> usize {
        self.params.len()
    }
}

// ===== MATERIAL PARAM ACCESSORS =====

impl MaterialParam {
    /// Get parameter name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get parameter value
    pub fn value(&self) -> &ParamValue {
        &self.value
    }

    /// Get as float (returns None if not a Float)
    pub fn as_float(&self) -> Option<f32> {
        match self.value {
            ParamValue::Float(v) => Some(v),
            _ => None,
        }
    }

    /// Get as vec2 (returns None if not a Vec2)
    pub fn as_vec2(&self) -> Option<[f32; 2]> {
        match self.value {
            ParamValue::Vec2(v) => Some(v),
            _ => None,
        }
    }

    /// Get as vec3 (returns None if not a Vec3)
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self.value {
            ParamValue::Vec3(v) => Some(v),
            _ => None,
        }
    }

    /// Get as vec4 (returns None if not a Vec4)
    pub fn as_vec4(&self) -> Option<[f32; 4]> {
        match self.value {
            ParamValue::Vec4(v) => Some(v),
            _ => None,
        }
    }

    /// Get as int (returns None if not an Int)
    pub fn as_int(&self) -> Option<i32> {
        match self.value {
            ParamValue::Int(v) => Some(v),
            _ => None,
        }
    }

    /// Get as uint (returns None if not a UInt)
    pub fn as_uint(&self) -> Option<u32> {
        match self.value {
            ParamValue::UInt(v) => Some(v),
            _ => None,
        }
    }

    /// Get as bool (returns None if not a Bool)
    pub fn as_bool(&self) -> Option<bool> {
        match self.value {
            ParamValue::Bool(v) => Some(v),
            _ => None,
        }
    }

    /// Get as mat3 (returns None if not a Mat3)
    pub fn as_mat3(&self) -> Option<[[f32; 3]; 3]> {
        match self.value {
            ParamValue::Mat3(v) => Some(v),
            _ => None,
        }
    }

    /// Get as mat4 (returns None if not a Mat4)
    pub fn as_mat4(&self) -> Option<[[f32; 4]; 4]> {
        match self.value {
            ParamValue::Mat4(v) => Some(v),
            _ => None,
        }
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

    /// Get the sampler type for this texture slot
    pub fn sampler_type(&self) -> SamplerType {
        self.sampler_type
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "material_tests.rs"]
mod tests;
