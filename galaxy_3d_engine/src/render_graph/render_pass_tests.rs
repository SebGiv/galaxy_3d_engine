use super::*;
use crate::render_graph::{AccessType, ResourceAccess, GraphResourceKey, PassAction};
use crate::error::Result;
use crate::graphics_device::{CommandList, ClearValue};
use crate::graphics_device::mock_graphics_device::MockRenderPass;
use crate::resource::resource_manager::PassInfo;
use crate::graphics_device::{TextureFormat, SampleCount};
use std::sync::Arc;

struct DummyPassAction {
    pub call_count: u32,
}

impl PassAction for DummyPassAction {
    fn execute(&mut self, _cmd: &mut dyn CommandList, _pass_info: &PassInfo) -> Result<()> {
        self.call_count += 1;
        Ok(())
    }
}

fn make_pass_info() -> PassInfo {
    PassInfo::new(
        vec![TextureFormat::R8G8B8A8_UNORM],
        Some(TextureFormat::D32_FLOAT),
        SampleCount::S1,
    )
}

fn make_pass(name: &str, accesses: Vec<ResourceAccess>) -> RenderPass {
    let action = Box::new(DummyPassAction { call_count: 0 });
    RenderPass::new(
        name.to_string(),
        accesses,
        action,
        None,
        None,
        None,
        Vec::new(),
    )
}

#[test]
fn test_render_pass_new_minimal() {
    let pass = make_pass("test", vec![]);
    assert_eq!(pass.name(), "test");
    assert_eq!(pass.accesses().len(), 0);
    assert!(pass.framebuffer_key().is_none());
    assert!(pass.pass_info().is_none());
    assert!(pass.gd_render_pass().is_none());
    assert_eq!(pass.clear_values().len(), 0);
}

#[test]
fn test_render_pass_with_accesses() {
    let access = ResourceAccess {
        graph_resource_key: GraphResourceKey::default(),
        access_type: AccessType::ColorAttachmentWrite,
        target_ops: None,
    };
    let pass = make_pass("with_accesses", vec![access]);
    assert_eq!(pass.accesses().len(), 1);
    assert_eq!(pass.accesses()[0].access_type, AccessType::ColorAttachmentWrite);
}

#[test]
fn test_render_pass_full_construction() {
    let action = Box::new(DummyPassAction { call_count: 0 });
    let gd_render_pass: Arc<dyn crate::graphics_device::RenderPass> = Arc::new(MockRenderPass::new());
    let pass = RenderPass::new(
        "full".to_string(),
        vec![],
        action,
        Some(FramebufferKey::default()),
        Some(make_pass_info()),
        Some(gd_render_pass),
        vec![ClearValue::Color([1.0, 0.0, 0.0, 1.0])],
    );
    assert_eq!(pass.name(), "full");
    assert!(pass.framebuffer_key().is_some());
    assert!(pass.pass_info().is_some());
    assert!(pass.gd_render_pass().is_some());
    assert_eq!(pass.clear_values().len(), 1);
}

#[test]
fn test_render_pass_pass_info_format_matches() {
    let action = Box::new(DummyPassAction { call_count: 0 });
    let pass = RenderPass::new(
        "fmt".to_string(),
        vec![],
        action,
        None,
        Some(make_pass_info()),
        None,
        Vec::new(),
    );
    let info = pass.pass_info().unwrap();
    assert_eq!(info.color_formats.len(), 1);
    assert_eq!(info.depth_format, Some(TextureFormat::D32_FLOAT));
}

#[test]
fn test_render_pass_action_mut_invokes_action() {
    let mut pass = make_pass("action", vec![]);
    let info = make_pass_info();
    let mut cmd = crate::graphics_device::mock_graphics_device::MockCommandList::new();
    pass.action_mut().execute(&mut cmd, &info).unwrap();
    pass.action_mut().execute(&mut cmd, &info).unwrap();
    // The DummyPassAction counter is internal — we can't read it directly through
    // the trait object, but we can verify execute() returned Ok twice.
}

#[test]
fn test_render_pass_accesses_mut_allows_push() {
    let mut pass = make_pass("muta", vec![]);
    pass.accesses_mut().push(ResourceAccess {
        graph_resource_key: GraphResourceKey::default(),
        access_type: AccessType::FragmentShaderRead,
        target_ops: None,
    });
    assert_eq!(pass.accesses().len(), 1);
    assert_eq!(pass.accesses()[0].access_type, AccessType::FragmentShaderRead);
}

#[test]
fn test_render_pass_replace_accesses() {
    let initial = vec![ResourceAccess {
        graph_resource_key: GraphResourceKey::default(),
        access_type: AccessType::FragmentShaderRead,
        target_ops: None,
    }];
    let mut pass = make_pass("replace", initial);
    let replaced = vec![
        ResourceAccess {
            graph_resource_key: GraphResourceKey::default(),
            access_type: AccessType::ComputeWrite,
            target_ops: None,
        },
        ResourceAccess {
            graph_resource_key: GraphResourceKey::default(),
            access_type: AccessType::ComputeRead,
            target_ops: None,
        },
    ];
    pass.replace_accesses(replaced);
    assert_eq!(pass.accesses().len(), 2);
    assert_eq!(pass.accesses()[0].access_type, AccessType::ComputeWrite);
    assert_eq!(pass.accesses()[1].access_type, AccessType::ComputeRead);
}

#[test]
fn test_render_pass_set_cache_replaces_all_cache_fields() {
    let mut pass = make_pass("cache", vec![]);
    let gd_render_pass: Arc<dyn crate::graphics_device::RenderPass> = Arc::new(MockRenderPass::new());
    let clear_values = vec![
        ClearValue::Color([0.0, 0.0, 0.0, 1.0]),
        ClearValue::DepthStencil { depth: 1.0, stencil: 0 },
    ];
    pass.set_cache(
        Some(FramebufferKey::default()),
        Some(make_pass_info()),
        Some(gd_render_pass),
        clear_values,
    );
    assert!(pass.framebuffer_key().is_some());
    assert!(pass.pass_info().is_some());
    assert!(pass.gd_render_pass().is_some());
    assert_eq!(pass.clear_values().len(), 2);
}

#[test]
fn test_render_pass_set_cache_clears_to_none() {
    let action = Box::new(DummyPassAction { call_count: 0 });
    let gd_rp: Arc<dyn crate::graphics_device::RenderPass> = Arc::new(MockRenderPass::new());
    let mut pass = RenderPass::new(
        "clear_cache".to_string(),
        vec![],
        action,
        Some(FramebufferKey::default()),
        Some(make_pass_info()),
        Some(gd_rp),
        vec![ClearValue::Color([0.5, 0.5, 0.5, 1.0])],
    );
    pass.set_cache(None, None, None, Vec::new());
    assert!(pass.framebuffer_key().is_none());
    assert!(pass.pass_info().is_none());
    assert!(pass.gd_render_pass().is_none());
    assert_eq!(pass.clear_values().len(), 0);
}

#[test]
fn test_render_pass_clear_values_color_and_depth() {
    let action = Box::new(DummyPassAction { call_count: 0 });
    let pass = RenderPass::new(
        "clear".to_string(),
        vec![],
        action,
        None,
        None,
        None,
        vec![
            ClearValue::Color([0.1, 0.2, 0.3, 1.0]),
            ClearValue::DepthStencil { depth: 0.0, stencil: 42 },
        ],
    );
    let cvs = pass.clear_values();
    assert_eq!(cvs.len(), 2);
    match cvs[0] {
        ClearValue::Color(c) => assert_eq!(c, [0.1, 0.2, 0.3, 1.0]),
        _ => panic!("expected Color"),
    }
    match cvs[1] {
        ClearValue::DepthStencil { depth, stencil } => {
            assert_eq!(depth, 0.0);
            assert_eq!(stencil, 42);
        }
        _ => panic!("expected DepthStencil"),
    }
}
