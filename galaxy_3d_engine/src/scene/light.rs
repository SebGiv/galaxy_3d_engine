/// Light types for the scene system.
///
/// A Light is the CPU-side representation of a light source.
/// Uses Position + Direction model (not Mat4) — lights are not meshes.

use glam::Vec3;
use slotmap::new_key_type;

// ===== SLOT MAP KEY =====

new_key_type! {
    /// Stable key for a Light within a Scene.
    ///
    /// Keys remain valid even after other lights are removed.
    /// A key becomes invalid only when its own light is removed.
    pub struct LightKey;
}

// ===== LIGHT TYPE =====

/// Type of light source (Point/Spot only).
///
/// Directional lights (sun) are handled separately in the frame buffer.
/// The light SSBO only contains Point and Spot lights.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    /// Omnidirectional point light. Position + range + attenuation.
    Point,
    /// Cone-shaped spotlight. Position + direction + range + cone angles.
    Spot,
}

// ===== LIGHT =====

/// CPU-side light representation (Point/Spot only).
///
/// Stored in a SlotMap in the Scene. Individual fields can be updated
/// via Scene setters that automatically track dirty state in two sets:
/// - **Spatial** (dirty_light_transforms): position, direction, range, type
/// - **Data** (dirty_light_data): color, intensity, attenuation, spot angles, enabled
pub struct Light {
    /// Index in the GPU light buffer (SSBO)
    pub(crate) light_slot: u32,
    /// World-space position
    position: Vec3,
    /// Light direction (normalized). For Spot lights.
    direction: Vec3,
    /// Light color (linear RGB, typically 0..1)
    color: Vec3,
    /// Intensity multiplier
    intensity: f32,
    /// Maximum range (for Point/Spot attenuation cutoff)
    range: f32,
    /// Attenuation: constant factor
    attenuation_constant: f32,
    /// Attenuation: linear factor
    attenuation_linear: f32,
    /// Attenuation: quadratic factor (inverse square default)
    attenuation_quadratic: f32,
    /// Spot inner cone half-angle in radians (full intensity)
    spot_inner_angle: f32,
    /// Spot outer cone half-angle in radians (falloff to zero)
    spot_outer_angle: f32,
    /// Light type
    light_type: LightType,
    /// Whether the light is active
    enabled: bool,
}

// ===== LIGHT DESC =====

/// Descriptor for creating a Light (Point/Spot only).
///
/// Directional lights (sun) are handled separately in the frame buffer.
pub enum LightDesc {
    /// Point light. Position + range + attenuation + color + intensity.
    Point {
        position: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        attenuation_constant: f32,
        attenuation_linear: f32,
        attenuation_quadratic: f32,
    },
    /// Spotlight. Position + direction + range + cone angles + attenuation + color + intensity.
    Spot {
        position: Vec3,
        direction: Vec3,
        color: Vec3,
        intensity: f32,
        range: f32,
        attenuation_constant: f32,
        attenuation_linear: f32,
        attenuation_quadratic: f32,
        spot_inner_angle: f32,
        spot_outer_angle: f32,
    },
}

// ===== LIGHT IMPLEMENTATION =====

impl Light {
    /// Create a Light from a descriptor.
    pub(crate) fn from_desc(desc: LightDesc) -> Self {
        match desc {
            LightDesc::Point {
                position, color, intensity, range,
                attenuation_constant, attenuation_linear, attenuation_quadratic,
            } => Self {
                light_slot: 0,
                position,
                direction: Vec3::NEG_Y,
                color,
                intensity,
                range,
                attenuation_constant,
                attenuation_linear,
                attenuation_quadratic,
                spot_inner_angle: 0.0,
                spot_outer_angle: 0.0,
                light_type: LightType::Point,
                enabled: true,
            },
            LightDesc::Spot {
                position, direction, color, intensity, range,
                attenuation_constant, attenuation_linear, attenuation_quadratic,
                spot_inner_angle, spot_outer_angle,
            } => Self {
                light_slot: 0,
                position,
                direction: direction.normalize_or_zero(),
                color,
                intensity,
                range,
                attenuation_constant,
                attenuation_linear,
                attenuation_quadratic,
                spot_inner_angle,
                spot_outer_angle,
                light_type: LightType::Spot,
                enabled: true,
            },
        }
    }

    // ===== ACCESSORS =====

    /// Get the GPU light buffer slot index
    pub fn light_slot(&self) -> u32 { self.light_slot }

    /// Get the world-space position
    pub fn position(&self) -> Vec3 { self.position }

    /// Get the light direction (normalized)
    pub fn direction(&self) -> Vec3 { self.direction }

    /// Get the light color (linear RGB)
    pub fn color(&self) -> Vec3 { self.color }

    /// Get the intensity multiplier
    pub fn intensity(&self) -> f32 { self.intensity }

    /// Get the maximum range
    pub fn range(&self) -> f32 { self.range }

    /// Get the constant attenuation factor
    pub fn attenuation_constant(&self) -> f32 { self.attenuation_constant }

    /// Get the linear attenuation factor
    pub fn attenuation_linear(&self) -> f32 { self.attenuation_linear }

    /// Get the quadratic attenuation factor
    pub fn attenuation_quadratic(&self) -> f32 { self.attenuation_quadratic }

    /// Get the spot inner cone half-angle in radians
    pub fn spot_inner_angle(&self) -> f32 { self.spot_inner_angle }

    /// Get the spot outer cone half-angle in radians
    pub fn spot_outer_angle(&self) -> f32 { self.spot_outer_angle }

    /// Get the light type
    pub fn light_type(&self) -> LightType { self.light_type }

    /// Whether the light is enabled
    pub fn enabled(&self) -> bool { self.enabled }

    // ===== SETTERS (crate-internal, called by Scene setters) =====

    pub(crate) fn set_position(&mut self, position: Vec3) { self.position = position; }
    pub(crate) fn set_direction(&mut self, direction: Vec3) { self.direction = direction.normalize_or_zero(); }
    pub(crate) fn set_color(&mut self, color: Vec3) { self.color = color; }
    pub(crate) fn set_intensity(&mut self, intensity: f32) { self.intensity = intensity; }
    pub(crate) fn set_range(&mut self, range: f32) { self.range = range; }
    pub(crate) fn set_attenuation(&mut self, constant: f32, linear: f32, quadratic: f32) {
        self.attenuation_constant = constant;
        self.attenuation_linear = linear;
        self.attenuation_quadratic = quadratic;
    }
    pub(crate) fn set_spot_angles(&mut self, inner: f32, outer: f32) {
        self.spot_inner_angle = inner;
        self.spot_outer_angle = outer;
    }
    pub(crate) fn set_light_type(&mut self, light_type: LightType) { self.light_type = light_type; }
    pub(crate) fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
}

#[cfg(test)]
#[path = "light_tests.rs"]
mod tests;
