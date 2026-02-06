//! Resource management module
//!
//! Provides centralized storage and access to engine resources.

mod resource_manager;
pub mod texture;
pub mod mesh;
pub mod pipeline;

pub use resource_manager::ResourceManager;
pub use texture::{
    Texture, TextureLayer,
    AtlasRegion, AtlasRegionDesc,
    TextureDesc, LayerDesc,
};
pub use mesh::{
    Mesh, MeshEntry, MeshLOD, SubMesh,
    MeshDesc, MeshEntryDesc, MeshLODDesc, SubMeshDesc,
};
pub use pipeline::{
    Pipeline, PipelineVariant,
    PipelineDesc, PipelineVariantDesc,
};
