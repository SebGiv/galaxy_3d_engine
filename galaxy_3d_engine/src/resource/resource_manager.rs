//! Central resource manager for the engine.
//!
//! Stores and provides access to all engine resources (textures, meshes, etc.).
//! Resources will be added incrementally as the engine evolves.

use std::collections::HashMap;
use std::sync::Arc;
use crate::engine::Engine;
use crate::error::{Error, Result};
use crate::renderer::{TextureDesc, TextureData, TextureLayerData, BufferDesc, BufferUsage};
use crate::resource::texture::{
    Texture, SimpleTexture, AtlasTexture, ArrayTexture,
    AtlasRegion, AtlasRegionDesc, ArrayLayerDesc,
};
use crate::resource::mesh::{
    Mesh, MeshDesc, MeshEntryDesc, MeshLODDesc, SubMeshDesc,
};

pub struct ResourceManager {
    textures: HashMap<String, Arc<dyn Texture>>,
    meshes: HashMap<String, Arc<Mesh>>,
}

impl ResourceManager {
    /// Create a new empty resource manager
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            meshes: HashMap::new(),
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
    /// If any `ArrayLayerDesc` has `data`, the pixel data will be uploaded
    /// to the GPU at creation time.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for this texture resource
    /// * `desc` - GPU texture description (note: `desc.data` will be overwritten if layers have data)
    /// * `layers` - Initial layer mappings with optional pixel data (can be empty)
    pub fn create_array_texture(
        &mut self,
        name: String,
        mut desc: TextureDesc,
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

        // Build TextureData::Layers from ArrayLayerDesc entries that have data
        let layer_data: Vec<TextureLayerData> = layers
            .iter()
            .filter_map(|ld| {
                ld.data.as_ref().map(|d| TextureLayerData {
                    layer: ld.layer,
                    data: d.clone(),
                })
            })
            .collect();

        // If any layers have data, set desc.data to TextureData::Layers
        if !layer_data.is_empty() {
            desc.data = Some(TextureData::Layers(layer_data));
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
    /// If `data` is provided, uploads pixel data to the specified layer.
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
        data: Option<&[u8]>,
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
        texture.add_array_layer(layer_name, layer, data)
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
    /// * `desc` - Mesh description with vertex/index data and entries
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mesh = resource_manager.create_mesh(
    ///     "characters".to_string(),
    ///     MeshDesc {
    ///         vertex_data: vertex_bytes,
    ///         index_data: Some(index_bytes),
    ///         vertex_layout: layout,
    ///         index_type: IndexType::U16,
    ///         meshes: vec![
    ///             MeshEntryDesc {
    ///                 name: "hero".to_string(),
    ///                 lods: vec![
    ///                     MeshLODDesc {
    ///                         lod_index: 0,
    ///                         submeshes: vec![
    ///                             SubMeshDesc {
    ///                                 name: "body".to_string(),
    ///                                 vertex_offset: 0,
    ///                                 vertex_count: 5000,
    ///                                 index_offset: 0,
    ///                                 index_count: 15000,
    ///                                 topology: PrimitiveTopology::TriangleList,
    ///                             },
    ///                         ],
    ///                     },
    ///                 ],
    ///             },
    ///         ],
    ///     },
    /// )?;
    /// ```
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<Arc<Mesh>> {
        if self.meshes.contains_key(&name) {
            return Err(Error::BackendError(format!(
                "Mesh '{}' already exists in ResourceManager", name
            )));
        }

        // Calculate stride from vertex layout
        let stride = desc.vertex_layout.bindings.first()
            .map(|b| b.stride as usize)
            .unwrap_or(0);

        if stride == 0 {
            return Err(Error::BackendError(
                "Vertex layout has no bindings or stride is 0".to_string()
            ));
        }

        // Validate vertex data size
        if desc.vertex_data.len() % stride != 0 {
            return Err(Error::BackendError(format!(
                "Vertex data size {} is not a multiple of stride {}",
                desc.vertex_data.len(), stride
            )));
        }

        let total_vertex_count = (desc.vertex_data.len() / stride) as u32;

        // Validate index data size (if indexed)
        let index_size = desc.index_type.size_bytes() as usize;
        let total_index_count = if let Some(ref index_data) = desc.index_data {
            if index_data.len() % index_size != 0 {
                return Err(Error::BackendError(format!(
                    "Index data size {} is not a multiple of index type size {}",
                    index_data.len(), index_size
                )));
            }
            (index_data.len() / index_size) as u32
        } else {
            0
        };

        let renderer_arc = Engine::renderer()?;

        // Create vertex buffer and upload data
        let vertex_buffer;
        {
            let mut renderer = renderer_arc.lock()
                .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

            vertex_buffer = renderer.create_buffer(BufferDesc {
                size: desc.vertex_data.len() as u64,
                usage: BufferUsage::Vertex,
            })?;

            vertex_buffer.update(0, &desc.vertex_data)?;
        }

        // Create index buffer (if indexed) and upload data
        let index_buffer = if let Some(ref index_data) = desc.index_data {
            let mut renderer = renderer_arc.lock()
                .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

            let buffer = renderer.create_buffer(BufferDesc {
                size: index_data.len() as u64,
                usage: BufferUsage::Index,
            })?;

            buffer.update(0, index_data)?;
            Some(buffer)
        } else {
            None
        };

        // Create internal Mesh struct
        let mut mesh = Mesh::new(
            name.clone(),
            renderer_arc,
            vertex_buffer,
            index_buffer,
            desc.vertex_layout,
            desc.index_type,
            total_vertex_count,
            total_index_count,
        );

        // Add initial mesh entries (validation happens in add_mesh_entry)
        for entry_desc in desc.meshes {
            mesh.add_mesh_entry(entry_desc)?;
        }

        let entry_count = mesh.mesh_entry_count();
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
    /// # Errors
    ///
    /// Returns an error if:
    /// - The mesh does not exist
    /// - Other Arc references prevent mutable access
    /// - A mesh entry with the same name already exists
    /// - Submesh validation fails (offsets exceed buffer sizes)
    pub fn add_mesh_entry(&mut self, mesh_name: &str, desc: MeshEntryDesc) -> Result<()> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| Error::BackendError(format!(
                "Mesh '{}' not found in ResourceManager", mesh_name
            )))?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| Error::BackendError(format!(
                "Cannot mutate Mesh '{}': other references exist", mesh_name
            )))?;

        mesh.add_mesh_entry(desc)
    }

    /// Add a LOD to an existing mesh entry
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the mesh Arc exist.
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
        entry_name: &str,
        desc: MeshLODDesc,
    ) -> Result<()> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| Error::BackendError(format!(
                "Mesh '{}' not found in ResourceManager", mesh_name
            )))?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| Error::BackendError(format!(
                "Cannot mutate Mesh '{}': other references exist", mesh_name
            )))?;

        mesh.add_mesh_lod(entry_name, desc)
    }

    /// Add a submesh to an existing LOD
    ///
    /// Uses `Arc::get_mut` for safe mutable access. This will fail if other
    /// references to the mesh Arc exist.
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
        entry_name: &str,
        lod_index: usize,
        desc: SubMeshDesc,
    ) -> Result<()> {
        let arc = self.meshes.get_mut(mesh_name)
            .ok_or_else(|| Error::BackendError(format!(
                "Mesh '{}' not found in ResourceManager", mesh_name
            )))?;

        let mesh = Arc::get_mut(arc)
            .ok_or_else(|| Error::BackendError(format!(
                "Cannot mutate Mesh '{}': other references exist", mesh_name
            )))?;

        mesh.add_submesh(entry_name, lod_index, desc)
    }
}
