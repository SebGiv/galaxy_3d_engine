use super::*;
use glam::Vec3;

// ============================================================================
// Tests: Light creation from LightDesc
// ============================================================================

#[test]
fn test_directional_light_from_desc() {
    let light = Light::from_desc(LightDesc::Directional {
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: Vec3::new(1.0, 1.0, 1.0),
        intensity: 2.0,
    });

    assert_eq!(light.light_type(), LightType::Directional);
    assert_eq!(light.direction(), Vec3::new(0.0, -1.0, 0.0));
    assert_eq!(light.color(), Vec3::new(1.0, 1.0, 1.0));
    assert_eq!(light.intensity(), 2.0);
    assert_eq!(light.position(), Vec3::ZERO);
    assert_eq!(light.range(), f32::MAX);
    assert!(light.enabled());
}

#[test]
fn test_point_light_from_desc() {
    let light = Light::from_desc(LightDesc::Point {
        position: Vec3::new(1.0, 2.0, 3.0),
        color: Vec3::new(1.0, 0.5, 0.0),
        intensity: 5.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    assert_eq!(light.light_type(), LightType::Point);
    assert_eq!(light.position(), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(light.color(), Vec3::new(1.0, 0.5, 0.0));
    assert_eq!(light.intensity(), 5.0);
    assert_eq!(light.range(), 10.0);
    assert_eq!(light.attenuation_constant(), 0.0);
    assert_eq!(light.attenuation_linear(), 0.0);
    assert_eq!(light.attenuation_quadratic(), 1.0);
    assert!(light.enabled());
}

#[test]
fn test_spot_light_from_desc() {
    let light = Light::from_desc(LightDesc::Spot {
        position: Vec3::new(0.0, 5.0, 0.0),
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: Vec3::ONE,
        intensity: 3.0,
        range: 20.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
        spot_inner_angle: 0.3,
        spot_outer_angle: 0.5,
    });

    assert_eq!(light.light_type(), LightType::Spot);
    assert_eq!(light.position(), Vec3::new(0.0, 5.0, 0.0));
    assert_eq!(light.direction(), Vec3::new(0.0, -1.0, 0.0));
    assert_eq!(light.intensity(), 3.0);
    assert_eq!(light.range(), 20.0);
    assert_eq!(light.spot_inner_angle(), 0.3);
    assert_eq!(light.spot_outer_angle(), 0.5);
    assert!(light.enabled());
}

#[test]
fn test_directional_direction_is_normalized() {
    let light = Light::from_desc(LightDesc::Directional {
        direction: Vec3::new(1.0, 1.0, 1.0),
        color: Vec3::ONE,
        intensity: 1.0,
    });

    let len = light.direction().length();
    assert!((len - 1.0).abs() < 1e-6, "direction should be normalized, got length {}", len);
}

#[test]
fn test_spot_direction_is_normalized() {
    let light = Light::from_desc(LightDesc::Spot {
        position: Vec3::ZERO,
        direction: Vec3::new(3.0, 0.0, 4.0),
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
        spot_inner_angle: 0.3,
        spot_outer_angle: 0.5,
    });

    let len = light.direction().length();
    assert!((len - 1.0).abs() < 1e-6, "direction should be normalized, got length {}", len);
}

// ============================================================================
// Tests: Light setters
// ============================================================================

#[test]
fn test_set_position() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    light.set_position(Vec3::new(5.0, 6.0, 7.0));
    assert_eq!(light.position(), Vec3::new(5.0, 6.0, 7.0));
}

#[test]
fn test_set_direction_normalizes() {
    let mut light = Light::from_desc(LightDesc::Directional {
        direction: Vec3::NEG_Y,
        color: Vec3::ONE,
        intensity: 1.0,
    });

    light.set_direction(Vec3::new(0.0, 0.0, 10.0));
    let len = light.direction().length();
    assert!((len - 1.0).abs() < 1e-6);
    assert!((light.direction().z - 1.0).abs() < 1e-6);
}

#[test]
fn test_set_color() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    light.set_color(Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(light.color(), Vec3::new(1.0, 0.0, 0.0));
}

#[test]
fn test_set_intensity() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    light.set_intensity(42.0);
    assert_eq!(light.intensity(), 42.0);
}

#[test]
fn test_set_range() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    light.set_range(50.0);
    assert_eq!(light.range(), 50.0);
}

#[test]
fn test_set_attenuation() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    light.set_attenuation(1.0, 0.09, 0.032);
    assert_eq!(light.attenuation_constant(), 1.0);
    assert_eq!(light.attenuation_linear(), 0.09);
    assert_eq!(light.attenuation_quadratic(), 0.032);
}

#[test]
fn test_set_spot_angles() {
    let mut light = Light::from_desc(LightDesc::Spot {
        position: Vec3::ZERO,
        direction: Vec3::NEG_Y,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
        spot_inner_angle: 0.3,
        spot_outer_angle: 0.5,
    });

    light.set_spot_angles(0.1, 0.8);
    assert_eq!(light.spot_inner_angle(), 0.1);
    assert_eq!(light.spot_outer_angle(), 0.8);
}

#[test]
fn test_set_light_type() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    assert_eq!(light.light_type(), LightType::Point);
    light.set_light_type(LightType::Spot);
    assert_eq!(light.light_type(), LightType::Spot);
}

#[test]
fn test_set_enabled() {
    let mut light = Light::from_desc(LightDesc::Point {
        position: Vec3::ZERO,
        color: Vec3::ONE,
        intensity: 1.0,
        range: 10.0,
        attenuation_constant: 0.0,
        attenuation_linear: 0.0,
        attenuation_quadratic: 1.0,
    });

    assert!(light.enabled());
    light.set_enabled(false);
    assert!(!light.enabled());
    light.set_enabled(true);
    assert!(light.enabled());
}
