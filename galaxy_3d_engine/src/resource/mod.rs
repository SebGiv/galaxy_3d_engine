/// Resource management module
///
/// Provides centralized storage and access to engine resources.

mod resource_manager;
pub mod texture;

pub use resource_manager::ResourceManager;
pub use texture::{
    Texture, SimpleTexture, AtlasTexture, ArrayTexture,
    AtlasRegion, AtlasRegionDesc, ArrayLayerDesc,
};
