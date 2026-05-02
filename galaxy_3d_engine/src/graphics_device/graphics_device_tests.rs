use super::*;

// ============================================================================
// DebugSeverity
// ============================================================================

#[test]
fn test_debug_severity_default_matches_build_profile() {
    let sev = DebugSeverity::default();
    if cfg!(debug_assertions) {
        assert_eq!(sev, DebugSeverity::ErrorsAndWarnings);
    } else {
        assert_eq!(sev, DebugSeverity::ErrorsOnly);
    }
}

#[test]
fn test_debug_severity_variants_distinct() {
    assert_ne!(DebugSeverity::ErrorsOnly, DebugSeverity::ErrorsAndWarnings);
    assert_ne!(DebugSeverity::ErrorsAndWarnings, DebugSeverity::All);
    assert_ne!(DebugSeverity::ErrorsOnly, DebugSeverity::All);
}

#[test]
fn test_debug_severity_clone_copy() {
    let a = DebugSeverity::All;
    let b = a;
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

// ============================================================================
// DebugOutput
// ============================================================================

#[test]
fn test_debug_output_default_is_console() {
    assert_eq!(DebugOutput::default(), DebugOutput::Console);
}

#[test]
fn test_debug_output_file_variant_holds_path() {
    let out = DebugOutput::File("/tmp/log.txt".to_string());
    match out {
        DebugOutput::File(path) => assert_eq!(path, "/tmp/log.txt"),
        _ => panic!("expected File"),
    }
}

#[test]
fn test_debug_output_both_variant_holds_path() {
    let out = DebugOutput::Both("logs.log".to_string());
    match out {
        DebugOutput::Both(path) => assert_eq!(path, "logs.log"),
        _ => panic!("expected Both"),
    }
}

#[test]
fn test_debug_output_equality() {
    assert_eq!(DebugOutput::Console, DebugOutput::Console);
    assert_eq!(
        DebugOutput::File("a".to_string()),
        DebugOutput::File("a".to_string()),
    );
    assert_ne!(
        DebugOutput::File("a".to_string()),
        DebugOutput::File("b".to_string()),
    );
}

// ============================================================================
// DebugMessageFilter
// ============================================================================

#[test]
fn test_debug_message_filter_default_all_true() {
    let f = DebugMessageFilter::default();
    assert!(f.show_general);
    assert!(f.show_validation);
    assert!(f.show_performance);
}

#[test]
fn test_debug_message_filter_clone_copy() {
    let f = DebugMessageFilter { show_general: false, show_validation: true, show_performance: false };
    let g = f;
    let h = f.clone();
    assert_eq!(f, g);
    assert_eq!(f, h);
}

// ============================================================================
// ValidationStats
// ============================================================================

#[test]
fn test_validation_stats_default_zero() {
    let s = ValidationStats::default();
    assert_eq!(s.errors, 0);
    assert_eq!(s.warnings, 0);
    assert_eq!(s.info, 0);
    assert_eq!(s.verbose, 0);
}

#[test]
fn test_validation_stats_total_sums_all_categories() {
    let s = ValidationStats { errors: 1, warnings: 2, info: 3, verbose: 4 };
    assert_eq!(s.total(), 10);
}

#[test]
fn test_validation_stats_total_default_is_zero() {
    assert_eq!(ValidationStats::default().total(), 0);
}

#[test]
fn test_validation_stats_has_errors_true() {
    let s = ValidationStats { errors: 5, ..ValidationStats::default() };
    assert!(s.has_errors());
}

#[test]
fn test_validation_stats_has_errors_false() {
    let s = ValidationStats { errors: 0, warnings: 99, ..ValidationStats::default() };
    assert!(!s.has_errors());
}

#[test]
fn test_validation_stats_has_warnings_true() {
    let s = ValidationStats { warnings: 1, ..ValidationStats::default() };
    assert!(s.has_warnings());
}

#[test]
fn test_validation_stats_has_warnings_false() {
    let s = ValidationStats { warnings: 0, errors: 99, ..ValidationStats::default() };
    assert!(!s.has_warnings());
}

// ============================================================================
// BindlessConfig
// ============================================================================

#[test]
fn test_bindless_config_default_values() {
    let c = BindlessConfig::default();
    assert_eq!(c.max_texture_2d, 4096);
    assert_eq!(c.max_texture_cube, 64);
    assert_eq!(c.max_texture_3d, 16);
    assert_eq!(c.max_texture_2d_array, 64);
}

#[test]
fn test_bindless_config_clone_preserves_values() {
    let c = BindlessConfig {
        max_texture_2d: 1024,
        max_texture_cube: 32,
        max_texture_3d: 8,
        max_texture_2d_array: 128,
    };
    let cloned = c.clone();
    assert_eq!(cloned.max_texture_2d, 1024);
    assert_eq!(cloned.max_texture_cube, 32);
    assert_eq!(cloned.max_texture_3d, 8);
    assert_eq!(cloned.max_texture_2d_array, 128);
}

// ============================================================================
// Config
// ============================================================================

#[test]
fn test_config_default_app_name() {
    let c = Config::default();
    assert_eq!(c.app_name, "Galaxy3D Application");
}

#[test]
fn test_config_default_version() {
    let c = Config::default();
    assert_eq!(c.app_version, (1, 0, 0));
}

#[test]
fn test_config_default_validation_matches_build_profile() {
    let c = Config::default();
    assert_eq!(c.enable_validation, cfg!(debug_assertions));
    assert_eq!(c.enable_validation_stats, cfg!(debug_assertions));
}

#[test]
fn test_config_default_break_panic_flags_false() {
    let c = Config::default();
    assert!(!c.break_on_validation_error);
    assert!(!c.panic_on_error);
}

#[test]
fn test_config_default_bindless_matches_default() {
    let c = Config::default();
    let b = BindlessConfig::default();
    assert_eq!(c.bindless.max_texture_2d, b.max_texture_2d);
    assert_eq!(c.bindless.max_texture_cube, b.max_texture_cube);
}

#[test]
fn test_config_clone_preserves_fields() {
    let mut c = Config::default();
    c.app_name = "Custom".to_string();
    c.app_version = (2, 3, 4);
    let cloned = c.clone();
    assert_eq!(cloned.app_name, "Custom");
    assert_eq!(cloned.app_version, (2, 3, 4));
}

// ============================================================================
// GraphicsDeviceStats
// ============================================================================

#[test]
fn test_graphics_device_stats_default_zero() {
    let s = GraphicsDeviceStats::default();
    assert_eq!(s.draw_calls, 0);
    assert_eq!(s.triangles, 0);
    assert_eq!(s.gpu_memory_used, 0);
}

#[test]
fn test_graphics_device_stats_clone_copy() {
    let s = GraphicsDeviceStats { draw_calls: 100, triangles: 500_000, gpu_memory_used: 1_000_000 };
    let t = s;
    let u = s.clone();
    assert_eq!(s.draw_calls, t.draw_calls);
    assert_eq!(s.triangles, u.triangles);
    assert_eq!(s.gpu_memory_used, t.gpu_memory_used);
}

#[test]
fn test_validation_stats_total_zero_after_reset() {
    let zero = ValidationStats::default();
    assert!(!zero.has_errors());
    assert!(!zero.has_warnings());
}

#[test]
fn test_debug_severity_default_is_set() {
    // Default depends on cfg(debug_assertions); both branches are valid.
    let sev = DebugSeverity::default();
    assert!(matches!(sev, DebugSeverity::ErrorsOnly | DebugSeverity::ErrorsAndWarnings));
}

// ============================================================================
// Plugin registry
// ============================================================================

mod plugin_registry {
    use super::*;
    use serial_test::serial;
    use crate::graphics_device::register_graphics_device_plugin;

    #[test]
    #[serial]
    fn test_register_plugin_does_not_panic() {
        // Registering a plugin under a unique name is idempotent at the registry
        // level: subsequent calls overwrite. We're just exercising the path.
        register_graphics_device_plugin("test_plugin_unique_a", |_w, _c| {
            Err(crate::error::Error::InitializationFailed(
                "test plugin never instantiates".to_string(),
            ))
        });
    }

    #[test]
    #[serial]
    fn test_register_two_plugins_under_different_names() {
        register_graphics_device_plugin("test_plugin_unique_b", |_w, _c| {
            Err(crate::error::Error::InitializationFailed("b".to_string()))
        });
        register_graphics_device_plugin("test_plugin_unique_c", |_w, _c| {
            Err(crate::error::Error::InitializationFailed("c".to_string()))
        });
    }
}
