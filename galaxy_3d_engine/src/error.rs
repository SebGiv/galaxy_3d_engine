//! Error types for the Galaxy3D engine
//!
//! This module defines the error types used throughout the engine,
//! including rendering, initialization, and resource management.

use std::fmt;

/// Result type for Galaxy3D engine operations
pub type Result<T> = std::result::Result<T, Error>;

/// Galaxy3D engine errors
#[derive(Debug, Clone)]
pub enum Error {
    /// Backend-specific error (Vulkan, DirectX, etc.)
    BackendError(String),

    /// Out of GPU memory
    OutOfMemory,

    /// Invalid resource (texture, buffer, shader, etc.)
    InvalidResource(String),

    /// Initialization failed (engine, renderer, subsystems)
    InitializationFailed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BackendError(msg) => write!(f, "Backend error: {}", msg),
            Error::OutOfMemory => write!(f, "Out of GPU memory"),
            Error::InvalidResource(msg) => write!(f, "Invalid resource: {}", msg),
            Error::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
        }
    }
}

impl std::error::Error for Error {}
