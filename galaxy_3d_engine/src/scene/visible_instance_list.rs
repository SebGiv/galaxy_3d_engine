//! Visible instance list — output of frustum culling.
//!
//! Each entry pairs a `RenderInstanceKey` with a pre-computed u16 distance
//! from the camera, used as a sort key component for draw call sorting.

use super::render_instance::RenderInstanceKey;

/// A visible instance with its pre-computed sort distance.
///
/// `distance` is the high-order 16 bits of the f32 view-space depth
/// (i.e. `(depth_f32.to_bits() >> 16) as u16`). The IEEE 754 representation
/// of positive floats preserves ordering through the bit shift, so this u16
/// sorts equivalently to the original f32. It is intended to be combined
/// with other 16-bit components into a u64 sort key.
///
/// Negative depths (instances behind the camera) encode to large u16 values
/// (sign bit preserved at bit 15), so they sort after all positive depths.
#[derive(Debug, Clone, Copy)]
pub struct VisibleInstance {
    pub key: RenderInstanceKey,
    pub distance: u16,
}

/// Encapsulates the list of visible instances produced by frustum culling.
///
/// Each entry pairs a `RenderInstanceKey` with a pre-computed u16 distance.
/// Created by `CameraCuller::cull` (and ultimately by `SceneIndex::query_frustum`
/// when a spatial index is used).
#[derive(Debug, Clone, Default)]
pub struct VisibleInstanceList {
    instances: Vec<VisibleInstance>,
}

impl VisibleInstanceList {
    /// Create an empty list.
    pub fn new() -> Self {
        Self { instances: Vec::new() }
    }

    /// Create with a pre-allocated capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self { instances: Vec::with_capacity(cap) }
    }

    /// Push an instance with a pre-encoded u16 distance.
    pub fn push(&mut self, key: RenderInstanceKey, distance: u16) {
        self.instances.push(VisibleInstance { key, distance });
    }

    /// Push an instance, encoding the f32 view-space depth to its u16 sort form.
    ///
    /// Encoding: `(depth.to_bits() >> 16) as u16` — the high 16 bits of the f32
    /// IEEE 754 representation. Sign-preserving: positive depths sort first
    /// (front-to-back), negative depths sort last (sign bit set in bit 15).
    pub fn push_with_depth(&mut self, key: RenderInstanceKey, depth: f32) {
        let encoded = (depth.to_bits() >> 16) as u16;
        self.instances.push(VisibleInstance { key, distance: encoded });
    }

    /// Number of visible instances.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Read-only slice of all visible instances.
    pub fn as_slice(&self) -> &[VisibleInstance] {
        &self.instances
    }

    /// Iterator over `VisibleInstance` entries.
    pub fn iter(&self) -> std::slice::Iter<'_, VisibleInstance> {
        self.instances.iter()
    }

    /// Clear the list (preserves capacity for reuse).
    pub fn clear(&mut self) {
        self.instances.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    fn make_key(idx: u32) -> RenderInstanceKey {
        let mut sm = SlotMap::<RenderInstanceKey, ()>::with_key();
        let mut key = sm.insert(());
        for _ in 1..idx {
            key = sm.insert(());
        }
        key
    }

    #[test]
    fn test_new_is_empty() {
        let list = VisibleInstanceList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_push_with_pre_encoded_distance() {
        let mut list = VisibleInstanceList::new();
        let key = make_key(1);
        list.push(key, 42);
        assert_eq!(list.len(), 1);
        assert_eq!(list.as_slice()[0].key, key);
        assert_eq!(list.as_slice()[0].distance, 42);
    }

    #[test]
    fn test_push_with_depth_encodes_high_bits() {
        let mut list = VisibleInstanceList::new();
        let key = make_key(1);
        // 1.0_f32 → 0x3F800000 → high 16 bits = 0x3F80
        list.push_with_depth(key, 1.0);
        assert_eq!(list.as_slice()[0].distance, 0x3F80);
    }

    #[test]
    fn test_push_with_depth_preserves_order_for_positive() {
        let mut list = VisibleInstanceList::new();
        list.push_with_depth(make_key(1), 1.0);
        list.push_with_depth(make_key(2), 2.0);
        list.push_with_depth(make_key(3), 10.0);
        // Encoded distances should be strictly increasing (IEEE 754 sortable)
        let s = list.as_slice();
        assert!(s[0].distance < s[1].distance);
        assert!(s[1].distance < s[2].distance);
    }

    #[test]
    fn test_push_with_depth_negative_sorts_after_positive() {
        let mut list = VisibleInstanceList::new();
        list.push_with_depth(make_key(1), 5.0);   // positive
        list.push_with_depth(make_key(2), -1.0);  // negative (behind camera)
        // Negative float has bit 31 set → high 16 bits >= 0x8000 → > 32768
        // Positive float < 65504 → high 16 bits < 0x8000
        let s = list.as_slice();
        assert!(s[0].distance < 0x8000);
        assert!(s[1].distance >= 0x8000);
    }

    #[test]
    fn test_clear_preserves_capacity() {
        let mut list = VisibleInstanceList::with_capacity(64);
        for i in 1..=10 {
            list.push(make_key(i), i as u16);
        }
        assert_eq!(list.len(), 10);
        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn test_iter() {
        let mut list = VisibleInstanceList::new();
        list.push(make_key(1), 100);
        list.push(make_key(2), 200);
        let collected: Vec<u16> = list.iter().map(|vi| vi.distance).collect();
        assert_eq!(collected, vec![100, 200]);
    }
}
