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

// Error handling module
pub mod error;

// Renderer module containing all rendering-related types and traits
pub mod renderer;

// Engine singleton manager module
mod engine;

// Re-export error types at crate root
pub use error::{Galaxy3dError, Galaxy3dResult};

// Re-export everything from renderer module
pub use renderer::*;

// Re-export Galaxy3dEngine singleton manager
pub use engine::Galaxy3dEngine;

// Re-export math library
pub use glam;
