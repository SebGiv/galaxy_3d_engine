/// Resource-level mesh type.
///
/// A Mesh is a renderable combination of geometry and materials.
/// It associates a GeometryMesh from a Geometry resource with Materials
/// for each submesh at each LOD level.
///
/// Architecture:
/// - References a specific GeometryMesh within a Geometry (by key)
/// - Each LOD has material assignments (by key) for every submesh
/// - No pipeline stored at Mesh level: each Material has its own Pipeline reference
/// - SubMesh entries are ordered to match GeometryLOD submesh order (O(1) access)

use rustc_hash::{FxHashMap, FxHashSet};
use crate::error::Result;
use crate::{engine_bail, engine_err};
use crate::resource::resource_manager::{ResourceManager, GeometryKey, MaterialKey};

// ===== REFERENCE TYPES =====

/// Reference to a GeometryMesh by name or index
///
/// Used in descriptors to let the user choose the most convenient way
/// to reference a GeometryMesh. Resolved to a usize id at creation time.
pub enum GeometryMeshRef {
    Index(usize),
    Name(String),
}

/// Reference to a GeometrySubMesh by name or index
///
/// Used in descriptors to let the user choose the most convenient way
/// to reference a GeometrySubMesh. Resolved to a usize id at creation time.
pub enum GeometrySubMeshRef {
    Index(usize),
    Name(String),
}

// ===== SUBMESH =====

/// A submesh with its assigned material (resolved)
///
/// After creation, the submesh reference is resolved to a usize id.
/// Entries are ordered to match the GeometryLOD submesh order.
pub struct SubMesh {
    submesh_id: usize,
    material: MaterialKey,
}

// ===== MESH LOD =====

/// Material assignments for a specific LOD level (resolved)
///
/// Submeshes are ordered to match the corresponding GeometryLOD:
/// `submeshes[i]` is the material for `geometry_lod.submesh(i)`.
pub struct MeshLOD {
    submeshes: Vec<SubMesh>,
}

// ===== MESH =====

/// A renderable mesh: geometry + materials per submesh per LOD
///
/// Pure data resource. References a Geometry (by key, for shape) and Materials
/// (by key, for appearance) at each submesh of each LOD level.
pub struct Mesh {
    geometry: GeometryKey,
    geometry_mesh_id: usize,
    lods: Vec<MeshLOD>,
}

// ===== DESCRIPTORS =====

/// Mesh creation descriptor
pub struct MeshDesc {
    pub geometry: GeometryKey,
    pub geometry_mesh: GeometryMeshRef,
    pub lods: Vec<MeshLODDesc>,
}

/// LOD descriptor for mesh creation
pub struct MeshLODDesc {
    pub lod_index: usize,
    pub submeshes: Vec<SubMeshDesc>,
}

/// SubMesh descriptor (user-facing, accepts names or indices)
pub struct SubMeshDesc {
    pub submesh: GeometrySubMeshRef,
    pub material: MaterialKey,
}

// ===== MESH IMPLEMENTATION =====

impl Mesh {
    /// Create mesh from descriptor (internal use by ResourceManager)
    ///
    /// Resolves GeometryKey via the ResourceManager to validate LODs and submeshes.
    /// Stores only keys.
    pub(crate) fn from_desc(desc: MeshDesc, resource_manager: &ResourceManager) -> Result<Self> {
        let MeshDesc { geometry, geometry_mesh, lods } = desc;

        // ========== RESOLVE GEOMETRY ==========
        let geometry_arc = resource_manager.geometry(geometry)
            .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                "Geometry key not found in ResourceManager"))?;

        // ========== RESOLVE GEOMETRY MESH ==========
        let geometry_mesh_id = match &geometry_mesh {
            GeometryMeshRef::Index(i) => {
                if geometry_arc.mesh(*i).is_none() {
                    engine_bail!("galaxy3d::Mesh",
                        "GeometryMesh index {} does not exist", i);
                }
                *i
            }
            GeometryMeshRef::Name(name) => {
                geometry_arc.mesh_id(name)
                    .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                        "GeometryMesh '{}' not found", name))?
            }
        };

        let geom_mesh = geometry_arc.mesh(geometry_mesh_id).unwrap();
        let geom_lod_count = geom_mesh.lod_count();

        // ========== VALIDATE LOD INDICES ==========

        // No duplicate lod_index
        let mut seen_lod_indices = FxHashSet::default();
        for lod_desc in &lods {
            if !seen_lod_indices.insert(lod_desc.lod_index) {
                engine_bail!("galaxy3d::Mesh",
                    "Duplicate lod_index {}", lod_desc.lod_index);
            }
        }

        // Each lod_index must exist in GeometryMesh
        for lod_desc in &lods {
            if geom_mesh.lod(lod_desc.lod_index).is_none() {
                engine_bail!("galaxy3d::Mesh",
                    "LOD index {} does not exist in GeometryMesh", lod_desc.lod_index);
            }
        }

        // Complete LOD coverage
        if lods.len() != geom_lod_count {
            engine_bail!("galaxy3d::Mesh",
                "Incomplete LOD coverage: got {} LOD(s), GeometryMesh has {}",
                lods.len(), geom_lod_count);
        }

        // ========== BUILD MESH LODs ==========

        // Sort LOD descs by lod_index to match GeometryMesh order
        let mut sorted_lods: Vec<_> = lods.into_iter().collect();
        sorted_lods.sort_by_key(|d| d.lod_index);

        let mut result_lods = Vec::with_capacity(geom_lod_count);

        for lod_desc in sorted_lods {
            let geom_lod = geom_mesh.lod(lod_desc.lod_index).unwrap();
            let geom_submesh_count = geom_lod.submesh_count();

            // Resolve all submesh refs into a map: submesh_id → material key
            let mut submesh_map: FxHashMap<usize, MaterialKey> = FxHashMap::default();

            for submesh_desc in lod_desc.submeshes {
                let submesh_id = match &submesh_desc.submesh {
                    GeometrySubMeshRef::Index(i) => {
                        if geom_lod.submesh(*i).is_none() {
                            engine_bail!("galaxy3d::Mesh",
                                "LOD {}: GeometrySubMesh index {} does not exist",
                                lod_desc.lod_index, i);
                        }
                        *i
                    }
                    GeometrySubMeshRef::Name(name) => {
                        geom_lod.submesh_id(name)
                            .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                                "LOD {}: GeometrySubMesh '{}' not found",
                                lod_desc.lod_index, name))?
                    }
                };

                if submesh_map.contains_key(&submesh_id) {
                    engine_bail!("galaxy3d::Mesh",
                        "LOD {}: duplicate submesh assignment (resolved id {})",
                        lod_desc.lod_index, submesh_id);
                }

                submesh_map.insert(submesh_id, submesh_desc.material);
            }

            // Complete submesh coverage
            if submesh_map.len() != geom_submesh_count {
                engine_bail!("galaxy3d::Mesh",
                    "LOD {}: incomplete submesh coverage: got {}, GeometryLOD has {}",
                    lod_desc.lod_index, submesh_map.len(), geom_submesh_count);
            }

            // Build SubMesh vec in order matching GeometryLOD
            let mut submeshes = Vec::with_capacity(geom_submesh_count);
            for id in 0..geom_submesh_count {
                let material = submesh_map.remove(&id)
                    .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                        "LOD {}: submesh id {} has no material assigned",
                        lod_desc.lod_index, id))?;
                submeshes.push(SubMesh {
                    submesh_id: id,
                    material,
                });
            }

            result_lods.push(MeshLOD { submeshes });
        }

        Ok(Self {
            geometry,
            geometry_mesh_id,
            lods: result_lods,
        })
    }

    // ===== ACCESSORS =====

    /// Get the geometry key
    pub fn geometry(&self) -> GeometryKey {
        self.geometry
    }

    /// Get the resolved GeometryMesh id
    pub fn geometry_mesh_id(&self) -> usize {
        self.geometry_mesh_id
    }

    /// Get a MeshLOD by index
    pub fn lod(&self, index: usize) -> Option<&MeshLOD> {
        self.lods.get(index)
    }

    /// Get the number of LOD levels
    pub fn lod_count(&self) -> usize {
        self.lods.len()
    }
}

// ===== MESH LOD ACCESSORS =====

impl MeshLOD {
    /// Get a SubMesh by id (matches GeometryLOD submesh order)
    pub fn submesh(&self, id: usize) -> Option<&SubMesh> {
        self.submeshes.get(id)
    }

    /// Get the number of submeshes
    pub fn submesh_count(&self) -> usize {
        self.submeshes.len()
    }
}

// ===== SUBMESH ACCESSORS =====

impl SubMesh {
    /// Get the resolved GeometrySubMesh id
    pub fn submesh_id(&self) -> usize {
        self.submesh_id
    }

    /// Get the assigned material key
    pub fn material(&self) -> MaterialKey {
        self.material
    }
}

// ===== HELPER: NAME MAPPING =====

/// Build a MeshDesc by mapping submesh names to materials.
///
/// Iterates all LODs of the GeometryMesh and assigns materials
/// based on submesh names. All submeshes across all LODs with
/// the same name get the same material.
///
/// # Errors
///
/// Returns an error if:
/// - The geometry key is not found in the ResourceManager
/// - The geometry_mesh reference is invalid
/// - A submesh name is not found in the mapping
pub fn mesh_desc_from_name_mapping(
    geometry: GeometryKey,
    geometry_mesh: GeometryMeshRef,
    name_to_material: &FxHashMap<String, MaterialKey>,
    resource_manager: &ResourceManager,
) -> Result<MeshDesc> {
    let geometry_arc = resource_manager.geometry(geometry)
        .ok_or_else(|| engine_err!("galaxy3d::Mesh",
            "mesh_desc_from_name_mapping: Geometry key not found in ResourceManager"))?;

    // Resolve GeometryMeshRef to iterate LODs
    let mesh_id = match &geometry_mesh {
        GeometryMeshRef::Index(i) => {
            if geometry_arc.mesh(*i).is_none() {
                engine_bail!("galaxy3d::Mesh",
                    "mesh_desc_from_name_mapping: GeometryMesh index {} does not exist", i);
            }
            *i
        }
        GeometryMeshRef::Name(name) => {
            geometry_arc.mesh_id(name)
                .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                    "mesh_desc_from_name_mapping: GeometryMesh '{}' not found", name))?
        }
    };

    let geom_mesh = geometry_arc.mesh(mesh_id).unwrap();
    let mut lods = Vec::with_capacity(geom_mesh.lod_count());

    for lod_index in 0..geom_mesh.lod_count() {
        let geom_lod = geom_mesh.lod(lod_index).unwrap();
        let names = geom_lod.submesh_names();
        let mut submeshes = Vec::with_capacity(names.len());

        for name in names {
            let material = name_to_material.get(name)
                .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                    "mesh_desc_from_name_mapping: LOD {}: no material for submesh '{}'",
                    lod_index, name))?;

            submeshes.push(SubMeshDesc {
                submesh: GeometrySubMeshRef::Name(name.to_string()),
                material: *material,
            });
        }

        lods.push(MeshLODDesc {
            lod_index,
            submeshes,
        });
    }

    Ok(MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Index(mesh_id),
        lods,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
