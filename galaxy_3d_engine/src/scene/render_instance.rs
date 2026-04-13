/// Render instance types for the scene system.
///
/// A RenderInstance is a thin scene-level reference to a resource::Geometry,
/// associated with materials, a transform, and per-instance render state.
/// It does NOT duplicate the geometry data — it stores a GeometryKey + mesh
/// id and queries the Geometry at draw time.

use glam::{Vec3, Mat4};
use slotmap::new_key_type;
use crate::error::Result;
use crate::engine_err;
use crate::resource::mesh::Mesh;
use crate::resource::resource_manager::{
    ResourceManager, GeometryKey, MaterialKey, ShaderKey, PipelineKey,
};
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
        let center = (self.min + self.max) * 0.5;
        let extents = (self.max - self.min) * 0.5;

        let new_center = matrix.transform_point3(center);

        let abs_x = matrix.x_axis.truncate().abs();
        let abs_y = matrix.y_axis.truncate().abs();
        let abs_z = matrix.z_axis.truncate().abs();

        let new_extents =
            abs_x * extents.x +
            abs_y * extents.y +
            abs_z * extents.z;

        AABB {
            min: new_center - new_extents,
            max: new_center + new_extents,
        }
    }

    /// Check if this AABB contains another AABB entirely
    pub fn contains(&self, other: &AABB) -> bool {
        other.min.x >= self.min.x && other.max.x <= self.max.x &&
        other.min.y >= self.min.y && other.max.y <= self.max.y &&
        other.min.z >= self.min.z && other.max.z <= self.max.z
    }

    /// Check if this AABB intersects another AABB
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Get the center of the AABB
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Return the point inside (or on the surface of) this AABB closest
    /// to the given point. Equivalent to clamping the point to the box.
    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        Vec3::new(
            point.x.clamp(self.min.x, self.max.x),
            point.y.clamp(self.min.y, self.max.y),
            point.z.clamp(self.min.z, self.max.z),
        )
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

// ===== VERTEX SHADER OVERRIDE =====

/// Override the default vertex shader for a specific (submesh, pass_type) pair.
///
/// Used in `RenderInstance::from_mesh` to assign a different vertex shader
/// to specific passes of specific submeshes, while keeping the default for
/// all other combinations.
pub struct VertexShaderOverride {
    /// Index of the submesh within the RenderInstance
    pub submesh: usize,
    /// Pass type to override (must be < 64)
    pub pass_type: u8,
    /// Vertex shader to use instead of the default
    pub vertex_shader: ShaderKey,
}

// ===== RENDER SUBMESH PASS =====

/// Per-pass pipeline data for a RenderSubMesh.
///
/// Each entry corresponds to one rendering pass of the submesh. The material
/// and material pass index are stored per-pass so that different passes of the
/// same submesh can reference different materials (e.g. a PBR material for
/// forward rendering and a simpler material for shadow casting).
pub struct RenderSubMeshPass {
    /// Material used for this pass
    material: MaterialKey,
    /// Index of the pass within the Material's `passes` vec
    material_pass_index: u8,
    /// Vertex shader used for pipeline resolution for this pass
    vertex_shader: ShaderKey,
    /// Cached pipeline key (resolved lazily on first draw of this pass)
    cached_pipeline_key: Option<PipelineKey>,
    /// PassInfo generation at the time of pipeline resolution
    cached_pass_info_gen: u64,
    /// Material generation at the time of pipeline resolution
    cached_material_gen: u64,
}

// ===== RENDER SUBMESH =====

/// Sentinel value in `pass_type_to_index` indicating the pass_type is not
/// present for this submesh.
const PASS_INDEX_NONE: u8 = 0xFF;

/// A single drawable submesh of a RenderInstance.
///
/// Each submesh has its own material and a set of rendering passes derived
/// from that material's `MaterialPass` list. The `pass_mask` bitmask
/// controls which passes are active for drawing (mutable at runtime), while
/// the `pass_type_to_index` table provides O(1) lookup from a pass_type to
/// the compact `passes` vec.
pub struct RenderSubMesh {
    /// Index of the corresponding GeometrySubMesh in the parent GeometryMesh.
    geometry_submesh_id: usize,
    /// Unique slot index in the GPU scene SSBO
    draw_slot: u32,
    /// Bitmask — bit N = 1 if pass_type N is active for drawing.
    /// Mutable at runtime: toggle passes on/off without rebuilding.
    pass_mask: u64,
    /// Static mapping: pass_type → index in `passes` (0xFF = not present).
    /// Immutable after construction.
    pass_type_to_index: [u8; 64],
    /// Compact array of per-pass data. One entry per `MaterialPass` of the
    /// assigned material. Indices match `pass_type_to_index` values.
    passes: Vec<RenderSubMeshPass>,
}

// ===== RENDER INSTANCE =====

/// A scene-level renderable object.
///
/// Stores a reference to a `resource::Geometry` (via GeometryKey + mesh id)
/// instead of duplicating its GPU data. Geometry data (buffers, layout,
/// submesh offsets, topology) is read from the Geometry at draw time.
pub struct RenderInstance {
    /// Reference to the underlying Geometry resource
    geometry: GeometryKey,
    /// Index of the GeometryMesh within the Geometry
    geometry_mesh_id: usize,
    /// Per-instance submeshes (one per GeometrySubMesh of the referenced
    /// GeometryMesh). Each submesh owns its own per-pass pipeline caches.
    sub_meshes: Vec<RenderSubMesh>,
    /// World transform matrix (pre-computed by game engine)
    world_matrix: Mat4,
    /// Bit flags (visibility, shadow casting, etc.)
    flags: u64,
    /// Axis-Aligned Bounding Box in local space
    bounding_box: AABB,
}

// ===== RENDER INSTANCE IMPLEMENTATION =====

impl RenderInstance {
    /// Create a RenderInstance from a resource::Mesh.
    ///
    /// Validates that the referenced Geometry, GeometryMesh, GeometrySubMeshes,
    /// and Materials exist via the ResourceManager. For each submesh, builds
    /// the per-pass pipeline data from the material's `MaterialPass` list.
    ///
    /// `vertex_shader` is the default vertex shader for all passes of all
    /// submeshes. `vertex_shader_overrides` can override the VS for specific
    /// (submesh index, pass_type) pairs.
    pub(crate) fn from_mesh(
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        vertex_shader: ShaderKey,
        vertex_shader_overrides: &[VertexShaderOverride],
        slot_allocator: &mut SlotAllocator,
        resource_manager: &ResourceManager,
    ) -> Result<Self> {
        // ===== Validation: geometry + mesh exist =====
        let geometry_arc = resource_manager.geometry(mesh.geometry())
            .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                "Geometry key not found in ResourceManager"))?;
        let geom_mesh = geometry_arc.mesh(mesh.geometry_mesh_id())
            .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                "GeometryMesh id {} not found", mesh.geometry_mesh_id()))?;

        // ===== Build RenderSubMeshes from mesh submeshes =====
        let mut sub_meshes = Vec::with_capacity(mesh.submesh_count());

        for sm_idx in 0..mesh.submesh_count() {
            let submesh = mesh.submesh(sm_idx)
                .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                    "SubMesh index {} out of range", sm_idx))?;

            // Validate geometry submesh
            if geom_mesh.submesh(submesh.submesh_id()).is_none() {
                return Err(engine_err!("galaxy3d::RenderInstance",
                    "GeometrySubMesh id {} not found in GeometryMesh",
                    submesh.submesh_id()));
            }

            let material_key = submesh.material();
            let material = resource_manager.material(material_key)
                .ok_or_else(|| engine_err!("galaxy3d::RenderInstance",
                    "Material key not found in ResourceManager"))?;

            // Build per-pass data from the material's passes
            let mut pass_mask: u64 = 0;
            let mut pass_type_to_index = [PASS_INDEX_NONE; 64];
            let mut passes = Vec::with_capacity(material.pass_count());

            for pass_idx in 0..material.pass_count() {
                let mat_pass = material.pass(pass_idx).unwrap();
                let pt = mat_pass.pass_type();

                // Lookup override for this (submesh, pass_type)
                let vs = vertex_shader_overrides.iter()
                    .find(|o| o.submesh == sm_idx && o.pass_type == pt)
                    .map(|o| o.vertex_shader)
                    .unwrap_or(vertex_shader);

                let local_idx = passes.len();
                passes.push(RenderSubMeshPass {
                    material: material_key,
                    material_pass_index: pass_idx as u8,
                    vertex_shader: vs,
                    cached_pipeline_key: None,
                    cached_pass_info_gen: 0,
                    cached_material_gen: 0,
                });

                pass_mask |= 1u64 << pt;
                pass_type_to_index[pt as usize] = local_idx as u8;
            }

            sub_meshes.push(RenderSubMesh {
                geometry_submesh_id: submesh.submesh_id(),
                draw_slot: slot_allocator.alloc(),
                pass_mask,
                pass_type_to_index,
                passes,
            });
        }

        Ok(Self {
            geometry: mesh.geometry(),
            geometry_mesh_id: mesh.geometry_mesh_id(),
            sub_meshes,
            world_matrix,
            flags: FLAG_VISIBLE,
            bounding_box,
        })
    }

    // ===== ACCESSORS =====

    /// Get the underlying Geometry key
    pub fn geometry(&self) -> GeometryKey {
        self.geometry
    }

    /// Get the GeometryMesh id within the Geometry
    pub fn geometry_mesh_id(&self) -> usize {
        self.geometry_mesh_id
    }

    /// Get a submesh by index
    pub fn sub_mesh(&self, index: usize) -> Option<&RenderSubMesh> {
        self.sub_meshes.get(index)
    }

    /// Get a mutable submesh by index.
    ///
    /// Used by the drawer to update the per-submesh pipeline cache after
    /// lazy resolution.
    pub fn sub_mesh_mut(&mut self, index: usize) -> Option<&mut RenderSubMesh> {
        self.sub_meshes.get_mut(index)
    }

    /// Get the number of submeshes
    pub fn sub_mesh_count(&self) -> usize {
        self.sub_meshes.len()
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
        for submesh in &self.sub_meshes {
            slot_allocator.free(submesh.draw_slot);
        }
    }
}

// ===== RENDER SUBMESH ACCESSORS =====

impl RenderSubMesh {
    /// Get the corresponding GeometrySubMesh id
    pub fn geometry_submesh_id(&self) -> usize {
        self.geometry_submesh_id
    }

    /// Get the draw slot index (position in the GPU scene SSBO)
    pub fn draw_slot(&self) -> u32 {
        self.draw_slot
    }

    // ===== PASS MASK (activation/deactivation) =====

    /// Get the pass activation bitmask
    pub fn pass_mask(&self) -> u64 {
        self.pass_mask
    }

    /// Set the entire pass activation bitmask
    pub fn set_pass_mask(&mut self, mask: u64) {
        self.pass_mask = mask;
    }

    /// Enable a pass_type for drawing
    pub fn enable_pass(&mut self, pass_type: u8) {
        self.pass_mask |= 1u64 << pass_type;
    }

    /// Disable a pass_type for drawing
    pub fn disable_pass(&mut self, pass_type: u8) {
        self.pass_mask &= !(1u64 << pass_type);
    }

    /// Check if a pass_type is active (enabled AND present)
    pub fn is_pass_active(&self, pass_type: u8) -> bool {
        self.pass_mask & (1u64 << pass_type) != 0
    }

    /// Check if a pass_type is present (regardless of activation)
    pub fn has_pass(&self, pass_type: u8) -> bool {
        self.pass_type_to_index[pass_type as usize] != PASS_INDEX_NONE
    }

    /// Get the local pass index for a given pass_type.
    /// Returns the index into the compact `passes` vec.
    /// Caller must ensure the pass_type is present (check `has_pass` first).
    pub fn pass_index_for_type(&self, pass_type: u8) -> u8 {
        self.pass_type_to_index[pass_type as usize]
    }

    // ===== PASS ACCESS =====

    /// Get a pass by pass_type (checks bitmask + lookup table).
    /// Returns None if the pass_type is not active or not present.
    pub fn pass(&self, pass_type: u8) -> Option<&RenderSubMeshPass> {
        if self.pass_mask & (1u64 << pass_type) == 0 {
            return None;
        }
        let idx = self.pass_type_to_index[pass_type as usize];
        if idx == PASS_INDEX_NONE { return None; }
        self.passes.get(idx as usize)
    }

    /// Get a mutable pass by pass_type (checks bitmask + lookup table).
    /// Returns None if the pass_type is not active or not present.
    pub fn pass_mut(&mut self, pass_type: u8) -> Option<&mut RenderSubMeshPass> {
        if self.pass_mask & (1u64 << pass_type) == 0 {
            return None;
        }
        let idx = self.pass_type_to_index[pass_type as usize];
        if idx == PASS_INDEX_NONE { return None; }
        self.passes.get_mut(idx as usize)
    }

    /// Get a pass by its local index in the compact `passes` vec.
    /// Does NOT check the bitmask — direct access for V1 drawer.
    pub fn pass_by_index(&self, index: usize) -> Option<&RenderSubMeshPass> {
        self.passes.get(index)
    }

    /// Get a mutable pass by its local index in the compact `passes` vec.
    /// Does NOT check the bitmask — direct access for V1 drawer.
    pub fn pass_by_index_mut(&mut self, index: usize) -> Option<&mut RenderSubMeshPass> {
        self.passes.get_mut(index)
    }

    /// Get the number of passes
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

// ===== RENDER SUBMESH PASS ACCESSORS =====

impl RenderSubMeshPass {
    /// Get the material key for this pass
    pub fn material(&self) -> MaterialKey {
        self.material
    }

    /// Get the index of the pass within the Material's passes vec
    pub fn material_pass_index(&self) -> usize {
        self.material_pass_index as usize
    }

    /// Get the vertex shader key for this pass
    pub fn vertex_shader(&self) -> ShaderKey {
        self.vertex_shader
    }

    /// Get the cached pipeline key (None if not yet resolved)
    pub fn cached_pipeline_key(&self) -> Option<PipelineKey> {
        self.cached_pipeline_key
    }

    /// Check if the cached pipeline is still valid for the given generations
    pub fn is_pipeline_valid(&self, pass_info_gen: u64, material_gen: u64) -> bool {
        self.cached_pipeline_key.is_some()
            && self.cached_pass_info_gen == pass_info_gen
            && self.cached_material_gen == material_gen
    }

    /// Cache a resolved pipeline key with the current generation counters
    pub fn set_cached_pipeline(&mut self, key: PipelineKey, pass_info_gen: u64, material_gen: u64) {
        self.cached_pipeline_key = Some(key);
        self.cached_pass_info_gen = pass_info_gen;
        self.cached_material_gen = material_gen;
    }
}

#[cfg(test)]
#[path = "render_instance_tests.rs"]
mod tests;
