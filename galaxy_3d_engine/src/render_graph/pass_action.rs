//! `PassAction` trait and shared bindings for render-pass actions.
//!
//! Defines the `PassAction` trait — implemented by every concrete action
//! (`FullscreenPassAction`, `CustomPassAction`, `ScenePassAction`,
//! `DebugPassAction`) — and the `SceneBinding` enum used by the actions
//! that need a per-pass descriptor set (set 1).

use std::sync::Arc;

use crate::error::Result;
use crate::graphics_device::{self, CommandList, SamplerType};
use crate::resource::buffer::Buffer;
use crate::resource::resource_manager::PassInfo;
use crate::resource::texture::Texture;

/// Action executed by a render pass.
pub trait PassAction: Send + Sync {
    /// Record draw commands into the command list.
    ///
    /// `graphics_device` is passed explicitly: it is the device on which
    /// the pass's backend resources were created. Implementations needing
    /// to allocate transient GPU objects (pipelines, buffers) at execute
    /// time should use it directly rather than looking up a named device.
    fn execute(
        &mut self,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<()>;
}

// ===== SCENE BINDING =====

/// A binding resource for the scene pass descriptor set.
///
/// Each entry maps to a binding index (0, 1, 2, ...) in declaration order.
pub enum SceneBinding {
    /// Uniform buffer
    UniformBuffer(Arc<Buffer>),
    /// Storage buffer
    StorageBuffer(Arc<Buffer>),
    /// Sampled texture with sampler type
    SampledTexture(Arc<Texture>, SamplerType),
}
