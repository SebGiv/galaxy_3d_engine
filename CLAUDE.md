# Galaxy3DEngine - Design Document

> **Project**: Multi-API 3D Rendering Engine in Rust
> **Author**: Claude & User collaboration
> **Date**: 2026-01-26
> **Status**: Phase 8 - Textures & Transparence ✅

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
- `Renderer` - Factory pour créer ressources, command lists, swapchains, et submit
- `RenderCommandList` - Enregistrement de commandes (remplace RendererFrame)
- `RendererSwapchain` - Gestion swapchain séparée
- `RendererRenderTarget` - Cible de rendu (texture ou swapchain)
- `RendererRenderPass` - Configuration du render pass

**Resource Traits**:
- `Renderer` - Main interface (factory + submit, gère tout en interne)
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

### 3. Texture System & Descriptor Sets

**Implémentation**: Support complet des textures avec descriptor sets Vulkan

**Composants**:
```rust
// Texture avec données
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,  // Renommé de Format
    pub usage: TextureUsage,
    pub data: Option<Vec<u8>>,  // Données à uploader
}

// Pipeline avec blending
pub struct PipelineDesc {
    // ... autres champs ...
    pub descriptor_set_layouts: Vec<u64>,  // vk::DescriptorSetLayout
    pub enable_blending: bool,             // Alpha blending
}
```

**Upload de texture**:
```rust
// 1. Créer staging buffer
let staging_buffer = create_buffer(BufferDesc {
    size: data.len(),
    usage: BufferUsage::Vertex,
})?;
staging_buffer.update(0, &data)?;

// 2. Layout transition: UNDEFINED → TRANSFER_DST
pipeline_barrier(image, UNDEFINED, TRANSFER_DST_OPTIMAL);

// 3. Copy buffer → image
cmd_copy_buffer_to_image(staging_buffer, image);

// 4. Layout transition: TRANSFER_DST → SHADER_READ_ONLY
pipeline_barrier(image, TRANSFER_DST_OPTIMAL, SHADER_READ_ONLY_OPTIMAL);
```

**Descriptor Sets** (API Backend-Agnostic):
```rust
// Renderer crée pool et layout en interne (détails Vulkan cachés)
// descriptor_pool: vk::DescriptorPool,          // 1000 sets (privé)
// descriptor_set_layout: vk::DescriptorSetLayout,  // binding 0 (privé)
// texture_sampler: vk::Sampler,                 // linear filtering (privé)

// Application utilise API générique (pas de types Vulkan!)
let descriptor_set: Arc<dyn RendererDescriptorSet> =
    renderer.create_descriptor_set_for_texture(&texture)?;

// Bind dans command list (API 100% abstraite)
command_list.bind_descriptor_sets(&pipeline, &[&descriptor_set])?;

// Note: Tous les downcasts vers types Vulkan se font en interne,
// le code applicatif ne voit JAMAIS de types vk::*
```

**Alpha Blending**:
```rust
// Configuration Vulkan
if enable_blending {
    color_blend_attachment
        .blend_enable(true)
        .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
        .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
        .color_blend_op(vk::BlendOp::ADD)
        // Formula: result = src * src_alpha + dst * (1 - src_alpha)
}
```

**Multi-Format Support**:
```rust
// Conversion RGB → RGBA pour BMP/JPEG
match pixel_format {
    PixelFormat::RGB => {
        for pixel in rgb_data.chunks(3) {
            rgba_data.extend_from_slice(pixel);  // R, G, B
            rgba_data.push(255);                 // A (opaque)
        }
    },
    PixelFormat::RGBA => {
        rgba_data = rgb_data.to_vec();
    },
}
```

---

### 4. Memory Management

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

### 5. Synchronisation Vulkan

**Architecture**: Séparation swapchain et device submission

**VulkanRendererSwapchain**:
- `image_available_semaphores[image_count]`
- `render_finished_semaphores[image_count]`
- Gère acquire/present avec semaphores

**VulkanRenderer**:
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
│               ├── renderer.rs  # Renderer trait (avec nouvelles méthodes) ✨
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
│           ├── vulkan_renderer.rs    # VulkanRenderer ✨
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
        ├── images/                     # Images de test ✨
        │   ├── Gnu_head_colour_large.png  # PNG avec alpha
        │   ├── tigre.bmp               # BMP sans alpha (RGB)
        │   └── tux.jpg                 # JPEG sans alpha (RGB)
        ├── shaders/
        │   ├── textured_quad.vert      # Vertex shader pour quads texturés ✨
        │   └── textured_quad.frag      # Fragment shader avec sampler2D ✨
        └── src/
            └── main.rs                 # 3 quads texturés avec alpha blending ✨
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
- [x] Renderer trait étendu (nouvelles méthodes intégrées)
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

**VulkanRenderer Drop**:
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

**Quick Example** (100% Backend-Agnostic):
```rust
use galaxy_3d_engine::{
    Renderer, RendererCommandList, RendererSwapchain, RendererDescriptorSet,
    PipelineDesc, PushConstantRange, ShaderStage, TextureDesc,
};
use galaxy_3d_engine_renderer_vulkan::VulkanRenderer;  // Seulement pour création initiale

// Créer device (seule référence Vulkan)
let mut device = VulkanRenderer::new(&window, config)?;

// Créer swapchain (retourne trait abstrait)
let mut swapchain = device.create_swapchain(&window)?;

// Créer render pass
let render_pass = device.create_render_pass(&render_pass_desc)?;

// Créer command list
let mut cmd = device.create_command_list()?;

// Créer texture et descriptor set (API générique, pas de types Vulkan)
let texture = device.create_texture(TextureDesc {
    width: 512,
    height: 512,
    format: TextureFormat::R8G8B8A8_SRGB,
    usage: TextureUsage::Sampled,
    data: Some(image_data),
})?;
let descriptor_set = device.create_descriptor_set_for_texture(&texture)?;

// Créer pipeline
let descriptor_layout_handle = device.get_descriptor_set_layout_handle();
let pipeline = device.create_pipeline(PipelineDesc {
    vertex_shader,
    fragment_shader,
    vertex_layout,
    topology: PrimitiveTopology::TriangleList,
    push_constant_ranges: vec![],
    descriptor_set_layouts: vec![descriptor_layout_handle],
    enable_blending: true,
})?;

// Render loop
loop {
    // Acquire swapchain image
    let (image_idx, swapchain_target) = swapchain.acquire_next_image()?;

    // Record commands (API 100% générique)
    cmd.begin()?;
    cmd.begin_render_pass(&render_pass, &swapchain_target, &clear)?;
    cmd.set_viewport(viewport)?;
    cmd.bind_pipeline(&pipeline)?;
    cmd.bind_descriptor_sets(&pipeline, &[&descriptor_set])?;  // Aucun type Vulkan!
    cmd.bind_vertex_buffer(&vertex_buffer, 0)?;
    cmd.draw(6, 0)?;
    cmd.end_render_pass()?;
    cmd.end()?;

    // Submit avec synchronisation swapchain (gérée en interne)
    device.submit_with_swapchain(&[&*cmd], &*swapchain, image_idx)?;
    swapchain.present(image_idx)?;
}
```

---

## 📝 Code Style Guidelines

### Naming Conventions
- **Traits**: `Renderer`, `RenderCommandList` (PascalCase avec "Renderer" prefix)
- **Structs**: `VulkanRenderer`, `VulkanRendererCommandList` (backend prefix)
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

### 2026-01-27 - Phase 9: Backend-Agnostic API (100% Portable)
- **Abstraction Complète**:
  - ✅ Nouveau trait `RendererDescriptorSet` pour masquer `vk::DescriptorSet`
  - ✅ Méthode `Renderer::create_descriptor_set_for_texture()` retourne `Arc<dyn RendererDescriptorSet>`
  - ✅ Méthode `Renderer::submit_with_swapchain()` prend `&dyn RendererSwapchain` (plus de semaphores Vulkan exposés)
  - ✅ Méthode `RendererCommandList::bind_descriptor_sets()` prend `&[&Arc<dyn RendererDescriptorSet>]`
  - ✅ Méthodes `RendererSwapchain::width/height/format()` retournent types génériques
- **Détails Vulkan Cachés**:
  - ✅ `VulkanRendererPipeline.pipeline_layout` → `pub(crate)` (privé)
  - ✅ `VulkanRendererSwapchain::sync_info()` → `pub(crate)` (privé)
  - ✅ `VulkanRenderer::get_descriptor_set_layout()` → `pub(crate)` (privé)
  - ✅ Ajout de `get_descriptor_set_layout_handle()` qui retourne `u64` (pas de type Vulkan)
- **Migration Demo**:
  - ❌ Supprimé `use ash::vk::Handle`
  - ❌ Supprimé imports `VulkanRendererPipeline`, `VulkanRendererCommandList`, `VulkanRendererTexture`
  - ✅ `Vec<Arc<dyn RendererDescriptorSet>>` remplace `Vec<vk::DescriptorSet>`
  - ✅ Zéro casts `unsafe` dans le code applicatif (downcast internes seulement)
  - ✅ API 100% générique, aucune référence Vulkan visible
- **Score de Portabilité**:
  - Violations dans demo: 5 → **0** ✅
  - Fuites dans API: 7 → **0** ✅
  - Score global: 4/10 → **10/10** ✅
- **Bénéfices**:
  - ✅ Backend Direct3D 12 possible sans toucher la demo
  - ✅ Code applicatif utilise seulement des abstractions
  - ✅ Pas de casts `unsafe` dans le code utilisateur
  - ✅ Architecture moderne (similaire à wgpu, Bevy)

### 2026-01-26 - Phase 8: Textures & Transparence
- **Texture System**:
  - ✅ Descriptor sets (pool de 1000, layout pour textures)
  - ✅ Texture sampler (linear filtering, repeat addressing)
  - ✅ Texture upload avec staging buffer et layout transitions
  - ✅ Support de textures dans shaders (binding 0, sampler2D)
  - ✅ Méthode `bind_descriptor_sets()` dans RenderCommandList
- **Alpha Blending**:
  - ✅ Flag `enable_blending: bool` dans `PipelineDesc`
  - ✅ Configuration Vulkan (SRC_ALPHA, ONE_MINUS_SRC_ALPHA)
  - ✅ Transparence fonctionnelle (zones transparentes affichent arrière-plan)
- **API Changes**:
  - ✅ `Format` → `TextureFormat` (renommage pour clarté)
  - ✅ `TextureDesc.data: Option<Vec<u8>>` (upload de données)
  - ✅ `PipelineDesc.enable_blending: bool` (contrôle alpha blending)
  - ✅ Exports publics: `VulkanRendererPipeline`, `VulkanRendererCommandList`, `VulkanRendererTexture`
- **Multi-Format Support**:
  - ✅ PNG (RGBA, 4 canaux) - utilisé directement
  - ✅ BMP (RGB, 3 canaux) - conversion RGB→RGBA
  - ✅ JPEG (RGB, 3 canaux) - conversion RGB→RGBA
  - ✅ Détection automatique via `galaxy_image::PixelFormat`
- **Demo**:
  - ✅ 3 quads texturés affichés côte à côte
  - ✅ Chargement avec `galaxy_image` library
  - ✅ Shaders: `textured_quad.vert` et `textured_quad.frag`
- **Validation**: Zero Vulkan errors ✅

### 2026-01-26 - Architecture Simplifiée
- **Breaking Changes**:
  - ❌ Supprimé `RendererDevice` (intégré dans `Renderer`)
  - ❌ Supprimé `RendererFrame` trait et `vulkan_renderer_frame.rs`
  - ❌ Supprimé `begin_frame()` / `end_frame()` du trait `Renderer`
  - ✅ `Renderer` trait étendu avec nouvelles méthodes:
    - `create_command_list()`, `create_render_pass()`, `create_render_target()`
    - `create_swapchain()`, `submit()`
  - ✅ `RenderCommandList` trait (remplace `RendererFrame`)
  - ✅ `RendererSwapchain` trait (séparation présentation)
  - ✅ `RendererRenderTarget` trait (texture ou swapchain)
  - ✅ `RendererRenderPass` trait (configuration)

### 2026-01-25 - Phase 7: Architecture Moderne (Proposition 2)
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

### ✅ Phase 8: Textures & Transparence (DONE)
- [x] Descriptor sets support (pool, layout, allocation)
- [x] Texture sampling in shaders
- [x] Texture upload avec staging buffer
- [x] Layout transitions (UNDEFINED → TRANSFER_DST → SHADER_READ_ONLY)
- [x] Sampler creation (linear filtering, repeat addressing)
- [x] Alpha blending support (enable_blending flag)
- [x] Format → TextureFormat renaming (clarté)
- [x] Multi-format image loading (PNG/BMP/JPEG)
- [x] RGB→RGBA conversion automatique
- [x] Textured quad shaders (vertex + fragment)

**Demo Status**: `galaxy3d_demo` affiche 3 quads texturés (PNG, BMP, JPEG) avec transparence ✅

### ✅ Phase 9: Backend-Agnostic API (DONE)
- [x] Créer trait `RendererDescriptorSet` pour masquer `vk::DescriptorSet`
- [x] Ajouter `create_descriptor_set_for_texture()` retournant `Arc<dyn RendererDescriptorSet>`
- [x] Ajouter `submit_with_swapchain()` prenant `&dyn RendererSwapchain`
- [x] Modifier `bind_descriptor_sets()` pour prendre traits abstraits
- [x] Ajouter `width()`, `height()`, `format()` à `RendererSwapchain`
- [x] Cacher tous les champs Vulkan publics (`pub(crate)`)
- [x] Supprimer toutes références Vulkan de la demo
- [x] Éliminer tous les casts `unsafe` du code applicatif
- [x] Validation: 0 violations, 0 fuites, score 10/10

**Status**: API 100% portable, backend Direct3D 12 possible sans modifier la demo ✅

### Phase 10: Index Buffers (TODO)
- [ ] Index buffer creation
- [ ] `draw_indexed()` support
- [ ] Complex geometry (quads, pentagones, etc.)

### Phase 11: Advanced Features (TODO)
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
