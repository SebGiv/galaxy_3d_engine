/// Buffer - Vulkan implementation of RendererBuffer trait

use galaxy_3d_engine::galaxy3d::{
    Result,
    render::{Buffer as RendererBuffer, BufferUpdateMode},
};
use galaxy_3d_engine::{engine_bail, engine_err};
use ash::vk;
use gpu_allocator::vulkan::Allocation;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::vulkan::FRAMES_IN_FLIGHT;
use crate::vulkan_context::GpuContext;

/// Sentinel value for `last_synced_slot` meaning "no slot has ever been
/// synchronised yet". Picked outside the valid range `[0, FRAMES_IN_FLIGHT)`
/// so the first `ensure_slot_synced()` always triggers a full memcpy.
const NO_SLOT_SYNCED: usize = usize::MAX;

/// Round `value` up to the nearest multiple of `alignment`.
/// `alignment` must be a power of two.
#[inline]
pub(crate) fn align_up(value: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two(), "alignment must be a power of two");
    (value + alignment - 1) & !(alignment - 1)
}

/// Vulkan buffer implementation
///
/// For `Static` mode: holds a single `VkBuffer` of `size` bytes.
/// For `Dynamic` mode: holds a single `VkBuffer` of `FRAMES_IN_FLIGHT × slot_size`
/// bytes, plus a CPU-side `master` of `slot_size` bytes that holds the
/// canonical state. Each frame's GPU slot is synchronised lazily from the
/// master via `ensure_slot_synced()` — the first `update()` or bind that
/// touches a new slot triggers a full memcpy from master to that slot.
pub struct Buffer {
    /// Shared GPU context (device, allocator, queue, command pool, fence cursor)
    pub(crate) ctx: Arc<GpuContext>,
    /// Vulkan buffer (size = `size` for Static, `FRAMES_IN_FLIGHT × slot_size` for Dynamic)
    pub(crate) buffer: vk::Buffer,
    /// GPU memory allocation
    pub(crate) allocation: Option<Allocation>,
    /// User-facing buffer size in bytes (what the user requested in `BufferDesc`)
    pub(crate) size: u64,
    /// How the buffer is updated. Drives `update()` / `mapped_ptr()` behaviour
    /// and how the rest of the backend computes bind-time offsets.
    pub(crate) update_mode: BufferUpdateMode,
    /// Padded size of one slot in bytes. For `Static` this equals `size`.
    /// For `Dynamic` this is `align_up(size, ctx.dynamic_buffer_alignment)`.
    pub(crate) slot_size: u64,

    /// CPU-side canonical content of the buffer. Empty for Static.
    /// For Dynamic, length == `slot_size`, zero-initialised at creation.
    /// Every `update()` writes to this master first, then the slot for the
    /// current frame-in-flight is synchronised lazily.
    master: Mutex<Vec<u8>>,
    /// Last frame-in-flight slot that was fully synchronised with `master`.
    /// `NO_SLOT_SYNCED` (sentinel) at startup so the very first call triggers
    /// a memcpy. Compared atomically against `ctx.current_submit_fence`.
    last_synced_slot: AtomicUsize,
}

impl Buffer {
    /// Create a Static-mode buffer (single slot, no padding, no master).
    pub fn new_static(
        ctx: Arc<GpuContext>,
        buffer: vk::Buffer,
        allocation: Allocation,
        size: u64,
    ) -> Self {
        Self {
            ctx,
            buffer,
            allocation: Some(allocation),
            size,
            update_mode: BufferUpdateMode::Static,
            slot_size: size,
            master: Mutex::new(Vec::new()),
            last_synced_slot: AtomicUsize::new(NO_SLOT_SYNCED),
        }
    }

    /// Create a Dynamic-mode buffer with `FRAMES_IN_FLIGHT` slots of
    /// `slot_size` bytes each (slot_size already padded to the device's
    /// dynamic alignment by the caller). The CPU master is allocated to
    /// `slot_size` bytes and zero-initialised.
    pub fn new_dynamic(
        ctx: Arc<GpuContext>,
        buffer: vk::Buffer,
        allocation: Allocation,
        user_size: u64,
        slot_size: u64,
    ) -> Self {
        Self {
            ctx,
            buffer,
            allocation: Some(allocation),
            size: user_size,
            update_mode: BufferUpdateMode::Dynamic,
            slot_size,
            master: Mutex::new(vec![0u8; slot_size as usize]),
            last_synced_slot: AtomicUsize::new(NO_SLOT_SYNCED),
        }
    }

    /// Byte offset of the slot the CPU is currently writing into (Dynamic only).
    /// Returns 0 for Static buffers.
    #[inline]
    pub(crate) fn current_slot_offset(&self) -> u64 {
        match self.update_mode {
            BufferUpdateMode::Static => 0,
            BufferUpdateMode::Dynamic => {
                let slot = self.ctx.current_submit_fence.load(Ordering::Relaxed) as u64;
                slot * self.slot_size
            }
        }
    }

    /// Synchronise the GPU slot for the current frame-in-flight with the CPU
    /// master via a single full memcpy. Cheap no-op when the slot was already
    /// synced this frame (or for Static buffers).
    ///
    /// Returns `true` if a memcpy was actually performed, `false` otherwise.
    /// Callers in `update()` use this return value to skip a redundant delta
    /// write (the memcpy already carries the freshly-written master content).
    pub(crate) fn ensure_slot_synced(&self) -> bool {
        if !matches!(self.update_mode, BufferUpdateMode::Dynamic) {
            return false;
        }
        let current = self.ctx.current_submit_fence.load(Ordering::Relaxed);
        let last = self.last_synced_slot.load(Ordering::Relaxed);
        if current == last {
            return false;
        }
        let allocation = self.allocation.as_ref()
            .expect("Dynamic buffer must have an allocation");
        let mapped = allocation.mapped_ptr()
            .expect("Dynamic buffer must be persistent-mapped")
            .as_ptr() as *mut u8;
        let master = self.master.lock().unwrap();
        unsafe {
            std::ptr::copy_nonoverlapping(
                master.as_ptr(),
                mapped.add((current as u64 * self.slot_size) as usize),
                self.slot_size as usize,
            );
        }
        drop(master);
        self.last_synced_slot.store(current, Ordering::Relaxed);
        true
    }
}

impl RendererBuffer for Buffer {
    fn update(&self, offset: u64, data: &[u8]) -> Result<()> {
        // Bounds check against the user-visible size (not the underlying
        // VkBuffer size, which is N × slot_size for Dynamic).
        let end = offset.checked_add(data.len() as u64)
            .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                "Buffer update failed: offset + data overflow"))?;
        if end > self.size {
            engine_bail!("galaxy3d::vulkan",
                "Buffer update failed: write past end of buffer (offset {} + {} bytes > size {})",
                offset, data.len(), self.size);
        }

        match self.update_mode {
            BufferUpdateMode::Static => unsafe {
                let allocation = self.allocation.as_ref()
                    .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                        "Buffer update failed: no GPU allocation"))?;
                let mapped = allocation
                    .mapped_ptr()
                    .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                        "Buffer update failed: buffer is not CPU-accessible"))?
                    .as_ptr() as *mut u8;
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped.add(offset as usize),
                    data.len(),
                );
                Ok(())
            },
            BufferUpdateMode::Dynamic => {
                // 1. Update the CPU master (canonical state).
                {
                    let mut master = self.master.lock().unwrap();
                    let start = offset as usize;
                    let end = start + data.len();
                    master[start..end].copy_from_slice(data);
                }

                // 2. Sync the current slot from master if first interaction with
                //    it this frame. If a full memcpy happened, the freshly-written
                //    delta is already in the slot and we're done.
                let did_full_sync = self.ensure_slot_synced();
                if did_full_sync {
                    return Ok(());
                }

                // 3. Slot was already synced this frame, but master has just
                //    changed — propagate only the delta to keep the slot in sync.
                let allocation = self.allocation.as_ref()
                    .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                        "Buffer update failed: no GPU allocation (Dynamic)"))?;
                let mapped = allocation
                    .mapped_ptr()
                    .ok_or_else(|| engine_err!("galaxy3d::vulkan",
                        "Buffer update failed: Dynamic buffer not persistent-mapped"))?
                    .as_ptr() as *mut u8;
                let absolute_offset = self.current_slot_offset() + offset;
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        mapped.add(absolute_offset as usize),
                        data.len(),
                    );
                }
                Ok(())
            }
        }
    }

    fn mapped_ptr(&self) -> Option<*mut u8> {
        // For Dynamic, ensure the slot is synchronised before exposing a
        // pointer to it (otherwise the caller may read/write stale data).
        self.ensure_slot_synced();
        self.allocation.as_ref()
            .and_then(|alloc| alloc.mapped_ptr())
            .map(|ptr| {
                let base = ptr.as_ptr() as *mut u8;
                // SAFETY: current_slot_offset() is always within the underlying
                // allocation by construction (slot * slot_size < N × slot_size).
                unsafe { base.add(self.current_slot_offset() as usize) }
            })
    }

    fn update_mode(&self) -> BufferUpdateMode {
        self.update_mode
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            // Free GPU memory
            if let Some(allocation) = self.allocation.take() {
                if let Ok(mut allocator) = self.ctx.allocator.lock() {
                    allocator.free(allocation).ok();
                }
            }

            // Destroy buffer
            self.ctx.device.destroy_buffer(self.buffer, None);
        }
    }
}

// Suppress unused import warning for FRAMES_IN_FLIGHT — referenced only in
// doc comments at the moment, but useful for future tooling that scrapes
// the symbol.
#[allow(dead_code)]
const _: usize = FRAMES_IN_FLIGHT;
