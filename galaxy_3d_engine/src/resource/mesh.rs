//! Resource-level mesh types.
//!
//! A `Mesh` groups multiple mesh entries sharing the same GPU buffers.
//!
//! # Hierarchy
//!
//! - **Mesh**: Container with shared vertex/index buffers and vertex layout
//! - **MeshEntry**: A named mesh (e.g., "hero", "enemy")
//! - **MeshLOD**: A level of detail (index 0 = most detailed)
//! - **SubMesh**: A drawable region with topology and buffer offsets
//!
//! # Example
//!
//! ```text
//! Mesh "characters"
//! ├── vertex_buffer (shared)
//! ├── index_buffer (shared, optional)
//! └── meshes
//!     ├── "hero" → MeshEntry
//!     │   └── lods[0] → MeshLOD
//!     │       ├── "body" → SubMesh
//!     │       └── "armor" → SubMesh
//!     └── "enemy" → MeshEntry
//!         └── ...
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::error::{Error, Result};
use crate::renderer::{
    Buffer,
    Renderer,
    VertexLayout,
    IndexType,
    PrimitiveTopology,
};

// ============================================================================
// SUBMESH
// ============================================================================

/// A drawable region within shared buffers.
///
/// Represents the smallest unit of geometry that can be drawn.
/// Contains all parameters needed for a draw call.
///
/// Note: The renderer is accessed via the parent `Mesh`, not stored here.
pub struct SubMesh {
    /// First vertex index (base vertex for indexed draw)
    vertex_offset: u32,
    /// Number of vertices
    vertex_count: u32,

    /// First index in index buffer (ignored if mesh is non-indexed)
    index_offset: u32,
    /// Number of indices (ignored if mesh is non-indexed)
    index_count: u32,

    /// Primitive topology for this submesh
    topology: PrimitiveTopology,
}

impl SubMesh {
    /// Get the vertex offset (base vertex)
    pub fn vertex_offset(&self) -> u32 {
        self.vertex_offset
    }

    /// Get the vertex count
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Get the index offset (only meaningful if mesh is indexed)
    pub fn index_offset(&self) -> u32 {
        self.index_offset
    }

    /// Get the index count (only meaningful if mesh is indexed)
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get the primitive topology
    pub fn topology(&self) -> PrimitiveTopology {
        self.topology
    }
}

// ============================================================================
// MESH LOD
// ============================================================================

/// A level of detail containing submeshes.
///
/// LOD index 0 is the most detailed, higher indices are less detailed.
/// Different LODs may have different submeshes (e.g., cape removed in LOD2).
pub struct MeshLOD {
    /// SubMeshes stored by index (id)
    submeshes: Vec<SubMesh>,
    /// Name to id (index) mapping
    submesh_names: HashMap<String, usize>,
}

impl MeshLOD {
    /// Create a new empty LOD
    fn new() -> Self {
        Self {
            submeshes: Vec::new(),
            submesh_names: HashMap::new(),
        }
    }

    /// Get a submesh by id (index)
    pub fn submesh(&self, id: usize) -> Option<&SubMesh> {
        self.submeshes.get(id)
    }

    /// Get submesh id by name
    pub fn submesh_id(&self, name: &str) -> Option<usize> {
        self.submesh_names.get(name).copied()
    }

    /// Get a submesh by name (convenience: name -> id -> submesh)
    pub fn submesh_by_name(&self, name: &str) -> Option<&SubMesh> {
        self.submesh_names.get(name).and_then(|&id| self.submeshes.get(id))
    }

    /// Get all submesh names
    pub fn submesh_names(&self) -> Vec<&str> {
        self.submesh_names.keys().map(|k| k.as_str()).collect()
    }

    /// Get the number of submeshes
    pub fn submesh_count(&self) -> usize {
        self.submeshes.len()
    }

    /// Iterate over all submeshes with their names
    pub fn submeshes(&self) -> impl Iterator<Item = (&str, &SubMesh)> {
        self.submesh_names.iter().filter_map(|(name, &id)| {
            self.submeshes.get(id).map(|submesh| (name.as_str(), submesh))
        })
    }

    /// Add a submesh (internal), returns id
    fn add_submesh_internal(&mut self, name: String, submesh: SubMesh) -> usize {
        let id = self.submeshes.len();
        self.submeshes.push(submesh);
        self.submesh_names.insert(name, id);
        id
    }

    /// Check if a submesh exists
    fn contains_submesh(&self, name: &str) -> bool {
        self.submesh_names.contains_key(name)
    }
}

// ============================================================================
// MESH ENTRY
// ============================================================================

/// A named mesh within a Mesh group.
///
/// Contains multiple LOD levels, each with potentially different submeshes.
pub struct MeshEntry {
    /// LOD levels (index 0 = most detailed)
    lods: Vec<MeshLOD>,
}

impl MeshEntry {
    /// Create a new mesh entry with empty LODs
    fn new() -> Self {
        Self { lods: Vec::new() }
    }

    /// Get a LOD by index (0 = most detailed)
    pub fn lod(&self, index: usize) -> Option<&MeshLOD> {
        self.lods.get(index)
    }

    /// Get mutable LOD by index
    fn lod_mut(&mut self, index: usize) -> Option<&mut MeshLOD> {
        self.lods.get_mut(index)
    }

    /// Get the number of LOD levels
    pub fn lod_count(&self) -> usize {
        self.lods.len()
    }

    /// Ensure LOD level exists, creating empty LODs if needed
    fn ensure_lod(&mut self, lod_index: usize) {
        while self.lods.len() <= lod_index {
            self.lods.push(MeshLOD::new());
        }
    }
}

// ============================================================================
// MESH (GROUP)
// ============================================================================

/// A mesh resource containing shared GPU buffers and multiple mesh entries.
///
/// # Hierarchy
///
/// ```text
/// Mesh (group)
/// ├── vertex_buffer (shared)
/// ├── index_buffer (shared, optional)
/// ├── vertex_layout (shared)
/// └── mesh_entries
///     ├── 0: "hero" → MeshEntry
///     │   └── lods[0..N] → MeshLOD
///     │       └── submeshes[0..M] → SubMesh
///     └── 1: "enemy" → MeshEntry
///         └── ...
/// ```
///
/// All submeshes share the same vertex and index buffers.
pub struct Mesh {
    /// Group name for ResourceManager lookup
    name: String,

    /// Reference to the renderer
    renderer: Arc<Mutex<dyn Renderer>>,

    /// Shared vertex buffer (interleaved vertex data)
    vertex_buffer: Arc<dyn Buffer>,

    /// Shared index buffer (None for non-indexed meshes)
    index_buffer: Option<Arc<dyn Buffer>>,

    /// Vertex layout description (shared by all submeshes)
    vertex_layout: VertexLayout,

    /// Index type (U16 or U32), ignored if no index buffer
    index_type: IndexType,

    /// Total vertex count in the buffer
    total_vertex_count: u32,

    /// Total index count in the buffer (0 if non-indexed)
    total_index_count: u32,

    /// Mesh entries stored by index (id)
    mesh_entries: Vec<MeshEntry>,

    /// Name to id (index) mapping
    entry_names: HashMap<String, usize>,
}

impl Mesh {
    /// Create a new Mesh (internal, used by ResourceManager)
    pub(crate) fn new(
        name: String,
        renderer: Arc<Mutex<dyn Renderer>>,
        vertex_buffer: Arc<dyn Buffer>,
        index_buffer: Option<Arc<dyn Buffer>>,
        vertex_layout: VertexLayout,
        index_type: IndexType,
        total_vertex_count: u32,
        total_index_count: u32,
    ) -> Self {
        Self {
            name,
            renderer,
            vertex_buffer,
            index_buffer,
            vertex_layout,
            index_type,
            total_vertex_count,
            total_index_count,
            mesh_entries: Vec::new(),
            entry_names: HashMap::new(),
        }
    }

    // ===== ACCESSORS =====

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the renderer reference
    pub fn renderer(&self) -> &Arc<Mutex<dyn Renderer>> {
        &self.renderer
    }

    /// Get the shared vertex buffer
    pub fn vertex_buffer(&self) -> &Arc<dyn Buffer> {
        &self.vertex_buffer
    }

    /// Get the shared index buffer (if indexed)
    pub fn index_buffer(&self) -> Option<&Arc<dyn Buffer>> {
        self.index_buffer.as_ref()
    }

    /// Check if this mesh uses indexed drawing
    pub fn is_indexed(&self) -> bool {
        self.index_buffer.is_some()
    }

    /// Get the vertex layout
    pub fn vertex_layout(&self) -> &VertexLayout {
        &self.vertex_layout
    }

    /// Get the index type (only meaningful if indexed)
    pub fn index_type(&self) -> IndexType {
        self.index_type
    }

    /// Get total vertex count
    pub fn total_vertex_count(&self) -> u32 {
        self.total_vertex_count
    }

    /// Get total index count (0 if non-indexed)
    pub fn total_index_count(&self) -> u32 {
        self.total_index_count
    }

    /// Get a mesh entry by id (index)
    pub fn mesh_entry(&self, id: usize) -> Option<&MeshEntry> {
        self.mesh_entries.get(id)
    }

    /// Get mutable mesh entry by id (internal)
    fn mesh_entry_mut(&mut self, id: usize) -> Option<&mut MeshEntry> {
        self.mesh_entries.get_mut(id)
    }

    /// Get mesh entry id by name
    pub fn mesh_entry_id(&self, name: &str) -> Option<usize> {
        self.entry_names.get(name).copied()
    }

    /// Get a mesh entry by name (convenience: name -> id -> entry)
    pub fn mesh_entry_by_name(&self, name: &str) -> Option<&MeshEntry> {
        self.entry_names.get(name).and_then(|&id| self.mesh_entries.get(id))
    }

    /// Get all mesh entry names
    pub fn mesh_entry_names(&self) -> Vec<&str> {
        self.entry_names.keys().map(|k| k.as_str()).collect()
    }

    /// Get the number of mesh entries
    pub fn mesh_entry_count(&self) -> usize {
        self.mesh_entries.len()
    }

    /// Get a submesh by full path: entry_id -> lod_index -> submesh_id
    pub fn submesh(&self, entry_id: usize, lod: usize, submesh_id: usize) -> Option<&SubMesh> {
        self.mesh_entries.get(entry_id)?
            .lod(lod)?
            .submesh(submesh_id)
    }

    /// Get a submesh by names: entry_name -> lod_index -> submesh_name
    pub fn submesh_by_name(&self, entry_name: &str, lod: usize, submesh_name: &str) -> Option<&SubMesh> {
        self.mesh_entry_by_name(entry_name)?
            .lod(lod)?
            .submesh_by_name(submesh_name)
    }

    // ===== MODIFICATION =====

    /// Add a mesh entry, returns its id (index)
    ///
    /// Validates all submesh offsets against buffer sizes.
    pub fn add_mesh_entry(&mut self, desc: MeshEntryDesc) -> Result<usize> {
        if self.entry_names.contains_key(&desc.name) {
            return Err(Error::BackendError(format!(
                "MeshEntry '{}' already exists in Mesh '{}'", desc.name, self.name
            )));
        }

        let mut entry = MeshEntry::new();

        for lod_desc in desc.lods {
            self.add_lod_to_entry(&mut entry, lod_desc)?;
        }

        let id = self.mesh_entries.len();
        self.mesh_entries.push(entry);
        self.entry_names.insert(desc.name, id);
        Ok(id)
    }

    /// Add a LOD to an existing mesh entry, returns the lod index
    pub fn add_mesh_lod(&mut self, entry_id: usize, desc: MeshLODDesc) -> Result<usize> {
        // Validate all submeshes first (before borrowing mesh_entries mutably)
        for submesh_desc in &desc.submeshes {
            self.validate_submesh_desc(submesh_desc)?;
        }

        let entry = self.mesh_entries.get_mut(entry_id)
            .ok_or_else(|| Error::BackendError(format!(
                "MeshEntry id {} not found in Mesh '{}'", entry_id, self.name
            )))?;

        entry.ensure_lod(desc.lod_index);

        let lod = entry.lod_mut(desc.lod_index)
            .expect("LOD should exist after ensure_lod");

        for submesh_desc in desc.submeshes {
            if lod.contains_submesh(&submesh_desc.name) {
                return Err(Error::BackendError(format!(
                    "SubMesh '{}' already exists in LOD {}", submesh_desc.name, desc.lod_index
                )));
            }

            let submesh = SubMesh {
                vertex_offset: submesh_desc.vertex_offset,
                vertex_count: submesh_desc.vertex_count,
                index_offset: submesh_desc.index_offset,
                index_count: submesh_desc.index_count,
                topology: submesh_desc.topology,
            };

            lod.add_submesh_internal(submesh_desc.name, submesh);
        }

        Ok(desc.lod_index)
    }

    /// Add a submesh to an existing LOD, returns the submesh id (index)
    pub fn add_submesh(
        &mut self,
        entry_id: usize,
        lod_index: usize,
        desc: SubMeshDesc,
    ) -> Result<usize> {
        // Validate offsets
        self.validate_submesh_desc(&desc)?;

        let entry = self.mesh_entries.get_mut(entry_id)
            .ok_or_else(|| Error::BackendError(format!(
                "MeshEntry id {} not found in Mesh '{}'", entry_id, self.name
            )))?;

        entry.ensure_lod(lod_index);

        let lod = entry.lod_mut(lod_index)
            .ok_or_else(|| Error::BackendError(format!(
                "LOD {} not found in MeshEntry id {}", lod_index, entry_id
            )))?;

        if lod.contains_submesh(&desc.name) {
            return Err(Error::BackendError(format!(
                "SubMesh '{}' already exists in LOD {} of MeshEntry id {}",
                desc.name, lod_index, entry_id
            )));
        }

        let submesh = SubMesh {
            vertex_offset: desc.vertex_offset,
            vertex_count: desc.vertex_count,
            index_offset: desc.index_offset,
            index_count: desc.index_count,
            topology: desc.topology,
        };

        let submesh_id = lod.add_submesh_internal(desc.name, submesh);
        Ok(submesh_id)
    }

    // ===== INTERNAL HELPERS =====

    /// Validate submesh descriptor against buffer sizes
    fn validate_submesh_desc(&self, desc: &SubMeshDesc) -> Result<()> {
        // Validate vertex range
        let vertex_end = desc.vertex_offset
            .checked_add(desc.vertex_count)
            .ok_or_else(|| Error::BackendError(
                format!("Vertex range overflow in submesh '{}'", desc.name)
            ))?;

        if vertex_end > self.total_vertex_count {
            return Err(Error::BackendError(format!(
                "SubMesh '{}' vertex range [{}, {}) exceeds total_vertex_count {}",
                desc.name, desc.vertex_offset, vertex_end, self.total_vertex_count
            )));
        }

        // Validate index range (if indexed)
        if self.is_indexed() {
            let index_end = desc.index_offset
                .checked_add(desc.index_count)
                .ok_or_else(|| Error::BackendError(
                    format!("Index range overflow in submesh '{}'", desc.name)
                ))?;

            if index_end > self.total_index_count {
                return Err(Error::BackendError(format!(
                    "SubMesh '{}' index range [{}, {}) exceeds total_index_count {}",
                    desc.name, desc.index_offset, index_end, self.total_index_count
                )));
            }
        }

        Ok(())
    }

    /// Add LOD to entry (used during initial creation)
    fn add_lod_to_entry(&self, entry: &mut MeshEntry, desc: MeshLODDesc) -> Result<()> {
        entry.ensure_lod(desc.lod_index);

        let lod = entry.lod_mut(desc.lod_index)
            .expect("LOD should exist after ensure_lod");

        for submesh_desc in desc.submeshes {
            self.validate_submesh_desc(&submesh_desc)?;

            if lod.contains_submesh(&submesh_desc.name) {
                return Err(Error::BackendError(format!(
                    "SubMesh '{}' already exists in LOD {}", submesh_desc.name, desc.lod_index
                )));
            }

            let submesh = SubMesh {
                vertex_offset: submesh_desc.vertex_offset,
                vertex_count: submesh_desc.vertex_count,
                index_offset: submesh_desc.index_offset,
                index_count: submesh_desc.index_count,
                topology: submesh_desc.topology,
            };

            lod.add_submesh_internal(submesh_desc.name, submesh);
        }

        Ok(())
    }
}

// ============================================================================
// DESCRIPTORS
// ============================================================================

/// Descriptor for creating a SubMesh
#[derive(Debug, Clone)]
pub struct SubMeshDesc {
    /// SubMesh name (unique within its LOD)
    pub name: String,
    /// First vertex offset
    pub vertex_offset: u32,
    /// Number of vertices
    pub vertex_count: u32,
    /// First index offset (ignored if mesh is non-indexed)
    pub index_offset: u32,
    /// Number of indices (ignored if mesh is non-indexed)
    pub index_count: u32,
    /// Primitive topology
    pub topology: PrimitiveTopology,
}

/// Descriptor for creating a MeshLOD
#[derive(Debug, Clone)]
pub struct MeshLODDesc {
    /// LOD index (0 = most detailed)
    pub lod_index: usize,
    /// SubMeshes at this LOD level
    pub submeshes: Vec<SubMeshDesc>,
}

/// Descriptor for creating a MeshEntry
#[derive(Debug, Clone)]
pub struct MeshEntryDesc {
    /// Mesh entry name (unique within the group)
    pub name: String,
    /// LOD levels
    pub lods: Vec<MeshLODDesc>,
}

/// Descriptor for creating a Mesh resource
///
/// The ResourceManager will create the GPU buffers from the provided data.
/// Vertex and index counts are computed automatically from data length and layout.
pub struct MeshDesc {
    /// Renderer to use for GPU buffer creation
    pub renderer: Arc<Mutex<dyn Renderer>>,
    /// Raw vertex data (bytes, interleaved according to vertex_layout)
    pub vertex_data: Vec<u8>,
    /// Raw index data (optional, None for non-indexed meshes)
    pub index_data: Option<Vec<u8>>,
    /// Vertex layout description (defines stride for vertex count calculation)
    pub vertex_layout: VertexLayout,
    /// Index type (U16 or U32, defines stride for index count calculation)
    pub index_type: IndexType,
    /// Initial mesh entries (can be empty, add later via add_mesh_entry)
    pub meshes: Vec<MeshEntryDesc>,
}
