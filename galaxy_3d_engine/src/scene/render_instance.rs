/// Render instance types for the scene system.
///
/// A RenderInstance is a flattened, GPU-ready representation of a resource::Mesh.
/// It extracts all graphics_device-level objects (buffers, pipelines, binding groups)
/// from the resource hierarchy into a flat structure optimized for rendering.

use std::sync::Arc;
use glam::{Vec3, Mat4};
use slotmap::new_key_type;
use crate::error::Result;
use crate::engine_err;
use crate::graphics_device::{
    self,
    Buffer,
    PrimitiveTopology,
};
use crate::resource::mesh::Mesh;
use crate::resource::resource_manager::{ResourceManager, MaterialKey};
use crate::utils::SlotAllocator;

// ===== SLOT MAP KEY =====

new_key_type! {
    /// Stable key for a RenderInstance within a Scene.
    ///
    /// Keys remain valid even after other instances are removed.
    /// A key becomes invalid only when its own instance is removed.
    pub struct RenderInstanceKey;
}

// ===== AABB =====

/// Axis-Aligned Bounding Box in local space
///
/// Used for frustum culling. Stored in local space and transformed
/// by the world_matrix at culling time.
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    /// Minimum corner (x, y, z)
    pub min: Vec3,
    /// Maximum corner (x, y, z)
    pub max: Vec3,
}

impl AABB {
    /// Transform this local-space AABB by a matrix, returning a new AABB.
    ///
    /// Uses the Arvo method: projects each matrix axis onto the AABB extents
    /// for an exact (tight) result without transforming all 8 corners.
    pub fn transformed(&self, matrix: &Mat4) -> AABB {
        let translation = matrix.col(3).truncate();
        let mut new_min = translation;
        let mut new_max = translation;

        for i in 0..3 {
            let axis = matrix.col(i).truncate();
            let a = axis * self.min[i];
            let b = axis * self.max[i];
            new_min += a.min(b);
            new_max += a.max(b);
        }

        AABB { min: new_min, max: new_max }
    }

    /// Test if this AABB fully contains another AABB.
    ///
    /// Returns `true` if `other` is entirely within `self`.
    /// Used by OctreeSceneIndex (Approach 1) to decide if an object
    /// fits entirely within a child node.
    pub fn contains(&self, other: &AABB) -> bool {
        self.min.x <= other.min.x && self.max.x >= other.max.x
        && self.min.y <= other.min.y && self.max.y >= other.max.y
        && self.min.z <= other.min.z && self.max.z >= other.max.z
    }

    /// Test if this AABB intersects (overlaps) another AABB.
    ///
    /// Returns `true` if the two AABBs overlap or touch.
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
        && self.min.y <= other.max.y && self.max.y >= other.min.y
        && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Closest point on this AABB to a given point.
    ///
    /// Returns the point itself if it lies inside the AABB.
    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        point.clamp(self.min, self.max)
    }

    /// Center of this AABB.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

// ===== FLAGS =====

/// Render instance flags (bitfield)
pub const FLAG_VISIBLE: u64        = 1 << 0;
/// Instance casts shadows
pub const FLAG_CAST_SHADOW: u64    = 1 << 1;
/// Instance receives shadows
pub const FLAG_RECEIVE_SHADOW: u64 = 1 << 2;
// Bits 3-63 reserved for future extensions

// ===== RENDER SUBMESH =====

/// A single drawable submesh within a RenderLOD.
///
/// Contains geometry offsets and a material key.
/// Pipeline, binding groups, render state, and material parameters
/// are all resolved from the Material (via ResourceManager) at draw time.
pub struct RenderSubMesh {
    /// Base vertex offset in the shared vertex buffer
    vertex_offset: u32,
    /// Number of vertices to draw
    vertex_count: u32,
    /// Base index offset in the shared index buffer
    index_offset: u32,
    /// Number of indices to draw (0 if non-indexed)
    index_count: u32,
    /// Primitive topology (TriangleList, LineList, etc.)
    topology: PrimitiveTopology,
    /// Material key (resolved via ResourceManager at draw time)
    material: MaterialKey,
    /// Unique slot index in the GPU scene SSBO
    draw_slot: u32,
    /// Material slot ID (cached from material at creation time)
    material_slot_id: u32,
}

// ===== RENDER LOD =====

/// A level of detail containing render submeshes.
///
/// LOD index 0 is the most detailed.
pub struct RenderLOD {
    /// Submeshes for this LOD level
    sub_meshes: Vec<RenderSubMesh>,
}

// ===== RENDER INSTANCE =====

/// A flattened, GPU-ready renderable object.
///
/// Created from a resource::Mesh, it contains all graphics_device-level references
/// needed to issue draw calls without traversing the resource hierarchy.
pub struct RenderInstance {
    /// Shared vertex buffer (from Geometry)
    vertex_buffer: Arc<dyn Buffer>,
    /// Shared index buffer (from Geometry, None for non-indexed)
    index_buffer: Option<Arc<dyn Buffer>>,
    /// Index type (U16 or U32), only meaningful if index_buffer is Some
    index_type: graphics_device::IndexType,
    /// LOD levels (index 0 = most detailed)
    lods: Vec<RenderLOD>,
    /// World transform matrix (pre-computed by game engine)
    world_matrix: Mat4,
    /// Bit flags (visibility, shadow casting, etc.)
    flags: u64,
    /// Axis-Aligned Bounding Box in local space
    bounding_box: AABB,
}

// ===== RENDER INSTANCE IMPLEMENTATION =====

impl RenderInstance {
    /// Create a RenderInstance from a resource::Mesh
    ///
    /// Resolves keys via the ResourceManager to extract geometry data.
    /// Stores MaterialKeys for draw-time resolution.
    pub(crate) fn from_mesh(
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        slot_allocator: &mut SlotAllocator,
        resource_manager: &ResourceManager,
    ) -> Result<Self> {
        let geometry = resource_manager.geometry(mesh.geometry())
            .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                "Geometry key not found in ResourceManager"))?;
        let geom_mesh = geometry.mesh(mesh.geometry_mesh_id())
            .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                "GeometryMesh id {} not found", mesh.geometry_mesh_id()))?;

        // Extract shared buffers and index type from Geometry
        let vertex_buffer = Arc::clone(geometry.vertex_buffer());
        let index_buffer = geometry.index_buffer().map(Arc::clone);
        let index_type = geometry.index_type();

        // Build RenderLODs from mesh LODs
        let mut lods = Vec::with_capacity(mesh.lod_count());

        for lod_idx in 0..mesh.lod_count() {
            let mesh_lod = mesh.lod(lod_idx)
                .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                    "LOD index {} out of range", lod_idx))?;

            let geom_lod = geom_mesh.lod(lod_idx)
                .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                    "GeometryLOD index {} out of range", lod_idx))?;

            let mut sub_meshes = Vec::with_capacity(mesh_lod.submesh_count());

            for sm_idx in 0..mesh_lod.submesh_count() {
                let submesh = mesh_lod.submesh(sm_idx)
                    .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                        "SubMesh index {} out of range in LOD {}", sm_idx, lod_idx))?;

                let geom_submesh = geom_lod.submesh(submesh.submesh_id())
                    .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                        "GeometrySubMesh id {} not found in LOD {}",
                        submesh.submesh_id(), lod_idx))?;

                let material_key = submesh.material();

                // Cache material slot ID at creation time
                let material_slot_id = resource_manager.material(material_key)
                    .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                        "Material key not found in ResourceManager"))?
                    .slot_id();

                sub_meshes.push(RenderSubMesh {
                    vertex_offset: geom_submesh.vertex_offset(),
                    vertex_count: geom_submesh.vertex_count(),
                    index_offset: geom_submesh.index_offset(),
                    index_count: geom_submesh.index_count(),
                    topology: geom_submesh.topology(),
                    material: material_key,
                    draw_slot: slot_allocator.alloc(),
                    material_slot_id,
                });
            }

            lods.push(RenderLOD { sub_meshes });
        }

        Ok(Self {
            vertex_buffer,
            index_buffer,
            index_type,
            lods,
            world_matrix,
            flags: FLAG_VISIBLE,
            bounding_box,
        })
    }

    // ===== ACCESSORS =====

    /// Get the shared vertex buffer
    pub fn vertex_buffer(&self) -> &Arc<dyn Buffer> {
        &self.vertex_buffer
    }

    /// Get the shared index buffer (None for non-indexed geometry)
    pub fn index_buffer(&self) -> Option<&Arc<dyn Buffer>> {
        self.index_buffer.as_ref()
    }

    /// Get the index type (U16 or U32, only meaningful if indexed)
    pub fn index_type(&self) -> graphics_device::IndexType {
        self.index_type
    }

    /// Get a LOD by index (0 = most detailed)
    pub fn lod(&self, index: usize) -> Option<&RenderLOD> {
        self.lods.get(index)
    }

    /// Get the number of LOD levels
    pub fn lod_count(&self) -> usize {
        self.lods.len()
    }

    /// Get the world transform matrix
    pub fn world_matrix(&self) -> &Mat4 {
        &self.world_matrix
    }

    /// Set the world transform matrix
    pub fn set_world_matrix(&mut self, matrix: Mat4) {
        self.world_matrix = matrix;
    }

    /// Get the flags
    pub fn flags(&self) -> u64 {
        self.flags
    }

    /// Set the flags
    pub fn set_flags(&mut self, flags: u64) {
        self.flags = flags;
    }

    /// Set visibility flag
    pub fn set_visible(&mut self, visible: bool) {
        if visible {
            self.flags |= FLAG_VISIBLE;
        } else {
            self.flags &= !FLAG_VISIBLE;
        }
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.flags & FLAG_VISIBLE != 0
    }

    /// Get the bounding box (local space)
    pub fn bounding_box(&self) -> &AABB {
        &self.bounding_box
    }

    /// Release all draw slots back to the allocator.
    ///
    /// Called automatically by Scene::remove_render_instance() and Scene::clear().
    pub(super) fn free_draw_slots(&self, slot_allocator: &mut SlotAllocator) {
        for lod in &self.lods {
            for submesh in &lod.sub_meshes {
                slot_allocator.free(submesh.draw_slot);
            }
        }
    }
}

// ===== RENDER LOD ACCESSORS =====

impl RenderLOD {
    /// Get a submesh by index
    pub fn sub_mesh(&self, index: usize) -> Option<&RenderSubMesh> {
        self.sub_meshes.get(index)
    }

    /// Get the number of submeshes
    pub fn sub_mesh_count(&self) -> usize {
        self.sub_meshes.len()
    }
}

// ===== RENDER SUBMESH ACCESSORS =====

impl RenderSubMesh {
    /// Get vertex offset
    pub fn vertex_offset(&self) -> u32 {
        self.vertex_offset
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Get index offset
    pub fn index_offset(&self) -> u32 {
        self.index_offset
    }

    /// Get index count
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get primitive topology
    pub fn topology(&self) -> PrimitiveTopology {
        self.topology
    }

    /// Get the material key
    pub fn material(&self) -> MaterialKey {
        self.material
    }

    /// Get the draw slot index (position in the GPU scene SSBO)
    pub fn draw_slot(&self) -> u32 {
        self.draw_slot
    }

    /// Get the material slot ID (position in the GPU material SSBO)
    pub fn material_slot_id(&self) -> u32 {
        self.material_slot_id
    }
}

#[cfg(test)]
#[path = "render_instance_tests.rs"]
mod tests;
