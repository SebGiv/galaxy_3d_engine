# Galaxy3DEngine - Design Document

> **Project**: Multi-API 3D Rendering Engine in Rust
> **Author**: Claude & User collaboration
> **Date**: 2026-01-25
> **Status**: Phase 7 - Architecture Moderne (Proposition 2) ✅

---

## 🎯 Project Goals

Create a modern 3D rendering engine in Rust with:
- **Multi-API abstraction**: Support for Vulkan (and future Direct3D 12)
- **Modern architecture**: Séparation render/présentation pour render-to-texture
- **High performance**: Zero-cost abstractions with trait-based polymorphism
- **Safety**: Leverage Rust's memory safety guarantees
- **Advanced features**: Push constants, render targets, multi-pass rendering

---

## 📋 Core Design Decisions

### 1. Architecture Moderne (Proposition 2)

**Changement majeur**: Séparation complète du rendu et de la présentation

**Ancienne architecture** (obsolète):
- `Renderer` trait avec `begin_frame()` / `end_frame()`
- `RendererFrame` pour l'enregistrement des commandes
- Couplage fort entre swapchain et rendering

**Nouvelle architecture** (actuelle):
- `RendererDevice` - Factory pour créer ressources et commander
- `RenderCommandList` - Enregistrement de commandes (remplace RendererFrame)
- `RendererSwapchain` - Gestion swapchain séparée
- `RendererRenderTarget` - Cible de rendu (texture ou swapchain)
- `RendererRenderPass` - Configuration du render pass

**Resource Traits**:
- `RendererDevice` - Main device interface (factory pour ressources + submit)
- `RenderCommandList` - Command recording interface
- `RendererSwapchain` - Swapchain management (acquire/present)
- `RendererRenderTarget` - Render target (texture ou swapchain image)
- `RendererRenderPass` - Render pass configuration
- `RendererTexture` - GPU texture handle
- `RendererBuffer` - GPU buffer handle (vertex, index, uniform)
- `RendererShader` - Compiled shader module handle
- `RendererPipeline` - Graphics pipeline state handle (avec push constants)

**Avantages**:
- ✅ Render-to-texture possible
- ✅ Multi-pass rendering
- ✅ Post-processing effects
- ✅ Deferred shading ready
- ✅ Découplage rendu/présentation

---

### 2. Push Constants Support

**Implémentation**: Support natif des push constants Vulkan

**Définition**:
```rust
pub struct PushConstantRange {
    pub stages: Vec<ShaderStage>,
    pub offset: u32,
    pub size: u32,
}

// Dans PipelineDesc
pub struct PipelineDesc {
    // ... autres champs ...
    pub push_constant_ranges: Vec<PushConstantRange>,
}
```

**Usage**:
```rust
// Créer pipeline avec push constants
let pipeline = device.create_pipeline(PipelineDesc {
    push_constant_ranges: vec![
        PushConstantRange {
            stages: vec![ShaderStage::Vertex],
            offset: 0,
            size: 4, // sizeof(float)
        },
    ],
    // ...
})?;

// Pousser les données
let time = elapsed.to_le_bytes();
command_list.push_constants(0, &time)?;
```

---

### 3. Memory Management

**Decision**: Integrate `gpu-allocator` avec gestion du cycle de vie

**Framebuffer Lifecycle** (CRITIQUE):
- Les framebuffers sont créés dans `begin_render_pass()`
- Stockés dans `Vec<vk::Framebuffer>` du command list
- Détruits soit dans `begin()` (prochain frame), soit dans `Drop`
- **Raison**: Un framebuffer doit rester valide tant que le command buffer l'utilise

**Pattern de destruction**:
```rust
pub struct VulkanRendererCommandList {
    framebuffers: Vec<vk::Framebuffer>,
    // ...
}

impl RendererCommandList for VulkanRendererCommandList {
    fn begin(&mut self) -> RenderResult<()> {
        // Détruire les framebuffers du frame précédent
        for framebuffer in self.framebuffers.drain(..) {
            self.device.destroy_framebuffer(framebuffer, None);
        }
        // ...
    }

    fn begin_render_pass(...) -> RenderResult<()> {
        let framebuffer = create_framebuffer(...)?;
        self.framebuffers.push(framebuffer); // Stocké pour plus tard
        // ...
    }
}

impl Drop for VulkanRendererCommandList {
    fn drop(&mut self) {
        // Cleanup final
        for framebuffer in self.framebuffers.drain(..) {
            self.device.destroy_framebuffer(framebuffer, None);
        }
    }
}
```

---

### 4. Synchronisation Vulkan

**Architecture**: Séparation swapchain et device submission

**VulkanRendererSwapchain**:
- `image_available_semaphores[image_count]`
- `render_finished_semaphores[image_count]`
- Gère acquire/present avec semaphores

**VulkanRendererDevice**:
- `submit_with_sync()` pour synchroniser avec swapchain
- Fences pour CPU-GPU sync

**Flow de rendu**:
```rust
// 1. Acquérir image swapchain
let (image_idx, swapchain_target) = swapchain.acquire_next_image()?;

// 2. Enregistrer commandes
command_list.begin()?;
command_list.begin_render_pass(&render_pass, &swapchain_target, &clear)?;
// ... draw calls ...
command_list.end_render_pass()?;
command_list.end()?;

// 3. Soumettre avec sync swapchain
let sync_info = swapchain.sync_info();
device.submit_with_sync(&command_list, &sync_info, image_idx)?;

// 4. Présenter
swapchain.present(image_idx)?;
```

---

## 🏗️ Architecture Overview

### Cargo Workspace Structure

```
Galaxy/                                  # Workspace root
├── Tools/
│   └── galaxy_3d_engine/               # Core engine
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── plugin.rs               # Plugin registry (deprecated)
│           └── renderer/
│               ├── mod.rs
│               ├── renderer.rs         # Ancien trait (en cours de dépréciation)
│               ├── renderer_device.rs  # RendererDevice trait ✨ NOUVEAU
│               ├── renderer_command_list.rs  # RenderCommandList trait ✨
│               ├── renderer_render_target.rs # RendererRenderTarget trait ✨
│               ├── renderer_render_pass.rs   # RendererRenderPass trait ✨
│               ├── renderer_swapchain.rs     # RendererSwapchain trait ✨
│               ├── renderer_texture.rs
│               ├── renderer_buffer.rs
│               ├── renderer_shader.rs
│               └── renderer_pipeline.rs (avec PushConstantRange ✨)
│
│   └── galaxy_3d_engine_renderer_vulkan/  # Vulkan backend
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── vulkan_renderer_device.rs    # VulkanRendererDevice ✨
│           ├── vulkan_renderer_command_list.rs  # VulkanRendererCommandList ✨
│           ├── vulkan_renderer_render_target.rs # VulkanRendererRenderTarget ✨
│           ├── vulkan_renderer_render_pass.rs   # VulkanRendererRenderPass ✨
│           ├── vulkan_renderer_swapchain.rs     # VulkanRendererSwapchain ✨
│           ├── vulkan_renderer_texture.rs
│           ├── vulkan_renderer_buffer.rs
│           ├── vulkan_renderer_shader.rs
│           └── vulkan_renderer_pipeline.rs
│
└── Games/
    └── galaxy3d_demo/                  # Demo application
        ├── Cargo.toml
        ├── shaders/
        │   ├── triangle_animated.vert  # Vertex shader avec push constants ✨
        │   └── triangle.frag
        └── src/
            └── main.rs                 # Utilise nouvelle architecture
```

### Architecture Principles

1. **Séparation des responsabilités**: Device / Swapchain / Command Lists / Render Targets
2. **Trait-Based Polymorphism**: All resources are `Arc<dyn Trait>`
3. **RAII Resource Management**: Drop trait ensures proper cleanup
4. **Framebuffer Lifecycle**: Destroyed after command buffer usage
5. **Flexible Rendering**: Render-to-texture et swapchain avec même API

---

## 🎨 Rendering Pipeline - Implementation Actuelle

### ✅ Phase 7: Architecture Moderne (DONE)

**Implemented Features**:
- [x] RendererDevice trait (remplace Renderer)
- [x] RenderCommandList trait (remplace RendererFrame)
- [x] RendererSwapchain séparé
- [x] RendererRenderTarget (texture et swapchain)
- [x] RendererRenderPass configurables
- [x] Push constants support (vertex shader)
- [x] Animation avec push constants (rotation)
- [x] Framebuffer lifecycle management (memory leak fixed)
- [x] Synchronisation Vulkan correcte
- [x] Command list double buffering

**Demo Status**: `galaxy3d_demo` affiche 3 triangles colorés animés (rotation) ✅

**Vulkan Validation**: Zero errors (framebuffer leaks fixed) ✅

---

## 🔧 Vulkan Implementation Details

### Command List Architecture

**VulkanRendererCommandList**:
- Possède son propre command pool et command buffer
- Réutilisable (reset dans `begin()`)
- Gère le cycle de vie des framebuffers

**Double Buffering**:
```rust
// Demo utilise 2 command lists
let command_lists = [
    device.create_command_list()?,
    device.create_command_list()?,
];

// Alterne entre les deux
let cmd = &mut command_lists[current_frame];
```

### Synchronization Model

**Swapchain Semaphores** (dans VulkanRendererSwapchain):
- `image_available_semaphores[image_count]`
- `render_finished_semaphores[image_count]`

**Device Fences**:
- Une fence par `submit_with_sync()`

**Frame Flow**:
```rust
// 1. Acquire image
let (image_idx, target) = swapchain.acquire_next_image()?;

// 2. Record commands
cmd.begin()?;
cmd.begin_render_pass(&render_pass, &target, &clear)?;
cmd.set_viewport(viewport)?;
cmd.bind_pipeline(&pipeline)?;
cmd.push_constants(0, &data)?;  // ✨ Push constants
cmd.draw(9, 0)?;
cmd.end_render_pass()?;
cmd.end()?;

// 3. Submit with sync
let sync = swapchain.sync_info();
device.submit_with_sync(&cmd, &sync, image_idx)?;

// 4. Present
swapchain.present(image_idx)?;
```

### Resource Destruction Order

**VulkanRendererDevice Drop**:
1. Wait device idle
2. Drop user-created resources (textures, buffers, etc.)
3. Drop allocator (ManuallyDrop)
4. Destroy device
5. Destroy instance

**VulkanRendererSwapchain Drop**:
1. Wait device idle
2. Destroy framebuffers (si encore présents)
3. Destroy image views
4. Destroy swapchain
5. Destroy semaphores

**VulkanRendererCommandList Drop**:
1. Destroy remaining framebuffers
2. Destroy command pool (libère command buffer)

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

### Using the Engine (New Architecture)

**Quick Example**:
```rust
use galaxy_3d_engine::{
    RendererDevice, RenderCommandList, RendererSwapchain,
    PipelineDesc, PushConstantRange, ShaderStage,
};
use galaxy_3d_engine_renderer_vulkan::{
    VulkanRendererDevice, VulkanRendererSwapchain,
};

// Créer device
let mut device = VulkanRendererDevice::new(&window, config)?;

// Créer swapchain
let mut swapchain = device.create_swapchain(&window)?;

// Créer render pass
let render_pass = device.create_render_pass(&render_pass_desc)?;

// Créer command list
let mut cmd = device.create_command_list()?;

// Créer pipeline avec push constants
let pipeline = device.create_pipeline(PipelineDesc {
    vertex_shader,
    fragment_shader,
    vertex_layout,
    topology: PrimitiveTopology::TriangleList,
    push_constant_ranges: vec![
        PushConstantRange {
            stages: vec![ShaderStage::Vertex],
            offset: 0,
            size: 4,
        },
    ],
})?;

// Render loop
loop {
    // Acquire swapchain image
    let (image_idx, swapchain_target) = swapchain.acquire_next_image()?;

    // Record commands
    cmd.begin()?;
    cmd.begin_render_pass(&render_pass, &swapchain_target, &clear)?;
    cmd.set_viewport(viewport)?;
    cmd.bind_pipeline(&pipeline)?;
    cmd.push_constants(0, &time_bytes)?;  // Animation
    cmd.bind_vertex_buffer(&vertex_buffer, 0)?;
    cmd.draw(3, 0)?;
    cmd.end_render_pass()?;
    cmd.end()?;

    // Submit and present
    let sync_info = swapchain.sync_info();
    device.submit_with_sync(&cmd, &sync_info, image_idx)?;
    swapchain.present(image_idx)?;
}
```

---

## 📝 Code Style Guidelines

### Naming Conventions
- **Traits**: `RendererDevice`, `RenderCommandList` (PascalCase avec "Renderer" prefix)
- **Structs**: `VulkanRendererDevice`, `VulkanRendererCommandList` (backend prefix)
- **Functions**: `create_buffer`, `begin_render_pass` (snake_case)
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

### 2026-01-25 - Phase 7: Architecture Moderne (Proposition 2)
- **Breaking Changes**:
  - ❌ Supprimé `RendererFrame` trait et `vulkan_renderer_frame.rs`
  - ❌ Supprimé `begin_frame()` / `end_frame()` du trait `Renderer`
  - ✅ Nouveau `RendererDevice` trait (remplace `Renderer` progressivement)
  - ✅ Nouveau `RenderCommandList` trait (remplace `RendererFrame`)
  - ✅ Nouveau `RendererSwapchain` trait (séparation présentation)
  - ✅ Nouveau `RendererRenderTarget` trait (texture ou swapchain)
  - ✅ Nouveau `RendererRenderPass` trait (configuration)
- **Features**:
  - ✅ Push constants support (PushConstantRange dans PipelineDesc)
  - ✅ Animation avec push constants (rotation triangle)
  - ✅ Framebuffer lifecycle management (memory leak fixed)
  - ✅ Synchronisation Vulkan séparée (device vs swapchain)
  - ✅ Command list double buffering (2 lists)
- **Bugfixes**:
  - ✅ Framebuffer memory leaks corrigés
  - ✅ Validation Vulkan errors: zero errors
  - ✅ Proper cleanup à la fermeture
- **Architecture**:
  - ✅ Séparation complète rendu/présentation
  - ✅ Ready for render-to-texture
  - ✅ Ready for multi-pass rendering
  - ✅ Ready for post-processing

### 2026-01-25 - Complete Graphics Pipeline Implementation
- **Architecture Refactor**: Renamed crates to `galaxy_3d_engine` and `galaxy_3d_engine_renderer_vulkan`
- **Trait-Based Polymorphism**: Implemented C++-style dynamic inheritance
- **Vulkan Backend**: Full implementation with triangle rendering
- **Memory Management**: `gpu-allocator` integration
- **Demo**: `galaxy3d_demo` renders colored triangle

### 2026-01-24 - Initial Design & Workspace Setup
- Created project structure
- Defined core trait abstractions
- Set up plugin system architecture
- Basic Vulkan initialization

---

## 🎯 Next Steps (Roadmap)

### Phase 8: Render-to-Texture (TODO)
- [ ] Descriptor sets support
- [ ] Texture sampling in shaders
- [ ] Offscreen render target creation
- [ ] Post-processing demo (blur, bloom, etc.)

### Phase 9: Index Buffers (TODO)
- [ ] Index buffer creation
- [ ] `draw_indexed()` support
- [ ] Complex geometry (quads, pentagones, etc.)

### Phase 10: Advanced Features (TODO)
- [ ] Uniform buffers
- [ ] Texture arrays
- [ ] Compute shaders
- [ ] Multi-pass deferred rendering

---

## 📚 References

- [Vulkan Tutorial](https://vulkan-tutorial.com/)
- [Ash Documentation](https://docs.rs/ash/)
- [gpu-allocator Documentation](https://docs.rs/gpu-allocator/)
- [Vulkan Specification](https://registry.khronos.org/vulkan/specs/1.3/)
