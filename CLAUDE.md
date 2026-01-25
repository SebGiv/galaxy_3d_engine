# Galaxy3DEngine - Design Document

> **Project**: Multi-API 3D Rendering Engine in Rust
> **Author**: Claude & User collaboration
> **Date**: 2026-01-25
> **Status**: Phase 3 - Graphics Pipeline ✅

---

## 🎯 Project Goals

Create a modern 3D rendering engine in Rust with:
- **Multi-API abstraction**: Support for Vulkan (and future Direct3D 12)
- **Plugin architecture**: Backends loaded as plugins
- **High performance**: Zero-cost abstractions with trait-based polymorphism
- **Safety**: Leverage Rust's memory safety guarantees
- **Modern features**: Full graphics pipeline, multi-screen support, proper resource management

---

## 📋 Core Design Decisions

### 1. Trait-Based Polymorphism (C++-Style Dynamic Inheritance)

**Architecture**: Traits with `Arc<dyn Trait>` for polymorphic GPU resources

**Key Design**:
- **Core Engine** (`galaxy_3d_engine`): Platform-agnostic trait definitions
- **Backend Plugins** (`galaxy_3d_engine_renderer_vulkan`): Concrete implementations
- **Runtime Polymorphism**: All resources use `Arc<dyn Trait>` for backend flexibility
- **Plugin Registration**: Factory function pattern for renderer creation

**Resource Traits**:
- `Renderer` - Main renderer interface (factory for all resources)
- `RendererTexture` - GPU texture handle
- `RendererBuffer` - GPU buffer handle (vertex, index, uniform)
- `RendererShader` - Compiled shader module handle
- `RendererPipeline` - Graphics pipeline state handle
- `RendererFrame` - Per-frame command recording interface

**Key Benefits**:
- Complete backend isolation at compile time
- Runtime backend selection via plugins
- Clean API similar to C++ virtual inheritance
- Automatic resource cleanup via RAII (Drop trait)

---

### 2. Plugin System

**Registration Pattern**:
```rust
// In galaxy_3d_engine_renderer_vulkan/src/lib.rs
pub fn register() {
    galaxy_3d_engine::register_renderer_plugin("vulkan", create_vulkan_renderer);
}

fn create_vulkan_renderer(
    window: &Window,
    config: RendererConfig,
) -> RenderResult<Arc<Mutex<dyn Renderer>>> {
    let renderer = VulkanRenderer::new(window, config)?;
    Ok(Arc::new(Mutex::new(renderer)))
}
```

**Usage**:
```rust
// In your application
galaxy_3d_engine_renderer_vulkan::register();

let renderer = galaxy_3d_engine::renderer_plugin_registry()
    .lock().unwrap()
    .as_ref().unwrap()
    .create_renderer("vulkan", &window, config)?;
```

---

### 3. Memory Management

**Decision**: Integrate `gpu-allocator` with proper lifecycle management

**Implementation**:
- All GPU resources use `Arc<Mutex<Allocator>>` for shared memory management
- Resources implement `Drop` trait for automatic cleanup
- `ManuallyDrop` used in renderer to control destruction order
- Allocator destroyed BEFORE Vulkan device to prevent validation errors

**Critical Pattern** (Vulkan):
```rust
pub struct VulkanRenderer {
    // ... other fields ...
    allocator: ManuallyDrop<Arc<Mutex<Allocator>>>,
    device: Arc<ash::Device>,
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            // ... destroy other resources ...

            // CRITICAL: Drop allocator before device
            ManuallyDrop::drop(&mut self.allocator);

            self.device.destroy_device(None);
        }
    }
}
```

---

### 4. Multi-Screen Support & Swapchain Recreation

**Implementation**:
- Window resize detection via `WindowEvent::Resized`
- Automatic swapchain recreation when window moves between monitors
- Proper handling of `VK_ERROR_OUT_OF_DATE_KHR` and `VK_SUBOPTIMAL_KHR`
- Frame skipping during swapchain recreation

**Key Method**:
```rust
fn recreate_swapchain(&mut self) -> RenderResult<()> {
    // Wait for GPU idle
    // Destroy old framebuffers and image views
    // Query new surface capabilities
    // Recreate swapchain with new dimensions
    // Recreate framebuffers and image views
}
```

---

## 🏗️ Architecture Overview

### Cargo Workspace Structure

```
Galaxy3DEngine/                          # Workspace root
├── Cargo.toml                           # Workspace manifest
│
├── galaxy_3d_engine/                    # Core engine (trait definitions)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                       # Public API exports
│       ├── plugin.rs                    # Plugin registry for backends
│       └── renderer/
│           ├── mod.rs
│           ├── renderer.rs              # Renderer trait (main interface)
│           ├── renderer_texture.rs      # Texture trait & descriptor
│           ├── renderer_buffer.rs       # Buffer trait & descriptor
│           ├── renderer_shader.rs       # Shader trait & descriptor
│           ├── renderer_pipeline.rs     # Pipeline trait & descriptor
│           └── renderer_frame.rs        # Frame trait (command recording)
│
└── galaxy_3d_engine_renderer_vulkan/   # Vulkan backend plugin
    ├── Cargo.toml
    └── src/
        ├── lib.rs                       # Plugin registration
        ├── vulkan_renderer.rs           # VulkanRenderer (implements Renderer)
        ├── vulkan_renderer_texture.rs   # VulkanRendererTexture
        ├── vulkan_renderer_buffer.rs    # VulkanRendererBuffer
        ├── vulkan_renderer_shader.rs    # VulkanRendererShader
        ├── vulkan_renderer_pipeline.rs  # VulkanRendererPipeline
        └── vulkan_renderer_frame.rs     # VulkanRendererFrame
```

### Key Architecture Principles

1. **Trait-Based Polymorphism**: All resources are `Arc<dyn Trait>` for runtime flexibility
2. **Plugin Registry**: Global registry for backend factory functions
3. **RAII Resource Management**: Drop trait ensures proper cleanup
4. **Frame-in-Flight Synchronization**: 2 frames in flight with fences and semaphores
5. **Proper Destruction Order**: ManuallyDrop ensures GPU resources freed before device

---

## 🎨 Rendering Pipeline - Current Implementation

### ✅ Phase 1-3: Complete Graphics Pipeline (DONE)

**Implemented Features**:
- [x] Vulkan instance, device, and queue management
- [x] Swapchain with automatic recreation
- [x] Render pass and framebuffers
- [x] Command buffers (2 frames in flight)
- [x] Synchronization (fences, semaphores)
- [x] GPU memory allocation (gpu-allocator)
- [x] Vertex buffers with staging
- [x] SPIR-V shader loading (vertex + fragment)
- [x] Graphics pipeline with vertex input layout
- [x] Multi-screen support (4K ↔ HD tested)
- [x] Proper resource cleanup (no validation errors)

**Demo Status**: `galaxy3d_demo` renders a colored triangle with multi-screen support ✅

**Vulkan Validation**: All layers pass with zero errors ✅

---

## 🔧 Vulkan Implementation Details

### Synchronization Model

**Frames in Flight**: 2 concurrent frames

**Semaphores**:
- `image_available_semaphores[2]` - Indexed by `current_frame`
- `render_finished_semaphores[image_count]` - Indexed by `image_idx`

**Fences**:
- `in_flight_fences[2]` - Indexed by `current_frame`

**Frame Flow**:
```rust
fn begin_frame() {
    // 1. Wait for fence from 2 frames ago
    wait_for_fences(&in_flight_fences[current_frame]);
    reset_fences(&in_flight_fences[current_frame]);

    // 2. Acquire next swapchain image
    let image_idx = acquire_next_image(
        semaphore: image_available_semaphores[current_frame]
    );

    // 3. Reset and begin command buffer
    reset_command_buffer(command_buffers[current_frame]);
    begin_command_buffer(command_buffers[current_frame]);
}

fn end_frame() {
    // 1. End command buffer
    end_command_buffer(command_buffers[current_frame]);

    // 2. Submit to queue
    queue_submit(
        wait_semaphore: image_available_semaphores[current_frame],
        signal_semaphore: render_finished_semaphores[image_idx],
        fence: in_flight_fences[current_frame]
    );

    // 3. Present
    queue_present(
        wait_semaphore: render_finished_semaphores[image_idx],
        image_index: image_idx
    );

    // 4. Advance frame
    current_frame = (current_frame + 1) % 2;
}
```

### Resource Destruction Order

**Critical**: GPU resources must be destroyed before the Vulkan device.

**VulkanRenderer Drop Order**:
1. Wait for device idle
2. Destroy synchronization primitives (semaphores, fences)
3. Destroy command pool (frees command buffers)
4. Destroy framebuffers
5. Destroy render pass
6. Destroy image views
7. Destroy swapchain
8. Destroy surface
9. **Drop allocator** (ManuallyDrop::drop)
10. Destroy device
11. Destroy instance

**VulkanRendererBuffer Drop**:
```rust
impl Drop for VulkanRendererBuffer {
    fn drop(&mut self) {
        unsafe {
            // 1. Free GPU memory (while device still valid)
            if let Some(allocation) = self.allocation.take() {
                if let Ok(mut allocator) = self.allocator.lock() {
                    allocator.free(allocation).ok();
                }
            }

            // 2. Destroy Vulkan buffer
            self.device.destroy_buffer(self.buffer, None);
        }
    }
}
```

---

## 📦 Dependencies

### galaxy_3d_engine (Core)
- `winit = "0.30"` - Cross-platform window creation
- `raw-window-handle = "0.6"` - Platform-agnostic window handles

### galaxy_3d_engine_renderer_vulkan (Vulkan Backend)
- `galaxy_3d_engine` - Core trait definitions
- `ash = "0.38"` - Low-level Vulkan bindings
- `ash-window = "0.13"` - Vulkan surface creation
- `gpu-allocator = "0.27"` - GPU memory allocator
- `winit = "0.30"` - Window system integration
- `raw-window-handle = "0.6"` - Window handle conversion

---

## 🚀 Getting Started

### Prerequisites
- Rust 1.92+ (2024 edition)
- Vulkan SDK 1.4+
- GPU with Vulkan 1.3+ support

### Build & Run Demo
```bash
cd F:/dev/rust/Galaxy/Games/galaxy3d_demo
cargo run
```

### Using the Engine

See [USAGE.md](./USAGE.md) (English) or [USAGE.fr.md](./USAGE.fr.md) (French) for complete API documentation.

**Quick Example**:
```rust
use galaxy_3d_engine::{Renderer, RendererConfig};
use galaxy_3d_engine_renderer_vulkan;

// Register Vulkan backend
galaxy_3d_engine_renderer_vulkan::register();

// Create renderer
let renderer = galaxy_3d_engine::renderer_plugin_registry()
    .lock().unwrap()
    .as_ref().unwrap()
    .create_renderer("vulkan", &window, config)?;

// Create resources
let vertex_buffer = renderer.lock().unwrap().create_buffer(desc)?;
let pipeline = renderer.lock().unwrap().create_pipeline(desc)?;

// Render loop
loop {
    let frame = renderer.lock().unwrap().begin_frame()?;
    frame.bind_pipeline(&pipeline)?;
    frame.bind_vertex_buffer(&vertex_buffer, 0)?;
    frame.draw(3, 0)?;
    renderer.lock().unwrap().end_frame(frame)?;
}
```

---

## 📝 Code Style Guidelines

### Naming Conventions
- **Traits**: `Renderer`, `RendererBuffer` (PascalCase with "Renderer" prefix)
- **Structs**: `VulkanRenderer`, `VulkanRendererBuffer` (backend prefix + trait name)
- **Functions**: `create_buffer`, `begin_frame` (snake_case)
- **Constants**: `MAX_FRAMES_IN_FLIGHT` (SCREAMING_SNAKE_CASE)

### Documentation
- All public traits and methods have doc comments
- Examples included for complex operations
- Safety notes for unsafe code

### Error Handling
- `RenderResult<T>` = `Result<T, RenderError>`
- Detailed error messages with context
- Never `unwrap()` in library code

---

## ✅ Changelog

### 2026-01-25 - Complete Graphics Pipeline Implementation
- **Architecture Refactor**: Renamed crates to `galaxy_3d_engine` and `galaxy_3d_engine_renderer_vulkan`
- **Trait-Based Polymorphism**: Implemented C++-style dynamic inheritance with `Arc<dyn Trait>`
- **Vulkan Backend**: Full implementation with:
  - ✅ Triangle rendering with vertex buffers
  - ✅ SPIR-V shader loading (vertex + fragment)
  - ✅ Graphics pipeline with vertex input layouts
  - ✅ 2 frames in flight synchronization
  - ✅ Multi-screen support with swapchain recreation
  - ✅ Proper resource cleanup (zero validation errors)
- **Memory Management**:
  - ✅ `gpu-allocator` integration
  - ✅ `ManuallyDrop` for correct destruction order
  - ✅ RAII pattern for automatic cleanup
- **Demo**: `galaxy3d_demo` renders colored triangle, tested across 4K and HD monitors

### 2026-01-24 - Initial Design & Workspace Setup
- Created project structure
- Defined core trait abstractions
- Set up plugin system architecture
- Basic Vulkan initialization

---

## 📚 References

- [Vulkan Tutorial](https://vulkan-tutorial.com/)
- [Ash Documentation](https://docs.rs/ash/)
- [gpu-allocator Documentation](https://docs.rs/gpu-allocator/)
- [Vulkan Specification](https://registry.khronos.org/vulkan/specs/1.3/)
