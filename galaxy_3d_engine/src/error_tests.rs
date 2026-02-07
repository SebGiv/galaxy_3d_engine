//! Unit tests for error.rs
//!
//! Tests all Error variants and their implementations (Display, Debug, Clone, std::error::Error).

use crate::error::{Error, Result};

// ============================================================================
// ERROR DISPLAY TESTS
// ============================================================================

#[test]
fn test_backend_error_display() {
    let err = Error::BackendError("Vulkan initialization failed".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Backend error"));
    assert!(display.contains("Vulkan initialization failed"));
}

#[test]
fn test_out_of_memory_display() {
    let err = Error::OutOfMemory;
    let display = format!("{}", err);
    assert_eq!(display, "Out of GPU memory");
}

#[test]
fn test_invalid_resource_display() {
    let err = Error::InvalidResource("Texture not found".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Invalid resource"));
    assert!(display.contains("Texture not found"));
}

#[test]
fn test_initialization_failed_display() {
    let err = Error::InitializationFailed("Window creation failed".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Initialization failed"));
    assert!(display.contains("Window creation failed"));
}

// ============================================================================
// ERROR TRAIT IMPLEMENTATIONS
// ============================================================================

#[test]
fn test_error_is_std_error() {
    let err = Error::OutOfMemory;
    // Verify Error implements std::error::Error trait
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_error_debug() {
    let err1 = Error::BackendError("test".to_string());
    let debug1 = format!("{:?}", err1);
    assert!(debug1.contains("BackendError"));

    let err2 = Error::OutOfMemory;
    let debug2 = format!("{:?}", err2);
    assert!(debug2.contains("OutOfMemory"));

    let err3 = Error::InvalidResource("resource".to_string());
    let debug3 = format!("{:?}", err3);
    assert!(debug3.contains("InvalidResource"));

    let err4 = Error::InitializationFailed("init".to_string());
    let debug4 = format!("{:?}", err4);
    assert!(debug4.contains("InitializationFailed"));
}

#[test]
fn test_error_clone() {
    let err1 = Error::BackendError("test".to_string());
    let err2 = err1.clone();
    assert_eq!(format!("{}", err1), format!("{}", err2));

    let err3 = Error::OutOfMemory;
    let err4 = err3.clone();
    assert_eq!(format!("{}", err3), format!("{}", err4));

    let err5 = Error::InvalidResource("res".to_string());
    let err6 = err5.clone();
    assert_eq!(format!("{}", err5), format!("{}", err6));

    let err7 = Error::InitializationFailed("init".to_string());
    let err8 = err7.clone();
    assert_eq!(format!("{}", err7), format!("{}", err8));
}

// ============================================================================
// RESULT TYPE TESTS
// ============================================================================

#[test]
fn test_result_type_ok() {
    fn returns_ok() -> Result<i32> {
        Ok(42)
    }

    let result = returns_ok();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn test_result_type_err() {
    fn returns_error() -> Result<i32> {
        Err(Error::OutOfMemory)
    }

    let result = returns_error();
    assert!(result.is_err());

    if let Err(e) = result {
        assert_eq!(format!("{}", e), "Out of GPU memory");
    }
}

#[test]
fn test_result_type_all_variants() {
    fn returns_backend_error() -> Result<()> {
        Err(Error::BackendError("test".to_string()))
    }

    fn returns_out_of_memory() -> Result<()> {
        Err(Error::OutOfMemory)
    }

    fn returns_invalid_resource() -> Result<()> {
        Err(Error::InvalidResource("test".to_string()))
    }

    fn returns_initialization_failed() -> Result<()> {
        Err(Error::InitializationFailed("test".to_string()))
    }

    assert!(returns_backend_error().is_err());
    assert!(returns_out_of_memory().is_err());
    assert!(returns_invalid_resource().is_err());
    assert!(returns_initialization_failed().is_err());
}

// ============================================================================
// ERROR PROPAGATION TESTS
// ============================================================================

#[test]
fn test_error_propagation_with_question_mark() {
    fn inner() -> Result<i32> {
        Err(Error::OutOfMemory)
    }

    fn outer() -> Result<i32> {
        inner()?;
        Ok(42)
    }

    let result = outer();
    assert!(result.is_err());
}

#[test]
fn test_error_message_content() {
    // Test that error messages contain meaningful information
    let err1 = Error::BackendError("Vulkan error code: -3".to_string());
    assert!(format!("{}", err1).contains("Vulkan error code: -3"));

    let err2 = Error::InvalidResource("Texture ID 42 not found in cache".to_string());
    assert!(format!("{}", err2).contains("Texture ID 42"));

    let err3 = Error::InitializationFailed("Failed to load vulkan-1.dll".to_string());
    assert!(format!("{}", err3).contains("vulkan-1.dll"));
}
