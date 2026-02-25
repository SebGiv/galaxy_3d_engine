# Vulkan Pools — Technical Reference

> **Engine**: Galaxy3D Engine
> **Date**: 2026-02-25
> **File**: `galaxy_3d_engine_renderer_vulkan/src/vulkan.rs`

---

## Overview

Vulkan provides **no default resource management**. The application must explicitly create **pools** — pre-allocated reservoirs from which GPU resources are allocated. Vulkan has three native pool types, plus a fourth managed by the memory allocator library (VMA / gpu-allocator).

---

## 1. Descriptor Pool (`VkDescriptorPool`)

### What it allocates
**Descriptor sets** — GPU-side "pointers" to resources (textures, uniform buffers, storage buffers, samplers). Each material binding group requires one descriptor set.

### Vulkan constraints
- `max_sets`: hard limit on total descriptor sets allocable from the pool
- `pool_sizes`: per-type limits (e.g. 2048 samplers, 1024 UBOs, 1024 SSBOs)
- When either limit is reached: `VK_ERROR_OUT_OF_POOL_MEMORY`
- **No automatic growth** — the application must handle exhaustion

### Our implementation: Dynamic Growing Pool

Instead of one fixed-size pool, we maintain a `Mutex<Vec<VkDescriptorPool>>`. When the current pool is exhausted, a new pool is created automatically.

```
Pool layout:
  descriptor_pools: Mutex<Vec<VkDescriptorPool>>

  [ Pool_0 (1024 sets) ] ← allocations go here first
  [ Pool_0, Pool_1 ]     ← Pool_0 full, new pool created
  [ Pool_0, Pool_1, Pool_2 ] ← and so on...
```

Each pool has:
- `max_sets = 1024`
- `COMBINED_IMAGE_SAMPLER = 2048` (2 per set average)
- `UNIFORM_BUFFER = 1024`
- `STORAGE_BUFFER = 1024`

**Allocation flow** (`create_binding_group`):
1. Try allocating from the last pool in the Vec
2. If `ERROR_OUT_OF_POOL_MEMORY` → create new pool, push to Vec, retry
3. If retry fails → real error, propagate

**Performance impact**: zero. Once allocated, a descriptor set is a plain GPU handle. The GPU doesn't know which pool it came from. `vkCmdBindDescriptorSets` works identically regardless.

**Code location**: `vulkan.rs`
- Helper: `VulkanRenderer::create_descriptor_pool()`
- Allocation + growth: `create_binding_group()` (~line 1205)
- Cleanup: `drop()` — iterates and destroys all pools

---

## 2. Command Pool (`VkCommandPool`)

### What it allocates
**Command buffers** — recorded lists of GPU commands (draw calls, dispatches, copies, pipeline barriers).

### Vulkan constraints
- Each command pool is bound to a **queue family** (graphics, compute, transfer)
- Command buffers from pool X can only be submitted to queues of pool X's family
- No hard set limit, but memory grows with active command buffers
- Can be **reset** (`vkResetCommandPool`) — recycles all command buffers at once (cost ≈ 0)

### Flags
| Flag | Meaning |
|------|---------|
| `TRANSIENT` | Command buffers are short-lived (driver optimizes for this) |
| `RESET_COMMAND_BUFFER` | Individual command buffers can be reset and re-recorded |

### Our implementation
One upload command pool for CPU→GPU transfers (texture uploads, buffer copies):

```rust
flags: TRANSIENT | RESET_COMMAND_BUFFER
queue_family: graphics
```

Render command buffers are managed by the render graph system separately.

**Code location**: `vulkan.rs` — `upload_command_pool` in `GpuContext`

---

## 3. Query Pool (`VkQueryPool`)

### What it allocates
**GPU queries** — measurement slots for GPU-side profiling and testing.

### Query types
| Type | Purpose | Use case |
|------|---------|----------|
| `TIMESTAMP` | GPU clock value at a pipeline stage | Measuring render pass duration |
| `OCCLUSION` | Pixel count passing depth test | Visibility testing, LOD decisions |
| `PIPELINE_STATISTICS` | Vertex/fragment/invocation counts | Performance profiling |

### Vulkan constraints
- Fixed query count set at creation
- Results read back via `vkGetQueryPoolResults` (CPU) or `vkCmdCopyQueryPoolResults` (GPU→buffer)
- Must be reset before reuse (`vkCmdResetQueryPool`)

### Our implementation
**Not used yet.** Would be useful for:
- GPU profiling (timestamp queries per render pass)
- Occlusion culling (conditional rendering)

---

## 4. Memory Pool (VMA / gpu-allocator)

### What it manages
**GPU memory allocations** — the actual VRAM backing textures, buffers, and images.

### Why a pool is needed
Vulkan's `vkAllocateMemory` has a hard driver limit (~4096 allocations on many GPUs). Creating one allocation per texture/buffer would exhaust this quickly. Memory allocator libraries solve this by:

1. Allocating large memory **blocks** (e.g. 256 MB)
2. Sub-allocating within blocks for individual resources
3. Tracking free regions, defragmentation, etc.

### Our implementation
We use `gpu-allocator` (Rust crate), wrapped in `Arc<Mutex<Allocator>>`. It handles:
- Block allocation strategy
- Memory type selection (device-local, host-visible, etc.)
- Sub-allocation and free tracking

**Code location**: `vulkan.rs` — `allocator: ManuallyDrop<Arc<Mutex<Allocator>>>`

---

## Summary Table

| Pool | Allocates | Limit | Growth | In our engine |
|------|-----------|-------|--------|---------------|
| Descriptor Pool | Descriptor sets | `max_sets` per pool | Dynamic (Vec of pools) | Yes |
| Command Pool | Command buffers | No hard limit | Recyclable (reset) | Yes (upload) |
| Query Pool | GPU queries | Fixed at creation | Create new pool | Not yet |
| Memory Pool (VMA) | GPU memory | ~4096 Vulkan allocs | Automatic (sub-alloc) | Yes (gpu-allocator) |

---

## Modern Engine Strategies (Reference)

### Descriptor management evolution

| Strategy | Complexity | Description |
|----------|------------|-------------|
| **Fixed pool** | Low | One pool, fixed size. Fails on large scenes. |
| **Dynamic growing pool** | Low | Vec of pools, grow on exhaustion. **Our current approach.** |
| **Pool-per-frame + reset** | Medium | One pool per frame-in-flight, `vkResetDescriptorPool` each frame. Fast for dynamic descriptors. |
| **Descriptor caching** | Medium | Hash descriptor content, reuse identical sets. Reduces allocation count. |
| **Bindless / descriptor indexing** | High | One giant descriptor array (100k+ textures), indexed by `uint` in shader. Eliminates per-material descriptor sets. Requires `VK_EXT_descriptor_indexing`. Used by UE5/Nanite, Frostbite. |
| **VK_EXT_descriptor_buffer** | High | Descriptors stored in plain GPU buffers. No pools at all. Newest approach, limited driver support. |

### Recommended evolution path
1. **Now**: Dynamic growing pool (done)
2. **Next**: Pool-per-frame for dynamic descriptors (frame UBO, instance data)
3. **Later**: Bindless descriptor indexing for materials/textures (major shader refactor)
