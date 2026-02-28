//! Resource-level geometry types.
//!
//! A `Geometry` groups multiple meshes sharing the same GPU buffers.
//!
//! # Hierarchy
//!
//! - **Geometry**: Container with shared vertex/index buffers and vertex layout
//! - **GeometryMesh**: A named mesh (e.g., "hero", "enemy")
//! - **GeometryLOD**: A level of detail (index 0 = most detailed)
//! - **GeometrySubMesh**: A drawable region with topology and buffer offsets
//!
//! # Example
//!
//! ```text
//! Geometry "characters"
//! ├── vertex_buffer (shared)
//! ├── index_buffer (shared, optional)
//! └── meshes
//!     ├── "hero" → GeometryMesh
//!     │   └── lods[0] → GeometryLOD
//!     │       ├── "body" → GeometrySubMesh
//!     │       └── "armor" → GeometrySubMesh
//!     └── "enemy" → GeometryMesh
//!         └── ...
//! ```

use rustc_hash::FxHashMap;
use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::graphics_device;

// ============================================================================
// GEOMETRY SUBMESH
// ============================================================================

/// A drawable region within shared buffers.
///
/// Represents the smallest unit of geometry that can be drawn.
/// Contains all parameters needed for a draw call.
///
/// Note: The graphics_device is accessed via the parent `Geometry`, not stored here.
pub struct GeometrySubMesh {
    /// First vertex index (base vertex for indexed draw)
    vertex_offset: u32,
    /// Number of vertices
    vertex_count: u32,

    /// First index in index buffer (ignored if geometry is non-indexed)
    index_offset: u32,
    /// Number of indices (ignored if geometry is non-indexed)
    index_count: u32,

    /// Primitive topology for this submesh
    topology: graphics_device::PrimitiveTopology,
}

impl GeometrySubMesh {
    /// Get the vertex offset (base vertex)
    pub fn vertex_offset(&self) -> u32 {
        self.vertex_offset
    }

    /// Get the vertex count
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Get the index offset (only meaningful if geometry is indexed)
    pub fn index_offset(&self) -> u32 {
        self.index_offset
    }

    /// Get the index count (only meaningful if geometry is indexed)
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get the primitive topology
    pub fn topology(&self) -> graphics_device::PrimitiveTopology {
        self.topology
    }
}

// ============================================================================
// GEOMETRY LOD
// ============================================================================

/// A level of detail containing submeshes.
///
/// LOD index 0 is the most detailed, higher indices are less detailed.
/// Different LODs may have different submeshes (e.g., cape removed in LOD2).
pub struct GeometryLOD {
    /// SubMeshes stored by index (id)
    submeshes: Vec<GeometrySubMesh>,
    /// Name to id (index) mapping
    submesh_names: FxHashMap<String, usize>,
}

impl GeometryLOD {
    /// Create a new empty LOD
    fn new() -> Self {
        Self {
            submeshes: Vec::new(),
            submesh_names: FxHashMap::default(),
        }
    }

    /// Get a submesh by id (index)
    pub fn submesh(&self, id: usize) -> Option<&GeometrySubMesh> {
        self.submeshes.get(id)
    }

    /// Get submesh id by name
    pub fn submesh_id(&self, name: &str) -> Option<usize> {
        self.submesh_names.get(name).copied()
    }

    /// Get a submesh by name (convenience: name -> id -> submesh)
    pub fn submesh_by_name(&self, name: &str) -> Option<&GeometrySubMesh> {
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
    pub fn submeshes(&self) -> impl Iterator<Item = (&str, &GeometrySubMesh)> {
        self.submesh_names.iter().filter_map(|(name, &id)| {
            self.submeshes.get(id).map(|submesh| (name.as_str(), submesh))
        })
    }

    /// Add a submesh (internal), returns id
    fn add_submesh_internal(&mut self, name: String, submesh: GeometrySubMesh) -> usize {
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
// GEOMETRY MESH
// ============================================================================

/// A named mesh within a Geometry group.
///
/// Contains multiple LOD levels, each with potentially different submeshes.
pub struct GeometryMesh {
    /// LOD levels (index 0 = most detailed)
    lods: Vec<GeometryLOD>,
}

impl GeometryMesh {
    /// Create a new geometry mesh with empty LODs
    fn new() -> Self {
        Self { lods: Vec::new() }
    }

    /// Get a LOD by index (0 = most detailed)
    pub fn lod(&self, index: usize) -> Option<&GeometryLOD> {
        self.lods.get(index)
    }

    /// Get mutable LOD by index
    fn lod_mut(&mut self, index: usize) -> Option<&mut GeometryLOD> {
        self.lods.get_mut(index)
    }

    /// Get the number of LOD levels
    pub fn lod_count(&self) -> usize {
        self.lods.len()
    }

    /// Ensure LOD level exists, creating empty LODs if needed
    fn ensure_lod(&mut self, lod_index: usize) {
        while self.lods.len() <= lod_index {
            self.lods.push(GeometryLOD::new());
        }
    }
}

// ============================================================================
// GEOMETRY (GROUP)
// ============================================================================

/// A geometry resource containing shared GPU buffers and multiple meshes.
///
/// # Hierarchy
///
/// ```text
/// Geometry (group)
/// ├── vertex_buffer (shared)
/// ├── index_buffer (shared, optional)
/// ├── vertex_layout (shared)
/// └── meshes
///     ├── 0: "hero" → GeometryMesh
///     │   └── lods[0..N] → GeometryLOD
///     │       └── submeshes[0..M] → GeometrySubMesh
///     └── 1: "enemy" → GeometryMesh
///         └── ...
/// ```
///
/// All submeshes share the same vertex and index buffers.
pub struct Geometry {
    /// Group name for ResourceManager lookup
    name: String,

    /// Reference to the graphics device
    graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,

    /// Shared vertex buffer (interleaved vertex data)
    vertex_buffer: Arc<dyn graphics_device::Buffer>,

    /// Shared index buffer (None for non-indexed geometries)
    index_buffer: Option<Arc<dyn graphics_device::Buffer>>,

    /// Vertex layout description (shared by all submeshes)
    vertex_layout: graphics_device::VertexLayout,

    /// Index type (U16 or U32), ignored if no index buffer
    index_type: graphics_device::IndexType,

    /// Total vertex count in the buffer
    total_vertex_count: u32,

    /// Total index count in the buffer (0 if non-indexed)
    total_index_count: u32,

    /// Meshes stored by index (id)
    meshes: Vec<GeometryMesh>,

    /// Name to id (index) mapping
    mesh_names: FxHashMap<String, usize>,
}

impl Geometry {
    /// Create a new Geometry (internal, used by ResourceManager)
    pub(crate) fn new(
        name: String,
        graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
        vertex_buffer: Arc<dyn graphics_device::Buffer>,
        index_buffer: Option<Arc<dyn graphics_device::Buffer>>,
        vertex_layout: graphics_device::VertexLayout,
        index_type: graphics_device::IndexType,
        total_vertex_count: u32,
        total_index_count: u32,
    ) -> Self {
        Self {
            name,
            graphics_device,
            vertex_buffer,
            index_buffer,
            vertex_layout,
            index_type,
            total_vertex_count,
            total_index_count,
            meshes: Vec::new(),
            mesh_names: FxHashMap::default(),
        }
    }

    /// Create a Geometry from a descriptor
    ///
    /// Creates the GPU buffers and populates meshes from the descriptor.
    pub(crate) fn from_desc(desc: GeometryDesc) -> Result<Self> {
        // Get stride from first binding (binding 0)
        let vertex_stride = desc.vertex_layout.bindings
            .first()
            .map(|b| b.stride as usize)
            .unwrap_or(0);

        // Validate stride
        if vertex_stride == 0 {
            engine_bail!("galaxy3d::Geometry", "Vertex layout has no bindings or stride is 0");
        }

        // Validate vertex data size
        if desc.vertex_data.len() % vertex_stride != 0 {
            engine_bail!("galaxy3d::Geometry", "Vertex data size {} is not a multiple of stride {}",
                desc.vertex_data.len(), vertex_stride);
        }

        let vertex_count = desc.vertex_data.len() / vertex_stride;

        // Create vertex buffer
        let vertex_buffer = {
            let mut graphics_device = desc.graphics_device.lock().unwrap();
            let buffer = graphics_device.create_buffer(graphics_device::BufferDesc {
                size: desc.vertex_data.len() as u64,
                usage: graphics_device::BufferUsage::Vertex,
            })?;
            buffer.update(0, &desc.vertex_data)?;
            buffer
        };

        // Create index buffer (if provided)
        let (index_buffer, index_count) = if let Some(ref index_data) = desc.index_data {
            let index_size = desc.index_type.size_bytes() as usize;

            // Validate index data size
            if index_data.len() % index_size != 0 {
                engine_bail!("galaxy3d::Geometry", "Index data size {} is not a multiple of index type size {}",
                    index_data.len(), index_size);
            }

            let count = index_data.len() / index_size;
            let buffer = {
                let mut graphics_device = desc.graphics_device.lock().unwrap();
                let buf = graphics_device.create_buffer(graphics_device::BufferDesc {
                    size: index_data.len() as u64,
                    usage: graphics_device::BufferUsage::Index,
                })?;
                buf.update(0, index_data)?;
                buf
            };
            (Some(buffer), count as u32)
        } else {
            (None, 0)
        };

        // Build Geometry
        let mut geometry = Self::new(
            desc.name,
            desc.graphics_device,
            vertex_buffer,
            index_buffer,
            desc.vertex_layout,
            desc.index_type,
            vertex_count as u32,
            index_count,
        );

        // Add meshes from descriptor
        for mesh_desc in desc.meshes {
            geometry.add_mesh(mesh_desc)?;
        }

        Ok(geometry)
    }

    // ===== ACCESSORS =====

    /// Get the group name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the graphics device reference
    pub fn graphics_device(&self) -> &Arc<Mutex<dyn graphics_device::GraphicsDevice>> {
        &self.graphics_device
    }

    /// Get the shared vertex buffer
    pub fn vertex_buffer(&self) -> &Arc<dyn graphics_device::Buffer> {
        &self.vertex_buffer
    }

    /// Get the shared index buffer (if indexed)
    pub fn index_buffer(&self) -> Option<&Arc<dyn graphics_device::Buffer>> {
        self.index_buffer.as_ref()
    }

    /// Check if this geometry uses indexed drawing
    pub fn is_indexed(&self) -> bool {
        self.index_buffer.is_some()
    }

    /// Get the vertex layout
    pub fn vertex_layout(&self) -> &graphics_device::VertexLayout {
        &self.vertex_layout
    }

    /// Get the index type (only meaningful if indexed)
    pub fn index_type(&self) -> graphics_device::IndexType {
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

    /// Get a mesh by id (index)
    pub fn mesh(&self, id: usize) -> Option<&GeometryMesh> {
        self.meshes.get(id)
    }

    /// Get mutable mesh by id (internal)
    #[allow(dead_code)]
    fn mesh_mut(&mut self, id: usize) -> Option<&mut GeometryMesh> {
        self.meshes.get_mut(id)
    }

    /// Get mesh id by name
    pub fn mesh_id(&self, name: &str) -> Option<usize> {
        self.mesh_names.get(name).copied()
    }

    /// Get a mesh by name (convenience: name -> id -> mesh)
    pub fn mesh_by_name(&self, name: &str) -> Option<&GeometryMesh> {
        self.mesh_names.get(name).and_then(|&id| self.meshes.get(id))
    }

    /// Get all mesh names
    pub fn mesh_names(&self) -> Vec<&str> {
        self.mesh_names.keys().map(|k| k.as_str()).collect()
    }

    /// Get the number of meshes
    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Get a submesh by full path: mesh_id -> lod_index -> submesh_id
    pub fn submesh(&self, mesh_id: usize, lod: usize, submesh_id: usize) -> Option<&GeometrySubMesh> {
        self.meshes.get(mesh_id)?
            .lod(lod)?
            .submesh(submesh_id)
    }

    /// Get a submesh by names: mesh_name -> lod_index -> submesh_name
    pub fn submesh_by_name(&self, mesh_name: &str, lod: usize, submesh_name: &str) -> Option<&GeometrySubMesh> {
        self.mesh_by_name(mesh_name)?
            .lod(lod)?
            .submesh_by_name(submesh_name)
    }

    // ===== MODIFICATION =====

    /// Add a mesh, returns its id (index)
    ///
    /// Validates all submesh offsets against buffer sizes.
    pub fn add_mesh(&mut self, desc: GeometryMeshDesc) -> Result<usize> {
        if self.mesh_names.contains_key(&desc.name) {
            engine_bail!("galaxy3d::Geometry", "GeometryMesh '{}' already exists in Geometry '{}'", desc.name, self.name);
        }

        let mut mesh = GeometryMesh::new();

        for lod_desc in desc.lods {
            self.add_lod_to_mesh(&mut mesh, lod_desc)?;
        }

        let id = self.meshes.len();
        self.meshes.push(mesh);
        self.mesh_names.insert(desc.name, id);
        Ok(id)
    }

    /// Add a LOD to an existing mesh, returns the lod index
    pub fn add_lod(&mut self, mesh_id: usize, desc: GeometryLODDesc) -> Result<usize> {
        // Validate all submeshes first (before borrowing meshes mutably)
        for submesh_desc in &desc.submeshes {
            self.validate_submesh_desc(submesh_desc)?;
        }

        let mesh = self.meshes.get_mut(mesh_id)
            .ok_or_else(|| engine_err!("galaxy3d::Geometry", "GeometryMesh id {} not found in Geometry '{}'", mesh_id, self.name))?;

        mesh.ensure_lod(desc.lod_index);

        let lod = mesh.lod_mut(desc.lod_index)
            .expect("LOD should exist after ensure_lod");

        for submesh_desc in desc.submeshes {
            if lod.contains_submesh(&submesh_desc.name) {
                engine_bail!("galaxy3d::Geometry", "GeometrySubMesh '{}' already exists in LOD {}", submesh_desc.name, desc.lod_index);
            }

            let submesh = GeometrySubMesh {
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
        mesh_id: usize,
        lod_index: usize,
        desc: GeometrySubMeshDesc,
    ) -> Result<usize> {
        // Validate offsets
        self.validate_submesh_desc(&desc)?;

        let mesh = self.meshes.get_mut(mesh_id)
            .ok_or_else(|| engine_err!("galaxy3d::Geometry", "GeometryMesh id {} not found in Geometry '{}'", mesh_id, self.name))?;

        mesh.ensure_lod(lod_index);

        let lod = mesh.lod_mut(lod_index)
            .ok_or_else(|| engine_err!("galaxy3d::Geometry", "LOD {} not found in GeometryMesh id {}", lod_index, mesh_id))?;

        if lod.contains_submesh(&desc.name) {
            engine_bail!("galaxy3d::Geometry", "GeometrySubMesh '{}' already exists in LOD {} of GeometryMesh id {}",
                desc.name, lod_index, mesh_id);
        }

        let submesh = GeometrySubMesh {
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
    fn validate_submesh_desc(&self, desc: &GeometrySubMeshDesc) -> Result<()> {
        // Validate vertex range
        let vertex_end = desc.vertex_offset
            .checked_add(desc.vertex_count)
            .ok_or_else(|| engine_err!("galaxy3d::Geometry", "Vertex range overflow in submesh '{}'", desc.name))?;

        if vertex_end > self.total_vertex_count {
            engine_bail!("galaxy3d::Geometry", "GeometrySubMesh '{}' vertex range [{}, {}) exceeds total_vertex_count {}",
                desc.name, desc.vertex_offset, vertex_end, self.total_vertex_count);
        }

        // Validate index range (if indexed)
        if self.is_indexed() {
            let index_end = desc.index_offset
                .checked_add(desc.index_count)
                .ok_or_else(|| engine_err!("galaxy3d::Geometry", "Index range overflow in submesh '{}'", desc.name))?;

            if index_end > self.total_index_count {
                engine_bail!("galaxy3d::Geometry", "GeometrySubMesh '{}' index range [{}, {}) exceeds total_index_count {}",
                    desc.name, desc.index_offset, index_end, self.total_index_count);
            }
        }

        Ok(())
    }

    /// Add LOD to mesh (used during initial creation)
    fn add_lod_to_mesh(&self, mesh: &mut GeometryMesh, desc: GeometryLODDesc) -> Result<()> {
        mesh.ensure_lod(desc.lod_index);

        let lod = mesh.lod_mut(desc.lod_index)
            .expect("LOD should exist after ensure_lod");

        for submesh_desc in desc.submeshes {
            self.validate_submesh_desc(&submesh_desc)?;

            if lod.contains_submesh(&submesh_desc.name) {
                engine_bail!("galaxy3d::Geometry", "GeometrySubMesh '{}' already exists in LOD {}", submesh_desc.name, desc.lod_index);
            }

            let submesh = GeometrySubMesh {
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

/// Descriptor for creating a GeometrySubMesh
#[derive(Debug, Clone)]
pub struct GeometrySubMeshDesc {
    /// SubMesh name (unique within its LOD)
    pub name: String,
    /// First vertex offset
    pub vertex_offset: u32,
    /// Number of vertices
    pub vertex_count: u32,
    /// First index offset (ignored if geometry is non-indexed)
    pub index_offset: u32,
    /// Number of indices (ignored if geometry is non-indexed)
    pub index_count: u32,
    /// Primitive topology
    pub topology: graphics_device::PrimitiveTopology,
}

/// Descriptor for creating a GeometryLOD
#[derive(Debug, Clone)]
pub struct GeometryLODDesc {
    /// LOD index (0 = most detailed)
    pub lod_index: usize,
    /// SubMeshes at this LOD level
    pub submeshes: Vec<GeometrySubMeshDesc>,
}

/// Descriptor for creating a GeometryMesh
#[derive(Debug, Clone)]
pub struct GeometryMeshDesc {
    /// Mesh name (unique within the group)
    pub name: String,
    /// LOD levels
    pub lods: Vec<GeometryLODDesc>,
}

/// Descriptor for creating a Geometry resource
///
/// The ResourceManager will create the GPU buffers from the provided data.
/// Vertex and index counts are computed automatically from data length and layout.
pub struct GeometryDesc {
    /// Geometry group name
    pub name: String,
    /// Graphics device to use for GPU buffer creation
    pub graphics_device: Arc<Mutex<dyn graphics_device::GraphicsDevice>>,
    /// Raw vertex data (bytes, interleaved according to vertex_layout)
    pub vertex_data: Vec<u8>,
    /// Raw index data (optional, None for non-indexed geometries)
    pub index_data: Option<Vec<u8>>,
    /// Vertex layout description (defines stride for vertex count calculation)
    pub vertex_layout: graphics_device::VertexLayout,
    /// Index type (U16 or U32, defines stride for index count calculation)
    pub index_type: graphics_device::IndexType,
    /// Initial meshes (can be empty, add later via add_mesh)
    pub meshes: Vec<GeometryMeshDesc>,
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
