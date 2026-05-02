use super::*;

#[test]
fn test_is_write_color_attachment_write() {
    assert!(AccessType::ColorAttachmentWrite.is_write());
}

#[test]
fn test_is_write_color_attachment_read() {
    assert!(!AccessType::ColorAttachmentRead.is_write());
}

#[test]
fn test_is_write_depth_stencil_write() {
    assert!(AccessType::DepthStencilWrite.is_write());
}

#[test]
fn test_is_write_depth_stencil_read_only() {
    assert!(!AccessType::DepthStencilReadOnly.is_write());
}

#[test]
fn test_is_write_compute_write() {
    assert!(AccessType::ComputeWrite.is_write());
}

#[test]
fn test_is_write_compute_read() {
    assert!(!AccessType::ComputeRead.is_write());
}

#[test]
fn test_is_write_transfer_write() {
    assert!(AccessType::TransferWrite.is_write());
}

#[test]
fn test_is_write_transfer_read() {
    assert!(!AccessType::TransferRead.is_write());
}

#[test]
fn test_is_write_fragment_shader_read() {
    assert!(!AccessType::FragmentShaderRead.is_write());
}

#[test]
fn test_is_write_vertex_shader_read() {
    assert!(!AccessType::VertexShaderRead.is_write());
}

#[test]
fn test_is_write_ray_tracing_read() {
    assert!(!AccessType::RayTracingRead.is_write());
}

#[test]
fn test_is_attachment_color_write() {
    assert!(AccessType::ColorAttachmentWrite.is_attachment());
}

#[test]
fn test_is_attachment_color_read() {
    assert!(AccessType::ColorAttachmentRead.is_attachment());
}

#[test]
fn test_is_attachment_depth_write() {
    assert!(AccessType::DepthStencilWrite.is_attachment());
}

#[test]
fn test_is_attachment_depth_read_only() {
    assert!(AccessType::DepthStencilReadOnly.is_attachment());
}

#[test]
fn test_is_attachment_compute_read_false() {
    assert!(!AccessType::ComputeRead.is_attachment());
}

#[test]
fn test_is_attachment_compute_write_false() {
    assert!(!AccessType::ComputeWrite.is_attachment());
}

#[test]
fn test_is_attachment_fragment_shader_read_false() {
    assert!(!AccessType::FragmentShaderRead.is_attachment());
}

#[test]
fn test_is_attachment_vertex_shader_read_false() {
    assert!(!AccessType::VertexShaderRead.is_attachment());
}

#[test]
fn test_is_attachment_transfer_read_false() {
    assert!(!AccessType::TransferRead.is_attachment());
}

#[test]
fn test_is_attachment_transfer_write_false() {
    assert!(!AccessType::TransferWrite.is_attachment());
}

#[test]
fn test_is_attachment_ray_tracing_read_false() {
    assert!(!AccessType::RayTracingRead.is_attachment());
}

#[test]
fn test_access_type_equality_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(AccessType::ColorAttachmentWrite);
    set.insert(AccessType::ColorAttachmentWrite);
    set.insert(AccessType::DepthStencilWrite);
    assert_eq!(set.len(), 2);
    assert_eq!(AccessType::ComputeRead, AccessType::ComputeRead);
    assert_ne!(AccessType::ComputeRead, AccessType::ComputeWrite);
}

#[test]
fn test_access_type_clone_copy() {
    let a = AccessType::TransferRead;
    let b = a;
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}
