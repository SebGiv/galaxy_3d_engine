//! Error types for the Galaxy3D engine
//!
//! This module defines the error types used throughout the engine,
//! including rendering, initialization, and resource management.

use std::fmt;

/// Result type for Galaxy3D engine operations
pub type Galaxy3dResult<T> = Result<T, Galaxy3dError>;

/// Galaxy3D engine errors
#[derive(Debug, Clone)]
pub enum Galaxy3dError {
    /// Backend-specific error (Vulkan, DirectX, etc.)
    BackendError(String),

    /// Out of GPU memory
    OutOfMemory,

    /// Invalid resource (texture, buffer, shader, etc.)
    InvalidResource(String),

    /// Initialization failed (engine, renderer, subsystems)
    InitializationFailed(String),
}

impl fmt::Display for Galaxy3dError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Galaxy3dError::BackendError(msg) => write!(f, "Backend error: {}", msg),
            Galaxy3dError::OutOfMemory => write!(f, "Out of GPU memory"),
            Galaxy3dError::InvalidResource(msg) => write!(f, "Invalid resource: {}", msg),
            Galaxy3dError::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
        }
    }
}

impl std::error::Error for Galaxy3dError {}
