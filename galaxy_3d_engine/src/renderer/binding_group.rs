/// BindingGroup trait and binding group descriptor
///
/// A BindingGroup is an immutable set of GPU resource bindings (textures, buffers, samplers).
/// It is Galaxy3D's abstraction over GPU descriptor sets, inspired by WebGPU's GPUBindGroup.
///
/// Key properties:
/// - Immutable after creation (no race conditions)
/// - Layout deduced from the Pipeline (user never manipulates layouts directly)
/// - Pool managed internally by the renderer

use crate::renderer::{Texture, Buffer, SamplerType, ShaderStage};

// ============================================================================
// Binding types and layout description
// ============================================================================

/// Type of resource bound at a given slot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingType {
    /// Uniform buffer (read-only structured data)
    UniformBuffer,
    /// Combined image sampler (texture + sampler in one binding)
    CombinedImageSampler,
    /// Storage buffer (read/write for compute shaders)
    StorageBuffer,
}

/// Shader stage visibility flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderStageFlags(u32);

impl ShaderStageFlags {
    pub const VERTEX: Self = Self(0x01);
    pub const FRAGMENT: Self = Self(0x02);
    pub const COMPUTE: Self = Self(0x04);
    pub const VERTEX_FRAGMENT: Self = Self(0x03);
    pub const ALL: Self = Self(0x07);

    /// Create from a slice of ShaderStage
    pub fn from_stages(stages: &[ShaderStage]) -> Self {
        let mut flags = 0u32;
        for stage in stages {
            flags |= match stage {
                ShaderStage::Vertex => 0x01,
                ShaderStage::Fragment => 0x02,
                ShaderStage::Compute => 0x04,
            };
        }
        Self(flags)
    }

    pub fn contains_vertex(&self) -> bool { self.0 & 0x01 != 0 }
    pub fn contains_fragment(&self) -> bool { self.0 & 0x02 != 0 }
    pub fn contains_compute(&self) -> bool { self.0 & 0x04 != 0 }
    pub fn bits(&self) -> u32 { self.0 }
}

/// Description of a single binding slot within a BindingGroupLayout
#[derive(Debug, Clone)]
pub struct BindingSlotDesc {
    /// Binding number (corresponds to `layout(binding = N)` in GLSL)
    pub binding: u32,
    /// Type of resource at this binding
    pub binding_type: BindingType,
    /// Number of descriptors at this binding (>1 for arrays)
    pub count: u32,
    /// Shader stages that access this binding
    pub stage_flags: ShaderStageFlags,
}

/// Description of a BindingGroup layout (blueprint for a set of bindings)
///
/// This replaces raw descriptor set layout handles in the abstract renderer layer.
/// The backend creates the actual GPU layout object from this description.
#[derive(Debug, Clone)]
pub struct BindingGroupLayoutDesc {
    /// Binding slot descriptions
    pub entries: Vec<BindingSlotDesc>,
}

// ============================================================================
// Binding resources (concrete data passed at creation time)
// ============================================================================

/// A concrete resource to bind into a BindingGroup
pub enum BindingResource<'a> {
    /// Uniform buffer binding
    UniformBuffer(&'a dyn Buffer),
    /// Sampled texture (the backend resolves the actual GPU sampler from the type)
    SampledTexture(&'a dyn Texture, SamplerType),
    /// Storage buffer binding
    StorageBuffer(&'a dyn Buffer),
}

// ============================================================================
// BindingGroup trait
// ============================================================================

/// An immutable set of GPU resource bindings.
///
/// The layout and pool are managed internally by the renderer.
/// Once created, a BindingGroup cannot be modified — create a new one
/// to change resources.
pub trait BindingGroup: Send + Sync {
    /// Returns the set index this BindingGroup was created for
    fn set_index(&self) -> u32;
}
