use super::*;

// ============================================================================
// ShaderStageFlags constants
// ============================================================================

#[test]
fn test_constants_vertex() {
    assert_eq!(ShaderStageFlags::VERTEX.bits(), 0x01);
}

#[test]
fn test_constants_fragment() {
    assert_eq!(ShaderStageFlags::FRAGMENT.bits(), 0x02);
}

#[test]
fn test_constants_compute() {
    assert_eq!(ShaderStageFlags::COMPUTE.bits(), 0x04);
}

#[test]
fn test_constants_vertex_fragment() {
    assert_eq!(ShaderStageFlags::VERTEX_FRAGMENT.bits(), 0x03);
}

#[test]
fn test_constants_all() {
    assert_eq!(ShaderStageFlags::ALL.bits(), 0x07);
}

// ============================================================================
// from_stages
// ============================================================================

#[test]
fn test_from_stages_empty() {
    let f = ShaderStageFlags::from_stages(&[]);
    assert_eq!(f.bits(), 0);
}

#[test]
fn test_from_stages_vertex_only() {
    let f = ShaderStageFlags::from_stages(&[ShaderStage::Vertex]);
    assert_eq!(f.bits(), 0x01);
}

#[test]
fn test_from_stages_fragment_only() {
    let f = ShaderStageFlags::from_stages(&[ShaderStage::Fragment]);
    assert_eq!(f.bits(), 0x02);
}

#[test]
fn test_from_stages_compute_only() {
    let f = ShaderStageFlags::from_stages(&[ShaderStage::Compute]);
    assert_eq!(f.bits(), 0x04);
}

#[test]
fn test_from_stages_vertex_fragment() {
    let f = ShaderStageFlags::from_stages(&[ShaderStage::Vertex, ShaderStage::Fragment]);
    assert_eq!(f.bits(), 0x03);
}

#[test]
fn test_from_stages_all() {
    let f = ShaderStageFlags::from_stages(&[
        ShaderStage::Vertex, ShaderStage::Fragment, ShaderStage::Compute,
    ]);
    assert_eq!(f.bits(), 0x07);
}

#[test]
fn test_from_stages_dedup_idempotent() {
    let f = ShaderStageFlags::from_stages(&[ShaderStage::Vertex, ShaderStage::Vertex]);
    assert_eq!(f.bits(), 0x01);
}

// ============================================================================
// from_bits
// ============================================================================

#[test]
fn test_from_bits_zero() {
    assert_eq!(ShaderStageFlags::from_bits(0).bits(), 0);
}

#[test]
fn test_from_bits_all() {
    assert_eq!(ShaderStageFlags::from_bits(0x07).bits(), 0x07);
}

#[test]
fn test_from_bits_round_trip() {
    let original = ShaderStageFlags::from_stages(&[ShaderStage::Fragment, ShaderStage::Compute]);
    let round = ShaderStageFlags::from_bits(original.bits());
    assert_eq!(original, round);
}

// ============================================================================
// contains_*
// ============================================================================

#[test]
fn test_contains_vertex_when_present() {
    assert!(ShaderStageFlags::VERTEX.contains_vertex());
    assert!(ShaderStageFlags::VERTEX_FRAGMENT.contains_vertex());
    assert!(ShaderStageFlags::ALL.contains_vertex());
}

#[test]
fn test_contains_vertex_when_absent() {
    assert!(!ShaderStageFlags::FRAGMENT.contains_vertex());
    assert!(!ShaderStageFlags::COMPUTE.contains_vertex());
    assert!(!ShaderStageFlags::from_bits(0).contains_vertex());
}

#[test]
fn test_contains_fragment_when_present() {
    assert!(ShaderStageFlags::FRAGMENT.contains_fragment());
    assert!(ShaderStageFlags::VERTEX_FRAGMENT.contains_fragment());
    assert!(ShaderStageFlags::ALL.contains_fragment());
}

#[test]
fn test_contains_fragment_when_absent() {
    assert!(!ShaderStageFlags::VERTEX.contains_fragment());
    assert!(!ShaderStageFlags::COMPUTE.contains_fragment());
}

#[test]
fn test_contains_compute_when_present() {
    assert!(ShaderStageFlags::COMPUTE.contains_compute());
    assert!(ShaderStageFlags::ALL.contains_compute());
}

#[test]
fn test_contains_compute_when_absent() {
    assert!(!ShaderStageFlags::VERTEX.contains_compute());
    assert!(!ShaderStageFlags::FRAGMENT.contains_compute());
    assert!(!ShaderStageFlags::VERTEX_FRAGMENT.contains_compute());
}

// ============================================================================
// Equality / Hash / Clone
// ============================================================================

#[test]
fn test_equality_same_bits() {
    let a = ShaderStageFlags::VERTEX_FRAGMENT;
    let b = ShaderStageFlags::from_bits(0x03);
    assert_eq!(a, b);
}

#[test]
fn test_inequality_different_bits() {
    assert_ne!(ShaderStageFlags::VERTEX, ShaderStageFlags::FRAGMENT);
}

#[test]
fn test_clone_copy_preserve_value() {
    let a = ShaderStageFlags::ALL;
    let b = a;
    let c = a.clone();
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn test_hashable_in_hash_set() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(ShaderStageFlags::VERTEX);
    set.insert(ShaderStageFlags::VERTEX);
    set.insert(ShaderStageFlags::FRAGMENT);
    assert_eq!(set.len(), 2);
}

// ============================================================================
// BindingType / BindingSlotDesc / BindingGroupLayoutDesc
// ============================================================================

#[test]
fn test_binding_type_equality_and_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(BindingType::UniformBuffer);
    set.insert(BindingType::UniformBuffer);
    set.insert(BindingType::CombinedImageSampler);
    set.insert(BindingType::StorageBuffer);
    assert_eq!(set.len(), 3);
}

#[test]
fn test_binding_slot_desc_clone_preserves_fields() {
    let desc = BindingSlotDesc {
        binding: 7,
        binding_type: BindingType::StorageBuffer,
        count: 3,
        stage_flags: ShaderStageFlags::COMPUTE,
    };
    let cloned = desc.clone();
    assert_eq!(cloned.binding, 7);
    assert_eq!(cloned.binding_type, BindingType::StorageBuffer);
    assert_eq!(cloned.count, 3);
    assert_eq!(cloned.stage_flags, ShaderStageFlags::COMPUTE);
}

#[test]
fn test_binding_group_layout_desc_clone_preserves_entries() {
    let layout = BindingGroupLayoutDesc {
        entries: vec![
            BindingSlotDesc {
                binding: 0,
                binding_type: BindingType::UniformBuffer,
                count: 1,
                stage_flags: ShaderStageFlags::VERTEX_FRAGMENT,
            },
            BindingSlotDesc {
                binding: 1,
                binding_type: BindingType::CombinedImageSampler,
                count: 4,
                stage_flags: ShaderStageFlags::FRAGMENT,
            },
        ],
    };
    let cloned = layout.clone();
    assert_eq!(cloned.entries.len(), 2);
    assert_eq!(cloned.entries[0].binding, 0);
    assert_eq!(cloned.entries[1].count, 4);
}
