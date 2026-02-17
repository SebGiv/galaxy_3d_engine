//! Resource management module
//!
//! Provides centralized storage and access to engine resources.

mod resource_manager;
pub mod texture;
pub mod geometry;
pub mod pipeline;
pub mod material;
pub mod mesh;

pub use resource_manager::ResourceManager;
pub use texture::{
    Texture, TextureLayer,
    AtlasRegion, AtlasRegionDesc,
    TextureDesc, LayerDesc,
};
pub use geometry::{
    Geometry, GeometryMesh, GeometryLOD, GeometrySubMesh,
    GeometryDesc, GeometryMeshDesc, GeometryLODDesc, GeometrySubMeshDesc,
};
pub use pipeline::{
    Pipeline, PipelineVariant, PipelinePass,
    PipelineDesc, PipelineVariantDesc, PipelinePassDesc,
};
pub use material::{
    Material, MaterialTextureSlot, MaterialParam,
    MaterialDesc, MaterialTextureSlotDesc,
    LayerRef, RegionRef, ParamValue,
};
pub use mesh::{
    Mesh, MeshLOD, SubMesh,
    MeshDesc, MeshLODDesc, SubMeshDesc,
    GeometryMeshRef, GeometrySubMeshRef,
    mesh_desc_from_name_mapping,
};
