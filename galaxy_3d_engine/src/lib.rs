/*!
# Galaxy 3D Engine

Core traits and types for the Galaxy 3D rendering engine.

This crate provides the platform-agnostic API for 3D rendering using trait-based
dynamic polymorphism (similar to C++ virtual inheritance). Backend implementations
(Vulkan, Direct3D 12, etc.) are loaded at runtime via the plugin system.

## Architecture

- **Renderer**: Factory trait for creating GPU resources
- **RendererTexture**: Texture resource trait
- **RendererBuffer**: Buffer resource trait
- **RendererShader**: Shader module trait
- **RendererPipeline**: Graphics pipeline trait
- **RendererFrame**: Frame recording trait

Backend implementations provide concrete types that implement these traits.
*/

// Internal modules
mod error;
mod engine;
pub mod log;
pub mod renderer;
pub mod resource;
pub mod scene;

// Main galaxy3d namespace module
pub mod galaxy3d {
    // Error types
    pub use crate::error::{Error, Result};

    // Engine singleton
    pub use crate::engine::Engine;

    // Renderer factory trait
    pub use crate::renderer::Renderer;

    // Logging sub-module (types only, NOT macros)
    pub mod log {
        pub use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};
        // Note: engine_* macros are NOT re-exported here - they are internal only
    }

    // Render sub-module with all rendering types
    pub mod render {
        pub use crate::renderer::*;
    }

    // Resource sub-module
    pub mod resource {
        pub use crate::resource::*;
    }

    // Scene sub-module
    pub mod scene {
        pub use crate::scene::*;
    }
}

// Re-export math library at crate root
pub use glam;
