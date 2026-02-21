/// Allocates and recycles unique `u32` indices.
///
/// Manages a pool of slot indices for GPU buffers (SSBO) or any
/// system that needs stable, reusable integer identifiers.
/// Freed indices are recycled on subsequent allocations.
///
/// # Example
///
/// ```ignore
/// let mut alloc = SlotAllocator::new();
/// let a = alloc.alloc();  // 0
/// let b = alloc.alloc();  // 1
/// alloc.free(a);           // 0 is now available
/// let c = alloc.alloc();  // 0 (recycled)
/// ```
pub struct SlotAllocator {
    free_list: Vec<u32>,
    next_id: u32,
    len: u32,
}

impl SlotAllocator {
    /// Create a new empty allocator
    pub fn new() -> Self {
        Self {
            free_list: Vec::new(),
            next_id: 0,
            len: 0,
        }
    }

    /// Allocate the next available slot index
    pub fn alloc(&mut self) -> u32 {
        self.len += 1;
        self.free_list.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        })
    }

    /// Return a slot index to the pool for reuse
    pub fn free(&mut self, id: u32) {
        debug_assert!(id < self.next_id, "freeing an unallocated slot: {}", id);
        self.len -= 1;
        self.free_list.push(id);
    }

    /// Highest index ever allocated + 1.
    ///
    /// This is the minimum capacity the backing storage must have
    /// to accommodate all allocated indices.
    pub fn high_water_mark(&self) -> u32 {
        self.next_id
    }

    /// Number of currently allocated slots
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether no slots are currently allocated
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for SlotAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "slot_allocator_tests.rs"]
mod tests;
