//! Engine-backed setup helpers for render-graph tests.
//!
//! Provides a one-call `setup_engine_for_render_graph()` that initializes
//! the global Engine with a `MockGraphicsDevice`, an empty `ResourceManager`,
//! and registers a couple of pre-made attachments. All tests using these
//! helpers must be `#[serial]` because the Engine state is global.

use std::sync::Arc;
use crate::engine::Engine;
use crate::graphics_device::{
    self, mock_graphics_device::MockGraphicsDevice,
    TextureFormat, TextureType, TextureUsage, MipmapMode, SampleCount,
};
use crate::resource::resource_manager::TextureKey;
use crate::resource::texture::{TextureDesc, LayerDesc};

pub(crate) struct GraphTestEnv {
    pub color_texture: TextureKey,
    pub depth_texture: TextureKey,
}

/// Bring the global Engine to a clean state with a MockGraphicsDevice and an
/// empty ResourceManager. Returns texture keys for a 64x64 R8G8B8A8 color
/// and a 64x64 D32 depth attachment, both registered in the manager.
pub(crate) fn setup_engine_for_render_graph() -> GraphTestEnv {
    Engine::initialize().unwrap();
    Engine::reset_for_testing();
    Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
    Engine::create_resource_manager().unwrap();

    let rm_arc = Engine::resource_manager().unwrap();
    let gd_arc = Engine::graphics_device("main").unwrap();
    let mut rm = rm_arc.lock().unwrap();

    let color_texture = rm.create_texture("color".to_string(), TextureDesc {
        graphics_device: gd_arc.clone(),
        texture: graphics_device::TextureDesc {
            width: 64, height: 64,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::RenderTarget,
            texture_type: TextureType::Tex2D,
            sample_count: SampleCount::S1,
            array_layers: 1,
            data: None,
            mipmap: MipmapMode::None,
        },
        layers: vec![LayerDesc { name: "default".to_string(), layer_index: 0, data: None, regions: vec![] }],
    }).unwrap();

    let depth_texture = rm.create_texture("depth".to_string(), TextureDesc {
        graphics_device: gd_arc.clone(),
        texture: graphics_device::TextureDesc {
            width: 64, height: 64,
            format: TextureFormat::D32_FLOAT,
            usage: TextureUsage::DepthStencil,
            texture_type: TextureType::Tex2D,
            sample_count: SampleCount::S1,
            array_layers: 1,
            data: None,
            mipmap: MipmapMode::None,
        },
        layers: vec![LayerDesc { name: "default".to_string(), layer_index: 0, data: None, regions: vec![] }],
    }).unwrap();

    drop(rm);
    GraphTestEnv { color_texture, depth_texture }
}

/// Default color `TargetOps`: clear to black, store at end, no resolve.
pub(crate) fn default_color_ops() -> super::access_type::TargetOps {
    super::access_type::TargetOps::Color {
        clear_color: [0.0, 0.0, 0.0, 1.0],
        load_op: graphics_device::LoadOp::Clear,
        store_op: graphics_device::StoreOp::Store,
        resolve_target: None,
    }
}

/// Default depth/stencil `TargetOps`: clear to 1.0, store, no stencil.
pub(crate) fn default_depth_ops() -> super::access_type::TargetOps {
    super::access_type::TargetOps::DepthStencil {
        depth_clear: 1.0,
        stencil_clear: 0,
        depth_load_op: graphics_device::LoadOp::Clear,
        depth_store_op: graphics_device::StoreOp::Store,
        stencil_load_op: graphics_device::LoadOp::DontCare,
        stencil_store_op: graphics_device::StoreOp::DontCare,
    }
}

/// Make a `RecordingPassAction` that captures the number of times its
/// `execute()` was invoked. Useful to verify a pass actually ran.
pub(crate) struct RecordingPassAction {
    pub call_count: Arc<std::sync::atomic::AtomicU32>,
}

impl super::pass_action::PassAction for RecordingPassAction {
    fn execute(
        &mut self,
        _cmd: &mut dyn graphics_device::CommandList,
        _pass_info: &crate::resource::resource_manager::PassInfo,
    ) -> crate::error::Result<()> {
        self.call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

pub(crate) fn make_recording_pass() -> (Box<RecordingPassAction>, Arc<std::sync::atomic::AtomicU32>) {
    let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let action = Box::new(RecordingPassAction { call_count: counter.clone() });
    (action, counter)
}
