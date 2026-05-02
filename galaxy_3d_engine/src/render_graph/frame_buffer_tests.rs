use super::*;
use crate::render_graph::GraphResourceKey;
use crate::graphics_device::mock_graphics_device::MockFramebuffer;
use std::sync::Arc;
use std::collections::HashSet;

fn make_fb(color_count: usize, depth: bool) -> Framebuffer {
    let gd_fb: Arc<dyn crate::graphics_device::Framebuffer> = Arc::new(MockFramebuffer::new(640, 480));
    let color = (0..color_count).map(|_| ColorAttachmentSlot {
        color: GraphResourceKey::default(),
        resolve: None,
    }).collect();
    let depth_stencil = if depth { Some(GraphResourceKey::default()) } else { None };
    Framebuffer::new(gd_fb, color, depth_stencil)
}

#[test]
fn test_framebuffer_new_no_attachments() {
    let fb = make_fb(0, false);
    assert_eq!(fb.color_attachments().len(), 0);
    assert!(fb.depth_stencil_attachment().is_none());
}

#[test]
fn test_framebuffer_new_color_only() {
    let fb = make_fb(2, false);
    assert_eq!(fb.color_attachments().len(), 2);
    assert!(fb.depth_stencil_attachment().is_none());
}

#[test]
fn test_framebuffer_new_color_and_depth() {
    let fb = make_fb(1, true);
    assert_eq!(fb.color_attachments().len(), 1);
    assert!(fb.depth_stencil_attachment().is_some());
}

#[test]
fn test_framebuffer_gd_framebuffer_dimensions() {
    let fb = make_fb(1, false);
    assert_eq!(fb.gd_framebuffer().width(), 640);
    assert_eq!(fb.gd_framebuffer().height(), 480);
}

#[test]
fn test_color_attachment_slot_equality_no_resolve() {
    let a = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    let b = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    assert_eq!(a, b);
}

#[test]
fn test_color_attachment_slot_inequality_resolve() {
    let a = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    let b = ColorAttachmentSlot {
        color: GraphResourceKey::default(),
        resolve: Some(GraphResourceKey::default()),
    };
    assert_eq!(a.color, b.color);
    assert_ne!(a.resolve, b.resolve);
}

#[test]
fn test_color_attachment_slot_clone_copy() {
    let a = ColorAttachmentSlot {
        color: GraphResourceKey::default(),
        resolve: Some(GraphResourceKey::default()),
    };
    let b = a;
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn test_color_attachment_slot_hashable() {
    let mut set = HashSet::new();
    set.insert(ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None });
    set.insert(ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None });
    assert_eq!(set.len(), 1);
}

#[test]
fn test_framebuffer_lookup_key_equality() {
    use super::FramebufferLookupKey;
    let slot = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    let a = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: None,
    };
    let b = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: None,
    };
    assert_eq!(a, b);
}

#[test]
fn test_framebuffer_lookup_key_inequality_different_depth() {
    use super::FramebufferLookupKey;
    let slot = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    let a = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: None,
    };
    let b = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: Some(GraphResourceKey::default()),
    };
    assert_ne!(a, b);
}

#[test]
fn test_framebuffer_lookup_key_hashable() {
    use super::FramebufferLookupKey;
    let slot = ColorAttachmentSlot { color: GraphResourceKey::default(), resolve: None };
    let key1 = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: None,
    };
    let key2 = FramebufferLookupKey {
        color_attachments: vec![slot],
        depth_stencil_attachment: None,
    };
    let key3 = FramebufferLookupKey {
        color_attachments: vec![],
        depth_stencil_attachment: None,
    };
    let mut set = HashSet::new();
    set.insert(key1);
    set.insert(key2);
    set.insert(key3);
    assert_eq!(set.len(), 2);
}
