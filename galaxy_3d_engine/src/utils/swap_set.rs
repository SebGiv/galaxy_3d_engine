/// Double-buffered HashSet with zero per-frame allocation.
///
/// Uses two `FxHashSet` buffers that alternate roles (front/back).
/// `flip(&self)` swaps the active index via `Cell` (interior mutability)
/// and returns a reference to the previous front (now back).
/// The new front is lazily cleared on the next `insert(&mut self)`,
/// preserving heap capacity across frames.
///
/// If `flip()` is called with no insert since the last flip (e.g. an
/// idle frame), a sentinel empty set is returned instead of stale data.
///
/// This avoids the allocation + deallocation cycle that `std::mem::take`
/// would cause every frame on a hot-path dirty set.

use std::cell::Cell;
use std::hash::Hash;
use rustc_hash::FxHashSet;

pub struct SwapSet<K> {
    buffers: [FxHashSet<K>; 2],
    /// Sentinel empty set returned when flip() is called with no prior insert.
    empty: FxHashSet<K>,
    active: Cell<u8>,
    pending_clear: Cell<bool>,
}

impl<K: Hash + Eq> SwapSet<K> {
    /// Create a new SwapSet with two empty buffers.
    pub fn new() -> Self {
        Self {
            buffers: [FxHashSet::default(), FxHashSet::default()],
            empty: FxHashSet::default(),
            active: Cell::new(0),
            pending_clear: Cell::new(false),
        }
    }

    /// Insert a key into the front buffer.
    ///
    /// If a flip happened since the last insert, the front is lazily
    /// cleared first (preserving heap capacity).
    pub fn insert(&mut self, key: K) -> bool {
        self.ensure_front_cleared();
        self.buffers[self.active.get() as usize].insert(key)
    }

    /// Remove a key from the front buffer.
    pub fn remove(&mut self, key: &K) -> bool {
        if self.pending_clear.get() {
            // Front is logically empty — nothing to remove.
            return false;
        }
        self.buffers[self.active.get() as usize].remove(key)
    }

    /// Check if the front buffer contains a key.
    pub fn contains(&self, key: &K) -> bool {
        if self.pending_clear.get() {
            return false;
        }
        self.buffers[self.active.get() as usize].contains(key)
    }

    /// Check if the front buffer is logically empty.
    pub fn is_empty(&self) -> bool {
        if self.pending_clear.get() {
            return true;
        }
        self.buffers[self.active.get() as usize].is_empty()
    }

    /// Number of elements in the front buffer.
    pub fn len(&self) -> usize {
        if self.pending_clear.get() {
            return 0;
        }
        self.buffers[self.active.get() as usize].len()
    }

    /// Flip the active buffer and return a reference to the previous front.
    ///
    /// The returned reference holds all keys inserted since the last flip.
    /// If no insert happened since the last flip (idle frame), returns an
    /// empty set — stale data is never exposed.
    ///
    /// Uses `Cell` for interior mutability — only flips a `u8` index,
    /// no unsafe code.
    pub fn flip(&self) -> &FxHashSet<K> {
        let old_active = self.active.get();
        self.active.set(1 - old_active);

        if self.pending_clear.get() {
            // No insert since last flip — front has stale data.
            // Return empty sentinel. pending stays true so next insert
            // will clear the new front.
            &self.empty
        } else {
            self.pending_clear.set(true);
            &self.buffers[old_active as usize]
        }
    }

    /// Get a reference to the back buffer without flipping.
    pub fn back(&self) -> &FxHashSet<K> {
        &self.buffers[(1 - self.active.get()) as usize]
    }

    /// Clear both buffers.
    pub fn clear(&mut self) {
        self.buffers[0].clear();
        self.buffers[1].clear();
        self.pending_clear.set(false);
    }

    /// Lazily clear the front buffer if a flip is pending.
    /// Preserves heap capacity (no allocation).
    fn ensure_front_cleared(&mut self) {
        if self.pending_clear.get() {
            self.buffers[self.active.get() as usize].clear();
            self.pending_clear.set(false);
        }
    }
}

#[cfg(test)]
#[path = "swap_set_tests.rs"]
mod tests;
