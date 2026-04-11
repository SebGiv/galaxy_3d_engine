//! Visible instance list — output of frustum culling.
//!
//! Each entry pairs a `RenderInstanceKey` with the view-space depth (f32) of
//! the instance, computed at culling time. The raw f32 depth is stored as-is;
//! consumers (sort key construction, LOD selection, distance fade, ...) are
//! free to convert it as they need.

use super::render_instance::RenderInstanceKey;

/// A visible instance with its view-space depth.
///
/// `distance` is the raw view-space depth in world units (positive in front
/// of the camera, negative behind). It is intentionally kept as a `f32` so
/// that multiple consumers — sort key encoding, LOD selection, distance
/// fade, etc. — can interpret it as needed without precision loss.
#[derive(Debug, Clone, Copy)]
pub struct VisibleInstance {
    pub key: RenderInstanceKey,
    pub distance: f32,
}

/// Encapsulates the list of visible instances produced by frustum culling.
///
/// Each entry pairs a `RenderInstanceKey` with the view-space depth.
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

    /// Push an instance with its view-space depth.
    pub fn push(&mut self, key: RenderInstanceKey, distance: f32) {
        self.instances.push(VisibleInstance { key, distance });
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
    fn test_push_with_distance() {
        let mut list = VisibleInstanceList::new();
        let key = make_key(1);
        list.push(key, 42.0);
        assert_eq!(list.len(), 1);
        assert_eq!(list.as_slice()[0].key, key);
        assert_eq!(list.as_slice()[0].distance, 42.0);
    }

    #[test]
    fn test_clear_preserves_capacity() {
        let mut list = VisibleInstanceList::with_capacity(64);
        for i in 1..=10 {
            list.push(make_key(i), i as f32);
        }
        assert_eq!(list.len(), 10);
        list.clear();
        assert!(list.is_empty());
    }

    #[test]
    fn test_iter() {
        let mut list = VisibleInstanceList::new();
        list.push(make_key(1), 100.0);
        list.push(make_key(2), 200.0);
        let collected: Vec<f32> = list.iter().map(|vi| vi.distance).collect();
        assert_eq!(collected, vec![100.0, 200.0]);
    }
}
