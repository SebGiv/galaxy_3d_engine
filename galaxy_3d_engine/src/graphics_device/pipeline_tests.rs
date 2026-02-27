//! Unit tests for Pipeline module
//!
//! Tests IndexType, PrimitiveTopology, VertexInputRate, and VertexLayout types.

use crate::graphics_device::{
    IndexType, PrimitiveTopology, VertexInputRate, VertexLayout,
    VertexBinding, VertexAttribute, BufferFormat,
};

// ============================================================================
// INDEX TYPE TESTS
// ============================================================================

#[test]
fn test_index_type_size_bytes() {
    assert_eq!(IndexType::U16.size_bytes(), 2);
    assert_eq!(IndexType::U32.size_bytes(), 4);
}

#[test]
fn test_index_type_size_calculations() {
    // Example: 1000 indices with U16
    let index_count = 1000u32;
    let index_type = IndexType::U16;
    let buffer_size = index_count * index_type.size_bytes();
    assert_eq!(buffer_size, 2000); // 1000 * 2 bytes

    // Example: 1000 indices with U32
    let index_type = IndexType::U32;
    let buffer_size = index_count * index_type.size_bytes();
    assert_eq!(buffer_size, 4000); // 1000 * 4 bytes
}

#[test]
fn test_index_type_equality() {
    assert_eq!(IndexType::U16, IndexType::U16);
    assert_eq!(IndexType::U32, IndexType::U32);
    assert_ne!(IndexType::U16, IndexType::U32);
}

#[test]
fn test_index_type_debug() {
    let u16_debug = format!("{:?}", IndexType::U16);
    assert!(u16_debug.contains("U16"));

    let u32_debug = format!("{:?}", IndexType::U32);
    assert!(u32_debug.contains("U32"));
}

#[test]
fn test_index_type_clone() {
    let idx1 = IndexType::U16;
    let idx2 = idx1.clone();
    assert_eq!(idx1, idx2);

    let idx3 = IndexType::U32;
    let idx4 = idx3.clone();
    assert_eq!(idx3, idx4);
}

#[test]
fn test_index_type_copy() {
    let idx1 = IndexType::U16;
    let idx2 = idx1; // Copy, not move
    assert_eq!(idx1, idx2);
    // Can still use idx1
    assert_eq!(idx1.size_bytes(), 2);
}

// ============================================================================
// PRIMITIVE TOPOLOGY TESTS
// ============================================================================

#[test]
fn test_primitive_topology_equality() {
    assert_eq!(PrimitiveTopology::TriangleList, PrimitiveTopology::TriangleList);
    assert_eq!(PrimitiveTopology::TriangleStrip, PrimitiveTopology::TriangleStrip);
    assert_eq!(PrimitiveTopology::LineList, PrimitiveTopology::LineList);
    assert_eq!(PrimitiveTopology::PointList, PrimitiveTopology::PointList);

    assert_ne!(PrimitiveTopology::TriangleList, PrimitiveTopology::LineList);
}

#[test]
fn test_primitive_topology_debug() {
    assert!(format!("{:?}", PrimitiveTopology::TriangleList).contains("TriangleList"));
    assert!(format!("{:?}", PrimitiveTopology::TriangleStrip).contains("TriangleStrip"));
    assert!(format!("{:?}", PrimitiveTopology::LineList).contains("LineList"));
    assert!(format!("{:?}", PrimitiveTopology::PointList).contains("PointList"));
}

#[test]
fn test_primitive_topology_clone() {
    let topo1 = PrimitiveTopology::TriangleList;
    let topo2 = topo1.clone();
    assert_eq!(topo1, topo2);
}

#[test]
fn test_primitive_topology_copy() {
    let topo1 = PrimitiveTopology::LineList;
    let topo2 = topo1; // Copy, not move
    assert_eq!(topo1, topo2);
}

// ============================================================================
// VERTEX INPUT RATE TESTS
// ============================================================================

#[test]
fn test_vertex_input_rate_equality() {
    assert_eq!(VertexInputRate::Vertex, VertexInputRate::Vertex);
    assert_eq!(VertexInputRate::Instance, VertexInputRate::Instance);
    assert_ne!(VertexInputRate::Vertex, VertexInputRate::Instance);
}

#[test]
fn test_vertex_input_rate_debug() {
    assert!(format!("{:?}", VertexInputRate::Vertex).contains("Vertex"));
    assert!(format!("{:?}", VertexInputRate::Instance).contains("Instance"));
}

#[test]
fn test_vertex_input_rate_clone() {
    let rate1 = VertexInputRate::Vertex;
    let rate2 = rate1.clone();
    assert_eq!(rate1, rate2);
}

#[test]
fn test_vertex_input_rate_copy() {
    let rate1 = VertexInputRate::Instance;
    let rate2 = rate1; // Copy, not move
    assert_eq!(rate1, rate2);
}

// ============================================================================
// VERTEX LAYOUT TESTS
// ============================================================================

#[test]
fn test_vertex_layout_default() {
    let layout = VertexLayout::default();
    assert_eq!(layout.bindings.len(), 0);
    assert_eq!(layout.attributes.len(), 0);
}

#[test]
fn test_vertex_layout_creation() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12, // 3 floats (x, y, z)
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32B32_SFLOAT, // vec3
                offset: 0,
            }
        ],
    };

    assert_eq!(layout.bindings.len(), 1);
    assert_eq!(layout.attributes.len(), 1);
    assert_eq!(layout.bindings[0].stride, 12);
    assert_eq!(layout.attributes[0].format, BufferFormat::R32G32B32_SFLOAT);
}

#[test]
fn test_vertex_layout_multiple_bindings() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12, // Position: vec3
                input_rate: VertexInputRate::Vertex,
            },
            VertexBinding {
                binding: 1,
                stride: 8, // UV: vec2
                input_rate: VertexInputRate::Vertex,
            },
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32B32_SFLOAT,
                offset: 0,
            },
            VertexAttribute {
                location: 1,
                binding: 1,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            },
        ],
    };

    assert_eq!(layout.bindings.len(), 2);
    assert_eq!(layout.attributes.len(), 2);
}

#[test]
fn test_vertex_layout_clone() {
    let layout1 = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    };

    let layout2 = layout1.clone();
    assert_eq!(layout1.bindings.len(), layout2.bindings.len());
    assert_eq!(layout1.attributes.len(), layout2.attributes.len());
}

#[test]
fn test_vertex_layout_debug() {
    let layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 12,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![],
    };

    let debug_str = format!("{:?}", layout);
    assert!(debug_str.contains("bindings"));
    assert!(debug_str.contains("attributes"));
}

// ============================================================================
// VERTEX BINDING TESTS
// ============================================================================

#[test]
fn test_vertex_binding_creation() {
    let binding = VertexBinding {
        binding: 0,
        stride: 20, // 5 floats
        input_rate: VertexInputRate::Vertex,
    };

    assert_eq!(binding.binding, 0);
    assert_eq!(binding.stride, 20);
    assert_eq!(binding.input_rate, VertexInputRate::Vertex);
}

#[test]
fn test_vertex_binding_debug() {
    let binding = VertexBinding {
        binding: 1,
        stride: 32,
        input_rate: VertexInputRate::Instance,
    };

    let debug_str = format!("{:?}", binding);
    assert!(debug_str.contains("binding"));
    assert!(debug_str.contains("stride"));
}

#[test]
fn test_vertex_binding_clone() {
    let binding1 = VertexBinding {
        binding: 0,
        stride: 12,
        input_rate: VertexInputRate::Vertex,
    };

    let binding2 = binding1.clone();
    assert_eq!(binding1.binding, binding2.binding);
    assert_eq!(binding1.stride, binding2.stride);
}

#[test]
fn test_vertex_binding_copy() {
    let binding1 = VertexBinding {
        binding: 0,
        stride: 8,
        input_rate: VertexInputRate::Vertex,
    };

    let binding2 = binding1; // Copy
    assert_eq!(binding1.stride, binding2.stride);
    assert_eq!(binding1.stride, 8); // Can still use binding1
}

// ============================================================================
// VERTEX ATTRIBUTE TESTS
// ============================================================================

#[test]
fn test_vertex_attribute_creation() {
    let attr = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32G32B32_SFLOAT,
        offset: 0,
    };

    assert_eq!(attr.location, 0);
    assert_eq!(attr.binding, 0);
    assert_eq!(attr.format, BufferFormat::R32G32B32_SFLOAT);
    assert_eq!(attr.offset, 0);
}

#[test]
fn test_vertex_attribute_with_offset() {
    let attr = VertexAttribute {
        location: 1,
        binding: 0,
        format: BufferFormat::R32G32_SFLOAT,
        offset: 12, // After a vec3 (12 bytes)
    };

    assert_eq!(attr.offset, 12);
}

#[test]
fn test_vertex_attribute_debug() {
    let attr = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32_SFLOAT,
        offset: 0,
    };

    let debug_str = format!("{:?}", attr);
    assert!(debug_str.contains("location"));
    assert!(debug_str.contains("format"));
}

#[test]
fn test_vertex_attribute_clone() {
    let attr1 = VertexAttribute {
        location: 2,
        binding: 1,
        format: BufferFormat::R32G32B32A32_SFLOAT,
        offset: 4,
    };

    let attr2 = attr1.clone();
    assert_eq!(attr1.location, attr2.location);
    assert_eq!(attr1.format, attr2.format);
}

#[test]
fn test_vertex_attribute_copy() {
    let attr1 = VertexAttribute {
        location: 0,
        binding: 0,
        format: BufferFormat::R32G32_SFLOAT,
        offset: 0,
    };

    let attr2 = attr1; // Copy
    assert_eq!(attr1.location, attr2.location);
    assert_eq!(attr1.location, 0); // Can still use attr1
}
