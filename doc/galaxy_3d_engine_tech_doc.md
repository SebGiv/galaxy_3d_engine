# Galaxy3DEngine - Technical Documentation

> **Version**: 0.1.0 (Phase 9 - Backend-Agnostic API Completed)
> **Last Updated**: 2026-01-30
> **Status**: Production-Ready Core, Advanced Features Planned

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Overview](#architecture-overview)
3. [Project Structure](#project-structure)
4. [Core Design Principles](#core-design-principles)
5. [Trait Hierarchy](#trait-hierarchy)
6. [Resource Management](#resource-management)
7. [Rendering Pipeline](#rendering-pipeline)
8. [Vulkan Backend Implementation](#vulkan-backend-implementation)
9. [Galaxy Image Library](#galaxy-image-library)
10. [Demo Application](#demo-application)
11. [Design Patterns](#design-patterns)
12. [Performance Considerations](#performance-considerations)
13. [Thread Safety & Synchronization](#thread-safety--synchronization)
14. [Error Handling](#error-handling)
15. [Future Extensibility](#future-extensibility)
16. [API Reference Summary](#api-reference-summary)

---

## Executive Summary

**Galaxy3DEngine** is a sophisticated, trait-based 3D rendering engine built in Rust with complete platform abstraction. It leverages Rust's trait system to decouple the abstract rendering API from backend implementations (currently Vulkan, with D3D12 planned).

### Key Features

- **Multi-API Abstraction**: Backend-agnostic trait-based design
- **Zero-Cost Abstractions**: Trait objects with minimal runtime overhead
- **Thread-Safe**: All APIs are `Send + Sync`
- **RAII Resource Management**: Automatic cleanup via Drop trait
- **Plugin Architecture**: Runtime backend selection
- **Comprehensive Validation**: Optional Vulkan validation layers with statistics
- **Modern Rendering**: Push constants, descriptor sets, render targets, multi-pass

### Technology Stack

| Component | Technology |
|-----------|------------|
| Language | Rust 2024 Edition |
| Graphics API | Vulkan 1.3+ |
| Window Management | winit 0.30 |
| GPU Memory | gpu-allocator 0.27 |
| Image Loading | Custom galaxy_image library |
| Validation | Vulkan Validation Layers |

---

## Architecture Overview

### Multi-Crate Organization

The project is organized as a Cargo workspace with specialized crates:

```
Galaxy/
├── Tools/
│   ├── galaxy_3d_engine/           (Workspace root)
│   │   ├── galaxy_3d_engine/       (Core traits & types)
│   │   └── galaxy_3d_engine_renderer_vulkan/  (Vulkan backend)
│   │
│   └── galaxy_image/               (Image loading library)
│
└── Games/
    └── galaxy3d_demo/              (Demo application)
```

### Separation of Concerns

1. **galaxy_3d_engine** (Core Library)
   - Defines all public trait interfaces
   - Platform-agnostic types (BufferDesc, TextureDesc, etc.)
   - Plugin registry system
   - Error types

2. **galaxy_3d_engine_renderer_vulkan** (Backend)
   - Concrete Vulkan implementations of all traits
   - Ash bindings for Vulkan API
   - GPU memory allocation (gpu-allocator)
   - Debug messenger and validation

3. **galaxy_image** (Utility Library)
   - PNG, BMP, JPEG loading/saving
   - Automatic format detection
   - Pixel format conversion

4. **galaxy3d_demo** (Application)
   - Example usage of the engine
   - Textured quad rendering
   - Demonstrates texture loading and rendering

### Design Philosophy

**Core Principles:**

- **Trait-Based Polymorphism**: All resources exposed as `Arc<dyn Trait>` or `Box<dyn Trait>`
- **Complete Backend Abstraction**: No Vulkan/D3D12 types leak into public API
- **Type Safety**: Strongly typed resource descriptors
- **Manual Memory Control**: Explicit resource creation with RAII cleanup
- **Thread Safety**: All traits require `Send + Sync`

---

## Project Structure

### galaxy_3d_engine (Core)

```
galaxy_3d_engine/src/
├── lib.rs                 # Public exports, plugin registry
├── engine.rs              # Galaxy3dEngine singleton manager
└── renderer/
    ├── mod.rs             # Module declarations
    ├── renderer.rs        # Renderer trait (factory interface)
    ├── buffer.rs          # RendererBuffer trait + BufferDesc
    ├── texture.rs         # RendererTexture trait + TextureDesc
    ├── shader.rs          # RendererShader trait + ShaderDesc
    ├── pipeline.rs        # RendererPipeline trait + PipelineDesc
    ├── command_list.rs    # RendererCommandList trait
    ├── render_target.rs   # RendererRenderTarget trait
    ├── render_pass.rs     # RendererRenderPass trait
    ├── swapchain.rs       # RendererSwapchain trait
    └── descriptor_set.rs  # RendererDescriptorSet trait
```

### galaxy_3d_engine_renderer_vulkan (Vulkan Backend)

```
galaxy_3d_engine_renderer_vulkan/src/
├── lib.rs                      # Exports, Vulkan registration
├── debug.rs                    # Validation layers, debug messenger
├── vulkan.rs                   # VulkanRenderer implementation
├── vulkan_buffer.rs            # VulkanRendererBuffer
├── vulkan_texture.rs           # VulkanRendererTexture
├── vulkan_shader.rs            # VulkanRendererShader
├── vulkan_pipeline.rs          # VulkanRendererPipeline
├── vulkan_command_list.rs      # VulkanRendererCommandList
├── vulkan_render_target.rs     # VulkanRendererRenderTarget
├── vulkan_render_pass.rs       # VulkanRendererRenderPass
├── vulkan_swapchain.rs         # VulkanRendererSwapchain
└── vulkan_descriptor_set.rs    # VulkanRendererDescriptorSet
```

### galaxy_image (Image Library)

```
galaxy_image/src/
├── lib.rs               # Public exports
├── error.rs             # ImageError, ImageResult
├── component_type.rs    # U8, U16, F32 component types
├── pixel_format.rs      # RGB, RGBA, BGR, BGRA, Grayscale
├── image_format.rs      # Png, Bmp, Jpeg format enum
├── image.rs             # Image struct (width, height, data)
├── galaxy_image.rs      # GalaxyImage manager (load/save)
└── loaders/
    ├── mod.rs           # Loader trait
    ├── png_loader.rs    # PNG loading/saving
    ├── bmp_loader.rs    # BMP loading/saving
    └── jpeg_loader.rs   # JPEG loading/saving
```

---

## Core Design Principles

### 1. Trait-Based Abstraction

All resources are exposed as trait objects to hide backend implementation:

```rust
// Public API (backend-agnostic)
pub trait RendererTexture: Send + Sync {}
pub trait RendererBuffer: Send + Sync {}
pub trait RendererPipeline: Send + Sync {}

// Backend implementation (concrete type, not exposed)
pub struct VulkanRendererTexture {
    image: vk::Image,
    view: vk::ImageView,
    allocation: Option<Allocation>,
    device: Arc<ash::Device>,
    allocator: Arc<Mutex<Allocator>>,
}

// Factory returns trait object
fn create_texture(&mut self, desc: TextureDesc)
    -> RenderResult<Arc<dyn RendererTexture>>
```

**Benefits:**
- Backend can be swapped without changing user code
- No monomorphization bloat
- Clean separation of interface and implementation

### 2. Smart Pointer Strategy

| Resource Type | Ownership | Reason |
|---------------|-----------|--------|
| `Arc<dyn RendererTexture>` | Shared | Textures used by multiple command lists |
| `Arc<dyn RendererBuffer>` | Shared | Buffers shared across frames |
| `Arc<dyn RendererPipeline>` | Shared | Pipelines reused |
| `Box<dyn RendererCommandList>` | Exclusive | Command lists recorded once per frame |
| `Box<dyn RendererSwapchain>` | Exclusive | Single owner per window |

### 3. RAII Resource Management

All resources implement `Drop` for automatic cleanup:

```rust
impl Drop for VulkanRendererTexture {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_image_view(self.view, None);
            if let Some(allocation) = self.allocation.take() {
                self.allocator.lock().unwrap().free(allocation).ok();
            }
            self.device.destroy_image(self.image, None);
        }
    }
}
```

**Cleanup Order:**
1. User drops last `Arc<dyn Trait>` reference
2. Concrete type's `Drop::drop()` is called
3. GPU resources destroyed (image views, images, allocations)
4. No manual cleanup required

### 4. Type Safety

Strong typing prevents misuse:

```rust
pub enum BufferUsage {
    Vertex,   // Can only be bound as vertex buffer
    Index,    // Can only be bound as index buffer
    Uniform,  // Can only be bound as uniform buffer
    Storage,  // Can only be bound as storage buffer
}

pub enum TextureUsage {
    Sampled,                  // Shader sampling only
    RenderTarget,             // Color attachment only
    SampledAndRenderTarget,   // Both
    DepthStencil,            // Depth/stencil attachment
}
```

### 5. Thread Safety

All public traits require `Send + Sync`:

```rust
pub trait Renderer: Send + Sync { ... }
pub trait RendererTexture: Send + Sync {}
pub trait RendererCommandList: Send + Sync { ... }
```

Renderer is typically wrapped in `Arc<Mutex<dyn Renderer>>` for multi-threaded access.

---

## Trait Hierarchy

### Core Trait: Renderer

The `Renderer` trait is the main factory interface:

```rust
pub trait Renderer: Send + Sync {
    // Resource creation
    fn create_texture(&mut self, desc: TextureDesc)
        -> RenderResult<Arc<dyn RendererTexture>>;
    fn create_buffer(&mut self, desc: BufferDesc)
        -> RenderResult<Arc<dyn RendererBuffer>>;
    fn create_shader(&mut self, desc: ShaderDesc)
        -> RenderResult<Arc<dyn RendererShader>>;
    fn create_pipeline(&mut self, desc: PipelineDesc)
        -> RenderResult<Arc<dyn RendererPipeline>>;

    // Rendering infrastructure
    fn create_command_list(&self)
        -> RenderResult<Box<dyn RendererCommandList>>;
    fn create_render_target(&self, desc: &RendererRenderTargetDesc)
        -> RenderResult<Arc<dyn RendererRenderTarget>>;
    fn create_render_pass(&self, desc: &RendererRenderPassDesc)
        -> RenderResult<Arc<dyn RendererRenderPass>>;
    fn create_swapchain(&self, window: &Window)
        -> RenderResult<Box<dyn RendererSwapchain>>;

    // Descriptor management
    fn create_descriptor_set_for_texture(&self, texture: &Arc<dyn RendererTexture>)
        -> RenderResult<Arc<dyn RendererDescriptorSet>>;
    fn get_descriptor_set_layout_handle(&self) -> u64;

    // Command submission
    fn submit(&self, commands: &[&dyn RendererCommandList])
        -> RenderResult<()>;
    fn submit_with_swapchain(&self, commands: &[&dyn RendererCommandList],
                             swapchain: &dyn RendererSwapchain,
                             image_index: u32)
        -> RenderResult<()>;

    // Synchronization
    fn wait_idle(&self) -> RenderResult<()>;

    // Utilities
    fn stats(&self) -> RendererStats;
    fn resize(&mut self, width: u32, height: u32);
}
```

### Resource Traits

| Trait | Methods | Purpose |
|-------|---------|---------|
| **RendererBuffer** | `update(offset, data)` | GPU buffer (vertex/index/uniform) |
| **RendererTexture** | _(marker)_ | GPU texture resource |
| **RendererShader** | _(marker)_ | Compiled shader module (SPIR-V) |
| **RendererPipeline** | _(marker)_ | Graphics pipeline state |
| **RendererDescriptorSet** | _(marker)_ | Resource binding (textures, uniforms) |
| **RendererRenderPass** | _(marker)_ | Render pass configuration |
| **RendererRenderTarget** | `width()`, `height()`, `format()` | Render destination |

### RendererCommandList Trait

Command recording interface:

```rust
pub trait RendererCommandList: Send + Sync {
    // Command buffer lifecycle
    fn begin(&mut self) -> RenderResult<()>;
    fn end(&mut self) -> RenderResult<()>;

    // Render pass management
    fn begin_render_pass(&mut self,
                         render_pass: &Arc<dyn RendererRenderPass>,
                         render_target: &Arc<dyn RendererRenderTarget>,
                         clear_values: &[ClearValue])
        -> RenderResult<()>;
    fn end_render_pass(&mut self) -> RenderResult<()>;

    // Pipeline state
    fn set_viewport(&mut self, viewport: Viewport) -> RenderResult<()>;
    fn set_scissor(&mut self, scissor: Rect2D) -> RenderResult<()>;
    fn bind_pipeline(&mut self, pipeline: &Arc<dyn RendererPipeline>)
        -> RenderResult<()>;

    // Resource binding
    fn bind_descriptor_sets(&mut self,
                           pipeline: &Arc<dyn RendererPipeline>,
                           descriptor_sets: &[&Arc<dyn RendererDescriptorSet>])
        -> RenderResult<()>;
    fn push_constants(&mut self, offset: u32, data: &[u8])
        -> RenderResult<()>;
    fn bind_vertex_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64)
        -> RenderResult<()>;
    fn bind_index_buffer(&mut self, buffer: &Arc<dyn RendererBuffer>, offset: u64)
        -> RenderResult<()>;

    // Drawing
    fn draw(&mut self, vertex_count: u32, first_vertex: u32)
        -> RenderResult<()>;
    fn draw_indexed(&mut self, index_count: u32, first_index: u32, vertex_offset: i32)
        -> RenderResult<()>;
}
```

### RendererSwapchain Trait

Window presentation interface:

```rust
pub trait RendererSwapchain: Send + Sync {
    fn acquire_next_image(&mut self)
        -> RenderResult<(u32, Arc<dyn RendererRenderTarget>)>;
    fn present(&mut self, image_index: u32) -> RenderResult<()>;
    fn recreate(&mut self, width: u32, height: u32) -> RenderResult<()>;

    fn image_count(&self) -> usize;
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn format(&self) -> TextureFormat;
}
```

---

## Resource Management

### Descriptor Types

#### BufferDesc

```rust
pub struct BufferDesc {
    pub size: u64,
    pub usage: BufferUsage,
}

pub enum BufferUsage {
    Vertex,   // Vertex buffer
    Index,    // Index buffer
    Uniform,  // Uniform buffer (constant buffer)
    Storage,  // Storage buffer (SSBO)
}
```

#### TextureDesc

```rust
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub data: Option<Vec<u8>>,  // Initial pixel data
}

pub enum TextureFormat {
    R8G8B8A8_SRGB,
    R8G8B8A8_UNORM,
    B8G8R8A8_SRGB,
    B8G8R8A8_UNORM,
    D16_UNORM,
    D32_FLOAT,
    D24_UNORM_S8_UINT,
    R32_SFLOAT,
    R32G32_SFLOAT,
    R32G32B32_SFLOAT,
    R32G32B32A32_SFLOAT,
}

pub enum TextureUsage {
    Sampled,                 // Shader sampling
    RenderTarget,            // Color attachment
    SampledAndRenderTarget,  // Both
    DepthStencil,           // Depth/stencil attachment
}
```

#### ShaderDesc

```rust
pub struct ShaderDesc<'a> {
    pub code: &'a [u8],        // SPIR-V bytecode
    pub stage: ShaderStage,
    pub entry_point: String,   // Typically "main"
}

pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}
```

#### PipelineDesc

```rust
pub struct PipelineDesc {
    pub vertex_shader: Arc<dyn RendererShader>,
    pub fragment_shader: Arc<dyn RendererShader>,
    pub vertex_layout: VertexLayout,
    pub topology: PrimitiveTopology,
    pub push_constant_ranges: Vec<PushConstantRange>,
    pub descriptor_set_layouts: Vec<u64>,  // vk::DescriptorSetLayout as u64
    pub enable_blending: bool,
}

pub struct VertexLayout {
    pub bindings: Vec<VertexBinding>,
    pub attributes: Vec<VertexAttribute>,
}

pub struct VertexBinding {
    pub binding: u32,
    pub stride: u32,
    pub input_rate: VertexInputRate,  // Vertex or Instance
}

pub struct VertexAttribute {
    pub location: u32,         // Shader location
    pub binding: u32,          // Which binding to pull from
    pub format: TextureFormat,
    pub offset: u32,           // Offset within vertex
}
```

#### RenderPassDesc

```rust
pub struct RendererRenderPassDesc {
    pub color_attachments: Vec<AttachmentDesc>,
    pub depth_attachment: Option<AttachmentDesc>,
}

pub struct AttachmentDesc {
    pub format: TextureFormat,
    pub samples: u32,  // 1 = no MSAA, 2/4/8 = MSAA
    pub load_op: LoadOp,        // Load, Clear, DontCare
    pub store_op: StoreOp,      // Store, DontCare
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
}
```

### Memory Allocation Strategy (Vulkan)

**GPU Allocator Integration:**

Uses `gpu-allocator` crate with three memory location types:

1. **GpuOnly** (VRAM) - Device-local memory
   - Render targets
   - Textures
   - Optimal performance
   - Not CPU-accessible

2. **CpuToGpu** (Mappable) - Host-visible memory
   - Vertex buffers
   - Index buffers
   - Uniform buffers
   - Staging buffers
   - CPU can write, GPU can read

3. **GpuToCpu** (Readback) - Download from GPU
   - Screenshot capture
   - GPU→CPU data transfer

**Allocation Example:**

```rust
// Creating a texture (GpuOnly)
let allocation_info = AllocationCreateDesc {
    name: "texture",
    requirements: image_memory_requirements,
    location: MemoryLocation::GpuOnly,
    linear: false,  // Optimal tiling
    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
};

let allocation = allocator.lock().unwrap()
    .allocate(&allocation_info)
    .map_err(|e| RenderError::OutOfMemory)?;
```

---

## Rendering Pipeline

### High-Level Rendering Flow

```
1. INITIALIZATION
   ├── Create Renderer (VulkanRenderer::new)
   ├── Create Swapchain (renderer.create_swapchain)
   ├── Create Render Pass (renderer.create_render_pass)
   └── Create Command Lists (renderer.create_command_list) × 2 for double buffering

2. RESOURCE CREATION
   ├── Load textures (renderer.create_texture)
   ├── Create descriptor sets (renderer.create_descriptor_set_for_texture)
   ├── Create vertex/index buffers (renderer.create_buffer)
   ├── Compile shaders (renderer.create_shader)
   └── Create pipelines (renderer.create_pipeline)

3. MAIN RENDER LOOP
   For each frame:
   ├── Acquire swapchain image
   │   └── (image_index, render_target) = swapchain.acquire_next_image()
   │
   ├── Record commands
   │   ├── cmd.begin()
   │   ├── cmd.begin_render_pass(render_pass, render_target, clear_values)
   │   ├── cmd.set_viewport(viewport)
   │   ├── cmd.set_scissor(scissor)
   │   ├── cmd.bind_pipeline(pipeline)
   │   ├── cmd.bind_descriptor_sets(pipeline, [descriptor_set])
   │   ├── cmd.bind_vertex_buffer(vertex_buffer, 0)
   │   ├── cmd.bind_index_buffer(index_buffer, 0)
   │   ├── cmd.draw_indexed(index_count, 0, 0)
   │   ├── cmd.end_render_pass()
   │   └── cmd.end()
   │
   ├── Submit commands
   │   └── renderer.submit_with_swapchain(&[cmd], swapchain, image_index)
   │
   └── Present
       └── swapchain.present(image_index)

4. CLEANUP (automatic via Drop)
   ├── Drop swapchain (destroys images, views, semaphores)
   ├── Drop command lists (destroys command pool, buffers)
   ├── Drop pipelines (destroys Vulkan pipeline)
   ├── Drop textures/buffers (frees GPU memory)
   └── Drop renderer (destroys device, instance)
```

### Command List State Machine

```
┌─────────────┐
│   Created   │
└──────┬──────┘
       │ begin()
       ▼
┌─────────────┐
│  Recording  │ ◄─────────┐
└──────┬──────┘           │
       │ begin_render_pass()
       ▼                   │
┌─────────────┐           │
│ In Render   │           │
│    Pass     │ ──────────┤
└──────┬──────┘  end_render_pass()
       │ end()
       ▼
┌─────────────┐
│ Executable  │ (ready for submit)
└─────────────┘
```

### Texture Upload Flow

```
1. Application creates TextureDesc with pixel data
2. Renderer creates staging buffer (CpuToGpu memory)
3. Copy pixel data to staging buffer (CPU side)
4. Create VkImage with GpuOnly memory
5. Create command buffer for transfer:
   a. Barrier: UNDEFINED → TRANSFER_DST_OPTIMAL
   b. Copy: staging buffer → image
   c. Barrier: TRANSFER_DST_OPTIMAL → SHADER_READ_ONLY_OPTIMAL
6. Submit transfer commands
7. Wait for completion (fence)
8. Destroy staging buffer
9. Return Arc<dyn RendererTexture>
```

---

## Vulkan Backend Implementation

### VulkanRenderer Initialization

**Steps:**

1. **Load Vulkan Library**
   - Create `ash::Entry` (Vulkan loader)

2. **Create Instance**
   - Application info (name, version)
   - Required extensions (KHR_surface, platform-specific)
   - Optional validation layers (VK_LAYER_KHRONOS_validation)

3. **Setup Debug Messenger** (if validation enabled)
   - Configure severity filter
   - Register debug callback function
   - Initialize stats tracking

4. **Select Physical Device**
   - Query for graphics queue family
   - Query for present queue family
   - Choose first suitable device

5. **Create Logical Device**
   - Enable swapchain extension
   - Create graphics queue
   - Create present queue (may be same as graphics)

6. **Create GPU Allocator**
   - Initialize `gpu-allocator::Allocator`
   - Configure pools for GpuOnly, CpuToGpu

7. **Create Synchronization Primitives**
   - 2 fences (for double buffering)
   - Descriptor pool (1000 texture descriptor sets)
   - Global texture sampler (linear filtering)
   - Descriptor set layout (binding 0 = COMBINED_IMAGE_SAMPLER)

### Synchronization Strategy

**Frame-Level Synchronization:**

```
Fence[0] ────┐                ┌──── Fence[1]
             │                │
Frame 0: ────┴────────────────┘
             Wait    Submit

Frame 1: ──────────────┬────────────┬──
                       │            │
                   Wait on      Submit with
                   Fence[1]      Fence[0]
```

**Swapchain Synchronization:**

```
acquire_next_image()
  └── Signals: image_available_semaphore[current_frame]

submit_with_swapchain()
  ├── Waits on: image_available
  └── Signals: render_finished_semaphore[image_index]

present()
  └── Waits on: render_finished_semaphore[image_index]
```

### Descriptor Set Management

**Global Layout:**

```rust
Binding 0: COMBINED_IMAGE_SAMPLER
  - Descriptor Type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER
  - Descriptor Count: 1
  - Shader Stage: Fragment
```

**Sampler Configuration:**

- Mag/Min Filter: LINEAR
- Address Mode: REPEAT
- Anisotropy: Disabled (max_anisotropy = 1.0)
- Mipmap LOD: 0.0 (no mipmaps yet)

**Descriptor Pool:**

- Type: COMBINED_IMAGE_SAMPLER
- Max Sets: 1000
- Allows dynamic allocation during rendering

### Pipeline Creation

**State Configuration:**

1. **Shader Stages**: Vertex + Fragment with SPIR-V modules
2. **Vertex Input**: Bindings (strides) + Attributes (locations, formats, offsets)
3. **Input Assembly**: Topology (TRIANGLE_LIST, LINE_LIST, POINT_LIST)
4. **Viewport**: Dynamic state (set via command list)
5. **Rasterization**: Fill mode (FILL), cull mode (BACK), front face (CCW)
6. **Multisample**: Sample count (default 1 = no MSAA)
7. **Color Blend**: Per-attachment blending configuration
8. **Dynamic State**: VIEWPORT, SCISSOR
9. **Push Constants**: Immediate data ranges
10. **Descriptor Set Layouts**: Resource binding layouts

**Blending Formula (if enabled):**

```
Result = Src * SrcAlpha + Dst * (1 - SrcAlpha)

src_color_blend_factor: SRC_ALPHA
dst_color_blend_factor: ONE_MINUS_SRC_ALPHA
color_blend_op: ADD
```

---

## Galaxy Image Library

### Overview

`galaxy_image` is a lightweight image loading/saving library with automatic format detection.

**Supported Formats:**

| Format | Extension | Loading | Saving | Notes |
|--------|-----------|---------|--------|-------|
| PNG | .png | ✅ | ✅ | Lossless, alpha support |
| BMP | .bmp | ✅ | ✅ | No compression |
| JPEG | .jpg/.jpeg | ✅ | ✅ | Lossy, no alpha |

### API

```rust
use galaxy_image::{GalaxyImage, ImageFormat, PixelFormat};

// Load image (format auto-detected from magic bytes)
let image = GalaxyImage::load_from_file("texture.png")?;

println!("Loaded {}x{} image", image.width(), image.height());
println!("Pixel format: {:?}", image.pixel_format());

// Access pixel data
let pixels: &[u8] = image.data();

// Save to different format
GalaxyImage::save_to_file(&image, "output.jpg", ImageFormat::Jpeg)?;
```

### Pixel Format Conversion

**Automatic RGB → RGBA Conversion:**

```rust
// If loaded image is RGB (3 bytes/pixel)
let rgba_data = match image.pixel_format() {
    PixelFormat::RGB => {
        let rgb_data = image.data();
        let pixel_count = (image.width() * image.height()) as usize;
        let mut rgba_data = Vec::with_capacity(pixel_count * 4);

        for i in 0..pixel_count {
            let idx = i * 3;
            rgba_data.push(rgb_data[idx]);     // R
            rgba_data.push(rgb_data[idx + 1]); // G
            rgba_data.push(rgb_data[idx + 2]); // B
            rgba_data.push(255);               // A (opaque)
        }
        rgba_data
    },
    PixelFormat::RGBA => image.data().to_vec(),
    _ => panic!("Unsupported pixel format"),
};
```

---

## Demo Application

### galaxy3d_demo

**Purpose:** Demonstrates texture loading and rendering with Galaxy3DEngine

**Features:**

- Loads 3 textures (PNG, BMP, JPEG)
- Renders 3 textured quads side-by-side
- Demonstrates descriptor sets
- Shows pixel format conversion
- Full validation layer integration

**Main Loop:**

```rust
fn render(&mut self) {
    // 1. Acquire swapchain image
    let (image_index, render_target) = self.swapchain
        .as_mut().unwrap()
        .acquire_next_image()
        .unwrap();

    // 2. Get current command list (double buffering)
    let cmd = &mut self.command_lists[self.current_frame];

    // 3. Record commands
    cmd.begin().unwrap();
    cmd.begin_render_pass(
        self.render_pass.as_ref().unwrap(),
        &render_target,
        &[ClearValue::Color([0.1, 0.1, 0.1, 1.0])],
    ).unwrap();

    cmd.set_viewport(viewport).unwrap();
    cmd.set_scissor(scissor).unwrap();
    cmd.bind_pipeline(self.pipeline.as_ref().unwrap()).unwrap();

    // Draw 3 quads (one for each texture)
    for i in 0..3 {
        cmd.bind_descriptor_sets(
            self.pipeline.as_ref().unwrap(),
            &[&self.descriptor_sets[i]],
        ).unwrap();
        cmd.bind_vertex_buffer(&self.vertex_buffers[i], 0).unwrap();
        cmd.draw(6, 0).unwrap();  // 2 triangles = 6 vertices
    }

    cmd.end_render_pass().unwrap();
    cmd.end().unwrap();

    // 4. Submit
    self.renderer.as_ref().unwrap()
        .lock().unwrap()
        .submit_with_swapchain(
            &[cmd.as_ref()],
            self.swapchain.as_ref().unwrap().as_ref(),
            image_index,
        ).unwrap();

    // 5. Present
    self.swapchain.as_mut().unwrap()
        .present(image_index)
        .unwrap();

    // 6. Alternate frame
    self.current_frame = (self.current_frame + 1) % 2;
}
```

---

## Design Patterns

### 1. Marker Trait Pattern

**Purpose:** Type safety without exposing implementation details

```rust
pub trait RendererTexture: Send + Sync {}
pub trait RendererShader: Send + Sync {}
pub trait RendererPipeline: Send + Sync {}
```

**Benefits:**
- Prevents accidental resource type confusion
- Allows future method additions without breaking changes
- Keeps public API minimal
- Backend can add methods via unsafe downcasts

### 2. Downcast Pattern

**Pattern:**

```rust
// Public API receives trait object
fn submit_with_swapchain(&self,
                         commands: &[&dyn RendererCommandList],
                         swapchain: &dyn RendererSwapchain,
                         image_index: u32) -> RenderResult<()>

// Backend downcasts to concrete type
let vk_cmd = *cmd as *const dyn RendererCommandList
    as *const VulkanRendererCommandList;
let vk_cmd = unsafe { &*vk_cmd };

// Access Vulkan-specific members
vk_cmd.command_buffer  // vk::CommandBuffer
```

**Safety Invariant:** Backend only creates trait objects for matching concrete types

### 3. Plugin Registry Pattern

**Global Registry:**

```rust
static RENDERER_REGISTRY: Mutex<Option<RendererPluginRegistry>>
    = Mutex::new(None);

pub fn register_renderer_plugin<F>(name: &'static str, factory: F)
where
    F: Fn(&Window, RendererConfig)
        -> RenderResult<Arc<Mutex<dyn Renderer>>>
        + Send + Sync + 'static
```

**Usage:**

```rust
// In Vulkan crate initialization
register_renderer_plugin("vulkan", |window, config| {
    Ok(Arc::new(Mutex::new(VulkanRenderer::new(window, config)?)))
});

// In application
let renderer = renderer_plugin_registry()
    .lock().unwrap()
    .as_mut().unwrap()
    .create_renderer("vulkan", &window, config)?;
```

---

## Performance Considerations

### Allocation Strategy

**Pre-Allocated Resources:**

- Descriptor Pool: 1000 sets (suitable for most scenes)
- Submit Fences: 2 (for double buffering)
- Command Pools: Per command list

**Dynamic Allocation:**

- Descriptor sets: Allocated on-demand from pool
- Textures/Buffers: Allocated via gpu-allocator

### Command Buffer Reuse

```rust
// Reset instead of recreating
self.device.reset_command_buffer(self.command_buffer, ...);
// No allocation overhead
```

### Sampler Reuse

**Single Global Sampler:**

All textures share one sampler object:

```rust
texture_sampler: vk::Sampler,  // Shared globally
```

Reduces state changes and resource consumption.

### Memory Barriers

**Implicit via Render Passes:**

- Attachment transitions happen automatically
- No manual barriers in public API
- Better optimization opportunities for drivers

---

## Thread Safety & Synchronization

### Thread-Safe Types

All public traits require `Send + Sync`:

```rust
pub trait Renderer: Send + Sync { ... }
pub trait RendererTexture: Send + Sync {}
pub trait RendererCommandList: Send + Sync { ... }
```

### Mutex-Wrapped Renderer

```rust
Arc<Mutex<dyn Renderer>>  // Thread-safe shared access
```

Allows multiple threads to create resources, though command recording typically happens on render thread.

### GPU Allocator Thread Safety

```rust
allocator: Arc<Mutex<Allocator>>  // Synchronized access
```

All allocations/deallocations are protected by mutex.

### CPU-GPU Synchronization

**Fences (CPU waits for GPU):**

```rust
// Before submitting frame N
device.wait_for_fences(&[submit_fences[current_submit_fence]], ...);
device.reset_fences(&[submit_fences[current_submit_fence]]);

// After submitting
device.queue_submit(..., submit_fences[current_submit_fence]);

current_submit_fence = (current_submit_fence + 1) % 2;
```

**Semaphores (GPU waits for GPU):**

```rust
// Acquire waits for image available
acquire_next_image() → signals image_available_semaphore

// Submit waits for image available, signals render finished
queue_submit(
    wait: image_available,
    signal: render_finished,
);

// Present waits for render finished
present(wait: render_finished);
```

---

## Error Handling

### RenderError Enum

```rust
pub enum RenderError {
    BackendError(String),           // Backend-specific failure
    OutOfMemory,                    // GPU memory exhausted
    InvalidResource(String),        // Invalid state/usage
    InitializationFailed(String),   // Initialization error
}

pub type RenderResult<T> = Result<T, RenderError>;
```

### Error Propagation

All fallible operations return `RenderResult<T>`:

```rust
fn create_texture(&mut self, desc: TextureDesc)
    -> RenderResult<Arc<dyn RendererTexture>>;

fn submit(&self, commands: &[&dyn RendererCommandList])
    -> RenderResult<()>;
```

### Validation Layer Integration

**Debug Configuration:**

```rust
pub struct DebugConfig {
    pub severity: DebugSeverity,  // ErrorsOnly, ErrorsAndWarnings, All
    pub output: DebugOutput,      // Console, File("path"), Both("path")
    pub message_filter: DebugMessageFilter,
    pub break_on_error: bool,     // Debugger break on validation error
    pub panic_on_error: bool,     // Panic on validation error
    pub enable_stats: bool,       // Track validation statistics
}
```

**Statistics Tracking:**

```rust
pub struct ValidationStats {
    pub errors: u32,
    pub warnings: u32,
    pub info: u32,
    pub verbose: u32,
}

pub fn get_validation_stats() -> ValidationStats;
pub fn print_validation_stats_report();
```

---

## Future Extensibility

### Planned Features (Phase 10+)

**Phase 10-12: Advanced Texture System**

- Texture atlases
- Texture arrays
- Bindless textures (descriptor indexing)
- Virtual texturing
- Mipmap generation (CPU: Lanczos-3, GPU: Box filter)
- DDS/KTX2 container support
- BC7 compression (CPU-side)

**Phase 13-15: Advanced Mesh System**

- Mesh batching (global vertex/index buffers)
- Indirect drawing (vkCmdDrawIndexedIndirect)
- GPU culling (frustum, occlusion, Hi-Z)
- LODs (Level of Detail)
- GPU skinning (skeletal animation)

**Phase 16+: Advanced Features**

- Compute shaders
- Ray tracing (VK_KHR_ray_tracing)
- Multi-threaded command recording
- Render graph system
- Material system
- Scene graph

### Multi-Backend Support

**Adding Direct3D 12:**

```rust
// Create new crate: galaxy_3d_engine_renderer_d3d12

// Implement all traits
pub struct D3D12Renderer { ... }
impl Renderer for D3D12Renderer { ... }

pub struct D3D12RendererTexture { ... }
impl RendererTexture for D3D12RendererTexture {}

// Register plugin
register_renderer_plugin("d3d12", |window, config| {
    Ok(Arc::new(Mutex::new(D3D12Renderer::new(window, config)?)))
});
```

**No changes needed in user code:**

```rust
// Selects backend at runtime
let renderer = create_renderer("d3d12", &window, config)?;
```

---

## API Reference Summary

### Core Traits

| Trait | Role | Key Methods |
|-------|------|-------------|
| `Renderer` | Factory/Device | `create_texture`, `create_buffer`, `create_shader`, `create_pipeline`, `create_command_list`, `submit` |
| `RendererCommandList` | Command Recording | `begin`, `begin_render_pass`, `bind_pipeline`, `bind_descriptor_sets`, `draw`, `end` |
| `RendererSwapchain` | Presentation | `acquire_next_image`, `present`, `recreate` |
| `RendererBuffer` | GPU Buffer | `update` |
| `RendererTexture` | GPU Texture | _(marker)_ |
| `RendererShader` | Shader Module | _(marker)_ |
| `RendererPipeline` | Graphics Pipeline | _(marker)_ |
| `RendererDescriptorSet` | Resource Binding | _(marker)_ |
| `RendererRenderPass` | Render Pass Config | _(marker)_ |
| `RendererRenderTarget` | Render Destination | `width`, `height`, `format` |

### Configuration Types

| Type | Purpose | Key Fields |
|------|---------|------------|
| `RendererConfig` | Engine Configuration | `enable_validation`, `debug_severity`, `debug_output` |
| `BufferDesc` | Buffer Creation | `size`, `usage` (Vertex/Index/Uniform/Storage) |
| `TextureDesc` | Texture Creation | `width`, `height`, `format`, `usage`, `data` |
| `ShaderDesc` | Shader Creation | `code` (SPIR-V), `stage`, `entry_point` |
| `PipelineDesc` | Pipeline Creation | `shaders`, `vertex_layout`, `topology`, `push_constants`, `blending` |
| `RendererRenderPassDesc` | Render Pass | `color_attachments`, `depth_attachment` |
| `RendererRenderTargetDesc` | Render Target | `width`, `height`, `format`, `usage`, `samples` |

### Enums

| Enum | Values |
|------|--------|
| `BufferUsage` | `Vertex`, `Index`, `Uniform`, `Storage` |
| `TextureFormat` | `R8G8B8A8_SRGB`, `B8G8R8A8_SRGB`, `D32_FLOAT`, etc. |
| `TextureUsage` | `Sampled`, `RenderTarget`, `SampledAndRenderTarget`, `DepthStencil` |
| `ShaderStage` | `Vertex`, `Fragment`, `Compute` |
| `PrimitiveTopology` | `TriangleList`, `LineList`, `PointList` |
| `LoadOp` | `Load`, `Clear`, `DontCare` |
| `StoreOp` | `Store`, `DontCare` |
| `ImageLayout` | `Undefined`, `ColorAttachment`, `ShaderReadOnly`, `PresentSrc`, etc. |

---

**End of Technical Documentation**
