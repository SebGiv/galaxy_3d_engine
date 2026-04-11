/// Resource-level mesh type.
///
/// A Mesh is a renderable combination of geometry and materials.
/// It associates a GeometryMesh from a Geometry resource with a Material
/// for each of its submeshes.
///
/// Architecture:
/// - References a specific GeometryMesh within a Geometry (by key + id)
/// - One Material per submesh — all LOD variants of a given submesh share
///   the same Material
/// - LOD selection is geometric (handled by the GeometrySubMesh) and
///   independent of the material
/// - SubMesh entries are ordered to match the parent GeometryMesh submesh
///   order (O(1) access)

use rustc_hash::FxHashMap;
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

// ===== MESH SUBMESH =====

/// A submesh with its assigned material (resolved).
///
/// Maps a GeometrySubMesh (by id) to a Material. All LOD variants of the
/// referenced GeometrySubMesh share this Material.
///
/// Entries in `Mesh::submeshes` are ordered to match the parent GeometryMesh
/// submesh order.
pub struct MeshSubMesh {
    submesh_id: usize,
    material: MaterialKey,
}

// ===== MESH =====

/// A renderable mesh: geometry + a material per submesh.
///
/// Pure data resource. References a Geometry (by key, for shape) and a
/// Material (by key, for appearance) for each submesh of the referenced
/// GeometryMesh. The geometric LODs are owned by the Geometry — the Mesh
/// does not store any LOD information.
pub struct Mesh {
    geometry: GeometryKey,
    geometry_mesh_id: usize,
    submeshes: Vec<MeshSubMesh>,
}

// ===== DESCRIPTORS =====

/// Mesh creation descriptor
pub struct MeshDesc {
    pub geometry: GeometryKey,
    pub geometry_mesh: GeometryMeshRef,
    pub submeshes: Vec<MeshSubMeshDesc>,
}

/// SubMesh descriptor (user-facing, accepts names or indices)
pub struct MeshSubMeshDesc {
    pub submesh: GeometrySubMeshRef,
    pub material: MaterialKey,
}

// ===== MESH IMPLEMENTATION =====

impl Mesh {
    /// Create mesh from descriptor (internal use by ResourceManager).
    ///
    /// Resolves the GeometryKey via the ResourceManager to validate the
    /// referenced GeometryMesh and its submeshes. Stores only keys.
    pub(crate) fn from_desc(desc: MeshDesc, resource_manager: &ResourceManager) -> Result<Self> {
        let MeshDesc { geometry, geometry_mesh, submeshes } = desc;

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
        let geom_submesh_count = geom_mesh.submesh_count();

        // ========== RESOLVE SUBMESHES ==========

        // Resolve all submesh refs into a map: submesh_id → material key
        let mut submesh_map: FxHashMap<usize, MaterialKey> = FxHashMap::default();

        for submesh_desc in submeshes {
            let submesh_id = match &submesh_desc.submesh {
                GeometrySubMeshRef::Index(i) => {
                    if geom_mesh.submesh(*i).is_none() {
                        engine_bail!("galaxy3d::Mesh",
                            "GeometrySubMesh index {} does not exist", i);
                    }
                    *i
                }
                GeometrySubMeshRef::Name(name) => {
                    geom_mesh.submesh_id(name)
                        .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                            "GeometrySubMesh '{}' not found", name))?
                }
            };

            if submesh_map.contains_key(&submesh_id) {
                engine_bail!("galaxy3d::Mesh",
                    "Duplicate submesh assignment (resolved id {})", submesh_id);
            }

            submesh_map.insert(submesh_id, submesh_desc.material);
        }

        // Complete submesh coverage
        if submesh_map.len() != geom_submesh_count {
            engine_bail!("galaxy3d::Mesh",
                "Incomplete submesh coverage: got {}, GeometryMesh has {}",
                submesh_map.len(), geom_submesh_count);
        }

        // Build SubMesh vec in order matching GeometryMesh
        let mut result_submeshes = Vec::with_capacity(geom_submesh_count);
        for id in 0..geom_submesh_count {
            let material = submesh_map.remove(&id)
                .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                    "Submesh id {} has no material assigned", id))?;
            result_submeshes.push(MeshSubMesh {
                submesh_id: id,
                material,
            });
        }

        Ok(Self {
            geometry,
            geometry_mesh_id,
            submeshes: result_submeshes,
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

    /// Get a MeshSubMesh by id (matches the parent GeometryMesh submesh order)
    pub fn submesh(&self, id: usize) -> Option<&MeshSubMesh> {
        self.submeshes.get(id)
    }

    /// Get the number of submeshes
    pub fn submesh_count(&self) -> usize {
        self.submeshes.len()
    }
}

// ===== MESH SUBMESH ACCESSORS =====

impl MeshSubMesh {
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
/// Iterates the submeshes of the referenced GeometryMesh and assigns
/// materials based on names.
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

    // Resolve GeometryMeshRef
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
    let names = geom_mesh.submesh_names();
    let mut submeshes = Vec::with_capacity(names.len());

    for name in names {
        let material = name_to_material.get(name)
            .ok_or_else(|| engine_err!("galaxy3d::Mesh",
                "mesh_desc_from_name_mapping: no material for submesh '{}'", name))?;

        submeshes.push(MeshSubMeshDesc {
            submesh: GeometrySubMeshRef::Name(name.to_string()),
            material: *material,
        });
    }

    Ok(MeshDesc {
        geometry,
        geometry_mesh: GeometryMeshRef::Index(mesh_id),
        submeshes,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
