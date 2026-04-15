/// Resource-level material type.
///
/// A Material describes a surface's visual properties as a list of MaterialPass.
/// Each pass has its own pipeline state (fragment shader, blend, polygon mode,
/// dynamic render state) and its own subset of texture slots and parameters.
///
/// Cross-pass uniqueness:
/// - `pass_type` must be unique across the passes of a Material.
/// - Texture slot names must be globally unique across all passes of a Material
///   (a given name appears in exactly one pass).
/// - Parameter names must be globally unique across all passes of a Material.
///
/// The same TextureKey may be referenced by two slots with different names in
/// different passes — uniqueness applies to slot NAMES, not to underlying
/// TextureKeys.
///
/// At creation time, the Material resolves keys via the ResourceManager and
/// resolves layer/region references for each texture slot.

use rustc_hash::{FxHashMap, FxHashSet};
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::resource::resource_manager::{ResourceManager, ShaderKey, TextureKey};
use crate::graphics_device::{self, SamplerType, ColorBlendState, PolygonMode, DynamicRenderState};

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

// ===== MATERIAL TEXTURE SLOT =====

/// A texture bound to a named slot in a material pass (resolved indices)
///
/// After creation, layer and region references are resolved to u32 indices.
/// The texture is stored as a key, resolved via the ResourceManager.
pub struct MaterialTextureSlot {
    name: String,
    texture: TextureKey,
    bindless_index: u32,
    sampler_index: u32,
    layer: Option<u32>,
    region: Option<u32>,
    sampler_type: SamplerType,
}

// ===== MATERIAL PASS =====

/// A single rendering pass within a Material.
///
/// Each pass has its own pipeline state and its own subset of texture slots
/// and parameters. Texture slot names and parameter names are globally unique
/// across all passes of a Material — a given name appears in exactly one pass.
pub struct MaterialPass {
    pass_type: u8,
    fragment_shader: ShaderKey,
    color_blend: ColorBlendState,
    polygon_mode: PolygonMode,
    render_state: DynamicRenderState,
    /// Stable u16 id identifying this pass's `DynamicRenderState`. Assigned by
    /// `ResourceManager::get_or_assign_material_render_state_signature_id()`
    /// after the Material is built. Two passes with identical render states
    /// share the same id — used by the drawer to skip redundant
    /// `set_dynamic_state` emissions.
    render_state_signature_id: u16,
    textures: Vec<MaterialTextureSlot>,
    texture_names: FxHashMap<String, usize>,
    params: Vec<MaterialParam>,
    param_names: FxHashMap<String, usize>,
}

// ===== MATERIAL =====

/// Material resource: visual description of a surface as a list of passes.
///
/// A Material owns a `Vec<MaterialPass>`. Each pass has its own pipeline state
/// and its own subset of textures/params, with globally unique slot/param names
/// across all passes of the Material.
pub struct Material {
    slot_id: u32,
    passes: Vec<MaterialPass>,
    /// Generation counter for pipeline cache invalidation
    generation: u64,
}

// ===== DESCRIPTORS =====

/// Material pass creation descriptor
pub struct MaterialPassDesc {
    pub pass_type: u8,
    pub fragment_shader: ShaderKey,
    pub color_blend: ColorBlendState,
    pub polygon_mode: PolygonMode,
    pub textures: Vec<MaterialTextureSlotDesc>,
    pub params: Vec<(String, ParamValue)>,
    /// Dynamic render state for this pass.
    /// If None, uses DynamicRenderState::default().
    pub render_state: Option<DynamicRenderState>,
}

/// Material creation descriptor
pub struct MaterialDesc {
    pub passes: Vec<MaterialPassDesc>,
}

/// Texture slot descriptor (user-facing, accepts names or indices)
pub struct MaterialTextureSlotDesc {
    pub name: String,
    pub texture: TextureKey,
    pub layer: Option<LayerRef>,
    pub region: Option<RegionRef>,
    pub sampler_type: SamplerType,
}

// ===== MATERIAL IMPLEMENTATION =====

impl Material {
    /// Create material from descriptor (internal use by ResourceManager)
    ///
    /// Resolves TextureKeys via the ResourceManager, then stores only keys.
    /// Validates pass_type uniqueness and global uniqueness of texture/param
    /// slot names across all passes.
    pub(crate) fn from_desc(
        slot_id: u32,
        desc: MaterialDesc,
        resource_manager: &ResourceManager,
        _graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<Self> {

        // ========== VALIDATION 1: At least one pass ==========
        if desc.passes.is_empty() {
            engine_bail!("galaxy3d::Material",
                "Material must have at least one pass");
        }

        // ========== VALIDATION 2: pass_type must be < 64 (bitmask + [u8; 64]) ==========
        for pass_desc in &desc.passes {
            if pass_desc.pass_type >= 64 {
                engine_bail!("galaxy3d::Material",
                    "pass_type {} exceeds maximum (63)", pass_desc.pass_type);
            }
        }

        // ========== VALIDATION 3: Unique pass_type across passes ==========
        let mut seen_pass_types: FxHashSet<u8> = FxHashSet::default();
        for pass_desc in &desc.passes {
            if !seen_pass_types.insert(pass_desc.pass_type) {
                engine_bail!("galaxy3d::Material",
                    "Duplicate pass_type {} in Material passes", pass_desc.pass_type);
            }
        }

        // ========== VALIDATION 4: Globally unique texture/param names across passes ==========
        let mut global_texture_names: FxHashSet<String> = FxHashSet::default();
        let mut global_param_names: FxHashSet<String> = FxHashSet::default();

        let mut passes: Vec<MaterialPass> = Vec::with_capacity(desc.passes.len());

        for pass_desc in desc.passes.into_iter() {
            // ===== Local validation: no duplicate names within this pass =====
            let mut local_texture_names: FxHashSet<&str> = FxHashSet::default();
            for slot_desc in &pass_desc.textures {
                if !local_texture_names.insert(slot_desc.name.as_str()) {
                    engine_bail!("galaxy3d::Material",
                        "Duplicate texture slot name '{}' within pass_type {}",
                        slot_desc.name, pass_desc.pass_type);
                }
            }
            let mut local_param_names: FxHashSet<&str> = FxHashSet::default();
            for (param_name, _) in &pass_desc.params {
                if !local_param_names.insert(param_name.as_str()) {
                    engine_bail!("galaxy3d::Material",
                        "Duplicate parameter name '{}' within pass_type {}",
                        param_name, pass_desc.pass_type);
                }
            }

            // ===== Global validation: no name shared across passes =====
            for slot_desc in &pass_desc.textures {
                if !global_texture_names.insert(slot_desc.name.clone()) {
                    engine_bail!("galaxy3d::Material",
                        "Texture slot name '{}' is used in more than one pass \
                         (slot names must be globally unique across all passes \
                         of a Material)",
                        slot_desc.name);
                }
            }
            for (param_name, _) in &pass_desc.params {
                if !global_param_names.insert(param_name.clone()) {
                    engine_bail!("galaxy3d::Material",
                        "Parameter name '{}' is used in more than one pass \
                         (param names must be globally unique across all passes \
                         of a Material)",
                        param_name);
                }
            }

            // ===== Resolve texture slots for this pass =====
            let pass_type = pass_desc.pass_type;
            let mut textures = Vec::with_capacity(pass_desc.textures.len());
            let mut texture_names = FxHashMap::default();

            for (vec_index, slot_desc) in pass_desc.textures.into_iter().enumerate() {
                // Resolve texture key
                let texture_arc = resource_manager.texture(slot_desc.texture)
                    .ok_or_else(|| engine_err!("galaxy3d::Material",
                        "Texture slot '{}' (pass_type {}): texture key not found in ResourceManager",
                        slot_desc.name, pass_type))?;

                // Resolve layer reference
                let resolved_layer = match slot_desc.layer {
                    None => None,
                    Some(LayerRef::Index(i)) => {
                        if texture_arc.layer(i).is_none() {
                            engine_bail!("galaxy3d::Material",
                                "Texture slot '{}' (pass_type {}): layer index {} does not exist",
                                slot_desc.name, pass_type, i);
                        }
                        Some(i)
                    }
                    Some(LayerRef::Name(ref name)) => {
                        let idx = texture_arc.layer_index_by_name(name)
                            .ok_or_else(|| engine_err!("galaxy3d::Material",
                                "Texture slot '{}' (pass_type {}): layer '{}' not found",
                                slot_desc.name, pass_type, name))?;
                        Some(idx)
                    }
                };

                // Resolve region reference (requires a resolved layer)
                let resolved_region = match slot_desc.region {
                    None => None,
                    Some(region_ref) => {
                        let layer_idx = resolved_layer
                            .ok_or_else(|| engine_err!("galaxy3d::Material",
                                "Texture slot '{}' (pass_type {}): region specified without a layer",
                                slot_desc.name, pass_type))?;

                        let layer = texture_arc.layer(layer_idx)
                            .ok_or_else(|| engine_err!("galaxy3d::Material",
                                "Texture slot '{}' (pass_type {}): layer {} not found during region resolution",
                                slot_desc.name, pass_type, layer_idx))?;

                        match region_ref {
                            RegionRef::Index(i) => {
                                if layer.region(i).is_none() {
                                    engine_bail!("galaxy3d::Material",
                                        "Texture slot '{}' (pass_type {}): region index {} does not exist in layer {}",
                                        slot_desc.name, pass_type, i, layer_idx);
                                }
                                Some(i)
                            }
                            RegionRef::Name(ref name) => {
                                let idx = layer.region_index_by_name(name)
                                    .ok_or_else(|| engine_err!("galaxy3d::Material",
                                        "Texture slot '{}' (pass_type {}): region '{}' not found in layer {}",
                                        slot_desc.name, pass_type, name, layer_idx))?;
                                Some(idx)
                            }
                        }
                    }
                };

                // Read bindless index from the GPU texture
                let gd_texture = texture_arc.graphics_device_texture();
                let bindless_index = gd_texture.bindless_index();
                let sampler_index = slot_desc.sampler_type as u32;

                texture_names.insert(slot_desc.name.clone(), vec_index);
                textures.push(MaterialTextureSlot {
                    name: slot_desc.name,
                    texture: slot_desc.texture,
                    bindless_index,
                    sampler_index,
                    layer: resolved_layer,
                    region: resolved_region,
                    sampler_type: slot_desc.sampler_type,
                });
            }

            // ===== Build params for this pass =====
            let mut params = Vec::with_capacity(pass_desc.params.len());
            let mut param_names = FxHashMap::default();

            for (vec_index, (name, value)) in pass_desc.params.into_iter().enumerate() {
                param_names.insert(name.clone(), vec_index);
                params.push(MaterialParam { name, value });
            }

            let render_state = pass_desc.render_state.unwrap_or_default();

            passes.push(MaterialPass {
                pass_type,
                fragment_shader: pass_desc.fragment_shader,
                color_blend: pass_desc.color_blend,
                polygon_mode: pass_desc.polygon_mode,
                render_state,
                // Assigned after `from_desc` by ResourceManager::create_material().
                render_state_signature_id: 0,
                textures,
                texture_names,
                params,
                param_names,
            });
        }

        Ok(Self {
            slot_id,
            passes,
            generation: 0,
        })
    }

    // ===== SLOT ID =====

    /// Get the unique material slot ID (for GPU buffer indexing)
    pub fn slot_id(&self) -> u32 {
        self.slot_id
    }

    /// Get the generation counter (for pipeline cache invalidation)
    pub fn generation(&self) -> u64 {
        self.generation
    }

    // ===== PASS ACCESS =====

    /// Get the number of passes in this material
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Get a pass by index (primary access, hot path)
    pub fn pass(&self, index: usize) -> Option<&MaterialPass> {
        self.passes.get(index)
    }

    /// Get a pass by its pass_type value
    pub fn pass_by_type(&self, pass_type: u8) -> Option<&MaterialPass> {
        self.passes.iter().find(|p| p.pass_type == pass_type)
    }

    /// Get all passes as a slice
    pub fn passes(&self) -> &[MaterialPass] {
        &self.passes
    }

    /// Get all passes as a mutable slice (crate-internal, used by
    /// `ResourceManager::create_material()` to assign render-state signature ids).
    pub(crate) fn passes_mut(&mut self) -> &mut [MaterialPass] {
        &mut self.passes
    }

    // ===== GLOBAL ITERATION (for SSBO upload) =====

    /// Iterate over ALL parameters across all passes (for SSBO upload).
    ///
    /// Order is: pass 0 params, then pass 1 params, etc. Since param names are
    /// globally unique across passes, this iterator yields the union without
    /// duplicates.
    pub fn iter_all_params(&self) -> impl Iterator<Item = &MaterialParam> {
        self.passes.iter().flat_map(|p| p.params.iter())
    }

    /// Iterate over ALL texture slots across all passes (for SSBO upload).
    ///
    /// Order is: pass 0 slots, then pass 1 slots, etc. Since slot names are
    /// globally unique across passes, this iterator yields the union without
    /// duplicates.
    pub fn iter_all_texture_slots(&self) -> impl Iterator<Item = &MaterialTextureSlot> {
        self.passes.iter().flat_map(|p| p.textures.iter())
    }

    /// Total number of parameters across all passes (= union since names are
    /// globally unique).
    pub fn total_param_count(&self) -> usize {
        self.passes.iter().map(|p| p.params.len()).sum()
    }

    /// Total number of texture slots across all passes (= union since names are
    /// globally unique).
    pub fn total_texture_slot_count(&self) -> usize {
        self.passes.iter().map(|p| p.textures.len()).sum()
    }
}

// ===== MATERIAL PASS ACCESSORS =====

impl MaterialPass {
    /// Get the pass type identifier
    pub fn pass_type(&self) -> u8 {
        self.pass_type
    }

    /// Get the fragment shader key
    pub fn fragment_shader(&self) -> ShaderKey {
        self.fragment_shader
    }

    /// Get the color blend state
    pub fn color_blend(&self) -> &ColorBlendState {
        &self.color_blend
    }

    /// Get the polygon mode
    pub fn polygon_mode(&self) -> PolygonMode {
        self.polygon_mode
    }

    /// Get the DynamicRenderState for this pass.
    pub fn render_state(&self) -> &DynamicRenderState {
        &self.render_state
    }

    /// Get the stable u16 id identifying this pass's render state.
    ///
    /// Two passes with identical `DynamicRenderState` values share the same id.
    /// Used by the drawer to skip redundant `set_dynamic_state` emissions.
    pub fn render_state_signature_id(&self) -> u16 {
        self.render_state_signature_id
    }

    /// Set the render state signature id. Called by `ResourceManager::create_material()`
    /// after the Material is built.
    pub(crate) fn set_render_state_signature_id(&mut self, id: u16) {
        self.render_state_signature_id = id;
    }

    // ===== TEXTURE SLOT ACCESS =====

    /// Get texture slot by index within this pass
    pub fn texture_slot(&self, index: usize) -> Option<&MaterialTextureSlot> {
        self.textures.get(index)
    }

    /// Get texture slot by name within this pass
    pub fn texture_slot_by_name(&self, name: &str) -> Option<&MaterialTextureSlot> {
        let idx = self.texture_names.get(name)?;
        self.textures.get(*idx)
    }

    /// Get texture slot index from name within this pass
    pub fn texture_slot_id(&self, name: &str) -> Option<usize> {
        self.texture_names.get(name).copied()
    }

    /// Get all texture slots of this pass as a slice
    pub fn texture_slots(&self) -> &[MaterialTextureSlot] {
        &self.textures
    }

    /// Get number of texture slots in this pass
    pub fn texture_slot_count(&self) -> usize {
        self.textures.len()
    }

    // ===== PARAM ACCESS =====

    /// Get parameter by index within this pass
    pub fn param(&self, index: usize) -> Option<&MaterialParam> {
        self.params.get(index)
    }

    /// Get parameter by name within this pass
    pub fn param_by_name(&self, name: &str) -> Option<&MaterialParam> {
        let idx = self.param_names.get(name)?;
        self.params.get(*idx)
    }

    /// Get parameter index from name within this pass
    pub fn param_id(&self, name: &str) -> Option<usize> {
        self.param_names.get(name).copied()
    }

    /// Get all parameters of this pass as a slice
    pub fn params(&self) -> &[MaterialParam] {
        &self.params
    }

    /// Get number of parameters in this pass
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

    /// Get the texture key
    pub fn texture(&self) -> TextureKey {
        self.texture
    }

    /// Get the bindless index for this texture in its type-specific bindless table
    pub fn bindless_index(&self) -> u32 {
        self.bindless_index
    }

    /// Get the sampler index in the bindless sampler table (= SamplerType as u32)
    pub fn sampler_index(&self) -> u32 {
        self.sampler_index
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
