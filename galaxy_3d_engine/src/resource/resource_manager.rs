////! Central resource manager for the engine.
//!
//! Stores and provides access to all engine resources (textures, geometries, etc.).
//! Uses SlotMaps for O(1) key-based access with stable keys, plus name-based lookup.

use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::graphics_device;
use crate::resource::texture::{
    Texture,
    TextureDesc, LayerDesc, AtlasRegionDesc,
};
use crate::resource::geometry::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::shader::Shader;
use crate::resource::pipeline::{
    Pipeline, PipelineDesc,
};
use crate::resource::material::{
    Material, MaterialDesc,
};
use crate::resource::mesh::{
    Mesh, MeshDesc,
};
use crate::resource::buffer::{
    Buffer, BufferDesc, BufferKind, FieldDesc, FieldType,
};
use crate::resource::material::ParamValue;
use crate::utils::SlotAllocator;

// ===== RESOURCE KEYS =====

slotmap::new_key_type! {
    /// Stable key for a Texture in the ResourceManager.
    pub struct TextureKey;
    /// Stable key for a Geometry in the ResourceManager.
    pub struct GeometryKey;
    /// Stable key for a Shader in the ResourceManager.
    pub struct ShaderKey;
    /// Stable key for a Pipeline in the ResourceManager.
    pub struct PipelineKey;
    /// Stable key for a Material in the ResourceManager.
    pub struct MaterialKey;
    /// Stable key for a Mesh in the ResourceManager.
    pub struct MeshKey;
    /// Stable key for a Buffer in the ResourceManager.
    pub struct BufferKey;
}

// ===== PIPELINE CACHE KEY =====

/// Composite key for the pipeline cache.
///
/// Contains all parameters that uniquely identify a Vulkan pipeline.
/// Used as HashMap key — Rust's HashMap computes the hash internally
/// and uses Eq for collision resolution.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct PipelineCacheKey {
    pub vertex_shader: ShaderKey,
    pub fragment_shader: ShaderKey,
    pub vertex_layout: graphics_device::VertexLayout,
    pub topology: graphics_device::PrimitiveTopology,
    pub color_blend: graphics_device::ColorBlendState,
    pub polygon_mode: graphics_device::PolygonMode,
    pub color_formats: Vec<graphics_device::TextureFormat>,
    pub depth_format: Option<graphics_device::TextureFormat>,
    pub sample_count: graphics_device::SampleCount,
}

/// Render pass attachment information needed for pipeline creation.
///
/// Derived automatically by `RenderGraph::compile()` from the resolved
/// render pass attachments. Carries a generation counter that is incremented
/// only when the actual formats or sample count change.
///
/// Also accepted as a manual construction (e.g. in tests or demos)
/// via `PassInfo::new()`.
#[derive(Debug, Clone, PartialEq)]
pub struct PassInfo {
    pub color_formats: Vec<graphics_device::TextureFormat>,
    pub depth_format: Option<graphics_device::TextureFormat>,
    pub sample_count: graphics_device::SampleCount,
    /// Generation counter — incremented when any format field changes.
    /// Used by RenderInstance pipeline cache for invalidation.
    generation: u64,
}

impl PassInfo {
    /// Create a new PassInfo with generation 1.
    pub fn new(
        color_formats: Vec<graphics_device::TextureFormat>,
        depth_format: Option<graphics_device::TextureFormat>,
        sample_count: graphics_device::SampleCount,
    ) -> Self {
        Self { color_formats, depth_format, sample_count, generation: 1 }
    }

    /// Get the generation counter.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Increment the generation counter (crate-internal, used by RenderPass
    /// when attachment formats change).
    pub(crate) fn increment_generation(&mut self) {
        self.generation += 1;
    }
}

// ===== PRIVATE HELPERS =====

/// Map a ParamValue to its compatible FieldType.
/// Bool maps to UInt (GLSL convention: bools are u32 in GPU buffers).
fn compatible_field_type(value: &ParamValue) -> FieldType {
    match value {
        ParamValue::Float(_) => FieldType::Float,
        ParamValue::Vec2(_)  => FieldType::Vec2,
        ParamValue::Vec3(_)  => FieldType::Vec3,
        ParamValue::Vec4(_)  => FieldType::Vec4,
        ParamValue::Int(_)   => FieldType::Int,
        ParamValue::UInt(_)  => FieldType::UInt,
        ParamValue::Bool(_)  => FieldType::UInt,
        ParamValue::Mat3(_)  => FieldType::Mat3,
        ParamValue::Mat4(_)  => FieldType::Mat4,
    }
}

/// Convert a ParamValue to padded bytes matching FieldType::size_bytes().
///
/// FieldType::size_bytes() is the same for UBO (std140) and SSBO (std430),
/// so a single padding function covers both buffer kinds.
///
/// Vec3: 12 → 16 bytes (4 bytes zero-padding)
/// Mat3: 36 → 48 bytes (each row padded from 12 to 16 bytes)
/// All others: identical to ParamValue::as_bytes() (already correct size)
fn param_to_padded_bytes(value: &ParamValue) -> Vec<u8> {
    match value {
        ParamValue::Vec3(v) => {
            let mut bytes = Vec::with_capacity(16);
            for f in v { bytes.extend_from_slice(&f.to_ne_bytes()); }
            bytes.extend_from_slice(&[0u8; 4]);
            bytes
        }
        ParamValue::Mat3(m) => {
            let mut bytes = Vec::with_capacity(48);
            for row in m {
                for f in row { bytes.extend_from_slice(&f.to_ne_bytes()); }
                bytes.extend_from_slice(&[0u8; 4]);
            }
            bytes
        }
        _ => value.as_bytes(),
    }
}

pub struct ResourceManager {
    textures: SlotMap<TextureKey, Arc<Texture>>,
    geometries: SlotMap<GeometryKey, Arc<Geometry>>,
    shaders: SlotMap<ShaderKey, Arc<Shader>>,
    pipelines: SlotMap<PipelineKey, Arc<Pipeline>>,
    materials: SlotMap<MaterialKey, Arc<Material>>,
    meshes: SlotMap<MeshKey, Arc<Mesh>>,
    buffers: SlotMap<BufferKey, Arc<Buffer>>,

    texture_names: FxHashMap<String, TextureKey>,
    geometry_names: FxHashMap<String, GeometryKey>,
    shader_names: FxHashMap<String, ShaderKey>,
    pipeline_names: FxHashMap<String, PipelineKey>,
    material_names: FxHashMap<String, MaterialKey>,
    mesh_names: FxHashMap<String, MeshKey>,
    buffer_names: FxHashMap<String, BufferKey>,

    material_slot_allocator: SlotAllocator,

    /// Pipeline cache: maps composite pipeline parameters to an existing PipelineKey.
    /// Pipelines created by the cache are stored in the same `pipelines` SlotMap
    /// as manually created pipelines, with an auto-generated name.
    pipeline_cache: HashMap<PipelineCacheKey, PipelineKey>,
}

impl ResourceManager {
    /// Create a new empty resource manager
    pub(crate) fn new() -> Self {
        Self {
            textures: SlotMap::with_key(),
            geometries: SlotMap::with_key(),
            shaders: SlotMap::with_key(),
            pipelines: SlotMap::with_key(),
            materials: SlotMap::with_key(),
            meshes: SlotMap::with_key(),
            buffers: SlotMap::with_key(),

            texture_names: FxHashMap::default(),
            geometry_names: FxHashMap::default(),
            shader_names: FxHashMap::default(),
            pipeline_names: FxHashMap::default(),
            material_names: FxHashMap::default(),
            mesh_names: FxHashMap::default(),
            buffer_names: FxHashMap::default(),

            material_slot_allocator: SlotAllocator::new(),

            pipeline_cache: HashMap::new(),
        }
    }

    // ===== TEXTURE CREATION =====

    /// Create a texture (simple or indexed, with optional atlas regions per layer)
    ///
    /// Internally creates the GPU texture and descriptor set via the graphics_device.
    /// Returns the TextureKey for future access.
    pub fn create_texture(&mut self, name: String, desc: TextureDesc) -> Result<TextureKey> {
        if self.texture_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Texture '{}' already exists", name);
        }

        let texture = Texture::from_desc(desc)?;
        let is_simple = texture.is_simple();
        let layer_count = texture.layer_count();

        let key = self.textures.insert(Arc::new(texture));
        self.texture_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created {} texture '{}' ({} layer{})",
            if is_simple { "Simple" } else { "Indexed" },
            name, layer_count, if layer_count > 1 { "s" } else { "" });

        Ok(key)
    }

    // ===== TEXTURE ACCESS =====

    /// Get a texture by key
    pub fn texture(&self, key: TextureKey) -> Option<&Arc<Texture>> {
        self.textures.get(key)
    }

    /// Get a texture by name
    pub fn texture_by_name(&self, name: &str) -> Option<&Arc<Texture>> {
        let key = self.texture_names.get(name)?;
        self.textures.get(*key)
    }

    /// Get texture key by name
    pub fn texture_key(&self, name: &str) -> Option<TextureKey> {
        self.texture_names.get(name).copied()
    }

    /// Remove a texture by key
    pub fn remove_texture(&mut self, key: TextureKey) -> bool {
        if let Some(_) = self.textures.remove(key) {
            self.texture_names.retain(|_, v| *v != key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Texture resource");
            true
        } else {
            false
        }
    }

    /// Remove a texture by name
    pub fn remove_texture_by_name(&mut self, name: &str) -> bool {
        if let Some(key) = self.texture_names.remove(name) {
            self.textures.remove(key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Texture resource '{}'", name);
            true
        } else {
            false
        }
    }

    /// Get the number of registered textures
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    // ===== TEXTURE MODIFICATION =====

    /// Add a layer to an existing indexed texture
    pub fn add_texture_layer(
        &mut self,
        texture_key: TextureKey,
        desc: LayerDesc,
    ) -> Result<u32> {
        let arc = self.textures.get_mut(texture_key)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Texture not found"))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate texture: other references exist"))?;

        texture.add_layer(desc)
    }

    /// Add a region to an existing texture layer
    pub fn add_texture_region(
        &mut self,
        texture_key: TextureKey,
        layer_name: &str,
        desc: AtlasRegionDesc,
    ) -> Result<u32> {
        let arc = self.textures.get_mut(texture_key)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Texture not found"))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate texture: other references exist"))?;

        texture.add_region(layer_name, desc)
    }

    // ===== GEOMETRY CREATION =====

    /// Create a geometry resource and register it
    pub fn create_geometry(&mut self, name: String, desc: GeometryDesc) -> Result<GeometryKey> {
        if self.geometry_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Geometry '{}' already exists", name);
        }

        let geometry = Geometry::from_desc(desc)?;
        let mesh_count = geometry.mesh_count();
        let total_vertex_count = geometry.total_vertex_count();
        let total_index_count = geometry.total_index_count();

        let key = self.geometries.insert(Arc::new(geometry));
        self.geometry_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Geometry resource '{}' ({} vertices, {} indices, {} meshes)",
            name, total_vertex_count, total_index_count, mesh_count);

        Ok(key)
    }

    // ===== GEOMETRY ACCESS =====

    /// Get a geometry by key
    pub fn geometry(&self, key: GeometryKey) -> Option<&Arc<Geometry>> {
        self.geometries.get(key)
    }

    /// Get a geometry by name
    pub fn geometry_by_name(&self, name: &str) -> Option<&Arc<Geometry>> {
        let key = self.geometry_names.get(name)?;
        self.geometries.get(*key)
    }

    /// Get geometry key by name
    pub fn geometry_key(&self, name: &str) -> Option<GeometryKey> {
        self.geometry_names.get(name).copied()
    }

    /// Remove a geometry by name
    pub fn remove_geometry(&mut self, name: &str) -> bool {
        if let Some(key) = self.geometry_names.remove(name) {
            self.geometries.remove(key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Geometry resource '{}'", name);
            true
        } else {
            false
        }
    }

    /// Get the number of registered geometries
    pub fn geometry_count(&self) -> usize {
        self.geometries.len()
    }

    // ===== GEOMETRY MODIFICATION =====

    /// Add a mesh to an existing geometry resource
    pub fn add_geometry_mesh(&mut self, geom_key: GeometryKey, desc: GeometryMeshDesc) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_key)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry not found"))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry: other references exist"))?;

        geometry.add_mesh(desc)
    }

    /// Add a LOD to an existing mesh
    pub fn add_geometry_lod(
        &mut self,
        geom_key: GeometryKey,
        mesh_id: usize,
        desc: GeometryLODDesc,
    ) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_key)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry not found"))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry: other references exist"))?;

        geometry.add_lod(mesh_id, desc)
    }

    /// Add a submesh to an existing LOD
    pub fn add_geometry_submesh(
        &mut self,
        geom_key: GeometryKey,
        mesh_id: usize,
        lod_index: usize,
        desc: GeometrySubMeshDesc,
    ) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_key)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry not found"))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry: other references exist"))?;

        geometry.add_submesh(mesh_id, lod_index, desc)
    }

    // ===== SHADER CREATION =====

    /// Create a shader resource
    pub fn create_shader(
        &mut self,
        name: String,
        desc: crate::resource::shader::ShaderDesc,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<ShaderKey> {
        if self.shader_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Shader '{}' already exists", name);
        }

        let gd_shader = graphics_device.create_shader(graphics_device::ShaderDesc {
            code: desc.code,
            stage: desc.stage,
            entry_point: desc.entry_point,
        })?;
        let shader = Shader::from_gpu_shader(gd_shader, desc.stage);

        let key = self.shaders.insert(Arc::new(shader));
        self.shader_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Shader resource '{}'", name);

        Ok(key)
    }

    // ===== SHADER ACCESS =====

    /// Get a shader by key
    pub fn shader(&self, key: ShaderKey) -> Option<&Arc<Shader>> {
        self.shaders.get(key)
    }

    /// Get a shader by name
    pub fn shader_by_name(&self, name: &str) -> Option<&Arc<Shader>> {
        let key = self.shader_names.get(name)?;
        self.shaders.get(*key)
    }

    /// Get shader key by name
    pub fn shader_key(&self, name: &str) -> Option<ShaderKey> {
        self.shader_names.get(name).copied()
    }

    /// Get the number of registered shaders
    pub fn shader_count(&self) -> usize {
        self.shaders.len()
    }

    // ===== PIPELINE CREATION =====

    /// Create a pipeline resource
    ///
    /// Resolves ShaderKeys from the descriptor and passes the GPU shaders
    /// to the backend for pipeline compilation.
    pub fn create_pipeline(
        &mut self,
        name: String,
        desc: PipelineDesc,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<PipelineKey> {
        if self.pipeline_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Pipeline '{}' already exists", name);
        }

        let vert = self.shaders.get(desc.vertex_shader)
            .ok_or_else(|| crate::engine_err!("galaxy3d::ResourceManager",
                "Pipeline '{}': vertex shader not found", name))?;
        let frag = self.shaders.get(desc.fragment_shader)
            .ok_or_else(|| crate::engine_err!("galaxy3d::ResourceManager",
                "Pipeline '{}': fragment shader not found", name))?;

        let gd_desc = graphics_device::PipelineDesc {
            vertex_layout: desc.vertex_layout,
            topology: desc.topology,
            rasterization: desc.rasterization,
            color_blend: desc.color_blend,
            multisample: desc.multisample,
            color_formats: desc.color_formats,
            depth_format: desc.depth_format,
        };

        let gd_pipeline = graphics_device.create_pipeline(
            gd_desc,
            vert.graphics_device_shader(),
            frag.graphics_device_shader(),
        )?;
        let pipeline = Pipeline::from_gpu_pipeline(gd_pipeline, desc.vertex_shader, desc.fragment_shader);

        let key = self.pipelines.insert(Arc::new(pipeline));
        self.pipeline_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Pipeline resource '{}'", name);

        Ok(key)
    }

    // ===== PIPELINE ACCESS =====

    /// Get a pipeline by key
    pub fn pipeline(&self, key: PipelineKey) -> Option<&Arc<Pipeline>> {
        self.pipelines.get(key)
    }

    /// Get a pipeline by name
    pub fn pipeline_by_name(&self, name: &str) -> Option<&Arc<Pipeline>> {
        let key = self.pipeline_names.get(name)?;
        self.pipelines.get(*key)
    }

    /// Get pipeline key by name
    pub fn pipeline_key(&self, name: &str) -> Option<PipelineKey> {
        self.pipeline_names.get(name).copied()
    }

    /// Remove a pipeline by name
    pub fn remove_pipeline(&mut self, name: &str) -> bool {
        if let Some(key) = self.pipeline_names.remove(name) {
            self.pipelines.remove(key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Pipeline resource '{}'", name);
            true
        } else {
            false
        }
    }

    /// Get the number of registered pipelines
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.len()
    }

    // ===== PIPELINE CACHE =====

    /// Resolve a pipeline from cache, or create it if not found.
    ///
    /// Builds a `PipelineCacheKey` from the provided parameters. If a pipeline
    /// with this exact combination already exists in the cache, returns its key.
    /// Otherwise, creates a new `resource::Pipeline` (stored in the same SlotMap
    /// as manual pipelines) and caches the mapping.
    pub fn resolve_pipeline(
        &mut self,
        vertex_shader: ShaderKey,
        fragment_shader: ShaderKey,
        vertex_layout: &graphics_device::VertexLayout,
        topology: graphics_device::PrimitiveTopology,
        color_blend: &graphics_device::ColorBlendState,
        polygon_mode: graphics_device::PolygonMode,
        pass_info: &PassInfo,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<PipelineKey> {
        let cache_key = PipelineCacheKey {
            vertex_shader,
            fragment_shader,
            vertex_layout: vertex_layout.clone(),
            topology,
            color_blend: *color_blend,
            polygon_mode,
            color_formats: pass_info.color_formats.clone(),
            depth_format: pass_info.depth_format,
            sample_count: pass_info.sample_count,
        };

        // Cache hit — pipeline already exists
        if let Some(&pipeline_key) = self.pipeline_cache.get(&cache_key) {
            return Ok(pipeline_key);
        }

        // Cache miss — create a new pipeline
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        cache_key.hash(&mut hasher);
        let hash = hasher.finish();
        let name = format!("_cache_{:016X}", hash);

        let pipeline_key = self.create_pipeline(
            name,
            PipelineDesc {
                vertex_shader,
                fragment_shader: cache_key.fragment_shader,
                vertex_layout: cache_key.vertex_layout.clone(),
                topology: cache_key.topology,
                rasterization: graphics_device::RasterizationState {
                    polygon_mode: cache_key.polygon_mode,
                    ..Default::default()
                },
                color_blend: cache_key.color_blend,
                multisample: graphics_device::MultisampleState {
                    sample_count: cache_key.sample_count,
                    alpha_to_coverage_enable: false,
                },
                color_formats: cache_key.color_formats.clone(),
                depth_format: cache_key.depth_format,
            },
            graphics_device,
        )?;

        self.pipeline_cache.insert(cache_key, pipeline_key);
        Ok(pipeline_key)
    }

    // ===== MATERIAL CREATION =====

    /// Create a material resource and register it
    pub fn create_material(
        &mut self,
        name: String,
        desc: MaterialDesc,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<MaterialKey> {
        if self.material_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Material '{}' already exists", name);
        }

        let slot_id = self.material_slot_allocator.alloc();
        let material = Material::from_desc(slot_id, desc, &*self, graphics_device)?;
        let texture_count = material.total_texture_slot_count();
        let param_count = material.total_param_count();

        let key = self.materials.insert(Arc::new(material));
        self.material_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Material resource '{}' slot {} ({} texture slot{}, {} param{})",
            name, slot_id,
            texture_count, if texture_count != 1 { "s" } else { "" },
            param_count, if param_count != 1 { "s" } else { "" });

        Ok(key)
    }

    // ===== MATERIAL ACCESS =====

    /// Get a material by key
    pub fn material(&self, key: MaterialKey) -> Option<&Arc<Material>> {
        self.materials.get(key)
    }

    /// Get a material by name
    pub fn material_by_name(&self, name: &str) -> Option<&Arc<Material>> {
        let key = self.material_names.get(name)?;
        self.materials.get(*key)
    }

    /// Get material key by name
    pub fn material_key(&self, name: &str) -> Option<MaterialKey> {
        self.material_names.get(name).copied()
    }

    /// Remove a material by name
    pub fn remove_material(&mut self, name: &str) -> bool {
        if let Some(key) = self.material_names.remove(name) {
            if let Some(material) = self.materials.remove(key) {
                self.material_slot_allocator.free(material.slot_id());
                crate::engine_info!("galaxy3d::ResourceManager",
                    "Removed Material resource '{}' (freed slot {})", name, material.slot_id());
                return true;
            }
        }
        false
    }

    /// Get the number of registered materials
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Get the high water mark for material slot allocation
    pub fn material_slot_high_water_mark(&self) -> u32 {
        self.material_slot_allocator.high_water_mark()
    }

    /// Get the number of currently allocated material slots
    pub fn material_slot_count(&self) -> u32 {
        self.material_slot_allocator.len()
    }

    // ===== MATERIAL SYNC =====

    /// Sync all material parameters into a GPU buffer.
    ///
    /// For each material, matches params by name against buffer fields.
    /// Copies values only when name AND type match. Non-blocking warnings
    /// for mismatches (the function never fails on a mismatch).
    pub fn sync_materials_to_buffer(&self, buffer: &Buffer) -> Result<()> {
        for (_, material) in &self.materials {
            let slot_id = material.slot_id();

            if slot_id >= buffer.count() {
                crate::engine_warn!("galaxy3d::ResourceManager",
                    "sync_materials: material slot_id {} exceeds buffer count {}",
                    slot_id, buffer.count());
                continue;
            }

            for param in material.iter_all_params() {
                // 1. Find field by name
                let field_index = match buffer.field_id(param.name()) {
                    Some(idx) => idx,
                    None => {
                        crate::engine_warn!("galaxy3d::ResourceManager",
                            "sync_materials: param '{}' not found in buffer layout",
                            param.name());
                        continue;
                    }
                };

                // 2. Check type compatibility
                let field_type = buffer.fields()[field_index].field_type;
                let param_type = compatible_field_type(param.value());

                if param_type != field_type {
                    crate::engine_warn!("galaxy3d::ResourceManager",
                        "sync_materials: param '{}' type mismatch (param: {:?}, field: {:?})",
                        param.name(), param_type, field_type);
                    continue;
                }

                // 3. Specific Bool→UInt info warning
                if matches!(param.value(), ParamValue::Bool(_)) {
                    crate::engine_warn!("galaxy3d::ResourceManager",
                        "sync_materials: param '{}' is Bool, \
                         mapped to UInt field (GLSL convention)",
                        param.name());
                }

                // 4. Convert to padded bytes and write
                let bytes = param_to_padded_bytes(param.value());
                buffer.update_field(slot_id, field_index, &bytes)?;
            }

            // ===== TEXTURE SLOTS → BUFFER FIELDS (bindless index, sampler index, layer) =====
            // Convention: slot name "albedo" maps to fields "albedoTexture", "albedoSampler", "albedoLayer"
            for slot in material.iter_all_texture_slots() {
                let slot_name = slot.name();

                // Write bindless texture index → "{name}Texture"
                let tex_field_name = format!("{}Texture", slot_name);
                if let Some(field_index) = buffer.field_id(&tex_field_name) {
                    let value = slot.bindless_index();
                    buffer.update_field(slot_id, field_index, &value.to_ne_bytes())?;
                }

                // Write sampler index → "{name}Sampler"
                let sampler_field_name = format!("{}Sampler", slot_name);
                if let Some(field_index) = buffer.field_id(&sampler_field_name) {
                    let value = slot.sampler_index();
                    buffer.update_field(slot_id, field_index, &value.to_ne_bytes())?;
                }

                // Write layer index → "{name}Layer"
                let layer_field_name = format!("{}Layer", slot_name);
                if let Some(field_index) = buffer.field_id(&layer_field_name) {
                    let value: u32 = slot.layer().unwrap_or(0);
                    buffer.update_field(slot_id, field_index, &value.to_ne_bytes())?;
                }
            }
        }
        Ok(())
    }

    // ===== MESH CREATION =====

    /// Create a mesh resource and register it
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<MeshKey> {
        if self.mesh_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Mesh '{}' already exists", name);
        }

        let mesh = Mesh::from_desc(desc, &*self)?;
        let lod_count = mesh.lod_count();

        let key = self.meshes.insert(Arc::new(mesh));
        self.mesh_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Mesh resource '{}' ({} LOD{})",
            name, lod_count, if lod_count != 1 { "s" } else { "" });

        Ok(key)
    }

    // ===== MESH ACCESS =====

    /// Get a mesh by key
    pub fn mesh(&self, key: MeshKey) -> Option<&Arc<Mesh>> {
        self.meshes.get(key)
    }

    /// Get a mesh by name
    pub fn mesh_by_name(&self, name: &str) -> Option<&Arc<Mesh>> {
        let key = self.mesh_names.get(name)?;
        self.meshes.get(*key)
    }

    /// Get mesh key by name
    pub fn mesh_key(&self, name: &str) -> Option<MeshKey> {
        self.mesh_names.get(name).copied()
    }

    /// Remove a mesh by name
    pub fn remove_mesh(&mut self, name: &str) -> bool {
        if let Some(key) = self.mesh_names.remove(name) {
            self.meshes.remove(key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Mesh resource '{}'", name);
            true
        } else {
            false
        }
    }

    /// Get the number of registered meshes
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    // ===== BUFFER CREATION =====

    /// Create a structured GPU buffer resource (UBO or SSBO)
    pub fn create_buffer(&mut self, name: String, desc: BufferDesc) -> Result<BufferKey> {
        if self.buffer_names.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Buffer '{}' already exists", name);
        }

        let buffer = Buffer::from_desc(desc)?;
        let kind = buffer.kind();
        let count = buffer.count();
        let stride = buffer.stride();
        let size = buffer.size();

        let key = self.buffers.insert(Arc::new(buffer));
        self.buffer_names.insert(name.clone(), key);

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created {:?} buffer '{}' ({} elements, stride {} bytes, total {} bytes)",
            kind, name, count, stride, size);

        Ok(key)
    }

    // ===== BUFFER ACCESS =====

    /// Get a buffer by key
    pub fn buffer(&self, key: BufferKey) -> Option<&Arc<Buffer>> {
        self.buffers.get(key)
    }

    /// Get a buffer by name
    pub fn buffer_by_name(&self, name: &str) -> Option<&Arc<Buffer>> {
        let key = self.buffer_names.get(name)?;
        self.buffers.get(*key)
    }

    /// Get buffer key by name
    pub fn buffer_key(&self, name: &str) -> Option<BufferKey> {
        self.buffer_names.get(name).copied()
    }

    /// Remove a buffer by name
    pub fn remove_buffer(&mut self, name: &str) -> bool {
        if let Some(key) = self.buffer_names.remove(name) {
            self.buffers.remove(key);
            crate::engine_info!("galaxy3d::ResourceManager", "Removed Buffer resource '{}'", name);
            true
        } else {
            false
        }
    }

    /// Get the number of registered buffers
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Create a default per-frame uniform buffer (UBO) with standard engine fields.
    ///
    /// Layout (std140, 304 bytes):
    /// - Camera: view, projection, viewProjection (Mat4), cameraPosition, cameraDirection (Vec4)
    /// - Lighting: sunDirection, sunColor, ambientColor (Vec4)
    /// - Time: time, deltaTime (Float), frameIndex (UInt)
    /// - Post-process: exposure, gamma (Float)
    /// - Depth: nearPlane, farPlane (Float)
    /// - Ambient: ambientIntensity (Float)
    ///
    /// Fields that would cause artifacts or crashes at zero are initialized
    /// with safe defaults.
    pub fn create_default_frame_uniform_buffer(
        &mut self,
        name: String,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    ) -> Result<BufferKey> {
        let key = self.create_buffer(name, BufferDesc {
            graphics_device,
            kind: BufferKind::Uniform,
            fields: vec![
                FieldDesc { name: "view".to_string(),             field_type: FieldType::Mat4 },
                FieldDesc { name: "projection".to_string(),       field_type: FieldType::Mat4 },
                FieldDesc { name: "viewProjection".to_string(),   field_type: FieldType::Mat4 },
                FieldDesc { name: "cameraPosition".to_string(),   field_type: FieldType::Vec4 },
                FieldDesc { name: "cameraDirection".to_string(),  field_type: FieldType::Vec4 },
                FieldDesc { name: "sunDirection".to_string(),     field_type: FieldType::Vec4 },
                FieldDesc { name: "sunColor".to_string(),         field_type: FieldType::Vec4 },
                FieldDesc { name: "ambientColor".to_string(),     field_type: FieldType::Vec4 },
                FieldDesc { name: "time".to_string(),             field_type: FieldType::Float },
                FieldDesc { name: "deltaTime".to_string(),        field_type: FieldType::Float },
                FieldDesc { name: "frameIndex".to_string(),       field_type: FieldType::UInt },
                FieldDesc { name: "exposure".to_string(),         field_type: FieldType::Float },
                FieldDesc { name: "gamma".to_string(),            field_type: FieldType::Float },
                FieldDesc { name: "nearPlane".to_string(),        field_type: FieldType::Float },
                FieldDesc { name: "farPlane".to_string(),         field_type: FieldType::Float },
                FieldDesc { name: "ambientIntensity".to_string(), field_type: FieldType::Float },
            ],
            count: 1,
        })?;

        let buffer = self.buffer(key).unwrap();

        // Defaults for fields that cause artifacts or crashes if left at 0
        let f = |name: &str| buffer.field_id(name).unwrap();

        buffer.update_field(0, f("sunDirection"),     &[0.0f32, -1.0, 0.0, 0.0].map(|v| v.to_ne_bytes()).concat())?;
        buffer.update_field(0, f("sunColor"),         &[1.0f32, 1.0, 1.0, 1.0].map(|v| v.to_ne_bytes()).concat())?;
        buffer.update_field(0, f("ambientColor"),     &[0.1f32, 0.1, 0.1, 1.0].map(|v| v.to_ne_bytes()).concat())?;
        buffer.update_field(0, f("exposure"),         &1.0f32.to_ne_bytes())?;
        buffer.update_field(0, f("gamma"),            &2.2f32.to_ne_bytes())?;
        buffer.update_field(0, f("nearPlane"),        &0.1f32.to_ne_bytes())?;
        buffer.update_field(0, f("farPlane"),         &1000.0f32.to_ne_bytes())?;
        buffer.update_field(0, f("ambientIntensity"), &1.0f32.to_ne_bytes())?;

        Ok(key)
    }

    /// Create a default per-instance storage buffer (SSBO) with standard engine fields.
    pub fn create_default_instance_buffer(
        &mut self,
        name: String,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        count: u32,
    ) -> Result<BufferKey> {
        let key = self.create_buffer(name, BufferDesc {
            graphics_device,
            kind: BufferKind::Storage,
            fields: vec![
                FieldDesc { name: "world".to_string(),          field_type: FieldType::Mat4 },
                FieldDesc { name: "previousWorld".to_string(),  field_type: FieldType::Mat4 },
                FieldDesc { name: "inverseWorld".to_string(),   field_type: FieldType::Mat4 },
                FieldDesc { name: "materialSlotId".to_string(), field_type: FieldType::UInt },
                FieldDesc { name: "flags".to_string(),          field_type: FieldType::UInt },
                FieldDesc { name: "lightCount".to_string(),     field_type: FieldType::UInt },
                FieldDesc { name: "customData".to_string(),     field_type: FieldType::Vec4 },
                FieldDesc { name: "lightIndices0".to_string(),  field_type: FieldType::UVec4 },
                FieldDesc { name: "lightIndices1".to_string(),  field_type: FieldType::UVec4 },
            ],
            count,
        })?;

        let buffer = self.buffer(key).unwrap();

        // Safe defaults: lightCount = 0 (zero-initialized), no lights assigned (sentinel 0xFFFFFFFF)
        let no_light = [0xFFFFFFFFu32; 4];
        for i in 0..count {
            buffer.update_field(i, 7, bytemuck::bytes_of(&no_light))?; // lightIndices0
            buffer.update_field(i, 8, bytemuck::bytes_of(&no_light))?; // lightIndices1
        }

        Ok(key)
    }

    /// Create a default material storage buffer (SSBO) with standard PBR fields.
    pub fn create_default_material_buffer(
        &mut self,
        name: String,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        count: u32,
    ) -> Result<BufferKey> {
        let key = self.create_buffer(name, BufferDesc {
            graphics_device,
            kind: BufferKind::Storage,
            fields: vec![
                FieldDesc { name: "baseColor".to_string(),                    field_type: FieldType::Vec4 },
                FieldDesc { name: "emissiveColor".to_string(),                field_type: FieldType::Vec4 },
                FieldDesc { name: "metallic".to_string(),                     field_type: FieldType::Float },
                FieldDesc { name: "roughness".to_string(),                    field_type: FieldType::Float },
                FieldDesc { name: "normalScale".to_string(),                  field_type: FieldType::Float },
                FieldDesc { name: "ao".to_string(),                           field_type: FieldType::Float },
                FieldDesc { name: "alphaCutoff".to_string(),                  field_type: FieldType::Float },
                FieldDesc { name: "ior".to_string(),                          field_type: FieldType::Float },
                FieldDesc { name: "albedoTexture".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "albedoSampler".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "albedoLayer".to_string(),                  field_type: FieldType::UInt },
                FieldDesc { name: "normalTexture".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "normalSampler".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "normalLayer".to_string(),                  field_type: FieldType::UInt },
                FieldDesc { name: "metallicRoughnessTexture".to_string(),     field_type: FieldType::UInt },
                FieldDesc { name: "metallicRoughnessSampler".to_string(),     field_type: FieldType::UInt },
                FieldDesc { name: "metallicRoughnessLayer".to_string(),       field_type: FieldType::UInt },
                FieldDesc { name: "emissiveTexture".to_string(),              field_type: FieldType::UInt },
                FieldDesc { name: "emissiveSampler".to_string(),              field_type: FieldType::UInt },
                FieldDesc { name: "emissiveLayer".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "aoTexture".to_string(),                    field_type: FieldType::UInt },
                FieldDesc { name: "aoSampler".to_string(),                    field_type: FieldType::UInt },
                FieldDesc { name: "aoLayer".to_string(),                      field_type: FieldType::UInt },
                FieldDesc { name: "flags".to_string(),                        field_type: FieldType::UInt },
            ],
            count,
        })?;

        let buffer = self.buffer(key).unwrap();

        // Safe defaults for all slots
        let f = |name: &str| buffer.field_id(name).unwrap();
        let no_texture = u32::MAX.to_ne_bytes();

        for i in 0..count {
            buffer.update_field(i, f("baseColor"),   &[1.0f32, 1.0, 1.0, 1.0].map(|v| v.to_ne_bytes()).concat())?;
            buffer.update_field(i, f("roughness"),   &0.5f32.to_ne_bytes())?;
            buffer.update_field(i, f("normalScale"), &1.0f32.to_ne_bytes())?;
            buffer.update_field(i, f("ao"),          &1.0f32.to_ne_bytes())?;
            buffer.update_field(i, f("alphaCutoff"), &0.5f32.to_ne_bytes())?;
            buffer.update_field(i, f("ior"),         &1.5f32.to_ne_bytes())?;
            buffer.update_field(i, f("albedoTexture"),                &no_texture)?;
            buffer.update_field(i, f("albedoSampler"),                &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("albedoLayer"),                  &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("normalTexture"),                &no_texture)?;
            buffer.update_field(i, f("normalSampler"),                &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("normalLayer"),                  &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("metallicRoughnessTexture"),     &no_texture)?;
            buffer.update_field(i, f("metallicRoughnessSampler"),     &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("metallicRoughnessLayer"),       &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("emissiveTexture"),              &no_texture)?;
            buffer.update_field(i, f("emissiveSampler"),              &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("emissiveLayer"),                &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("aoTexture"),                    &no_texture)?;
            buffer.update_field(i, f("aoSampler"),                    &0u32.to_ne_bytes())?;
            buffer.update_field(i, f("aoLayer"),                      &0u32.to_ne_bytes())?;
        }

        Ok(key)
    }

    /// Create a default light storage buffer (SSBO) with standard light fields.
    pub fn create_default_light_buffer(
        &mut self,
        name: String,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        count: u32,
    ) -> Result<BufferKey> {
        let key = self.create_buffer(name, BufferDesc {
            graphics_device,
            kind: BufferKind::Storage,
            fields: vec![
                FieldDesc { name: "positionType".to_string(),   field_type: FieldType::Vec4 },
                FieldDesc { name: "directionRange".to_string(), field_type: FieldType::Vec4 },
                FieldDesc { name: "colorIntensity".to_string(), field_type: FieldType::Vec4 },
                FieldDesc { name: "spotParams".to_string(),     field_type: FieldType::Vec4 },
                FieldDesc { name: "attenuation".to_string(),    field_type: FieldType::Vec4 },
            ],
            count,
        })?;

        let buffer = self.buffer(key).unwrap();

        // Safe defaults: all lights disabled, inverse square attenuation
        for i in 0..count {
            buffer.update_field(i, 3, // spotParams: disabled
                &[0.0f32, 0.0, 0.0, 0.0].map(|v| v.to_ne_bytes()).concat())?;
            buffer.update_field(i, 4, // attenuation: constant=0, linear=0, quadratic=1
                &[0.0f32, 0.0, 1.0, 0.0].map(|v| v.to_ne_bytes()).concat())?;
        }

        Ok(key)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "resource_manager_tests.rs"]
mod tests;
