////! Central resource manager for the engine.
//!
//! Stores and provides access to all engine resources (textures, meshes, etc.).
//! Resources will be added incrementally as the engine evolves.

use std::collections::HashMap;
use std::sync::Arc;
use crate::error::{Error, Result};
use crate::resource::texture::{
    Texture,
    TextureDesc, LayerDesc, AtlasRegionDesc,
};
use crate::resource::mesh::{
    Mesh, MeshDesc, MeshEntryDesc, MeshLODDesc, SubMeshDesc,
};
use crate::resource::pipeline::{
    Pipeline, PipelineDesc, PipelineVariantDesc,
};

pub struct ResourceManager {
    textures: HashMap<String, Arc<Texture>>,
    meshes: HashMap<String, Arc<Mesh>>,
    pipelines: HashMap<String, Arc<Pipeline>>,
}

impl ResourceManager {
    /// Create a new empty resource manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            meshes: HashMap::new(),
            pipelines: HashMap::new(),
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
            crate::engine_warn!("galaxy3d::ResourceManager", "Texture '{}' already exists", name);
            return Err(Error::BackendError(format!(
                "Texture '{}' already exists in ResourceManager", name
            )));
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
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Texture '{}' not found", texture_name);
                Error::BackendError(format!(
                    "Texture '{}' not found in ResourceManager", texture_name
                ))
            })?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate texture '{}': other references exist", texture_name);
                Error::BackendError(format!(
                    "Cannot mutate texture '{}': other references exist", texture_name
                ))
            })?;

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
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Texture '{}' not found", texture_name);
                Error::BackendError(format!(
                    "Texture '{}' not found in ResourceManager", texture_name
                ))
            })?;

        let texture = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate texture '{}': other references exist", texture_name);
                Error::BackendError(format!(
                    "Cannot mutate texture '{}': other references exist", texture_name
                ))
            })?;

        texture.add_region(layer_name, desc)
    }

    // ===== MESH CREATION =====

    /// Create a mesh resource and register it
    ///
    /// Internally creates the GPU vertex and index buffers from the provided data.
    /// Vertex and index counts are computed automatically from data length and layout.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this mesh resource (group name)
    /// * `desc` - Mesh description with renderer, vertex/index data and entries
    ///
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<Arc<Mesh>> {
        if self.meshes.contains_key(&name) {
            crate::engine_warn!("galaxy3d::ResourceManager", "Mesh '{}' already exists", name);
            return Err(Error::BackendError(format!(
                "Mesh '{}' already exists in ResourceManager", name
            )));
        }

        let mesh = Mesh::from_desc(desc)?;
        let entry_count = mesh.mesh_entry_count();
        let total_vertex_count = mesh.total_vertex_count();
        let total_index_count = mesh.total_index_count();
        let mesh_arc = Arc::new(mesh);
        self.meshes.insert(name.clone(), Arc::clone(&mesh_arc));

        crate::engine_info!("galaxy3d::ResourceManager",
            "Created Mesh resource '{}' ({} vertices, {} indices, {} entries)",
            name, total_vertex_count, total_index_count, entry_count);

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

    // ===== MESH MODIFICATION =====

    /// Add a mesh entry to an existing mesh resource
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the mesh Arc exist.
    ///
    /// # Returns
    ///
    /// The id (index) of the newly created mesh entry.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The mesh does not exist
    /// - Other Arc references prevent mutable access
    /// - A mesh entry with the same name already exists
    /// - Submesh validation fails (offsets exceed buffer sizes)
    pub fn add_mesh_entry(&mut self, mesh_name: &str, desc: MeshEntryDesc) -> Result<usize> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Mesh '{}' not found", mesh_name);
                Error::BackendError(format!(
                    "Mesh '{}' not found in ResourceManager", mesh_name
                ))
            })?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate Mesh '{}': other references exist", mesh_name);
                Error::BackendError(format!(
                    "Cannot mutate Mesh '{}': other references exist", mesh_name
                ))
            })?;

        mesh.add_mesh_entry(desc)
    }

    /// Add a LOD to an existing mesh entry
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the mesh Arc exist.
    ///
    /// # Returns
    ///
    /// The lod index of the newly created LOD.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The mesh does not exist
    /// - The mesh entry does not exist
    /// - Other Arc references prevent mutable access
    /// - Submesh validation fails
    pub fn add_mesh_lod(
        &mut self,
        mesh_name: &str,
        entry_id: usize,
        desc: MeshLODDesc,
    ) -> Result<usize> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Mesh '{}' not found", mesh_name);
                Error::BackendError(format!(
                    "Mesh '{}' not found in ResourceManager", mesh_name
                ))
            })?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate Mesh '{}': other references exist", mesh_name);
                Error::BackendError(format!(
                    "Cannot mutate Mesh '{}': other references exist", mesh_name
                ))
            })?;

        mesh.add_mesh_lod(entry_id, desc)
    }

    /// Add a submesh to an existing LOD
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the mesh Arc exist.
    ///
    /// # Returns
    ///
    /// The id (index) of the newly created submesh.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The mesh does not exist
    /// - The mesh entry does not exist
    /// - Other Arc references prevent mutable access
    /// - A submesh with the same name already exists in the LOD
    /// - Submesh validation fails (offsets exceed buffer sizes)
    pub fn add_submesh(
        &mut self,
        mesh_name: &str,
        entry_id: usize,
        lod_index: usize,
        desc: SubMeshDesc,
    ) -> Result<usize> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Mesh '{}' not found", mesh_name);
                Error::BackendError(format!(
                    "Mesh '{}' not found in ResourceManager", mesh_name
                ))
            })?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate Mesh '{}': other references exist", mesh_name);
                Error::BackendError(format!(
                    "Cannot mutate Mesh '{}': other references exist", mesh_name
                ))
            })?;

        mesh.add_submesh(entry_id, lod_index, desc)
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
            crate::engine_warn!("galaxy3d::ResourceManager", "Pipeline '{}' already exists", name);
            return Err(Error::BackendError(format!(
                "Pipeline '{}' already exists in ResourceManager", name
            )));
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
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Pipeline '{}' not found", pipeline_name);
                Error::BackendError(format!(
                    "Pipeline '{}' not found in ResourceManager", pipeline_name
                ))
            })?;

        let pipeline = Arc::get_mut(arc)
            .ok_or_else(|| {
                crate::engine_warn!("galaxy3d::ResourceManager", "Cannot mutate Pipeline '{}': other references exist", pipeline_name);
                Error::BackendError(format!(
                    "Cannot mutate Pipeline '{}': other references exist", pipeline_name
                ))
            })?;

        pipeline.add_variant(desc)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "resource_manager_tests.rs"]
mod tests;
