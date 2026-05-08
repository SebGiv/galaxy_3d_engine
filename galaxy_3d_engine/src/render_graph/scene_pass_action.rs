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
        // Build layout description from bindings
        let layout = BindingGroupLayoutDesc {
            entries: bindings.iter().enumerate().map(|(i, b)| {
                BindingSlotDesc {
                    binding: i as u32,
                    binding_type: match b {
                        SceneBinding::UniformBuffer(_) => BindingType::UniformBuffer,
                        SceneBinding::StorageBuffer(_) => BindingType::StorageBuffer,
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
                    BindingResource::UniformBuffer(buf.graphics_device_buffer().as_ref()),
                SceneBinding::StorageBuffer(buf) =>
                    BindingResource::StorageBuffer(buf.graphics_device_buffer().as_ref()),
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

        Ok(Self { scene, drawer, render_view, binding_group, bind_textures })
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
                graphics_device,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "scene_pass_action_tests.rs"]
mod tests;
