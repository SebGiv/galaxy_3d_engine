/// Render instance types for the scene system.
///
/// A RenderInstance is a flattened, GPU-ready representation of a resource::Mesh.
/// It extracts all renderer-level objects (buffers, pipelines, descriptor sets)
/// from the resource hierarchy into a flat structure optimized for rendering.

use std::sync::Arc;
use glam::{Vec3, Mat4};
use slotmap::new_key_type;
use crate::error::Result;
use crate::engine_err;
use crate::renderer::{
    Buffer,
    Pipeline as RendererPipeline,
    PrimitiveTopology,
};
use crate::resource::material::ParamValue;
use crate::resource::mesh::Mesh;

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
/// Contains all data needed for a single draw call:
/// buffer offsets, pipeline bindings, descriptor sets, and material parameters.
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
    /// Renderer pipelines, one per pass of the selected variant
    passes: Vec<Arc<dyn RendererPipeline>>,
    /// Material parameters for push constants
    params: Vec<(String, ParamValue)>,
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
/// Created from a resource::Mesh, it contains all renderer-level references
/// needed to issue draw calls without traversing the resource hierarchy.
pub struct RenderInstance {
    /// Shared vertex buffer (from Geometry)
    vertex_buffer: Arc<dyn Buffer>,
    /// Shared index buffer (from Geometry, None for non-indexed)
    index_buffer: Option<Arc<dyn Buffer>>,
    /// LOD levels (index 0 = most detailed)
    lods: Vec<RenderLOD>,
    /// World transform matrix (pre-computed by game engine)
    world_matrix: Mat4,
    /// Bit flags (visibility, shadow casting, etc.)
    flags: u64,
    /// Axis-Aligned Bounding Box in local space
    bounding_box: AABB,
    /// Active pipeline variant index
    variant_index: usize,
}

// ===== RENDER INSTANCE IMPLEMENTATION =====

impl RenderInstance {
    /// Create a RenderInstance from a resource::Mesh
    ///
    /// Extracts all renderer-level objects from the resource hierarchy
    /// into a flat structure optimized for rendering.
    ///
    /// # Arguments
    ///
    /// * `mesh` - Source mesh resource
    /// * `world_matrix` - World transform matrix
    /// * `bounding_box` - AABB in local space
    /// * `variant_index` - Pipeline variant to use (0 = default)
    pub(crate) fn from_mesh(
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        variant_index: usize,
    ) -> Result<Self> {
        let geometry = mesh.geometry();
        let geom_mesh = mesh.geometry_mesh();

        // Extract shared buffers from Geometry
        let vertex_buffer = Arc::clone(geometry.vertex_buffer());
        let index_buffer = geometry.index_buffer().map(Arc::clone);

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

                let material = submesh.material();

                // Extract pipeline passes for the selected variant
                let pipeline = material.pipeline();
                let variant = pipeline.variant(variant_index as u32)
                    .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                        "Pipeline variant index {} out of range (pipeline has {} variants)",
                        variant_index, pipeline.variant_count()))?;

                let mut passes = Vec::with_capacity(variant.pass_count());
                for pass_idx in 0..variant.pass_count() {
                    let pass = variant.pass(pass_idx as u32)
                        .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                            "Pass index {} out of range in variant {}",
                            pass_idx, variant_index))?;
                    passes.push(Arc::clone(pass.renderer_pipeline()));
                }

                // Clone material parameters
                let mut params = Vec::with_capacity(material.param_count());
                for p_idx in 0..material.param_count() {
                    if let Some((name, value)) = material.param_at(p_idx) {
                        params.push((name.to_string(), value.clone()));
                    }
                }

                sub_meshes.push(RenderSubMesh {
                    vertex_offset: geom_submesh.vertex_offset(),
                    vertex_count: geom_submesh.vertex_count(),
                    index_offset: geom_submesh.index_offset(),
                    index_count: geom_submesh.index_count(),
                    topology: geom_submesh.topology(),
                    passes,
                    params,
                });
            }

            lods.push(RenderLOD { sub_meshes });
        }

        Ok(Self {
            vertex_buffer,
            index_buffer,
            lods,
            world_matrix,
            flags: FLAG_VISIBLE,
            bounding_box,
            variant_index,
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

    /// Get the active variant index
    pub fn variant_index(&self) -> usize {
        self.variant_index
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

    /// Get pipeline passes
    pub fn passes(&self) -> &[Arc<dyn RendererPipeline>] {
        &self.passes
    }

    /// Get material parameters
    pub fn params(&self) -> &[(String, ParamValue)] {
        &self.params
    }
}

#[cfg(test)]
#[path = "render_instance_tests.rs"]
mod tests;
