//! Scene pass action: draws visible submeshes from a `RenderView`.
//!
//! Builds the per-pass descriptor set (set 1) at construction time from a
//! list of `SceneBinding`s and forwards `execute()` to the configured
//! `Drawer`. Holds the `Scene`, `Drawer`, and `RenderView` behind mutexes;
//! locking happens at execute time only.

use std::sync::{Arc, Mutex};

use crate::error::Result;
use crate::graphics_device::{
    self, BindingGroup, BindingGroupLayoutDesc, BindingResource, BindingSlotDesc, BindingType,
    CommandList, ShaderStageFlags,
};
use crate::resource::resource_manager::PassInfo;
use crate::scene::{Drawer, RenderView, Scene};

use super::pass_action::{PassAction, SceneBinding};

/// Scene pass action — draws visible submeshes from a `RenderView`.
///
/// The constructor receives the buffers (`Vec<SceneBinding>`) and builds
/// the `BindingGroup` immediately via `create_binding_group_from_layout`.
/// No lazy construction, no pipeline dependency.
pub struct ScenePassAction {
    scene: Arc<Mutex<Scene>>,
    drawer: Arc<Mutex<dyn Drawer>>,
    render_view: Arc<Mutex<Option<RenderView>>>,
    binding_group: Arc<dyn BindingGroup>,
    bind_textures: bool,
    /// Pre-computed dynamic-bindings mask for set 1, derived from the
    /// `SceneBinding` list at `new()`. Forwarded to the `Drawer` at every
    /// execute so on-the-fly pipeline cache misses produce a layout
    /// matching this BindingGroup's dynamic-offset descriptors.
    dynamic_bindings: graphics_device::DynamicBindings,
}

impl ScenePassAction {
    /// Create a `ScenePassAction`.
    ///
    /// `bindings` is the list of buffers/textures for the per-pass
    /// descriptor set (set 1). The `BindingGroup` is created immediately —
    /// no lazy, no pipeline needed.
    pub fn new(
        scene: Arc<Mutex<Scene>>,
        drawer: Arc<Mutex<dyn Drawer>>,
        render_view: Arc<Mutex<Option<RenderView>>>,
        bindings: Vec<SceneBinding>,
        bind_textures: bool,
        graphics_device: &dyn graphics_device::GraphicsDevice,
    ) -> Result<Self> {
        // Build layout description from bindings.
        // For UBO/SSBO bindings we pick the static or dynamic variant based on
        // the underlying buffer's `update_mode()`, so a Dynamic buffer wired
        // here automatically lands on `*BufferDynamic`. The descriptor backend
        // then computes `slot * slot_size` at bind time.
        let layout = BindingGroupLayoutDesc {
            entries: bindings.iter().enumerate().map(|(i, b)| {
                BindingSlotDesc {
                    binding: i as u32,
                    binding_type: match b {
                        SceneBinding::UniformBuffer(buf) => match buf.graphics_device_buffer().update_mode() {
                            graphics_device::BufferUpdateMode::Static => BindingType::UniformBuffer,
                            graphics_device::BufferUpdateMode::Dynamic => BindingType::UniformBufferDynamic,
                        },
                        SceneBinding::StorageBuffer(buf) => match buf.graphics_device_buffer().update_mode() {
                            graphics_device::BufferUpdateMode::Static => BindingType::StorageBuffer,
                            graphics_device::BufferUpdateMode::Dynamic => BindingType::StorageBufferDynamic,
                        },
                        SceneBinding::SampledTexture(_, _) => BindingType::CombinedImageSampler,
                    },
                    count: 1,
                    stage_flags: ShaderStageFlags::VERTEX_FRAGMENT,
                }
            }).collect(),
        };

        // Build binding resources
        let resources: Vec<BindingResource> = bindings.iter()
            .map(|b| match b {
                SceneBinding::UniformBuffer(buf) =>
                    BindingResource::UniformBuffer(buf.graphics_device_buffer()),
                SceneBinding::StorageBuffer(buf) =>
                    BindingResource::StorageBuffer(buf.graphics_device_buffer()),
                SceneBinding::SampledTexture(tex, sampler_type) =>
                    BindingResource::SampledTexture(
                        tex.graphics_device_texture().as_ref(), *sampler_type,
                    ),
            })
            .collect();

        // Create the BindingGroup immediately
        let binding_group = graphics_device.create_binding_group_from_layout(
            &layout,
            1, // Set 1: scene bindings (set 0 is reserved for bindless textures)
            &resources,
        )?;

        // Pre-compute the dynamic-bindings mask for set 1 from the same scene
        // bindings. Reused at every execute and forwarded to the drawer so any
        // pipeline cache miss creates a layout matching the dynamic-offset
        // descriptor types the BindingGroup just registered for set 1.
        let mut dynamic_bindings = graphics_device::DynamicBindings::new();
        for (i, b) in bindings.iter().enumerate() {
            let is_dynamic = match b {
                SceneBinding::UniformBuffer(buf) | SceneBinding::StorageBuffer(buf) => matches!(
                    buf.graphics_device_buffer().update_mode(),
                    graphics_device::BufferUpdateMode::Dynamic
                ),
                SceneBinding::SampledTexture(_, _) => false,
            };
            if is_dynamic {
                dynamic_bindings.add(1, i as u32);
            }
        }

        Ok(Self {
            scene, drawer, render_view, binding_group, bind_textures, dynamic_bindings,
        })
    }
}

impl PassAction for ScenePassAction {
    fn execute(
        &mut self,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<()> {
        let mut scene = self.scene.lock().unwrap();
        let mut drawer = self.drawer.lock().unwrap();
        let view = self.render_view.lock().unwrap();
        if let Some(ref view) = *view {
            drawer.draw(
                &mut scene, view, cmd, pass_info, &self.binding_group, self.bind_textures,
                &self.dynamic_bindings,
                graphics_device,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "scene_pass_action_tests.rs"]
mod tests;
