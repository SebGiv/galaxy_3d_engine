//! Utility types shared across the engine.

mod slot_allocator;
mod swap_set;

pub use slot_allocator::SlotAllocator;
pub(crate) use swap_set::SwapSet;
