////! Central resource manager for the engine.
//!
//! Stores and provides access to all engine resources (textures, geometries, etc.).
//! Resources will be added incrementally as the engine evolves.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::renderer;
use crate::resource::texture::{
    Texture,
    TextureDesc, LayerDesc, AtlasRegionDesc,
};
use crate::resource::geometry::{
    Geometry, GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
use crate::resource::pipeline::{
    Pipeline, PipelineDesc, PipelineVariantDesc,
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

pub struct ResourceManager {
    textures: HashMap<String, Arc<Texture>>,
    geometries: HashMap<String, Arc<Geometry>>,
    pipelines: HashMap<String, Arc<Pipeline>>,
    materials: HashMap<String, Arc<Material>>,
    meshes: HashMap<String, Arc<Mesh>>,
    buffers: HashMap<String, Arc<Buffer>>,
    material_slot_allocator: SlotAllocator,
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

impl ResourceManager {
    /// Create a new empty resource manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            geometries: HashMap::new(),
            pipelines: HashMap::new(),
            materials: HashMap::new(),
            meshes: HashMap::new(),
            buffers: HashMap::new(),
            material_slot_allocator: SlotAllocator::new(),
        }
    }

    // ===== TEXTURE CREATION =====

    /// Create a texture (simple or indexed, with optional atlas regions per layer)
    ///
    /// Internally creates the GPU texture and descriptor set via the renderer.
    /// Returns the created texture for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this texture resource
    /// * `desc` - Texture descriptor with renderer, texture settings, and layers
    ///
    pub fn create_texture(&mut self, name: String, desc: TextureDesc) -> Result<Arc<Texture>> {
        if self.textures.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Texture '{}' already exists", name);
        }

        let texture = Texture::from_desc(desc)?;
        let is_simple = texture.is_simple();
        let layer_count = texture.layer_count();

        let texture_arc = Arc::new(texture);
        self.textures.insert(name.clone(), Arc::clone(&texture_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created {} texture '{}' ({} layer{})",
            if is_simple { "Simple" } else { "Indexed" },
            name, layer_count, if layer_count > 1 { "s" } else { "" });

        Ok(texture_arc)
    }

    // ===== TEXTURE ACCESS =====

    /// Get a texture by name
    pub fn texture(&self, name: &str) -> Option<&Arc<Texture>> {
        self.textures.get(name)
    }

    /// Remove a texture by name
    ///
    /// Returns `true` if the texture was found and removed.
    pub fn remove_texture(&mut self, name: &str) -> bool {
        if self.textures.remove(name).is_some() {
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
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the texture Arc exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The texture does not exist
    /// - The texture is a simple texture (array_layers=1)
    /// - Other Arc references prevent mutable access
    /// - Layer validation fails
    pub fn add_texture_layer(
        &mut self,
        texture_name: &str,
        desc: LayerDesc,
    ) -> Result<u32> {
        let arc = self.textures.get_mut(texture_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Texture '{}' not found", texture_name))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate texture '{}': other references exist", texture_name))?;

        texture.add_layer(desc)
    }

    /// Add a region to an existing texture layer
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the texture Arc exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The texture does not exist
    /// - The layer does not exist
    /// - Other Arc references prevent mutable access
    /// - Region validation fails
    pub fn add_texture_region(
        &mut self,
        texture_name: &str,
        layer_name: &str,
        desc: AtlasRegionDesc,
    ) -> Result<u32> {
        let arc = self.textures.get_mut(texture_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Texture '{}' not found", texture_name))?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate texture '{}': other references exist", texture_name))?;

        texture.add_region(layer_name, desc)
    }

    // ===== GEOMETRY CREATION =====

    /// Create a geometry resource and register it
    ///
    /// Internally creates the GPU vertex and index buffers from the provided data.
    /// Vertex and index counts are computed automatically from data length and layout.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this geometry resource (group name)
    /// * `desc` - Geometry description with renderer, vertex/index data and meshes
    ///
    pub fn create_geometry(&mut self, name: String, desc: GeometryDesc) -> Result<Arc<Geometry>> {
        if self.geometries.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Geometry '{}' already exists", name);
        }

        let geometry = Geometry::from_desc(desc)?;
        let mesh_count = geometry.mesh_count();
        let total_vertex_count = geometry.total_vertex_count();
        let total_index_count = geometry.total_index_count();
        let geometry_arc = Arc::new(geometry);
        self.geometries.insert(name.clone(), Arc::clone(&geometry_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Geometry resource '{}' ({} vertices, {} indices, {} meshes)",
            name, total_vertex_count, total_index_count, mesh_count);

        Ok(geometry_arc)
    }

    // ===== GEOMETRY ACCESS =====

    /// Get a geometry by name
    pub fn geometry(&self, name: &str) -> Option<&Arc<Geometry>> {
        self.geometries.get(name)
    }

    /// Remove a geometry by name
    ///
    /// Returns `true` if the geometry was found and removed.
    pub fn remove_geometry(&mut self, name: &str) -> bool {
        if self.geometries.remove(name).is_some() {
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
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the geometry Arc exist.
    ///
    /// # Returns
    ///
    /// The id (index) of the newly created mesh.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The geometry does not exist
    /// - Other Arc references prevent mutable access
    /// - A mesh with the same name already exists
    /// - Submesh validation fails (offsets exceed buffer sizes)
    pub fn add_geometry_mesh(&mut self, geom_name: &str, desc: GeometryMeshDesc) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry '{}' not found", geom_name))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry '{}': other references exist", geom_name))?;

        geometry.add_mesh(desc)
    }

    /// Add a LOD to an existing mesh
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the geometry Arc exist.
    ///
    /// # Returns
    ///
    /// The lod index of the newly created LOD.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The geometry does not exist
    /// - The mesh does not exist
    /// - Other Arc references prevent mutable access
    /// - Submesh validation fails
    pub fn add_geometry_lod(
        &mut self,
        geom_name: &str,
        mesh_id: usize,
        desc: GeometryLODDesc,
    ) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry '{}' not found", geom_name))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry '{}': other references exist", geom_name))?;

        geometry.add_lod(mesh_id, desc)
    }

    /// Add a submesh to an existing LOD
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the geometry Arc exist.
    ///
    /// # Returns
    ///
    /// The id (index) of the newly created submesh.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The geometry does not exist
    /// - The mesh does not exist
    /// - Other Arc references prevent mutable access
    /// - A submesh with the same name already exists in the LOD
    /// - Submesh validation fails (offsets exceed buffer sizes)
    pub fn add_geometry_submesh(
        &mut self,
        geom_name: &str,
        mesh_id: usize,
        lod_index: usize,
        desc: GeometrySubMeshDesc,
    ) -> Result<usize> {
        let arc = self.geometries.get_mut(geom_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Geometry '{}' not found", geom_name))?;

        let geometry = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Geometry '{}': other references exist", geom_name))?;

        geometry.add_submesh(mesh_id, lod_index, desc)
    }

    // ===== PIPELINE CREATION =====

    /// Create a pipeline resource with optional variants
    ///
    /// Internally creates GPU pipelines for each variant via the renderer.
    /// Returns the created pipeline for immediate use.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this pipeline resource
    /// * `desc` - Pipeline descriptor with renderer and variant configurations
    ///
    pub fn create_pipeline(&mut self, name: String, desc: PipelineDesc) -> Result<Arc<Pipeline>> {
        if self.pipelines.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Pipeline '{}' already exists", name);
        }

        let pipeline = Pipeline::from_desc(desc)?;
        let variant_count = pipeline.variant_count();

        let pipeline_arc = Arc::new(pipeline);
        self.pipelines.insert(name.clone(), Arc::clone(&pipeline_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Pipeline resource '{}' ({} variant{})",
            name, variant_count, if variant_count != 1 { "s" } else { "" });

        Ok(pipeline_arc)
    }

    // ===== PIPELINE ACCESS =====

    /// Get a pipeline by name
    pub fn pipeline(&self, name: &str) -> Option<&Arc<Pipeline>> {
        self.pipelines.get(name)
    }

    /// Remove a pipeline by name
    ///
    /// Returns `true` if the pipeline was found and removed.
    pub fn remove_pipeline(&mut self, name: &str) -> bool {
        if self.pipelines.remove(name).is_some() {
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

    // ===== PIPELINE MODIFICATION =====

    /// Add a variant to an existing pipeline
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the pipeline Arc exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The pipeline does not exist
    /// - Other Arc references prevent mutable access
    /// - A variant with the same name already exists
    /// - GPU pipeline creation fails
    pub fn add_pipeline_variant(
        &mut self,
        pipeline_name: &str,
        desc: PipelineVariantDesc,
    ) -> Result<u32> {
        let arc = self.pipelines.get_mut(pipeline_name)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Pipeline '{}' not found", pipeline_name))?;

        let pipeline = Arc::get_mut(arc)
            .ok_or_else(|| crate::engine_warn_err!("galaxy3d::ResourceManager", "Cannot mutate Pipeline '{}': other references exist", pipeline_name))?;

        pipeline.add_variant(desc)
    }

    // ===== MATERIAL CREATION =====

    /// Create a material resource and register it
    ///
    /// A material is a pure data description: pipeline reference, texture slots,
    /// and named parameters. No GPU resources are created at this level.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this material resource
    /// * `desc` - Material descriptor with pipeline, textures, and parameters
    ///
    pub fn create_material(&mut self, name: String, desc: MaterialDesc) -> Result<Arc<Material>> {
        if self.materials.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Material '{}' already exists", name);
        }

        let slot_id = self.material_slot_allocator.alloc();
        let material = Material::from_desc(slot_id, desc)?;
        let texture_count = material.texture_slot_count();
        let param_count = material.param_count();

        let material_arc = Arc::new(material);
        self.materials.insert(name.clone(), Arc::clone(&material_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Material resource '{}' slot {} ({} texture slot{}, {} param{})",
            name, slot_id,
            texture_count, if texture_count != 1 { "s" } else { "" },
            param_count, if param_count != 1 { "s" } else { "" });

        Ok(material_arc)
    }

    // ===== MATERIAL ACCESS =====

    /// Get a material by name
    pub fn material(&self, name: &str) -> Option<&Arc<Material>> {
        self.materials.get(name)
    }

    /// Remove a material by name
    ///
    /// Returns `true` if the material was found and removed.
    pub fn remove_material(&mut self, name: &str) -> bool {
        if let Some(material) = self.materials.remove(name) {
            self.material_slot_allocator.free(material.slot_id());
            crate::engine_info!("galaxy3d::ResourceManager",
                "Removed Material resource '{}' (freed slot {})", name, material.slot_id());
            true
        } else {
            false
        }
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
        for (mat_name, material) in &self.materials {
            let slot_id = material.slot_id();

            if slot_id >= buffer.count() {
                crate::engine_warn!("galaxy3d::ResourceManager",
                    "sync_materials: material '{}' slot_id {} exceeds buffer count {}",
                    mat_name, slot_id, buffer.count());
                continue;
            }

            for param in material.params() {
                // 1. Find field by name
                let field_index = match buffer.field_id(param.name()) {
                    Some(idx) => idx,
                    None => {
                        crate::engine_warn!("galaxy3d::ResourceManager",
                            "sync_materials: material '{}' param '{}' not found in buffer layout",
                            mat_name, param.name());
                        continue;
                    }
                };

                // 2. Check type compatibility
                let field_type = buffer.fields()[field_index].field_type;
                let param_type = compatible_field_type(param.value());

                if param_type != field_type {
                    crate::engine_warn!("galaxy3d::ResourceManager",
                        "sync_materials: material '{}' param '{}' type mismatch (param: {:?}, field: {:?})",
                        mat_name, param.name(), param_type, field_type);
                    continue;
                }

                // 3. Specific Bool→UInt info warning
                if matches!(param.value(), ParamValue::Bool(_)) {
                    crate::engine_warn!("galaxy3d::ResourceManager",
                        "sync_materials: material '{}' param '{}' is Bool, \
                         mapped to UInt field (GLSL convention)",
                        mat_name, param.name());
                }

                // 4. Convert to padded bytes and write
                let bytes = param_to_padded_bytes(param.value());
                buffer.update_field(slot_id, field_index, &bytes)?;
            }

            // ===== TEXTURE SLOTS → BUFFER FIELDS (layer index) =====
            for slot in material.texture_slots() {
                // 1. Find field by slot name
                let field_index = match buffer.field_id(slot.name()) {
                    Some(idx) => idx,
                    None => {
                        crate::engine_warn!("galaxy3d::ResourceManager",
                            "sync_materials: material '{}' texture slot '{}' \
                             not found in buffer layout",
                            mat_name, slot.name());
                        continue;
                    }
                };

                // 2. Check field is UInt
                let field_type = buffer.fields()[field_index].field_type;
                if field_type != FieldType::UInt {
                    crate::engine_warn!("galaxy3d::ResourceManager",
                        "sync_materials: material '{}' texture slot '{}' \
                         expects UInt field, found {:?}",
                        mat_name, slot.name(), field_type);
                    continue;
                }

                // 3. Write layer index (0 if no layer specified)
                let layer_value: u32 = slot.layer().unwrap_or(0);
                buffer.update_field(slot_id, field_index, &layer_value.to_ne_bytes())?;
            }
        }
        Ok(())
    }

    // ===== MESH CREATION =====

    /// Create a mesh resource and register it
    ///
    /// A mesh combines a GeometryMesh with Materials per submesh per LOD,
    /// forming a renderable object. No GPU resources beyond those already
    /// created for Geometry/Material are needed.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this mesh resource
    /// * `desc` - Mesh descriptor with geometry, mesh reference, and LOD material assignments
    ///
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<Arc<Mesh>> {
        if self.meshes.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Mesh '{}' already exists", name);
        }

        let mesh = Mesh::from_desc(desc)?;
        let lod_count = mesh.lod_count();

        let mesh_arc = Arc::new(mesh);
        self.meshes.insert(name.clone(), Arc::clone(&mesh_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Mesh resource '{}' ({} LOD{})",
            name, lod_count, if lod_count != 1 { "s" } else { "" });

        Ok(mesh_arc)
    }

    // ===== MESH ACCESS =====

    /// Get a mesh by name
    pub fn mesh(&self, name: &str) -> Option<&Arc<Mesh>> {
        self.meshes.get(name)
    }

    /// Remove a mesh by name
    ///
    /// Returns `true` if the mesh was found and removed.
    pub fn remove_mesh(&mut self, name: &str) -> bool {
        if self.meshes.remove(name).is_some() {
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
    ///
    /// Computes the layout from the field descriptors (std140 for UBO, std430 for SSBO), allocates
    /// the GPU buffer, and registers the resource.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this buffer resource
    /// * `desc` - Buffer descriptor with renderer, kind, fields, and element count
    ///
    pub fn create_buffer(&mut self, name: String, desc: BufferDesc) -> Result<Arc<Buffer>> {
        if self.buffers.contains_key(&name) {
            crate::engine_bail_warn!("galaxy3d::ResourceManager", "Buffer '{}' already exists", name);
        }

        let buffer = Buffer::from_desc(desc)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.insert(name.clone(), Arc::clone(&buffer_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created {:?} buffer '{}' ({} elements, stride {} bytes, total {} bytes)",
            buffer_arc.kind(), name, buffer_arc.count(),
            buffer_arc.stride(), buffer_arc.size());

        Ok(buffer_arc)
    }

    // ===== BUFFER ACCESS =====

    /// Get a buffer by name
    pub fn buffer(&self, name: &str) -> Option<&Arc<Buffer>> {
        self.buffers.get(name)
    }

    /// Remove a buffer by name
    ///
    /// Returns `true` if the buffer was found and removed.
    pub fn remove_buffer(&mut self, name: &str) -> bool {
        if self.buffers.remove(name).is_some() {
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
        renderer: Arc<Mutex<dyn renderer::Renderer>>,
    ) -> Result<Arc<Buffer>> {
        let buffer = self.create_buffer(name, BufferDesc {
            renderer,
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

        Ok(buffer)
    }

    /// Create a default per-instance storage buffer (SSBO) with standard engine fields.
    ///
    /// Layout (std430, 224 bytes per element):
    /// - Transform: world, previousWorld, inverseWorld (Mat4)
    /// - References: materialSlotId, flags (UInt)
    /// - 8 bytes padding (Vec4 alignment)
    /// - Custom: customData (Vec4)
    ///
    /// No default values — instances are populated individually.
    pub fn create_default_instance_buffer(
        &mut self,
        name: String,
        renderer: Arc<Mutex<dyn renderer::Renderer>>,
        count: u32,
    ) -> Result<Arc<Buffer>> {
        self.create_buffer(name, BufferDesc {
            renderer,
            kind: BufferKind::Storage,
            fields: vec![
                FieldDesc { name: "world".to_string(),          field_type: FieldType::Mat4 },
                FieldDesc { name: "previousWorld".to_string(),  field_type: FieldType::Mat4 },
                FieldDesc { name: "inverseWorld".to_string(),   field_type: FieldType::Mat4 },
                FieldDesc { name: "materialSlotId".to_string(), field_type: FieldType::UInt },
                FieldDesc { name: "flags".to_string(),          field_type: FieldType::UInt },
                FieldDesc { name: "customData".to_string(),     field_type: FieldType::Vec4 },
            ],
            count,
        })
    }

    /// Create a default material storage buffer (SSBO) with standard PBR fields.
    ///
    /// Layout (std430, 80 bytes per element):
    /// - Color: baseColor, emissiveColor (Vec4)
    /// - PBR factors: metallic, roughness, normalScale, ao (Float)
    /// - Transparency: alphaCutoff, ior (Float)
    /// - Texture indices: albedoTexture, normalTexture, metallicRoughnessTexture,
    ///   emissiveTexture, aoTexture (UInt) — u32::MAX = no texture
    /// - Flags: flags (UInt) — bitfield for material properties
    ///
    /// Fields that would cause artifacts or crashes at zero are initialized
    /// with safe defaults.
    pub fn create_default_material_buffer(
        &mut self,
        name: String,
        renderer: Arc<Mutex<dyn renderer::Renderer>>,
        count: u32,
    ) -> Result<Arc<Buffer>> {
        let buffer = self.create_buffer(name, BufferDesc {
            renderer,
            kind: BufferKind::Storage,
            fields: vec![
                FieldDesc { name: "baseColor".to_string(),                field_type: FieldType::Vec4 },
                FieldDesc { name: "emissiveColor".to_string(),            field_type: FieldType::Vec4 },
                FieldDesc { name: "metallic".to_string(),                 field_type: FieldType::Float },
                FieldDesc { name: "roughness".to_string(),                field_type: FieldType::Float },
                FieldDesc { name: "normalScale".to_string(),              field_type: FieldType::Float },
                FieldDesc { name: "ao".to_string(),                       field_type: FieldType::Float },
                FieldDesc { name: "alphaCutoff".to_string(),              field_type: FieldType::Float },
                FieldDesc { name: "ior".to_string(),                      field_type: FieldType::Float },
                FieldDesc { name: "albedoTexture".to_string(),            field_type: FieldType::UInt },
                FieldDesc { name: "normalTexture".to_string(),            field_type: FieldType::UInt },
                FieldDesc { name: "metallicRoughnessTexture".to_string(), field_type: FieldType::UInt },
                FieldDesc { name: "emissiveTexture".to_string(),          field_type: FieldType::UInt },
                FieldDesc { name: "aoTexture".to_string(),                field_type: FieldType::UInt },
                FieldDesc { name: "flags".to_string(),                    field_type: FieldType::UInt },
            ],
            count,
        })?;

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
            buffer.update_field(i, f("albedoTexture"),            &no_texture)?;
            buffer.update_field(i, f("normalTexture"),            &no_texture)?;
            buffer.update_field(i, f("metallicRoughnessTexture"), &no_texture)?;
            buffer.update_field(i, f("emissiveTexture"),          &no_texture)?;
            buffer.update_field(i, f("aoTexture"),                &no_texture)?;
        }

        Ok(buffer)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "resource_manager_tests.rs"]
mod tests;
