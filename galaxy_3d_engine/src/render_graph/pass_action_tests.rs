use super::*;
use crate::graphics_device::mock_graphics_device::{MockCommandList, MockPipeline, MockBindingGroup};
use crate::graphics_device::{TextureFormat, SampleCount};
use crate::resource::resource_manager::PassInfo;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

fn make_pass_info() -> PassInfo {
    PassInfo::new(vec![TextureFormat::R8G8B8A8_UNORM], None, SampleCount::S1)
}

#[test]
fn test_fullscreen_action_execute_emits_three_commands() {
    let pipeline: Arc<dyn crate::graphics_device::Pipeline> =
        Arc::new(MockPipeline::new("fs_pipeline".to_string()));
    let binding_group: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("fs_bg".to_string(), 0));
    let mut action = FullscreenAction::new(pipeline, binding_group);
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    action.execute(&mut cmd, &info).unwrap();
    assert_eq!(cmd.commands, vec!["bind_pipeline", "bind_binding_group", "draw"]);
}

#[test]
fn test_fullscreen_action_execute_called_twice_emits_six_commands() {
    let pipeline: Arc<dyn crate::graphics_device::Pipeline> =
        Arc::new(MockPipeline::new("fs_pipeline".to_string()));
    let binding_group: Arc<dyn crate::graphics_device::BindingGroup> =
        Arc::new(MockBindingGroup::new("fs_bg".to_string(), 0));
    let mut action = FullscreenAction::new(pipeline, binding_group);
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    action.execute(&mut cmd, &info).unwrap();
    action.execute(&mut cmd, &info).unwrap();
    assert_eq!(cmd.commands.len(), 6);
}

#[test]
fn test_custom_action_execute_invokes_callback() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_inner = counter.clone();
    let mut action = CustomAction::new(move |_cmd, _pass_info| {
        counter_inner.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    action.execute(&mut cmd, &info).unwrap();
    action.execute(&mut cmd, &info).unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_custom_action_execute_propagates_error() {
    let mut action = CustomAction::new(|_cmd, _pass_info| {
        Err(crate::error::Error::InitializationFailed("bang".to_string()))
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    let result = action.execute(&mut cmd, &info);
    assert!(result.is_err());
}

#[test]
fn test_custom_action_callback_can_emit_commands() {
    let mut action = CustomAction::new(|cmd, _pass_info| {
        cmd.draw(6, 0)?;
        cmd.draw(3, 0)?;
        Ok(())
    });
    let mut cmd = MockCommandList::new();
    let info = make_pass_info();
    action.execute(&mut cmd, &info).unwrap();
    assert_eq!(cmd.commands, vec!["draw", "draw"]);
}

// ============================================================================
// ScenePassAction (Engine-backed)
// ============================================================================

mod scene_pass_action {
    use super::*;
    use crate::engine::Engine;
    use crate::scene::{Scene, ForwardDrawer, RenderView, Drawer};
    use crate::graphics_device::mock_graphics_device::MockGraphicsDevice;
    use serial_test::serial;
    use std::sync::Mutex;

    fn setup_engine_and_buffer() -> Arc<crate::resource::buffer::Buffer> {
        Engine::initialize().unwrap();
        Engine::reset_for_testing();
        Engine::create_graphics_device("main", MockGraphicsDevice::new()).unwrap();
        Engine::create_resource_manager().unwrap();

        let rm_arc = Engine::resource_manager().unwrap();
        let gd_arc = Engine::graphics_device("main").unwrap();
        let mut rm = rm_arc.lock().unwrap();
        let key = rm.create_default_frame_uniform_buffer("frame".to_string(), gd_arc).unwrap();
        rm.buffer(key).unwrap().clone()
    }

    #[test]
    #[serial]
    fn test_scene_pass_action_new_with_uniform_buffer() {
        let buf = setup_engine_and_buffer();
        let scene = Arc::new(Mutex::new(Scene::new()));
        let drawer: Arc<Mutex<dyn Drawer>> = Arc::new(Mutex::new(ForwardDrawer::new()));
        let render_view = Arc::new(Mutex::new(None));

        let action = ScenePassAction::new(
            scene, drawer, render_view,
            vec![SceneBinding::UniformBuffer(buf)],
            true,
        );
        assert!(action.is_ok(), "construction failed: {:?}", action.err());
    }

    #[test]
    #[serial]
    fn test_scene_pass_action_new_with_storage_buffer() {
        let buf = setup_engine_and_buffer();
        let scene = Arc::new(Mutex::new(Scene::new()));
        let drawer: Arc<Mutex<dyn Drawer>> = Arc::new(Mutex::new(ForwardDrawer::new()));
        let render_view = Arc::new(Mutex::new(None));

        let action = ScenePassAction::new(
            scene, drawer, render_view,
            vec![SceneBinding::StorageBuffer(buf)],
            false,
        );
        assert!(action.is_ok());
    }

    #[test]
    #[serial]
    fn test_scene_pass_action_execute_with_no_render_view_is_noop() {
        let buf = setup_engine_and_buffer();
        let scene = Arc::new(Mutex::new(Scene::new()));
        let drawer: Arc<Mutex<dyn Drawer>> = Arc::new(Mutex::new(ForwardDrawer::new()));
        let render_view: Arc<Mutex<Option<RenderView>>> = Arc::new(Mutex::new(None));

        let mut action = ScenePassAction::new(
            scene, drawer, render_view,
            vec![SceneBinding::UniformBuffer(buf)],
            true,
        ).unwrap();

        let mut cmd = MockCommandList::new();
        let info = make_pass_info();
        action.execute(&mut cmd, &info).unwrap();
        // No render_view → drawer never called → no draw commands.
        assert!(!cmd.commands.iter().any(|c| c == "draw" || c == "draw_indexed"));
    }

    #[test]
    #[serial]
    fn test_scene_pass_action_execute_with_empty_render_view_records_state() {
        let buf = setup_engine_and_buffer();
        let scene = Arc::new(Mutex::new(Scene::new()));
        let drawer: Arc<Mutex<dyn Drawer>> = Arc::new(Mutex::new(ForwardDrawer::new()));

        // Provide a RenderView with no items.
        use crate::camera::{Camera, Frustum};
        use crate::graphics_device::Viewport;
        use glam::Mat4;
        let frustum = Frustum::from_view_projection(&Mat4::IDENTITY);
        let viewport = Viewport { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, min_depth: 0.0, max_depth: 1.0 };
        let camera = Camera::new(Mat4::IDENTITY, Mat4::IDENTITY, frustum, viewport);
        let render_view = Arc::new(Mutex::new(Some(RenderView::new(camera, 0))));

        let mut action = ScenePassAction::new(
            scene, drawer, render_view,
            vec![SceneBinding::UniformBuffer(buf)],
            true,
        ).unwrap();

        let mut cmd = MockCommandList::new();
        let info = make_pass_info();
        action.execute(&mut cmd, &info).unwrap();
        // Drawer recorded viewport + scissor even with an empty view.
        assert!(cmd.commands.iter().any(|c| c == "set_viewport"));
    }
}
