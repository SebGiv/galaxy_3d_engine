//! Draw-call queue with sort keys for front-to-back opaque rendering.
//!
//! Pattern inspired by modern engines (bgfx, Stingray): a preallocated queue
//! where each frame appends draw calls with a packed 64-bit sort key, then
//! sorts the key/index pairs (radix sort) before issuing GPU commands.
//!
//! Key design points:
//! - Zero per-frame allocation: `clear()` preserves capacity.
//! - Indirect sort: we sort a small `SortEntry { sort_key, index }` array,
//!   never the heavier `DrawCall` records themselves (cache-friendly).
//! - Sort-key layout (MSB → LSB):
//!     [63..48] pipeline signature id — preserves descriptor set binds
//!     [47..32] pipeline sort id       — groups identical pipelines
//!     [31..16] geometry sort id       — groups identical vertex/index buffers
//!     [15..0]  distance               — front-to-back for opaque passes

use rdst::{RadixKey, RadixSort};
use crate::graphics_device::DynamicRenderState;
use crate::resource::resource_manager::{GeometryKey, PipelineKey};

/// Convert a signed f32 to a sortable u32 (ascending order preserved,
/// including negatives). Standard "float sort key" trick.
#[inline]
fn f32_to_sortable_u32(f: f32) -> u32 {
    let bits = f.to_bits();
    if bits & 0x8000_0000 != 0 {
        !bits                     // negative: invert all bits
    } else {
        bits ^ 0x8000_0000        // positive: flip sign bit only
    }
}

/// Extract the 16 most-significant bits of the sortable representation of a
/// signed f32. Ascending u16 ↔ ascending f32 (including negatives).
#[inline]
pub fn distance_to_u16(distance: f32) -> u16 {
    (f32_to_sortable_u32(distance) >> 16) as u16
}

/// Build a packed 64-bit sort key for opaque draw-call sorting.
#[inline]
pub fn build_sort_key(
    signature_id: u16,
    pipeline_sort_id: u16,
    geometry_sort_id: u16,
    distance: f32,
) -> u64 {
    ((signature_id as u64)     << 48)
  | ((pipeline_sort_id as u64) << 32)
  | ((geometry_sort_id as u64) << 16)
  |  (distance_to_u16(distance) as u64)
}

/// One entry in the auxiliary sort array.
/// 16 bytes with natural alignment (4 entries per 64-byte cache line).
#[repr(C)]
#[derive(Copy, Clone)]
struct SortEntry {
    sort_key: u64,
    draw_call_index: u32,
    _pad: u32,
}

impl RadixKey for SortEntry {
    const LEVELS: usize = 8;
    #[inline]
    fn get_level(&self, level: usize) -> u8 {
        (self.sort_key >> (level * 8)) as u8
    }
}

/// Minimal per-draw-call data captured during the fill phase and consumed
/// during the emit phase.
#[derive(Clone)]
pub struct DrawCall {
    pub pipeline_key: PipelineKey,
    pub geometry_key: GeometryKey,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub draw_slot: u32,
    pub render_state: DynamicRenderState,
}

/// Preallocated queue of draw calls + sort entries.
///
/// Usage pattern per frame (no allocation):
/// ```ignore
/// queue.clear();
/// for visible in ... { queue.push(dc, sort_key); }
/// queue.sort();
/// for dc in queue.iter_sorted() { /* emit */ }
/// ```
pub struct RenderQueue {
    draw_calls: Vec<DrawCall>,
    sort_entries: Vec<SortEntry>,
}

impl RenderQueue {
    /// Create a queue preallocated for `capacity` draw calls.
    /// No further allocation will occur until this capacity is exceeded.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            draw_calls: Vec::with_capacity(capacity),
            sort_entries: Vec::with_capacity(capacity),
        }
    }

    /// Reset both vectors' length to 0. Capacity is preserved (no dealloc).
    #[inline]
    pub fn clear(&mut self) {
        self.draw_calls.clear();
        self.sort_entries.clear();
    }

    /// Append a draw call with its precomputed sort key.
    #[inline]
    pub fn push(&mut self, dc: DrawCall, sort_key: u64) {
        let index = self.draw_calls.len() as u32;
        self.draw_calls.push(dc);
        self.sort_entries.push(SortEntry { sort_key, draw_call_index: index, _pad: 0 });
    }

    /// Number of queued draw calls.
    #[inline]
    pub fn len(&self) -> usize {
        self.draw_calls.len()
    }

    /// Whether the queue is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.draw_calls.is_empty()
    }

    /// Sort entries ascending by their 64-bit sort key (LSD radix, unstable).
    #[inline]
    pub fn sort(&mut self) {
        self.sort_entries.radix_sort_unstable();
    }

    /// Iterate draw calls in sorted order. Each entry indexes into `draw_calls`.
    pub fn iter_sorted(&self) -> impl Iterator<Item = &DrawCall> + '_ {
        self.sort_entries
            .iter()
            .map(move |e| &self.draw_calls[e.draw_call_index as usize])
    }
}
