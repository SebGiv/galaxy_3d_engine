/// Pass action trait and implementations.
///
/// Defines how a render pass records its draw commands between
/// begin_render_pass() and end_render_pass().

use std::sync::{Arc, Mutex};
use crate::error::Result;
use crate::engine::Engine;
use crate::graphics_device::{self, CommandList, BindingGroup, BindingResource, BindingGroupLayoutDesc, BindingSlotDesc, BindingType, ShaderStageFlags, SamplerType};
use crate::resource::resource_manager::PassInfo;
use crate::resource::buffer::Buffer;
use crate::resource::texture::Texture;
use crate::scene::RenderView;
use crate::scene::{Scene, Drawer};

/// Action executed by a render pass.
pub trait PassAction: Send + Sync {
    /// Record draw commands into the command list.
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()>;
}

/// Fullscreen pass action (data-driven, no closure)
pub struct FullscreenAction {
    pipeline: Arc<dyn graphics_device::Pipeline>,
    binding_group: Arc<dyn graphics_device::BindingGroup>,
}

impl FullscreenAction {
    pub fn new(
        pipeline: Arc<dyn graphics_device::Pipeline>,
        binding_group: Arc<dyn graphics_device::BindingGroup>,
    ) -> Self {
        Self { pipeline, binding_group }
    }
}

impl PassAction for FullscreenAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, _pass_info: &PassInfo) -> Result<()> {
        cmd.bind_pipeline(&self.pipeline)?;
        cmd.bind_binding_group(&self.pipeline, 0, &self.binding_group)?;
        cmd.draw(3, 0)
    }
}

/// Custom pass action (closure-based)
pub struct CustomAction {
    callback: Box<dyn FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync>,
}

impl CustomAction {
    pub fn new<F>(callback: F) -> Self
    where
        F: FnMut(&mut dyn CommandList, &PassInfo) -> Result<()> + Send + Sync + 'static,
    {
        Self { callback: Box::new(callback) }
    }
}

impl PassAction for CustomAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()> {
        (self.callback)(cmd, pass_info)
    }
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

// ===== SCENE PASS ACTION =====

/// Scene pass action — draws visible submeshes from a RenderView.
///
/// The constructor receives the buffers (`Vec<SceneBinding>`) and builds the
/// `BindingGroup` immediately via `create_binding_group_from_layout`. No lazy
/// construction, no pipeline dependency.
pub struct ScenePassAction {
    scene: Arc<Mutex<Scene>>,
    drawer: Arc<Mutex<dyn Drawer>>,
    render_view: Arc<Mutex<Option<RenderView>>>,
    binding_group: Arc<dyn BindingGroup>,
    bind_textures: bool,
}

impl ScenePassAction {
    /// Create a ScenePassAction.
    ///
    /// `bindings` is the list of buffers/textures for the per-pass descriptor
    /// set (set 1). The BindingGroup is created immediately — no lazy, no
    /// pipeline needed.
    pub fn new(
        scene: Arc<Mutex<Scene>>,
        drawer: Arc<Mutex<dyn Drawer>>,
        render_view: Arc<Mutex<Option<RenderView>>>,
        bindings: Vec<SceneBinding>,
        bind_textures: bool,
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
        let gd_arc = Engine::graphics_device("main")?;
        let gd = gd_arc.lock().unwrap();
        let binding_group = gd.create_binding_group_from_layout(
            &layout,
            1, // Set 1: scene bindings (set 0 is reserved for bindless textures)
            &resources,
        )?;

        Ok(Self { scene, drawer, render_view, binding_group, bind_textures })
    }
}

impl PassAction for ScenePassAction {
    fn execute(&mut self, cmd: &mut dyn CommandList, pass_info: &PassInfo) -> Result<()> {
        let mut scene = self.scene.lock().unwrap();
        let mut drawer = self.drawer.lock().unwrap();
        let view = self.render_view.lock().unwrap();
        if let Some(ref view) = *view {
            drawer.draw(&mut scene, view, cmd, pass_info, &self.binding_group, self.bind_textures)?;
        }
        Ok(())
    }
}
