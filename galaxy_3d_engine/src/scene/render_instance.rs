/// Render instance types for the scene system.
///
/// A RenderInstance is a flattened, GPU-ready representation of a resource::Mesh.
/// It extracts all renderer-level objects (buffers, pipelines, binding groups)
/// from the resource hierarchy into a flat structure optimized for rendering.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use glam::{Vec3, Mat4};
use slotmap::new_key_type;
use crate::error::Result;
use crate::engine_err;
use crate::renderer::{
    self,
    Buffer,
    Pipeline as RendererPipeline,
    BindingGroup,
    BindingResource,
    BindingType,
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

// ===== RESOLVED PUSH CONSTANT =====

/// A resolved push constant value ready for GPU submission
///
/// Pre-resolved at RenderInstance creation time from MaterialParam
/// and pipeline reflection data.
pub struct ResolvedPushConstant {
    /// Byte offset in the push constant block
    offset: u32,
    /// Size in bytes
    size: u32,
    /// The parameter value
    value: ParamValue,
}

// ===== RENDER PASS =====

/// A single rendering pass with pre-resolved GPU bindings
///
/// Contains a pipeline and all bindings (binding groups + push constants)
/// resolved from the Material against the pipeline's reflection data.
pub struct RenderPass {
    /// The renderer pipeline for this pass
    pipeline: Arc<dyn RendererPipeline>,
    /// Binding groups (textures, UBOs) per set index
    binding_groups: Vec<Arc<dyn BindingGroup>>,
    /// Pre-resolved push constant values with byte offsets
    push_constants: Vec<ResolvedPushConstant>,
}

// ===== RENDER SUBMESH =====

/// A single drawable submesh within a RenderLOD.
///
/// Contains geometry offsets and one RenderPass per pipeline pass.
/// Each RenderPass holds its own pipeline, binding groups, and push constants.
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
    /// Rendering passes with pre-resolved bindings
    passes: Vec<RenderPass>,
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
    /// into a flat structure optimized for rendering. Resolves binding groups
    /// and push constants against pipeline reflection data.
    ///
    /// # Arguments
    ///
    /// * `mesh` - Source mesh resource
    /// * `world_matrix` - World transform matrix
    /// * `bounding_box` - AABB in local space
    /// * `variant_index` - Pipeline variant to use (0 = default)
    /// * `renderer` - Renderer for creating binding groups
    pub(crate) fn from_mesh(
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        variant_index: usize,
        renderer: &Arc<Mutex<dyn renderer::Renderer>>,
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

                    let renderer_pipeline = pass.renderer_pipeline();
                    let reflection = renderer_pipeline.reflection();

                    // ========== RESOLVE PUSH CONSTANTS ==========
                    let mut push_constants = Vec::new();
                    for pc_block in reflection.push_constants() {
                        for member in &pc_block.members {
                            if let Some(param) = material.param_by_name(&member.name) {
                                push_constants.push(ResolvedPushConstant {
                                    offset: member.offset,
                                    size: member.size.unwrap_or(0),
                                    value: param.value().clone(),
                                });
                            }
                        }
                    }

                    // ========== RESOLVE BINDING GROUPS ==========
                    // Group texture bindings by set index
                    let mut sets: BTreeMap<u32, Vec<(u32, BindingResource)>> = BTreeMap::new();

                    for binding_idx in 0..reflection.binding_count() {
                        let binding = reflection.binding(binding_idx).unwrap();

                        if binding.binding_type == BindingType::CombinedImageSampler {
                            if let Some(slot) = material.texture_slot_by_name(&binding.name) {
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

                    // Create binding groups for each set
                    let mut binding_groups = Vec::new();
                    let renderer_lock = renderer.lock().unwrap();
                    for (set_index, mut resources) in sets {
                        // Sort by binding index to match layout order
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
                    drop(renderer_lock);

                    passes.push(RenderPass {
                        pipeline: Arc::clone(renderer_pipeline),
                        binding_groups,
                        push_constants,
                    });
                }

                sub_meshes.push(RenderSubMesh {
                    vertex_offset: geom_submesh.vertex_offset(),
                    vertex_count: geom_submesh.vertex_count(),
                    index_offset: geom_submesh.index_offset(),
                    index_count: geom_submesh.index_count(),
                    topology: geom_submesh.topology(),
                    passes,
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

    /// Get rendering passes
    pub fn passes(&self) -> &[RenderPass] {
        &self.passes
    }
}

// ===== RENDER PASS ACCESSORS =====

impl RenderPass {
    /// Get the renderer pipeline
    pub fn pipeline(&self) -> &Arc<dyn RendererPipeline> {
        &self.pipeline
    }

    /// Get the binding groups
    pub fn binding_groups(&self) -> &[Arc<dyn BindingGroup>] {
        &self.binding_groups
    }

    /// Get the resolved push constants
    pub fn push_constants(&self) -> &[ResolvedPushConstant] {
        &self.push_constants
    }
}

// ===== RESOLVED PUSH CONSTANT ACCESSORS =====

impl ResolvedPushConstant {
    /// Get the byte offset in the push constant block
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Get the size in bytes
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get the parameter value
    pub fn value(&self) -> &ParamValue {
        &self.value
    }
}

#[cfg(test)]
#[path = "render_instance_tests.rs"]
mod tests;
