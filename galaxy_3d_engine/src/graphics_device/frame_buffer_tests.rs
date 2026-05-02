use super::*;
use crate::graphics_device::mock_graphics_device::MockTexture;
use crate::graphics_device::TextureType;
use std::sync::Arc;

fn make_mock_tex() -> Arc<dyn Texture> {
    Arc::new(MockTexture::new(64, 64, 1, TextureType::Tex2D, "tex".to_string()))
}

#[test]
fn test_framebuffer_attachment_whole_defaults() {
    let attachment = FramebufferAttachment::whole(make_mock_tex());
    assert_eq!(attachment.base_mip_level, 0);
    assert_eq!(attachment.base_array_layer, 0);
    assert_eq!(attachment.layer_count, 1);
    assert_eq!(attachment.texture.info().width, 64);
}

#[test]
fn test_framebuffer_attachment_mip_layer() {
    let attachment = FramebufferAttachment::mip_layer(make_mock_tex(), 3, 7);
    assert_eq!(attachment.base_mip_level, 3);
    assert_eq!(attachment.base_array_layer, 7);
    assert_eq!(attachment.layer_count, 1);
}

#[test]
fn test_framebuffer_attachment_mip_layer_zero() {
    let attachment = FramebufferAttachment::mip_layer(make_mock_tex(), 0, 0);
    assert_eq!(attachment.base_mip_level, 0);
    assert_eq!(attachment.base_array_layer, 0);
    assert_eq!(attachment.layer_count, 1);
}

#[test]
fn test_framebuffer_attachment_struct_construction() {
    // Layered rendering: 6 layers (cubemap)
    let attachment = FramebufferAttachment {
        texture: make_mock_tex(),
        base_mip_level: 1,
        base_array_layer: 0,
        layer_count: 6,
    };
    assert_eq!(attachment.layer_count, 6);
}
