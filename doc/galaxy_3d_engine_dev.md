# Galaxy3DEngine - Document de Développement

> **Projet** : Moteur de rendu 3D multi-API en Rust
> **Auteur** : Collaboration Claude & Utilisateur
> **Date** : 2026-02-06
> **Statut** : Phase 16 - Pipeline Resource System ✅

---

## 📖 Préambule

Ce document est rédigé en **français** et constitue le journal de développement du projet Galaxy3D Engine. Il contient :

- **Réflexions** : Les discussions et décisions prises durant le développement
- **Philosophie** : Les principes directeurs et choix architecturaux
- **Buts** : Les objectifs à court et long terme du projet
- **Progression** : L'historique des phases de développement

Ce n'est pas une documentation technique (voir `galaxy_3d_engine_tech_doc.md` pour cela), mais plutôt un document de travail qui capture l'évolution du projet et les raisonnements derrière les décisions.

---

## 🎯 Objectifs du Projet

Créer un moteur de rendu 3D moderne en Rust avec :
- **Abstraction multi-API** : Support Vulkan (et futur Direct3D 12)
- **Architecture moderne** : Séparation rendu/présentation pour render-to-texture
- **Haute performance** : Abstractions à coût zéro avec polymorphisme basé sur les traits
- **Sécurité** : Exploitation des garanties de sécurité mémoire de Rust
- **Fonctionnalités avancées** : Push constants, cibles de rendu, rendu multi-passe

---

## 📋 Décisions de Conception Fondamentales

### 0. Pattern Vec+HashMap pour Collections Nommées (Phase 13)

**Problématique** : Comment stocker des éléments accessibles à la fois par nom (String) et par index numérique rapide ?

**Solution retenue** : Combiner `Vec<T>` et `HashMap<String, usize>`

```rust
pub struct MeshLOD {
    submeshes: Vec<SubMesh>,              // Stockage par id (index)
    submesh_names: HashMap<String, usize>, // Nom → id
}
```

**Avantages** :
- ✅ Accès O(1) par id (index direct dans Vec)
- ✅ Accès O(1) par nom (lookup HashMap puis index)
- ✅ Les méthodes `add_*()` retournent l'id pour stockage par l'utilisateur
- ✅ Évite les lookups répétés par nom si l'id est conservé
- ✅ Pattern consistant entre textures et meshes

**API résultante** :
```rust
// Par id (le plus rapide)
let submesh = lod.submesh(id)?;

// Par nom (convenience)
let submesh = lod.submesh_by_name("body")?;

// Récupérer l'id pour usage ultérieur
let id = lod.submesh_id("body")?;
```

**Pourquoi pas HashMap<String, T> seul ?**
- ❌ Pas d'accès par index numérique stable
- ❌ L'itération n'a pas d'ordre garanti
- ❌ Impossible de stocker un "handle" numérique léger

**Pourquoi pas Vec<(String, T)> ?**
- ❌ Lookup par nom en O(n)
- ❌ Nécessite un scan linéaire pour trouver par nom

---

### 1. Architecture Moderne (Proposition 2)

**Changement majeur**: Séparation complète du rendu et de la présentation

**Ancienne architecture** (obsolète):
- `Renderer` trait avec `begin_frame()` / `end_frame()`
- `RendererFrame` pour l'enregistrement des commandes
- Couplage fort entre swapchain et rendering

**Nouvelle architecture** (actuelle):
- `Renderer` - Factory pour créer ressources, command lists, swapchains, et submit
- `RenderCommandList` - Enregistrement de commandes (remplace RendererFrame)
- `galaxy_3d_engine::galaxy3d::render::Swapchain` - Gestion swapchain séparée
- `galaxy_3d_engine::galaxy3d::render::RenderTarget` - Cible de rendu (texture ou swapchain)
- `galaxy_3d_engine::galaxy3d::render::RenderPass` - Configuration du render pass

**Resource Traits**:
- `Renderer` - Main interface (factory + submit, gère tout en interne)
- `RenderCommandList` - Command recording interface
- `galaxy_3d_engine::galaxy3d::render::Swapchain` - Swapchain management (acquire/present)
- `galaxy_3d_engine::galaxy3d::render::RenderTarget` - Render target (texture ou swapchain image)
- `galaxy_3d_engine::galaxy3d::render::RenderPass` - Render pass configuration
- `galaxy_3d_engine::galaxy3d::render::Texture` - GPU texture handle
- `galaxy_3d_engine::galaxy3d::render::Buffer` - GPU buffer handle (vertex, index, uniform)
- `galaxy_3d_engine::galaxy3d::render::Shader` - Compiled shader module handle
- `galaxy_3d_engine::galaxy3d::render::Pipeline` - Graphics pipeline state handle (avec push constants)

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
let descriptor_set: Arc<dyn galaxy_3d_engine::galaxy3d::render::DescriptorSet> =
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

### 4. galaxy_3d_engine::galaxy3d::Engine Singleton Manager

**Implémentation** : Gestionnaire de singletons thread-safe pour les sous-systèmes du moteur

**Problème résolu** :
- Accès global simplifié au Renderer sans passer des références partout
- Gestion centralisée du cycle de vie des singletons
- API ergonomique pour créer et accéder aux sous-systèmes

**Architecture** :
```rust
// Structure singleton principale
pub struct galaxy_3d_engine::galaxy3d::Engine;

impl galaxy_3d_engine::galaxy3d::Engine {
    /// Initialiser le moteur (appeler au démarrage)
    pub fn initialize() -> galaxy_3d_engine::galaxy3d::Result<()>;

    /// Créer le renderer singleton
    pub fn create_renderer<R: Renderer + 'static>(renderer: R) -> galaxy_3d_engine::galaxy3d::Result<()>;

    /// Accéder au renderer global
    pub fn renderer() -> galaxy_3d_engine::galaxy3d::Result<Arc<Mutex<dyn Renderer>>>;

    /// Détruire le renderer singleton
    pub fn destroy_renderer() -> galaxy_3d_engine::galaxy3d::Result<()>;

    /// Shutdown complet du moteur
    pub fn shutdown();
}
```

**Implémentation interne** (thread-safe) :
```rust
// Storage global avec OnceLock (initialisé une seule fois)
static ENGINE_STATE: OnceLock<EngineState> = OnceLock::new();

struct EngineState {
    // RwLock pour lecture concurrente, écriture exclusive
    renderer: RwLock<Option<Arc<Mutex<dyn Renderer>>>>,
}
```

**Patterns utilisés** :
- `OnceLock` : Initialisation thread-safe one-time (Rust 1.70+)
- `RwLock` : Multiple readers, single writer (accès concurrent optimisé)
- `Arc<Mutex<dyn Renderer>>` : Shared ownership + interior mutability pour le trait object
- Generic `create_renderer<R: Renderer>` : Accepte tout type implémentant Renderer

**Usage dans l'application** :
```rust
use galaxy_3d_engine::{galaxy_3d_engine::galaxy3d::Engine, galaxy_3d_engine::galaxy3d::render::Config};
use galaxy_3d_engine_renderer_vulkan::galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;

fn main() -> Result<()> {
    // 1. Initialiser le moteur
    galaxy_3d_engine::galaxy3d::Engine::initialize()?;

    // 2. Créer le renderer singleton (API simplifiée)
    let config = galaxy_3d_engine::galaxy3d::render::Config::default();
    let vulkan_renderer = galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer::new(&window, config)?;
    galaxy_3d_engine::galaxy3d::Engine::create_renderer(vulkan_renderer)?;

    // 3. Accès global au renderer (n'importe où dans le code)
    let renderer = galaxy_3d_engine::galaxy3d::Engine::renderer()?;
    let mut renderer_guard = renderer.lock().unwrap();

    // Utiliser le renderer
    let buffer = renderer_guard.create_buffer(BufferDesc { /*...*/ })?;

    // 4. Cleanup
    drop(renderer_guard); // Libérer le lock
    galaxy_3d_engine::galaxy3d::Engine::destroy_renderer()?;
    galaxy_3d_engine::galaxy3d::Engine::shutdown();

    Ok(())
}
```

**Avantages** :
- ✅ API ergonomique : `galaxy_3d_engine::galaxy3d::Engine::create_renderer(galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer::new(...)?)`
- ✅ Accès global sans passer de références partout
- ✅ Thread-safe par design (RwLock + Mutex)
- ✅ Gestion centralisée du cycle de vie
- ✅ Préparé pour futurs singletons (ResourceManager, etc.)
- ✅ Zero overhead : résolu au compile-time

**Limitations** :
- ⚠️ Un seul renderer par processus (suffisant pour la plupart des cas)
- ⚠️ Nécessite `galaxy_3d_engine::galaxy3d::Engine::initialize()` avant utilisation
- ⚠️ Lock mutex sur chaque accès (négligeable en pratique)

---

### 5. Memory Management

**Decision**: Integrate `gpu-allocator` avec gestion du cycle de vie

**Framebuffer Lifecycle** (CRITIQUE):
- Les framebuffers sont créés dans `begin_render_pass()`
- Stockés dans `Vec<vk::Framebuffer>` du command list
- Détruits soit dans `begin()` (prochain frame), soit dans `Drop`
- **Raison**: Un framebuffer doit rester valide tant que le command buffer l'utilise

**Pattern de destruction**:
```rust
pub struct Vulkangalaxy_3d_engine::galaxy3d::render::CommandList {
    framebuffers: Vec<vk::Framebuffer>,
    // ...
}

impl galaxy_3d_engine::galaxy3d::render::CommandList for Vulkangalaxy_3d_engine::galaxy3d::render::CommandList {
    fn begin(&mut self) -> galaxy_3d_engine::galaxy3d::Result<()> {
        // Détruire les framebuffers du frame précédent
        for framebuffer in self.framebuffers.drain(..) {
            self.device.destroy_framebuffer(framebuffer, None);
        }
        // ...
    }

    fn begin_render_pass(...) -> galaxy_3d_engine::galaxy3d::Result<()> {
        let framebuffer = create_framebuffer(...)?;
        self.framebuffers.push(framebuffer); // Stocké pour plus tard
        // ...
    }
}

impl Drop for Vulkangalaxy_3d_engine::galaxy3d::render::CommandList {
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

**Vulkangalaxy_3d_engine::galaxy3d::render::Swapchain**:
- `image_available_semaphores[image_count]`
- `render_finished_semaphores[image_count]`
- Gère acquire/present avec semaphores

**galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer**:
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

### 6. Resource Texture Architecture (Implémenté Phase 11 + 11b + 11c + 11d)

**Decision**: Architecture à 3 niveaux de textures

**Les 3 niveaux** :

| Niveau | Module | Rôle |
|--------|--------|------|
| Bas | `render::Texture` | Handle GPU brut (`dyn Trait`, backend-specific : vk::Image, etc.) + propriétés via `info()` |
| Moyen | `resource::Texture` | Registre nommé : 1 GPU texture + N sous-régions nommées |
| Haut | `scene::Texture` | Texture finale utilisée dans un material/mesh d'une scène 3D (à développer plus tard) |

**Niveau render (Phase 11b)** :
- Trait `render::Texture` avec méthode `fn info(&self) -> &TextureInfo`
- `TextureInfo` : propriétés lecture seule (width, height, format, usage, array_layers) + `is_array()`
- `TextureDesc` supporte textures simples (`array_layers = 1`) et texture arrays (`array_layers > 1`)
- `TextureData` enum : `Single(Vec<u8>)` pour texture simple, `Layers(Vec<TextureLayerData>)` pour upload multi-layer
- Backend Vulkan crée `TYPE_2D` ou `TYPE_2D_ARRAY` selon `array_layers`

**Approche retenue (niveau resource)** : Trait object (`dyn Texture`) — style C++ virtual

- Un trait `resource::Texture` commun avec dispatch dynamique (vtable)
- Des implémentations concrètes par type de texture : `SimpleTexture`, `AtlasTexture`, `ArrayTexture`
- Stockage uniforme dans le ResourceManager : `HashMap<String, Arc<dyn Texture>>`
- Pas de `match`/`enum` pour différencier les types — le trait dispatch fait le travail
- Downcast explicite via méthodes `as_atlas()`, `as_array()` sur le trait (Option A, pas `Any`)

**Pourquoi pas les autres approches** :
- ❌ Enum `TextureRegion` : oblige un `match` à chaque manipulation
- ❌ Generics `Texture<R: Region>` : impossible de mélanger les types dans un même HashMap

**Types concrets implémentés** :

| Type | Description | Données spécifiques |
|------|-------------|---------------------|
| `SimpleTexture` | 1 texture = 1 image, mapping 1:1 | Aucune sous-région |
| `AtlasTexture` | 1 image, N sous-régions UV | `HashMap<String, AtlasRegion>` (u, v, width, height) |
| `ArrayTexture` | Texture array GPU, N layers nommés | `HashMap<String, u32>` (layer index) |

**Chaque implémentation contient** (Phase 11c):
- `renderer: Arc<Mutex<dyn Renderer>>` — référence au renderer (pour futures fonctionnalités)
- `render_texture: Arc<dyn render::Texture>` — le handle GPU
- `descriptor_set: Arc<dyn render::DescriptorSet>` — pour le binding au rendu
- Ses données spécifiques (régions, layers, etc.)

**Trait `resource::Texture` méthodes** (Phase 11c + 11d):
- `render_texture()`, `descriptor_set()`, `region_names()` — accès aux données
- `add_atlas_region(name, region) -> Result<()>` — implémentation par défaut (erreur)
- `add_array_layer(name, layer, data: Option<&[u8]>) -> Result<()>` — implémentation par défaut (erreur)
- Override dans `AtlasTexture` et `ArrayTexture` pour la vraie logique
- `ArrayTexture::add_array_layer()` upload les pixels si data fourni (Phase 11d)

**Trait `render::Renderer` nouvelles méthodes** (Phase 11d):
- `update_texture_layer(texture, layer, data) -> Result<()>` — upload pixels vers un layer existant

**Création des textures** :
- Se fait via le `ResourceManager` qui appelle le `Renderer` en interne
- L'utilisateur ne manipule pas le renderer directement pour les ressources
- **Les fonctions retournent maintenant la texture créée** (`Arc<dyn Texture>`) pour usage immédiat
- Régions/layers peuvent être passées à la création (`&[AtlasRegionDesc]` / `&[ArrayLayerDesc]`) ou ajoutées plus tard
- `ArrayLayerDesc` supporte `data: Option<Vec<u8>>` pour upload pixels à la création (Phase 11d)
- Ex: `let tex = rm.create_simple_texture("skybox".into(), TextureDesc { ... })?;`
- Ex: `let atlas = rm.create_atlas_texture("tileset".into(), desc, &[region1, region2])?;`
- Ex: `rm.create_atlas_texture("tileset".into(), desc, &[])?; rm.add_atlas_region(...)?`
- Ex: `rm.add_array_layer("tileset", "grass".into(), 0, Some(&pixels))?;` — upload post-création
- **`get_texture()` renommé en `texture()`** (convention Rust, Phase 11c)

**Nom du 3ème niveau** : `scene` (retenu)
- S'étend naturellement : `scene::Texture`, `scene::Material`, `scene::Mesh`, `scene::Light`, `scene::Camera`
- Correspond au niveau "objet dans une scène 3D"
- Non développé pour l'instant, c'est un concept futur

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
│           ├── engine.rs               # Engine singleton manager
│           ├── error.rs                # Error types
│           ├── plugin.rs               # Plugin registry (deprecated)
│           ├── resource/
│           │   ├── mod.rs              # Resource module exports
│           │   ├── resource_manager.rs # ResourceManager (texture storage + creation)
│           │   └── texture.rs          # Texture trait + SimpleTexture, AtlasTexture, ArrayTexture
│           └── renderer/
│               ├── mod.rs
│               ├── renderer.rs  # Renderer trait (avec nouvelles méthodes) ✨
│               ├── command_list.rs  # RenderCommandList trait ✨
│               ├── render_target.rs # galaxy_3d_engine::galaxy3d::render::RenderTarget trait ✨
│               ├── render_pass.rs   # galaxy_3d_engine::galaxy3d::render::RenderPass trait ✨
│               ├── swapchain.rs     # galaxy_3d_engine::galaxy3d::render::Swapchain trait ✨
│               ├── descriptor_set.rs # galaxy_3d_engine::galaxy3d::render::DescriptorSet trait ✨
│               ├── texture.rs
│               ├── buffer.rs
│               ├── shader.rs
│               └── pipeline.rs (avec PushConstantRange ✨)
│
│   └── galaxy_3d_engine_renderer_vulkan/  # Vulkan backend
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── vulkan.rs    # galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer ✨
│           ├── vulkan_command_list.rs  # Vulkangalaxy_3d_engine::galaxy3d::render::CommandList ✨
│           ├── vulkan_render_target.rs # Vulkangalaxy_3d_engine::galaxy3d::render::RenderTarget ✨
│           ├── vulkan_render_pass.rs   # Vulkangalaxy_3d_engine::galaxy3d::render::RenderPass ✨
│           ├── vulkan_swapchain.rs     # Vulkangalaxy_3d_engine::galaxy3d::render::Swapchain ✨
│           ├── vulkan_descriptor_set.rs # Vulkangalaxy_3d_engine::galaxy3d::render::DescriptorSet ✨
│           ├── vulkan_texture.rs
│           ├── vulkan_buffer.rs
│           ├── vulkan_shader.rs
│           ├── vulkan_pipeline.rs
│           └── debug.rs    # Debug utilities ✨
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
- [x] galaxy_3d_engine::galaxy3d::render::Swapchain séparé
- [x] galaxy_3d_engine::galaxy3d::render::RenderTarget (texture et swapchain)
- [x] galaxy_3d_engine::galaxy3d::render::RenderPass configurables
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

**Vulkangalaxy_3d_engine::galaxy3d::render::CommandList**:
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

**Swapchain Semaphores** (dans Vulkangalaxy_3d_engine::galaxy3d::render::Swapchain):
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

**galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer Drop**:
1. Wait device idle
2. Drop user-created resources (textures, buffers, etc.)
3. Drop allocator (ManuallyDrop)
4. Destroy device
5. Destroy instance

**Vulkangalaxy_3d_engine::galaxy3d::render::Swapchain Drop**:
1. Wait device idle
2. Destroy framebuffers (si encore présents)
3. Destroy image views
4. Destroy swapchain
5. Destroy semaphores

**Vulkangalaxy_3d_engine::galaxy3d::render::CommandList Drop**:
1. Destroy remaining framebuffers
2. Destroy command pool (libère command buffer)

### Optimisation Future : Staging Buffer Unique pour Upload Multi-Layer

**Contexte** : Lors de l'upload de données vers une texture array (N layers), plutôt que de créer N staging buffers séparés, on peut utiliser un **seul staging buffer** contenant toutes les données concaténées.

**Principe** :
```
Staging Buffer unique (taille = somme de toutes les layers) :
┌──────────────┬──────────────┬──────────────┬──────────────┐
│ layer 0 data │ layer 1 data │ layer 2 data │ layer 3 data │
│ offset: 0    │ offset: 4MB  │ offset: 8MB  │ offset: 12MB │
└──────────────┴──────────────┴──────────────┴──────────────┘

→ Un seul appel cmd_copy_buffer_to_image avec N régions BufferImageCopy :
  BufferImageCopy[0] : buffer_offset=0,   base_array_layer=0
  BufferImageCopy[1] : buffer_offset=4MB, base_array_layer=1
  BufferImageCopy[2] : buffer_offset=8MB, base_array_layer=2
  BufferImageCopy[3] : buffer_offset=12MB, base_array_layer=3
```

**Avantages** :
- 1 allocation mémoire au lieu de N
- 1 appel Vulkan au lieu de N
- Moins de overhead CPU et GPU

**Status** : À implémenter lors de l'optimisation du système de texture array. Pour la première implémentation, un staging buffer par layer est acceptable.

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
    Renderer, galaxy_3d_engine::galaxy3d::render::CommandList, galaxy_3d_engine::galaxy3d::render::Swapchain, galaxy_3d_engine::galaxy3d::render::DescriptorSet,
    PipelineDesc, PushConstantRange, ShaderStage, TextureDesc,
};
use galaxy_3d_engine_renderer_vulkan::galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer;  // Seulement pour création initiale

// Créer device (seule référence Vulkan)
let mut device = galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer::new(&window, config)?;

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
- **Structs**: `galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer`, `Vulkangalaxy_3d_engine::galaxy3d::render::CommandList` (backend prefix)
- **Functions**: `create_buffer`, `begin_render_pass` (snake_case)
- **Constants**: `MAX_FRAMES_IN_FLIGHT` (SCREAMING_SNAKE_CASE)

### Documentation
- All public traits and methods have doc comments
- Examples included for complex operations
- Safety notes for unsafe code

### Error Handling
- `galaxy_3d_engine::galaxy3d::Result<T>` = `Result<T, galaxy_3d_engine::galaxy3d::Error>`
- Detailed error messages with context
- Never `unwrap()` in library code

---

## ✅ Changelog

### 2026-02-05 - Phase 13: Pattern Vec+HashMap pour Accès par ID
- **Refactoring `MeshLOD`** :
  - ✅ `submeshes: HashMap<String, SubMesh>` → `submeshes: Vec<SubMesh>` + `submesh_names: HashMap<String, usize>`
  - ✅ `add_submesh_internal()` retourne maintenant `usize` (l'id du submesh)
  - ✅ Nouvelles méthodes : `submesh(id)`, `submesh_id(name)`, `submesh_by_name(name)`
- **Refactoring `Mesh`** :
  - ✅ `meshes: HashMap<String, MeshEntry>` → `mesh_entries: Vec<MeshEntry>` + `entry_names: HashMap<String, usize>`
  - ✅ `add_mesh_entry()` retourne maintenant `Result<usize>` (l'id de l'entry)
  - ✅ Nouvelles méthodes : `mesh_entry(id)`, `mesh_entry_id(name)`, `mesh_entry_by_name(name)`
- **API ResourceManager mise à jour** :
  - ✅ `add_mesh_entry()` retourne `Result<usize>`
  - ✅ `add_mesh_lod()` prend `entry_id: usize` au lieu de `entry_name: &str`
  - ✅ `add_submesh()` prend `entry_id: usize` au lieu de `entry_name: &str`
- **Philosophie** :
  - Accès O(1) par id (index) ou par nom via HashMap
  - Pattern similaire aux textures resource (consistance)
  - Les LODs restent indexés par index numérique (pas de noms)

**Justification** : Permet un accès rapide par id numérique tout en conservant la possibilité d'accès par nom. L'utilisateur peut stocker l'id retourné pour éviter les lookups répétés par nom.

### 2026-02-05 - Phase 12: Resource Mesh System
- **Nouveau module `resource::mesh`**:
  - ✅ Hiérarchie 4 niveaux : `Mesh` → `MeshEntry` → `MeshLOD` → `SubMesh`
  - ✅ `Mesh` : Groupe avec buffers GPU partagés (vertex + index optionnel)
  - ✅ `MeshEntry` : Mesh nommé dans le groupe (ex: "hero", "enemy")
  - ✅ `MeshLOD` : Niveau de détail (index 0 = plus détaillé)
  - ✅ `SubMesh` : Unité de draw call (offsets, counts, topology)
- **Descripteurs**:
  - ✅ `MeshDesc` : Données brutes vertex/index + layout + entries
  - ✅ `MeshEntryDesc`, `MeshLODDesc`, `SubMeshDesc` : Descripteurs hiérarchiques
- **ResourceManager étendu**:
  - ✅ `create_mesh(name, MeshDesc) -> Result<Arc<Mesh>>` — crée buffers GPU + valide offsets
  - ✅ `mesh(name)`, `remove_mesh(name)`, `mesh_count()` — accès/suppression
  - ✅ `add_mesh_entry()`, `add_mesh_lod()`, `add_submesh()` — modification post-création
- **Design optimisé**:
  - ✅ `renderer` stocké uniquement dans `Mesh` (pas dans chaque SubMesh)
  - ✅ HashMap pour entries et submeshes (accès O(1) par nom)
  - ✅ Validation automatique des offsets contre les tailles de buffer

### 2026-02-03 - Phase 11d: Array Texture Layer Upload
- **`ArrayLayerDesc` étendu**:
  - ✅ Nouveau champ `data: Option<Vec<u8>>` pour fournir les pixels à la création
- **Trait `render::Renderer` étendu**:
  - ✅ Nouvelle méthode `update_texture_layer(texture, layer, data) -> Result<()>`
  - ✅ Permet l'upload de pixels vers un layer spécifique d'une texture existante
- **Backend Vulkan**:
  - ✅ `update_texture_layer()` implémenté avec staging buffer + transitions layout
  - ✅ Transition: SHADER_READ_ONLY → TRANSFER_DST → SHADER_READ_ONLY
- **Trait `resource::Texture` modifié**:
  - ✅ Signature `add_array_layer(name, layer, data: Option<&[u8]>) -> Result<()>`
  - ✅ `ArrayTexture::add_array_layer()` appelle `renderer.update_texture_layer()` si data fourni
- **ResourceManager modifié**:
  - ✅ `create_array_texture()` construit `TextureData::Layers` depuis les `ArrayLayerDesc` avec data
  - ✅ `add_array_layer()` accepte `data: Option<&[u8]>` pour upload post-création

### 2026-02-03 - Phase 11c: Resource Texture Refactoring
- **Trait `resource::Texture` étendu**:
  - ✅ Nouvelles méthodes avec implémentation par défaut :
    - `add_atlas_region(name, region) -> Result<()>` — erreur par défaut
    - `add_array_layer(name, layer) -> Result<()>` — erreur par défaut
  - ✅ Override dans `AtlasTexture::add_atlas_region()` — ajoute région à la HashMap
  - ✅ Override dans `ArrayTexture::add_array_layer()` — ajoute layer à la HashMap
- **Structs modifiées (stockent le Renderer)**:
  - ✅ `SimpleTexture` — nouveau champ `renderer: Arc<Mutex<dyn Renderer>>`
  - ✅ `AtlasTexture` — nouveau champ `renderer: Arc<Mutex<dyn Renderer>>`
  - ✅ `ArrayTexture` — nouveau champ `renderer: Arc<Mutex<dyn Renderer>>`
  - ✅ Constructeurs `new()` prennent maintenant le renderer en premier argument
- **ResourceManager amélioré**:
  - ✅ `create_simple_texture()` retourne `Result<Arc<dyn Texture>>` (au lieu de `Result<()>`)
  - ✅ `create_atlas_texture()` retourne `Result<Arc<dyn Texture>>` (au lieu de `Result<()>`)
  - ✅ `create_array_texture()` retourne `Result<Arc<dyn Texture>>` (au lieu de `Result<()>`)
  - ✅ `get_texture()` renommé en `texture()` (convention Rust)
  - ✅ `add_atlas_region()` délègue au trait `Texture::add_atlas_region()`
  - ✅ `add_array_layer()` délègue au trait `Texture::add_array_layer()`
  - ✅ Logs info ajoutés pour la création de textures

### 2026-02-02 - Phase 11b: Texture Array GPU Support & TextureInfo
- **`render::TextureDesc` modifié**:
  - ✅ Nouveau champ `array_layers: u32` (1 = texture simple, >1 = texture array)
  - ✅ Champ `data` changé de `Option<Vec<u8>>` vers `Option<TextureData>`
- **Nouveaux types `render`**:
  - ✅ `TextureData` enum — `Single(Vec<u8>)` ou `Layers(Vec<TextureLayerData>)`
  - ✅ `TextureLayerData` struct — `{ layer: u32, data: Vec<u8> }`
  - ✅ `TextureInfo` struct — propriétés lecture seule (width, height, format, usage, array_layers) + `is_array()`
- **Trait `render::Texture` étendu**:
  - ✅ Méthode `fn info(&self) -> &TextureInfo` (était vide avant)
- **Backend Vulkan**:
  - ✅ `create_texture()` supporte `array_layers > 1`
  - ✅ Image view `TYPE_2D_ARRAY` pour texture arrays, `TYPE_2D` pour textures simples
  - ✅ Barriers couvrent toutes les layers
  - ✅ Upload multi-layer (un staging buffer par layer, copie vers `base_array_layer`)
  - ✅ Transition directe `UNDEFINED → SHADER_READ_ONLY` si aucune donnée
  - ✅ Validation des indices de layer
  - ✅ `VulkanTexture` stocke `TextureInfo`
- **ResourceManager**:
  - ✅ `create_simple_texture()` valide `array_layers == 1`
  - ✅ `create_atlas_texture()` valide `array_layers == 1`
  - ✅ `create_array_texture()` valide `array_layers > 1` + indices de layer
- **Demo**: Adaptée au nouveau format `TextureData::Single`

### 2026-02-02 - Phase 11: Resource Textures
- **Nouveau trait `resource::Texture`**:
  - ✅ Trait `Texture` avec `render_texture()`, `descriptor_set()`, `region_names()`
  - ✅ Downcast explicite : `as_simple()`, `as_atlas()`, `as_atlas_mut()`, `as_array()`, `as_array_mut()`
  - ✅ `SimpleTexture` — texture simple sans sous-régions
  - ✅ `AtlasTexture` — atlas avec `HashMap<String, AtlasRegion>` (u, v, width, height)
  - ✅ `ArrayTexture` — texture array avec `HashMap<String, u32>` (layer index)
- **Types de données**:
  - ✅ `AtlasRegion` — coordonnées UV d'une sous-région
  - ✅ `AtlasRegionDesc` — descripteur pour création batch de régions
  - ✅ `ArrayLayerDesc` — descripteur pour création batch de layers
- **ResourceManager étendu**:
  - ✅ `create_simple_texture(name, TextureDesc)` — crée texture GPU + descriptor set via renderer
  - ✅ `create_atlas_texture(name, TextureDesc, &[AtlasRegionDesc])` — atlas avec régions optionnelles
  - ✅ `create_array_texture(name, TextureDesc, &[ArrayLayerDesc])` — array avec layers optionnels
  - ✅ `get_texture(name)` — accès par nom
  - ✅ `remove_texture(name)` — suppression par nom
  - ✅ `texture_count()` — nombre de textures
  - ✅ `add_atlas_region(texture_name, region_name, AtlasRegion)` — ajout de région post-création
  - ✅ `add_array_layer(texture_name, layer_name, u32)` — ajout de layer post-création
  - ✅ Mutation via `Arc::get_mut` + downcast `as_atlas_mut()`/`as_array_mut()`
- **Architecture**:
  - Stockage uniforme : `HashMap<String, Arc<dyn Texture>>`
  - Création via ResourceManager appelle le Renderer en interne
  - Régions/layers flexibles : passées à la création OU ajoutées plus tard (`&[]` accepté)

### 2026-02-02 - Phase 10: ResourceManager (Empty Singleton)
- **Nouveau module `resource/`**:
  - ✅ Créé `resource/mod.rs` - Déclaration du module resource
  - ✅ Créé `resource/resource_manager.rs` - Struct `ResourceManager` (vide pour l'instant)
- **Intégration dans Engine singleton**:
  - ✅ `Engine::create_resource_manager()` - Crée et enregistre le singleton ResourceManager
  - ✅ `Engine::resource_manager()` - Accès global au ResourceManager (`Arc<Mutex<ResourceManager>>`)
  - ✅ `Engine::destroy_resource_manager()` - Détruit le singleton ResourceManager
  - ✅ `Engine::shutdown()` - Détruit le ResourceManager **avant** le Renderer (ordre de destruction sûr)
  - ✅ `EngineState` mis à jour avec champ `resource_manager`
- **Workspace**:
  - ✅ Retiré `galaxy3d_demo` du workspace Cargo.toml (la demo est un projet externe)
- **Architecture**:
  - Pas de trait/backend, struct concrète simple
  - Même pattern singleton que le Renderer (OnceLock + RwLock + Arc<Mutex>)
  - Les ressources seront ajoutées ultérieurement

### 2026-01-27 - Phase 9: Backend-Agnostic API (100% Portable)
- **Abstraction Complète**:
  - ✅ Nouveau trait `galaxy_3d_engine::galaxy3d::render::DescriptorSet` pour masquer `vk::DescriptorSet`
  - ✅ Méthode `Renderer::create_descriptor_set_for_texture()` retourne `Arc<dyn galaxy_3d_engine::galaxy3d::render::DescriptorSet>`
  - ✅ Méthode `Renderer::submit_with_swapchain()` prend `&dyn galaxy_3d_engine::galaxy3d::render::Swapchain` (plus de semaphores Vulkan exposés)
  - ✅ Méthode `galaxy_3d_engine::galaxy3d::render::CommandList::bind_descriptor_sets()` prend `&[&Arc<dyn galaxy_3d_engine::galaxy3d::render::DescriptorSet>]`
  - ✅ Méthodes `galaxy_3d_engine::galaxy3d::render::Swapchain::width/height/format()` retournent types génériques
- **Détails Vulkan Cachés**:
  - ✅ `Vulkangalaxy_3d_engine::galaxy3d::render::Pipeline.pipeline_layout` → `pub(crate)` (privé)
  - ✅ `Vulkangalaxy_3d_engine::galaxy3d::render::Swapchain::sync_info()` → `pub(crate)` (privé)
  - ✅ `galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer::get_descriptor_set_layout()` → `pub(crate)` (privé)
  - ✅ Ajout de `get_descriptor_set_layout_handle()` qui retourne `u64` (pas de type Vulkan)
- **Migration Demo**:
  - ❌ Supprimé `use ash::vk::Handle`
  - ❌ Supprimé imports `Vulkangalaxy_3d_engine::galaxy3d::render::Pipeline`, `Vulkangalaxy_3d_engine::galaxy3d::render::CommandList`, `Vulkangalaxy_3d_engine::galaxy3d::render::Texture`
  - ✅ `Vec<Arc<dyn galaxy_3d_engine::galaxy3d::render::DescriptorSet>>` remplace `Vec<vk::DescriptorSet>`
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
  - ✅ Exports publics: `Vulkangalaxy_3d_engine::galaxy3d::render::Pipeline`, `Vulkangalaxy_3d_engine::galaxy3d::render::CommandList`, `Vulkangalaxy_3d_engine::galaxy3d::render::Texture`
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
  - ✅ `galaxy_3d_engine::galaxy3d::render::Swapchain` trait (séparation présentation)
  - ✅ `galaxy_3d_engine::galaxy3d::render::RenderTarget` trait (texture ou swapchain)
  - ✅ `galaxy_3d_engine::galaxy3d::render::RenderPass` trait (configuration)

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
- [x] Créer trait `galaxy_3d_engine::galaxy3d::render::DescriptorSet` pour masquer `vk::DescriptorSet`
- [x] Ajouter `create_descriptor_set_for_texture()` retournant `Arc<dyn galaxy_3d_engine::galaxy3d::render::DescriptorSet>`
- [x] Ajouter `submit_with_swapchain()` prenant `&dyn galaxy_3d_engine::galaxy3d::render::Swapchain`
- [x] Modifier `bind_descriptor_sets()` pour prendre traits abstraits
- [x] Ajouter `width()`, `height()`, `format()` à `galaxy_3d_engine::galaxy3d::render::Swapchain`
- [x] Cacher tous les champs Vulkan publics (`pub(crate)`)
- [x] Supprimer toutes références Vulkan de la demo
- [x] Éliminer tous les casts `unsafe` du code applicatif
- [x] Validation: 0 violations, 0 fuites, score 10/10

**Status**: API 100% portable, backend Direct3D 12 possible sans modifier la demo ✅

### ✅ Phase 10: ResourceManager (DONE)
- [x] Module `resource/` avec `ResourceManager` struct (vide)
- [x] Singleton Engine: `create_resource_manager()`, `resource_manager()`, `destroy_resource_manager()`
- [x] `shutdown()` détruit ResourceManager avant Renderer (ordre sûr)
- [x] Retiré `galaxy3d_demo` du workspace

**Status**: ResourceManager singleton vide, prêt pour les ressources ✅

### ✅ Phase 11: Resource Textures (DONE)
- [x] Trait `resource::Texture` avec downcast explicite (as_simple, as_atlas, as_array)
- [x] `SimpleTexture`, `AtlasTexture`, `ArrayTexture` — 3 types concrets
- [x] `AtlasRegion`, `AtlasRegionDesc`, `ArrayLayerDesc` — types de données
- [x] ResourceManager étendu : création, accès, suppression, modification de textures
- [x] Création via renderer interne (GPU texture + descriptor set)
- [x] Régions/layers flexibles : passées à la création OU ajoutées plus tard

**Status**: Système de textures resource complet, intégré au ResourceManager ✅

### ✅ Phase 11b: Texture Array GPU Support & TextureInfo (DONE)
- [x] `TextureDesc` étendu avec `array_layers` et `data: Option<TextureData>`
- [x] Nouveaux types : `TextureData`, `TextureLayerData`, `TextureInfo`
- [x] Trait `render::Texture` avec méthode `info()` retournant `&TextureInfo`
- [x] Backend Vulkan : texture arrays GPU (TYPE_2D_ARRAY, upload multi-layer)
- [x] ResourceManager : validation `array_layers` dans les 3 méthodes de création
- [x] Demo adaptée au nouveau format

**Status**: Support complet texture array GPU au niveau render, propriétés accessibles via `info()` ✅

### ✅ Phase 11c: Resource Texture Refactoring (DONE)
- [x] Trait `resource::Texture` avec méthodes par défaut `add_atlas_region()` et `add_array_layer()`
- [x] Structs stockent `Arc<Mutex<dyn Renderer>>` pour futures fonctionnalités
- [x] Fonctions `create_*_texture()` retournent `Arc<dyn Texture>` (au lieu de `Result<()>`)
- [x] `get_texture()` renommé en `texture()` (convention Rust)
- [x] ResourceManager délègue aux méthodes du trait
- [x] Logs info ajoutés lors de la création de textures

**Status**: API améliorée, textures accessibles directement après création, délégation aux traits ✅

### ✅ Phase 11d: Array Texture Layer Upload (DONE)
- [x] `ArrayLayerDesc` étendu avec `data: Option<Vec<u8>>`
- [x] Trait `render::Renderer` étendu avec `update_texture_layer()`
- [x] VulkanRenderer : implémentation avec staging buffer + transitions layout
- [x] Trait `resource::Texture::add_array_layer()` signature modifiée pour accepter data
- [x] `ArrayTexture` appelle le renderer pour upload si data fourni
- [x] `ResourceManager::create_array_texture()` construit `TextureData::Layers` automatiquement
- [x] `ResourceManager::add_array_layer()` accepte `data: Option<&[u8]>`

**Status**: Upload pixels possible à la création ET après création des texture arrays ✅

### ✅ Phase 12: Resource Mesh System (DONE)
- [x] Hiérarchie 4 niveaux : `Mesh` → `MeshEntry` → `MeshLOD` → `SubMesh`
- [x] `Mesh` : Groupe avec buffers GPU partagés (vertex + index optionnel)
- [x] Descripteurs : `MeshDesc`, `MeshEntryDesc`, `MeshLODDesc`, `SubMeshDesc`
- [x] ResourceManager : `create_mesh()`, `mesh()`, `remove_mesh()`, `mesh_count()`
- [x] Modification post-création : `add_mesh_entry()`, `add_mesh_lod()`, `add_submesh()`
- [x] Renderer stocké uniquement dans `Mesh` (pas dans chaque SubMesh)
- [x] Validation automatique des offsets contre les tailles de buffer

**Status**: Système de mesh resource complet avec hiérarchie 4 niveaux ✅

### ✅ Phase 13: Pattern Vec+HashMap pour Accès par ID (DONE)
- [x] Refactoring `MeshLOD` : `Vec<SubMesh>` + `HashMap<String, usize>` pour submeshes
- [x] Refactoring `Mesh` : `Vec<MeshEntry>` + `HashMap<String, usize>` pour entries
- [x] `add_mesh_entry()`, `add_mesh_lod()`, `add_submesh()` retournent des ids (usize)
- [x] Nouvelles méthodes d'accès : par id (direct) ou par nom (via HashMap)
- [x] ResourceManager API mise à jour pour utiliser `entry_id: usize`

**Status**: Accès O(1) par id ou par nom, pattern consistant avec textures ✅

### Phase 14: Advanced Features (TODO)
- [ ] Uniform buffers
- [ ] Compute shaders
- [ ] Multi-pass deferred rendering
- [ ] Scene graph basique

---

## 🖼️ Phase 10-12 : Système de Textures Avancé (Planification)

### Vue d'Ensemble

Ces phases concernent l'amélioration du système de textures pour atteindre les standards AAA :
- **Phase 10** : Mipmaps CPU avec filtres de qualité (Lanczos-3)
- **Phase 11** : Support compression BC7/BC5/BC4 avec fichiers DDS
- **Phase 12** : Support KTX2 multi-plateforme et optimisations avancées

---

### 1. Types de Textures Modernes

#### 1.1 Texture Simple (Actuel - Phase 9)

**Définition** : Une texture = une ressource GPU

```rust
// Actuellement implémenté
let texture = renderer.create_texture(TextureDesc {
    format: TextureFormat::RGBA8Unorm,
    width: 1024,
    height: 1024,
    data: &rgba_bytes,
});
```

**Caractéristiques** :
- ✅ Simple à utiliser
- ✅ Un descriptor par texture
- ⚠️ Limité à 16-32 textures simultanées (limitation descriptors)

---

#### 1.2 Texture Atlas

**Définition** : Plusieurs textures packées dans une seule image physique

```
Atlas 2048×2048 :
┌─────────────────────────────────┐
│ Texture A  │ Texture B │ Tex C │
│ (512×512)  │ (512×512) │(256×) │
├────────────┼───────────┼───────┤
│ Texture D  │ Texture E │ Pad   │
│ (1024×512) │ (512×512) │       │
└─────────────────────────────────┘
```

**Usage** :
```rust
// UV mapping ajusté pour chaque sous-texture
let uv_texture_a = uv * vec2(0.25, 0.5) + vec2(0.0, 0.0);
let color = texture(atlas, uv_texture_a);
```

**Avantages** :
- ✅ Réduit le nombre de descriptors (1 atlas = 50+ textures)
- ✅ Bon pour sprites 2D, UI, particules

**Inconvénients** :
- ❌ Problèmes de bleeding avec mipmaps (filtrage déborde)
- ❌ Toutes les textures doivent avoir même format
- ❌ Complexe à gérer (packing, UV remapping)

**Recommandation** : Utiliser pour UI/sprites 2D uniquement (Phase 12+)

---

#### 1.3 Texture Array

**Définition** : Stack de textures de même taille, indexées

```
Texture Array (4 layers, 1024×1024) :
┌─────────────┐
│ Layer 0     │ ← Grass
├─────────────┤
│ Layer 1     │ ← Stone
├─────────────┤
│ Layer 2     │ ← Wood
├─────────────┤
│ Layer 3     │ ← Metal
└─────────────┘
```

**Usage** :
```glsl
// Shader
uniform sampler2DArray terrainTextures; // 1 descriptor!

void main() {
    int materialID = getMaterialID(); // 0-3
    vec4 color = texture(terrainTextures, vec3(uv, materialID));
}
```

**Avantages** :
- ✅ 1 descriptor = 256+ textures (Vulkan limite : 2048 layers)
- ✅ Mipmaps indépendants par layer (pas de bleeding)
- ✅ Idéal pour terrain, decals, material systems

**Inconvénients** :
- ⚠️ Toutes les layers doivent avoir même taille/format
- ⚠️ Gaspillage si textures de tailles variées

**Recommandation** : Utiliser pour terrains, materials (Phase 12+)

---

#### 1.4 Bindless Textures (Descriptor Indexing)

**Définition** : Array de descriptors, indexation dynamique en shader

```rust
// Créer descriptor pool large
let descriptors = renderer.create_descriptor_array(1000); // 1000 textures

// Bind toutes les textures dans un seul descriptor
for (i, texture) in textures.iter().enumerate() {
    descriptors.bind_texture(i, texture);
}
```

**Usage** :
```glsl
// Shader
layout(binding = 0) uniform sampler2D allTextures[1000]; // Non-uniform indexing

void main() {
    int textureID = material.diffuseTextureID; // Peut varier par pixel!
    vec4 color = texture(allTextures[textureID], uv);
}
```

**Avantages** :
- ✅ Pas de limite pratique (1000+ textures)
- ✅ Pas de rebinding entre draw calls
- ✅ Idéal pour open world, batching

**Prérequis** :
- Vulkan 1.2+ avec `VK_EXT_descriptor_indexing`
- Support GPU (97.8% des GPU modernes)

**Recommandation** : Implémenter en Phase 12+ (optimisation)

---

#### 1.5 Virtual Texturing (Mega Textures)

**Définition** : Streaming de tuiles de texture depuis disque

**Principe** :
- Texture virtuelle 32K×32K (trop grosse pour VRAM)
- Divisée en tuiles 512×512
- Seules les tuiles visibles sont chargées en VRAM

**Usage** : id Tech (Rage, Doom Eternal), Unreal Engine 5 (Virtual Textures)

**Recommandation** : Hors scope Galaxy3D (complexité AAA)

---

### 2. Mipmaps

#### 2.1 Qu'est-ce qu'un Mipmap ?

**Définition** : Chaîne de versions pré-calculées d'une texture, chacune 2× plus petite

```
Texture 1024×1024 avec mipmaps :
Mip 0 : 1024×1024 (original)    4 MB
Mip 1 :  512×512                1 MB
Mip 2 :  256×256                256 KB
Mip 3 :  128×128                64 KB
...
Mip 10:    1×1                  4 bytes

Total : 5.33 MB (original × 1.33)
```

**Pourquoi utiliser des mipmaps ?**

1. **Qualité visuelle** : Anti-aliasing, élimine moiré/scintillement
2. **Performance** : Cache coherence (accès mémoire contigus)
3. **Bande passante** : Moins de données à lire (1/4 par niveau)

**Sélection automatique GPU** :
```glsl
// GPU choisit automatiquement le mipmap selon distance
vec4 color = texture(sampler, uv);
// Proche : Mip 0 (détails max)
// Moyen : Mip 3-5 (bon équilibre)
// Loin : Mip 8-10 (économie bande passante)
```

---

#### 2.2 Génération Mipmaps : CPU vs GPU

**Option A : GPU (Actuel - Phase 9)**

```rust
// Galaxy3D Phase 9
let texture = renderer.create_texture(TextureDesc {
    data: &rgba_bytes,
    generate_mipmaps: true, // GPU génère (Box filter)
});
```

**Implémentation Vulkan** :
```cpp
vkCmdBlitImage(
    command_buffer,
    src_image, src_layout, // Mip N
    dst_image, dst_layout, // Mip N+1 (2× plus petit)
    VK_FILTER_LINEAR       // Box filter (moyenne 2×2)
);
```

**Avantages** :
- ✅ Rapide (< 1 ms GPU)
- ✅ Simple à implémenter

**Inconvénients** :
- ❌ Qualité faible (Box filter = moyenne 2×2)
- ❌ Artefacts visibles (aliasing, perte détails)
- ❌ Score qualité : 3/10

---

**Option B : CPU Offline (Recommandé AAA)**

```rust
// Phase 11 : Build pipeline
fn build_texture(source: &Path) {
    let rgba = load_png(source)?;

    // Générer mipmaps CPU (Lanczos-3)
    let mipmaps = generate_mipmaps_lanczos3(&rgba); // 50-100 ms

    // Compresser BC7
    let bc7_mipmaps = mipmaps.iter()
        .map(|m| compress_bc7(m, Quality::High))
        .collect();

    // Sauvegarder DDS
    save_dds("texture.dds", bc7_mipmaps);
}
```

**Avantages** :
- ✅ Qualité maximale (Lanczos-3, Kaiser, etc.)
- ✅ Mipmaps pré-calculés (runtime = 0 coût)
- ✅ Score qualité : 9-10/10

**Inconvénients** :
- ⚠️ Build time (50-200 ms par texture)

---

**Option C : CPU Runtime (Phase 10)**

```rust
// Phase 10 : Runtime avec crate image
fn load_texture_with_mipmaps(path: &str) -> Texture {
    let rgba = image::open(path)?.to_rgba8();

    // Générer mipmaps CPU (Lanczos-3)
    let mipmaps = generate_mipmaps_lanczos3(&rgba); // 50 ms

    renderer.create_texture(TextureDesc {
        data: &rgba,
        mipmap_data: Some(mipmaps), // Pré-calculés CPU
    })
}
```

**Avantages** :
- ✅ Qualité excellente (Lanczos-3)
- ✅ Pas de build pipeline nécessaire

**Inconvénients** :
- ⚠️ Chargement plus lent (+50 ms par texture)

---

#### 2.3 Filtres de Génération Mipmaps

| Filtre | Qualité | Vitesse CPU | Usage | Artefacts |
|--------|---------|-------------|-------|-----------|
| **Box** (GPU) | 3/10 | N/A (GPU) | Prototypage | Aliasing fort, perte détails |
| **Bilinear** | 5/10 | Rapide | Legacy | Aliasing modéré |
| **Bicubic** | 7/10 | Moyen | Bon compromis | Léger flou |
| **Lanczos-3** | 9/10 | Lent | AAA Standard | Minimal (sharpness excellente) |
| **Kaiser** | 10/10 | Très lent | Unity default | Aucun (qualité parfaite) |

**Recommandation** :
- **Phase 9 (actuel)** : Box GPU (prototypage)
- **Phase 10** : Lanczos-3 CPU runtime
- **Phase 11+** : Lanczos-3 CPU offline (build pipeline)

**Implémentation Lanczos-3** :
```rust
use image::imageops::FilterType;

fn generate_mipmaps_lanczos3(image: &RgbaImage) -> Vec<RgbaImage> {
    let mut mipmaps = vec![image.clone()];
    let (mut w, mut h) = image.dimensions();

    while w > 1 || h > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);

        let mip = image::imageops::resize(
            mipmaps.last().unwrap(),
            w, h,
            FilterType::Lanczos3 // Filtre Lanczos-3
        );
        mipmaps.push(mip);
    }

    mipmaps
}
```

---

### 3. Compression Textures

#### 3.1 DDS : Format Conteneur

**DDS** = DirectDraw Surface (Microsoft)

**Rôle** : Conteneur de fichier (comme .ZIP) qui stocke :
- Données texture (compressées ou non)
- Mipmaps (pré-calculés)
- Metadata (format, taille, flags)

**Structure fichier** :
```
texture.dds :
├─ Header (128 bytes)
│  ├─ Magic "DDS "
│  ├─ Width, Height
│  ├─ Mipmap count
│  └─ Format (BC7, BC5, RGBA8, etc.)
├─ Mipmap 0 (1024×1024) - BC7 data
├─ Mipmap 1 (512×512) - BC7 data
├─ Mipmap 2 (256×256) - BC7 data
└─ ...
```

**Important** : DDS peut contenir N'IMPORTE QUEL format :
- ✅ BC7 compressé
- ✅ BC1/BC3/BC5 compressés
- ✅ RGBA8 non compressé
- ✅ Float16/32 formats (HDR)

---

#### 3.2 Formats de Compression BC (Block Compression)

**BC** = Block Compression (DirectX 10+)

Principe : Compresser blocks 4×4 pixels (16 pixels → N bytes)

| Format | Channels | Ratio | Taille 1K | Usage | Qualité |
|--------|----------|-------|-----------|-------|---------|
| **BC1** (DXT1) | RGB(A*) | 6:1 ou 8:1 | 512 KB | Legacy diffuse | 6/10 |
| **BC3** (DXT5) | RGBA | 4:1 | 1 MB | Legacy diffuse+alpha | 6/10 |
| **BC4** | R | 8:1 | 512 KB | Grayscale (height, roughness) | 8/10 |
| **BC5** | RG | 4:1 | 1 MB | Normal maps | 10/10 |
| **BC6H** | RGB HDR | 6:1 | 512 KB | HDR lighting (16-bit float) | 10/10 |
| **BC7** | RGBA | 4:1 | 1 MB | Modern diffuse (best) | 10/10 |

\* BC1 alpha = 1-bit (0 ou 255, pas de semi-transparence)

**Comparaison RGBA8 vs BC7** :
```
Texture 1024×1024 (avec mipmaps) :

RGBA8 non compressé :
  - Taille VRAM : 5.33 MB
  - Bande passante : Élevée (4 bytes/pixel)
  - FPS : Baseline

BC7 compressé :
  - Taille VRAM : 1.33 MB (4× moins!)
  - Bande passante : Faible (1 byte/pixel)
  - FPS : +20-40% (cache GPU + bande passante)
  - Qualité : 99% identique (PSNR 45+ dB)
```

---

#### 3.3 BC7 : Lossy mais Imperceptible

**BC7 est une compression avec pertes** :
- ❌ **Pas lossless** (il y a des artefacts mathématiques)
- ✅ **Perceptuellement lossless** (invisible à l'œil 95% du temps)

**Test qualité** :
```
Original RGBA8    : PSNR = ∞ (référence)
BC7 (quality 100) : PSNR = 48 dB (excellent, imperceptible)
BC7 (quality 50)  : PSNR = 42 dB (bon, légèrement visible)
JPEG (quality 90) : PSNR = 35 dB (artefacts visibles)
```

**Cas où BC7 échoue** :
1. **Dégradés subtils** : Léger banding (solution : dithering avant compression)
2. **Texte haute résolution** : Flou (solution : garder RGBA8 pour UI)
3. **Alpha sharp** : Fringe autour bords (solution : BC7 sharp alpha mode)

**Recommandation** :
- ✅ BC7 pour 95% des textures (world, characters, props)
- ❌ RGBA8 pour UI/texte (5% des textures)

---

#### 3.4 Compression CPU vs GPU

**Question** : Qui compresse en BC7 ?

**Réponse** : **TOUJOURS le CPU** (jamais le GPU)

**Pourquoi ?**

```
BC7 Compression (RGBA → BC7) :
  - Complexité : NP-hard optimization
  - Temps : 10-200 ms par texture 1K
  - Algorithme : Essai/erreur, partitioning
  - Hardware : Software (CPU)

BC7 Decompression (BC7 → RGBA) :
  - Complexité : Simple (interpolation linéaire)
  - Temps : < 1 cycle GPU (gratuit)
  - Algorithme : Lookup table + lerp
  - Hardware : Texture units GPU (intégré)
```

**Vulkan ne peut PAS compresser BC7** :
```rust
// ❌ IMPOSSIBLE
vkCmdBlitImage(src_rgba8, dst_bc7, ...); // Erreur validation!

// ✅ POSSIBLE (déjà compressé)
let bc7_data = compress_bc7_cpu(&rgba); // CPU
vkCmdCopyBufferToImage(buffer(bc7_data), image_bc7); // Upload
```

---

#### 3.5 Usages Recommandés par Format

```rust
match texture_type {
    // Diffuse/Albedo avec alpha (character, props)
    TextureType::Diffuse => Format::BC7,

    // Normal maps (2 channels RG, blue recalculé)
    TextureType::NormalMap => Format::BC5,

    // Roughness/Metallic/AO (grayscale)
    TextureType::Grayscale => Format::BC4,

    // HDR environment maps (skybox, lightprobes)
    TextureType::HDR => Format::BC6H,

    // UI, texte (besoin sharpness)
    TextureType::UI => Format::RGBA8,
}
```

---

### 4. Roadmap Galaxy3DEngine

#### Phase 9 (ACTUEL) ✅

**État** : Système texture basique fonctionnel

```rust
let texture = renderer.create_texture(TextureDesc {
    format: TextureFormat::RGBA8Unorm,
    width: 1024,
    height: 1024,
    data: &png_rgba_bytes,
    generate_mipmaps: true, // GPU Box filter
});
```

**Caractéristiques** :
- ✅ Formats : RGBA8, RGB8, RG8, R8
- ✅ Chargement : PNG, BMP, JPEG (via galaxy_image)
- ✅ Mipmaps : GPU Box filter (qualité 3/10)
- ✅ Alpha blending fonctionnel
- ✅ Descriptor sets abstraction

**Limitations** :
- ⚠️ Pas de compression (VRAM 4× plus grande)
- ⚠️ Mipmaps qualité faible (Box filter)
- ⚠️ Chargement lent pour grandes textures

---

#### Phase 14 : Mipmaps pour render::Texture (En cours)

**Objectif** : Support complet des mipmaps pour les textures GPU

**Statut** : ✅ Structure API implémentée | ⏳ Génération GPU et upload manuel à implémenter

---

##### 1. API MipmapMode

Trois modes de gestion des mipmaps :

```rust
/// Mode de génération des mipmaps
#[derive(Debug, Clone)]
pub enum MipmapMode {
    /// Pas de mipmaps - uniquement le niveau de base (mip_levels = 1)
    /// Usage : textures UI, render targets, textures procédurales
    None,

    /// Génération automatique sur GPU via hardware blit
    /// - Génère la chaîne complète jusqu'à 1x1 par défaut
    /// - max_levels optionnel pour limiter la chaîne
    /// - Qualité : filtre bilinéaire/box (rapide mais qualité moyenne)
    Generate {
        max_levels: Option<u32>,
    },

    /// Mipmaps fournies manuellement (niveaux 1+)
    /// Le niveau 0 vient de TextureData
    /// Usage : assets pré-traités avec mipmaps haute qualité (Lanczos, Kaiser)
    Manual(ManualMipmapData),
}

impl Default for MipmapMode {
    fn default() -> Self { MipmapMode::None }
}
```

##### 2. Données manuelles pour mipmaps

```rust
/// Données mipmap manuelles (niveaux 1, 2, 3, ...)
/// Le niveau 0 est fourni via TextureData
#[derive(Debug, Clone)]
pub enum ManualMipmapData {
    /// Pour textures simples
    /// mips[0] = niveau 1 (demi-résolution)
    /// mips[1] = niveau 2 (quart de résolution), etc.
    Single(Vec<Vec<u8>>),

    /// Pour array textures - données mip par layer
    Layers(Vec<LayerMipmapData>),
}

/// Données mipmap par layer pour array textures
#[derive(Debug, Clone)]
pub struct LayerMipmapData {
    /// Index du layer cible (0-based)
    pub layer: u32,
    /// Niveaux mip pour ce layer
    /// mips[0] = niveau 1, mips[1] = niveau 2, etc.
    pub mips: Vec<Vec<u8>>,
}
```

##### 3. Calcul du nombre de niveaux

```rust
impl MipmapMode {
    /// Calcule le nombre de niveaux mip pour les dimensions données
    pub fn mip_levels(&self, width: u32, height: u32) -> u32 {
        match self {
            MipmapMode::None => 1,
            MipmapMode::Generate { max_levels } => {
                let full_chain = Self::max_mip_levels(width, height);
                max_levels.map(|m| m.min(full_chain)).unwrap_or(full_chain)
            }
            MipmapMode::Manual(data) => {
                let manual_levels = match data {
                    ManualMipmapData::Single(mips) => mips.len(),
                    ManualMipmapData::Layers(layers) => {
                        layers.iter().map(|l| l.mips.len()).max().unwrap_or(0)
                    }
                };
                1 + manual_levels as u32 // Niveau 0 + niveaux manuels
            }
        }
    }

    /// Calcule le nombre max de niveaux mip possibles
    /// Retourne floor(log2(max(width, height))) + 1
    pub fn max_mip_levels(width: u32, height: u32) -> u32 {
        (width.max(height) as f32).log2().floor() as u32 + 1
    }
}
```

##### 4. TextureDesc mis à jour

```rust
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub array_layers: u32,
    pub data: Option<TextureData>,      // Niveau 0
    pub mipmap: MipmapMode,             // ✨ NOUVEAU
}
```

##### 5. TextureInfo avec helpers mipmap

```rust
pub struct TextureInfo {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub array_layers: u32,
    pub mip_levels: u32,                // ✨ NOUVEAU
}

impl TextureInfo {
    /// Retourne true si la texture a des mipmaps
    pub fn has_mipmaps(&self) -> bool {
        self.mip_levels > 1
    }

    /// Calcule les dimensions pour un niveau mip spécifique
    pub fn mip_dimensions(&self, mip_level: u32) -> Option<(u32, u32)> {
        if mip_level >= self.mip_levels { return None; }
        let w = (self.width >> mip_level).max(1);
        let h = (self.height >> mip_level).max(1);
        Some((w, h))
    }

    /// Calcule la taille en octets pour un niveau mip spécifique
    pub fn mip_byte_size(&self, mip_level: u32) -> Option<usize> {
        self.mip_dimensions(mip_level).map(|(w, h)| {
            (w * h * self.format.bytes_per_pixel()) as usize
        })
    }
}
```

##### 6. Méthode update avec support mip_level

```rust
pub trait Texture: Send + Sync {
    fn info(&self) -> &TextureInfo;

    /// Met à jour les données à un layer et niveau mip spécifiques
    fn update(&self, layer: u32, mip_level: u32, data: &[u8]) -> Result<()>;
}
```

##### 7. Modifications Vulkan

**Image création** :
```rust
// Calcul des niveaux mip
let mip_levels = desc.mipmap.mip_levels(desc.width, desc.height);

// Flags d'usage - ajouter TRANSFER_SRC si génération mipmaps
let mut usage_flags = vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST;
if matches!(desc.mipmap, MipmapMode::Generate { .. }) && mip_levels > 1 {
    usage_flags |= vk::ImageUsageFlags::TRANSFER_SRC;
}

// Création image avec mip_levels
let image_create_info = vk::ImageCreateInfo::default()
    .mip_levels(mip_levels)
    // ...
```

**Image view** :
```rust
.subresource_range(vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: mip_levels,  // ✨ Tous les niveaux
    base_array_layer: 0,
    layer_count: array_layers,
})
```

**Barriers** : Toutes les barrières utilisent `level_count: mip_levels` pour couvrir tous les niveaux.

##### 8. Exemples d'utilisation

```rust
// Texture sans mipmaps (UI, render target)
let ui_texture = renderer.create_texture(TextureDesc {
    width: 256, height: 256,
    format: TextureFormat::R8G8B8A8_SRGB,
    usage: TextureUsage::Sampled,
    array_layers: 1,
    data: Some(TextureData::Single(pixels)),
    mipmap: MipmapMode::None,
})?;

// Texture avec mipmaps générés sur GPU
let world_texture = renderer.create_texture(TextureDesc {
    width: 1024, height: 1024,
    format: TextureFormat::R8G8B8A8_SRGB,
    usage: TextureUsage::Sampled,
    array_layers: 1,
    data: Some(TextureData::Single(pixels)),
    mipmap: MipmapMode::Generate { max_levels: None }, // Chaîne complète
})?;

// Texture avec mipmaps manuels haute qualité
let hq_texture = renderer.create_texture(TextureDesc {
    width: 512, height: 512,
    format: TextureFormat::R8G8B8A8_SRGB,
    usage: TextureUsage::Sampled,
    array_layers: 1,
    data: Some(TextureData::Single(level0_pixels)),
    mipmap: MipmapMode::Manual(ManualMipmapData::Single(vec![
        level1_pixels, // 256x256
        level2_pixels, // 128x128
        level3_pixels, // 64x64
    ])),
})?;
```

##### 9. TODO - Implémentation backend

| Tâche | Statut |
|-------|--------|
| Structure API MipmapMode | ✅ Fait |
| TextureDesc.mipmap | ✅ Fait |
| TextureInfo.mip_levels | ✅ Fait |
| Vulkan: Image avec mip_levels | ✅ Fait |
| Vulkan: ImageView level_count | ✅ Fait |
| Vulkan: Barriers level_count | ✅ Fait |
| Vulkan: update(layer, mip_level, data) | ✅ Fait |
| Vulkan: Génération GPU (vkCmdBlitImage) | ⏳ TODO |
| Vulkan: Upload mipmaps manuels | ⏳ TODO |

##### 10. Génération GPU (à implémenter)

Pour `MipmapMode::Generate`, utiliser `vkCmdBlitImage` :

```rust
// Pseudo-code pour génération GPU
for mip in 1..mip_levels {
    let src_mip = mip - 1;
    let (src_w, src_h) = mip_dimensions(src_mip);
    let (dst_w, dst_h) = mip_dimensions(mip);

    // Transition src → TRANSFER_SRC_OPTIMAL
    // Transition dst → TRANSFER_DST_OPTIMAL

    vkCmdBlitImage(
        command_buffer,
        image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
        image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        &[VkImageBlit {
            srcSubresource: { mip_level: src_mip, ... },
            srcOffsets: [0,0,0], [src_w, src_h, 1],
            dstSubresource: { mip_level: mip, ... },
            dstOffsets: [0,0,0], [dst_w, dst_h, 1],
        }],
        VK_FILTER_LINEAR,
    );

    // Transition dst → SHADER_READ_ONLY
}
```

**Note** : Le filtre LINEAR de Vulkan est équivalent à un box filter (qualité moyenne). Pour une meilleure qualité, utiliser `MipmapMode::Manual` avec des mipmaps pré-calculés (Lanczos, Kaiser) via une bibliothèque externe comme `image`

---

#### Phase 11 : Compression BC7 + DDS (Planifié)

**Objectif** : Support compression BC7/BC5/BC4 avec fichiers DDS

**Changements API** :

```rust
// Ajouter formats compressés
pub enum TextureFormat {
    // Existants
    RGBA8Unorm,
    RGB8Unorm,

    // ✨ NOUVEAUX
    BC7Unorm,      // RGBA compressed (4:1)
    BC5Unorm,      // RG compressed (4:1) - Normal maps
    BC4Unorm,      // R compressed (8:1) - Grayscale
    BC6HUfloat,    // RGB HDR compressed (6:1)
}

// Nouveau : create_texture_from_file (helper)
impl Renderer {
    fn create_texture_from_file(&self, path: &str)
        -> galaxy_3d_engine::galaxy3d::Result<Arc<dyn galaxy_3d_engine::galaxy3d::render::Texture>>
    {
        match path.extension() {
            "dds" => self.load_dds(path),
            "png" | "jpg" | "bmp" => self.load_image(path),
            _ => Err(galaxy_3d_engine::galaxy3d::Error::UnsupportedFormat),
        }
    }
}
```

**Implémentation** :

1. **Parser DDS** :
```rust
// Nouveau module : galaxy_3d_engine/src/formats/dds.rs
pub struct DdsFile {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat, // BC7, BC5, RGBA8, etc.
    pub mipmap_count: u32,
    pub mipmaps: Vec<Vec<u8>>, // Data BC7 brute
}

pub fn load_dds(path: &Path) -> Result<DdsFile> {
    let bytes = std::fs::read(path)?;

    // Parse header (128 bytes)
    let magic = &bytes[0..4]; // "DDS "
    assert_eq!(magic, b"DDS ");

    let width = read_u32(&bytes, 16);
    let height = read_u32(&bytes, 12);
    let mipmap_count = read_u32(&bytes, 28);

    // Detect format (DXT1/DXT5/DX10)
    let fourcc = &bytes[84..88];
    let format = match fourcc {
        b"DXT1" => TextureFormat::BC1Unorm,
        b"DXT5" => TextureFormat::BC3Unorm,
        b"DX10" => {
            // Extended header (DXGI format)
            let dxgi_format = read_u32(&bytes, 128);
            match dxgi_format {
                98 => TextureFormat::BC7Unorm,
                95 => TextureFormat::BC6HUfloat,
                83 => TextureFormat::BC5Unorm,
                80 => TextureFormat::BC4Unorm,
                _ => return Err(Error::UnsupportedFormat),
            }
        }
        _ => TextureFormat::RGBA8Unorm,
    };

    // Extract mipmap data
    let mut offset = if fourcc == b"DX10" { 148 } else { 128 };
    let mut mipmaps = vec![];

    for mip in 0..mipmap_count {
        let mip_size = calculate_mip_size(width, height, mip, format);
        let data = bytes[offset..offset + mip_size].to_vec();
        mipmaps.push(data);
        offset += mip_size;
    }

    Ok(DdsFile { width, height, format, mipmap_count, mipmaps })
}
```

2. **Support Vulkan BC7** :
```rust
// galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanRenderer::create_texture
let vk_format = match desc.format {
    TextureFormat::RGBA8Unorm => vk::Format::R8G8B8A8_UNORM,
    TextureFormat::BC7Unorm => vk::Format::BC7_UNORM_BLOCK, // ✨ NOUVEAU
    TextureFormat::BC5Unorm => vk::Format::BC5_UNORM_BLOCK,
    TextureFormat::BC4Unorm => vk::Format::BC4_UNORM_BLOCK,
    TextureFormat::BC6HUfloat => vk::Format::BC6H_UFLOAT_BLOCK,
};

// Upload data BC7 (directement, pas de conversion)
vkCmdCopyBufferToImage(staging_buffer(bc7_data), image, ...);
```

3. **Build Pipeline** (optionnel - build.rs) :
```rust
// Compresser PNG → DDS au build
fn main() {
    for png in glob("assets/textures/**/*.png") {
        let rgba = image::open(png)?;

        // Générer mipmaps (Lanczos-3)
        let mipmaps = generate_mipmaps_lanczos3(&rgba);

        // Compresser BC7 (via crate intel-tex)
        let bc7_mipmaps = mipmaps.iter()
            .map(|m| compress_bc7(m, Quality::High))
            .collect();

        // Sauvegarder DDS
        let dds_path = png.with_extension("dds");
        save_dds(&dds_path, bc7_mipmaps)?;
    }
}
```

**Dépendances** :
```toml
[dependencies]
# Pour compression BC7 (optionnel - build.rs seulement)
intel-tex = "0.2" # Intel ISPC Texture Compressor

[dev-dependencies]
# Pour build pipeline
glob = "0.3"
```

**Avantages** :
- ✅ VRAM 4× plus petite (5 GB → 1.3 GB pour 1000 textures)
- ✅ FPS +20-40% (bande passante GPU)
- ✅ Chargement 10× plus rapide (pas de calcul runtime)
- ✅ Standard AAA (Unity, Unreal, tous les jeux)

**Inconvénients** :
- ⚠️ Build time si compression offline (2 sec par texture 4K)
- ⚠️ Fichiers 2-3× plus gros que PNG (mipmaps inclus)

**Estimation** : 5-7 jours développement

---

#### Phase 12 : Optimisations Avancées (Futur)

**Objectifs** :
1. **KTX2** : Support multi-plateforme (BC7 + ASTC dans un fichier)
2. **Texture Arrays** : Batching materials (terrain, decals)
3. **Bindless Textures** : Descriptor indexing (1000+ textures)
4. **Streaming** : Chargement asynchrone (open world)

**Estimation** : 10-15 jours développement

---

### 5. Recommandations

#### Pour Prototypage (Actuel - Phase 9)

```rust
// Simple et rapide
let texture = renderer.create_texture(TextureDesc {
    format: TextureFormat::RGBA8Unorm,
    data: &png_rgba_bytes,
    generate_mipmaps: true, // GPU Box filter
});
```

**Quand utiliser** :
- ✅ Développement rapide
- ✅ < 100 textures
- ✅ Pas de contrainte VRAM

---

#### Pour Production (Phase 10+)

```rust
// Qualité maximale, VRAM optimisée
let texture = renderer.create_texture_from_file("texture.dds")?;
// En interne :
//   - Charge DDS (BC7 + mipmaps Lanczos-3)
//   - Upload direct GPU (pas de calcul)
//   - 15 ms total
```

**Build Pipeline** :
```bash
# Compresser toutes les textures au build
cargo build --release
# → build.rs compresse PNG → DDS automatiquement
```

**Quand utiliser** :
- ✅ Jeu final (distribution)
- ✅ 100+ textures
- ✅ Optimisation VRAM/FPS critique

---

#### Tableau Récapitulatif

| Phase | Format | Mipmaps | VRAM (1000 tex) | FPS | Qualité | Build Time |
|-------|--------|---------|-----------------|-----|---------|------------|
| **9 (actuel)** | RGBA8 | GPU Box | 21 GB | Baseline | 3/10 | 0 |
| **10** | RGBA8 | CPU Lanczos-3 | 21 GB | Baseline | 9/10 | 0 |
| **11** | BC7 | CPU Lanczos-3 | 5 GB | +30% | 9/10 | 50 min |

---

### 6. Références Techniques

#### Outils

- **Compressonator** (AMD) : GUI/CLI pour BC7/ASTC
- **Intel ISPC Texture Compressor** : Compression BC7 ultra rapide (Rust: intel-tex)
- **Basis Universal** : Compression universelle (transcode BC7/ASTC/ETC2)

#### Formats

- **DDS** : https://docs.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds
- **KTX2** : https://registry.khronos.org/KTX/specs/2.0/ktx20.html
- **BC7** : https://docs.microsoft.com/en-us/windows/win32/direct3d11/bc7-format

#### Benchmarks

- Call of Duty: Modern Warfare (2019) : 100% BC7, 60 GB VRAM économisés
- Unity Default Settings : Kaiser filter + BC7 (Desktop) / ASTC (Mobile)
- Unreal Engine 5 : Lanczos-3 + BC7 (quality 100)

---

## 🎮 Phase 13-15 : Système de Mesh et Indirect Drawing (Planification)

### Vue d'Ensemble

Ces phases concernent l'optimisation du système de mesh pour atteindre les performances AAA :
- **Phase 13** : Mesh Batching Global (tous les meshes dans 2 buffers)
- **Phase 14** : Indirect Drawing + GPU Culling (frustum + occlusion)
- **Phase 15** : LODs automatiques + GPU Skinning pour animations

---

### 1. Gestion des Mesh

#### 1.1 Mesh Simple (Actuel - Phase 9)

**Définition** : Un mesh = deux buffers GPU (vertex + index)

```rust
// Actuellement implémenté
let vertex_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::VERTEX,
    data: &vertices,
});

let index_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::INDEX,
    data: &indices,
});

// Dessiner
command_list.bind_vertex_buffer(&vertex_buffer, 0);
command_list.bind_index_buffer(&index_buffer, 0);
command_list.draw_indexed(index_count, 0, 0);
```

**Caractéristiques** :
- ✅ Simple à utiliser
- ✅ Flexible (un mesh = une ressource)
- ⚠️ CPU overhead : 1 draw call = 1 objet (limite ~5000 objets à 60 FPS)
- ⚠️ Beaucoup de state changes (bind buffers à répétition)

**Limitation majeure** : Le CPU devient le bottleneck avant le GPU.

---

#### 1.2 Mesh Batching Global (Phase 13+)

**Principe** : Tous les meshes dans 2 buffers géants

```
Buffer Vertex Global (50 MB) :
┌─────────────────────────────────────────┐
│ Mesh 0    │ Mesh 1      │ Mesh 2  │ ... │
│ (cube)    │ (sphere)    │ (car)   │     │
│ 0-35      │ 36-2083     │ 2084+   │     │
└─────────────────────────────────────────┘

Buffer Index Global (20 MB) :
┌─────────────────────────────────────────┐
│ Mesh 0    │ Mesh 1      │ Mesh 2  │ ... │
│ 0-35      │ 36-3071     │ 3072+   │     │
└─────────────────────────────────────────┘
```

**Usage** :

```rust
// Bind UNE SEULE FOIS au début de la frame
command_list.bind_vertex_buffer(&global_vertex_buffer, 0);
command_list.bind_index_buffer(&global_index_buffer, 0);

// Dessiner plein d'objets (pas de rebind!)
for object in objects {
    // Seulement push constants pour la position/rotation
    command_list.push_constants(0, &object.transform);

    // Draw avec offset dans les buffers globaux
    command_list.draw_indexed(
        object.index_count,
        object.first_index,      // Offset dans index buffer
        object.vertex_offset,    // Offset dans vertex buffer
    );
}
```

**Avantages** :
- ✅ 0 rebinding de buffers
- ✅ State changes minimaux
- ✅ CPU overhead divisé par 10
- ✅ Scale à 100k+ objets

**Table de Mesh** :

```rust
// Metadata des meshes (CPU-side)
struct MeshRegistry {
    meshes: Vec<MeshInfo>,
}

struct MeshInfo {
    mesh_id: u32,
    vertex_offset: i32,    // Offset dans global vertex buffer
    first_index: u32,      // Offset dans global index buffer
    index_count: u32,      // Nombre d'indices
}

// Usage
let cube_mesh = mesh_registry.get(MeshId::CUBE);
command_list.draw_indexed(
    cube_mesh.index_count,
    cube_mesh.first_index,
    cube_mesh.vertex_offset,
);
```

**Exemples AAA** :
- Fortnite : Global buffers, 500k+ objets (arbres, props)
- Assassin's Creed : Global buffers pour végétation dense
- Spider-Man : Global buffers pour buildings/debris

---

### 2. LODs (Level of Detail)

**Principe** : Plusieurs versions du même mesh à différentes résolutions

```
Mesh "Tree" (4 LODs) :
┌────────────────────────────────────┐
│ LOD 0 (proche)   : 10,000 triangles│  0-5 mètres
│ LOD 1 (moyen)    :  2,500 triangles│  5-20 mètres
│ LOD 2 (loin)     :    500 triangles│  20-50 mètres
│ LOD 3 (très loin):     50 triangles│  50-200 mètres
└────────────────────────────────────┘
```

**Sélection automatique** :

```rust
fn select_lod(distance_to_camera: f32, mesh: &Mesh) -> u32 {
    match distance_to_camera {
        d if d < 5.0   => 0, // LOD 0 (détails max)
        d if d < 20.0  => 1, // LOD 1
        d if d < 50.0  => 2, // LOD 2
        _              => 3, // LOD 3 (simplifié)
    }
}

// Dans le render loop
for object in objects {
    let distance = (object.position - camera.position).length();
    let lod = select_lod(distance, &object.mesh);
    let mesh_info = object.mesh.lods[lod];

    command_list.draw_indexed(
        mesh_info.index_count,
        mesh_info.first_index,
        mesh_info.vertex_offset,
    );
}
```

**Avantages** :
- ✅ Objets lointains = moins de triangles
- ✅ FPS +50-100% dans grandes scènes
- ✅ Qualité visuelle préservée (transition progressive)

**Techniques avancées** :
- **Smooth LOD transition** : Blend entre deux LODs (fade in/out)
- **LODs dans global buffer** : Tous les LODs packés ensemble
- **GPU LOD selection** : Compute shader choisit le LOD

**Exemples** :
- Unreal Engine : 4-8 LODs par mesh (auto-generated)
- Unity : LOD Groups avec distances configurables
- Far Cry : LODs + impostors (sprites pour objets très lointains)

---

### 3. GPU Skinning

**Problème** : Animer un personnage avec squelette (bones)

**CPU Skinning (traditionnel - lent)** :
```rust
// Pour chaque frame, pour chaque vertex :
for vertex in vertices {
    let transformed = vec3(0.0);

    // Blend de 4 bones maximum
    for i in 0..4 {
        let bone_index = vertex.bone_indices[i];
        let bone_weight = vertex.bone_weights[i];

        let bone_matrix = skeleton.bones[bone_index].matrix;
        transformed += (bone_matrix * vertex.position) * bone_weight;
    }

    vertex.final_position = transformed;
}
// Upload vers GPU (très lent!)
```

**GPU Skinning (moderne - rapide)** :

```glsl
// Vertex shader
layout(binding = 1) uniform BonesBuffer {
    mat4 bones[256]; // Matrices des bones (upload 1× par frame)
};

layout(location = 0) in vec3 position;
layout(location = 1) in vec4 bone_indices;  // 4 bones max par vertex
layout(location = 2) in vec4 bone_weights;  // Poids de chaque bone

void main() {
    // GPU fait le blending (ultra rapide!)
    vec4 skinned_pos = vec4(0.0);

    for (int i = 0; i < 4; i++) {
        int bone_idx = int(bone_indices[i]);
        float weight = bone_weights[i];

        skinned_pos += (bones[bone_idx] * vec4(position, 1.0)) * weight;
    }

    gl_Position = projection * view * skinned_pos;
}
```

**Données vertex** :

```rust
struct SkinnedVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    bone_indices: [u8; 4],   // Indices dans bones[]
    bone_weights: [f32; 4],  // Poids (sum = 1.0)
}
```

**Avantages** :
- ✅ Upload seulement 256 matrices (16 KB) au lieu de tous les vertices (1-10 MB)
- ✅ Calcul parallèle sur GPU (1000× plus rapide)
- ✅ CPU libre pour gameplay/IA

**Exemples** :
- Tous les jeux AAA modernes utilisent GPU skinning
- Unreal Engine : Supporte 256 bones par skeleton
- Unity : GPU skinning activé par défaut

---

### 4. Indirect Drawing

**Problème** : Draw calls = overhead CPU

```rust
// Approche traditionnelle (lente)
for object in objects { // 10,000 objets
    command_list.push_constants(&object.transform);
    command_list.draw_indexed(
        object.index_count,
        object.first_index,
        object.vertex_offset,
    ); // ← 10,000 appels CPU!
}
```

**Solution** : Un seul appel CPU, les commandes sont dans un buffer GPU

---

#### 4.1 DrawIndexedIndirect

**Structure Vulkan** :

```rust
// Structure d'une commande de draw
struct DrawIndexedIndirectCommand {
    index_count: u32,     // Nombre d'indices
    instance_count: u32,  // Instancing (1 = pas d'instancing)
    first_index: u32,     // Offset dans index buffer
    vertex_offset: i32,   // Offset dans vertex buffer
    first_instance: u32,  // Base instance (pour instancing)
}
```

**Usage** :

```rust
// 1. Créer buffer avec 10,000 commandes de draw
let mut draw_commands = Vec::new();
for object in objects {
    draw_commands.push(DrawIndexedIndirectCommand {
        index_count: object.mesh.index_count,
        instance_count: 1,
        first_index: object.mesh.first_index,
        vertex_offset: object.mesh.vertex_offset,
        first_instance: 0,
    });
}

// Upload vers GPU
let indirect_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::INDIRECT,
    data: &draw_commands,
});

// 2. UN SEUL appel pour dessiner 10,000 objets!
vkCmdDrawIndexedIndirect(
    command_buffer,
    indirect_buffer,
    0,                   // offset
    10000,               // draw count
    size_of::<DrawIndexedIndirectCommand>(), // stride
);
```

**Avantages** :
- ✅ 1 appel CPU au lieu de 10,000
- ✅ CPU overhead divisé par 1000
- ✅ GPU exécute les commandes en parallèle

**Limitation** : Les commandes sont statiques (créées sur CPU)

---

#### 4.2 MultiDrawIndirect + GPU Culling

**Encore mieux** : Compute shader génère les commandes

```glsl
// Compute shader de culling
layout(binding = 0) buffer ObjectsBuffer {
    Object objects[10000]; // Tous les objets de la scène
};

layout(binding = 1) buffer DrawCommandsBuffer {
    DrawIndexedIndirectCommand commands[10000]; // Output
};

layout(binding = 2) buffer DrawCountBuffer {
    uint draw_count; // Nombre de commandes générées
};

uniform mat4 view_projection;

void main() {
    uint idx = gl_GlobalInvocationID.x; // 1 thread = 1 objet
    Object obj = objects[idx];

    // Frustum culling
    bool in_frustum = test_frustum(obj.bounding_box, view_projection);

    // Occlusion culling (Hi-Z)
    bool visible = test_occlusion(obj.bounding_box);

    if (in_frustum && visible) {
        // Objet visible : écrire commande de draw
        uint command_idx = atomicAdd(draw_count, 1); // Thread-safe counter

        commands[command_idx] = DrawIndexedIndirectCommand(
            obj.mesh.index_count,
            1, // instance_count
            obj.mesh.first_index,
            obj.mesh.vertex_offset,
            0  // first_instance
        );
    }
    // Sinon : skip (pas de draw command générée)
}
```

**Vulkan API** :

```rust
// 1. Dispatch compute shader (culling)
vkCmdDispatch(command_buffer, 10000 / 256, 1, 1); // 10k threads

// 2. Barrier (attendre que compute finisse)
vkCmdPipelineBarrier(...);

// 3. Draw indirect avec count GPU!
vkCmdDrawIndexedIndirectCount(
    command_buffer,
    indirect_buffer,        // Buffer des commandes
    0,                      // offset
    count_buffer,           // Buffer avec draw_count (écrit par compute)
    0,                      // count offset
    10000,                  // max draws
    size_of::<DrawIndexedIndirectCommand>(),
);
```

**Résultat** :
- Input : 10,000 objets
- Après culling : 2,000 visibles
- GPU dessine seulement 2,000 objets
- CPU overhead : **ZÉRO** (tout sur GPU)

---

### 5. Culling

#### 5.1 Frustum Culling

**Principe** : Ne dessiner que ce qui est dans le champ de vision de la caméra

```
Frustum de la caméra (pyramide tronquée) :
     ┌────────┐ Far plane
    /│        │\
   / │        │ \
  /  │        │  \
 /   │        │   \
┌────┴────────┴────┐ Near plane
│     Camera        │
└───────────────────┘
```

**Test d'intersection** :

```rust
// Frustum = 6 plans (haut, bas, gauche, droite, proche, loin)
struct Frustum {
    planes: [Plane; 6],
}

struct Plane {
    normal: Vec3,
    distance: f32,
}

// Test si bounding box intersecte frustum
fn test_frustum(bbox: &BoundingBox, frustum: &Frustum) -> bool {
    for plane in &frustum.planes {
        // Si tous les coins sont derrière ce plan → objet dehors
        let mut all_outside = true;
        for corner in bbox.corners() {
            if plane.distance_to(corner) > 0.0 {
                all_outside = false;
                break;
            }
        }
        if all_outside {
            return false; // Objet complètement dehors
        }
    }
    true // Au moins partiellement visible
}
```

**Performance** :
- CPU : 10,000 objets = 2-3 ms
- GPU (compute) : 10,000 objets = 0.1 ms (20× plus rapide)

---

#### 5.2 Occlusion Culling

**Principe** : Ne pas dessiner les objets cachés derrière d'autres

```
Scène vue de dessus :
┌────────────────────────────┐
│  Camera                    │
│    ↓                       │
│  ┌─────┐  ┌─────┐          │
│  │ A   │  │  B  │ ← B caché│
│  └─────┘  └─────┘   par A  │
│                            │
└────────────────────────────┘
```

**Approche Hi-Z (moderne)** :

```
1. Dessiner la scène (ou juste les gros objets)
2. Générer Hi-Z pyramid (depth buffer mipmap):
   - Mip 0 : 1920×1080 (full res)
   - Mip 1 : 960×540 (max de 2×2 pixels)
   - Mip 2 : 480×270
   - ...
   - Mip 10 : 1×1 (profondeur max de la scène)

3. Dans compute shader de culling :
   for object in objects {
       // Projeter bounding box sur écran
       let screen_bbox = project(object.bbox, view_proj);

       // Choisir mip level selon taille écran
       let mip = log2(screen_bbox.width);

       // Lire profondeur max dans Hi-Z
       let depth_max = hi_z_texture.sample_lod(screen_bbox.center, mip);

       // Si objet plus loin que ce qui est déjà dessiné → caché
       if object.bbox.min_depth > depth_max {
           skip; // Objet occlus
       } else {
           draw; // Objet visible
       }
   }
```

**Avantages Hi-Z** :
- ✅ Pas de latence (contrairement aux occlusion queries)
- ✅ Ultra rapide (1 texture fetch par objet)
- ✅ Scale à 100k+ objets

**Exemples** :
- Assassin's Creed Valhalla : Hi-Z pour villes denses
- Horizon Forbidden West : Hi-Z + frustum culling
- Unreal Engine 5 Nanite : Hi-Z avancé (per-cluster)

---

#### 5.3 Backface Culling

**Principe** : GPU retire automatiquement les triangles "dos à la caméra"

```rust
// Configuration pipeline
let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
    .cull_mode(vk::CullModeFlags::BACK) // Cull back faces
    .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
```

**Résultat** : ~50% des triangles éliminés gratuitement

---

### 6. Pipeline GPU-Driven Complet

**Architecture moderne (Unreal 5, Unity HDRP)** :

```
Frame N :

1. [Compute Shader] Culling
   Input  : 100,000 objets (buffer GPU)
   Output : 5,000 objets visibles (indirect buffer)
   Temps  : 0.2 ms

   ┌─────────────────────────────┐
   │ Frustum Culling             │ 100k → 30k
   ├─────────────────────────────┤
   │ Occlusion Culling (Hi-Z)    │ 30k → 10k
   ├─────────────────────────────┤
   │ Distance Culling            │ 10k → 8k
   ├─────────────────────────────┤
   │ LOD Selection               │ (choisir LOD par objet)
   ├─────────────────────────────┤
   │ Write Indirect Commands     │ 8k commandes
   └─────────────────────────────┘

2. [Indirect Draw] Rendu
   vkCmdDrawIndexedIndirectCount(indirect_buffer, count = 8k)
   Temps : 10 ms (8000 objets visibles)

3. [Compute Shader] Hi-Z Generation
   Génère depth pyramid pour frame N+1
   Temps : 0.3 ms

Frame N+1 :
   Utilise Hi-Z de frame N pour culling
```

**Code complet** :

```rust
// Setup (une fois)
let objects_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::STORAGE,
    data: &objects, // 100k objets
});

let indirect_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::INDIRECT | BufferUsage::STORAGE,
    size: 100_000 * size_of::<DrawIndexedIndirectCommand>(),
});

let count_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::INDIRECT | BufferUsage::STORAGE,
    size: 4, // uint32 draw count
});

// Chaque frame
fn render_frame(&mut self) {
    let cmd = &mut self.command_list;

    cmd.begin()?;

    // 1. Compute shader de culling
    cmd.bind_pipeline(&self.culling_pipeline);
    cmd.bind_descriptor_sets(&self.culling_pipeline, &[
        &self.objects_descriptor,
        &self.indirect_descriptor,
        &self.count_descriptor,
        &self.hiz_descriptor, // Hi-Z de la frame précédente
    ]);
    cmd.push_constants(0, &self.camera.view_proj);
    cmd.dispatch(100_000 / 256, 1, 1); // 100k threads

    // 2. Barrier (compute → indirect draw)
    cmd.pipeline_barrier(
        PipelineStage::COMPUTE_SHADER,
        PipelineStage::DRAW_INDIRECT,
    );

    // 3. Render pass
    cmd.begin_render_pass(&self.render_pass, &self.render_target, &[...])?;

    // 4. Bind global buffers (une seule fois)
    cmd.bind_vertex_buffer(&self.global_vertex_buffer, 0);
    cmd.bind_index_buffer(&self.global_index_buffer, 0);
    cmd.bind_pipeline(&self.render_pipeline);

    // 5. Indirect draw (8000 objets visibles)
    cmd.draw_indexed_indirect_count(
        &self.indirect_buffer,
        0,
        &self.count_buffer,
        0,
        100_000, // max draws
    )?;

    cmd.end_render_pass()?;

    // 6. Générer Hi-Z pour frame suivante
    cmd.bind_pipeline(&self.hiz_pipeline);
    cmd.generate_hiz_pyramid(&self.depth_texture);

    cmd.end()?;

    // 7. Submit
    self.renderer.submit(&[cmd])?;
}
```

**Performances** :

| Métrique | Traditionnel CPU | GPU-Driven |
|----------|------------------|------------|
| Objets totaux | 10,000 | 100,000 |
| CPU overhead | 15 ms | 0.1 ms |
| Culling | 3 ms (CPU) | 0.2 ms (GPU) |
| Objets dessinés | 10,000 | 5,000 (culled) |
| FPS | 30 FPS | 120 FPS |

---

### 7. Roadmap Galaxy3DEngine

#### Phase 13 : Mesh Batching Global (Planifié)

**Objectif** : Global vertex/index buffers

**Changements API** :

```rust
// Nouveau : MeshRegistry
pub struct MeshRegistry {
    global_vertex_buffer: Arc<dyn galaxy_3d_engine::galaxy3d::render::Buffer>,
    global_index_buffer: Arc<dyn galaxy_3d_engine::galaxy3d::render::Buffer>,
    meshes: Vec<MeshInfo>,
}

pub struct MeshInfo {
    pub mesh_id: u32,
    pub vertex_offset: i32,
    pub first_index: u32,
    pub index_count: u32,
    pub lods: Vec<LodInfo>, // Phase 15
}

impl MeshRegistry {
    pub fn load_mesh(&mut self, path: &str) -> galaxy_3d_engine::galaxy3d::Result<MeshId> {
        // Charge mesh, append to global buffers
    }

    pub fn get_mesh(&self, id: MeshId) -> &MeshInfo {
        &self.meshes[id.0 as usize]
    }
}

// Usage
let mesh_id = mesh_registry.load_mesh("cube.obj")?;
let mesh = mesh_registry.get_mesh(mesh_id);

// Bind global buffers (une seule fois)
command_list.bind_vertex_buffer(&mesh_registry.global_vertex_buffer, 0);
command_list.bind_index_buffer(&mesh_registry.global_index_buffer, 0);

// Draw
command_list.draw_indexed(
    mesh.index_count,
    mesh.first_index,
    mesh.vertex_offset,
);
```

**Estimation** : 3-4 jours

---

#### Phase 14 : Indirect Drawing + GPU Culling (Planifié)

**Objectif** : vkCmdDrawIndexedIndirectCount + compute culling

**Changements API** :

```rust
// Nouveau trait galaxy_3d_engine::galaxy3d::render::CommandList
pub trait galaxy_3d_engine::galaxy3d::render::CommandList {
    // Existants
    fn draw_indexed(&mut self, ...);

    // ✨ NOUVEAUX
    fn draw_indexed_indirect(
        &mut self,
        buffer: &Arc<dyn galaxy_3d_engine::galaxy3d::render::Buffer>, // Indirect buffer
        offset: u64,
        draw_count: u32,
        stride: u32,
    ) -> galaxy_3d_engine::galaxy3d::Result<()>;

    fn draw_indexed_indirect_count(
        &mut self,
        buffer: &Arc<dyn galaxy_3d_engine::galaxy3d::render::Buffer>,
        offset: u64,
        count_buffer: &Arc<dyn galaxy_3d_engine::galaxy3d::render::Buffer>, // Draw count
        count_offset: u64,
        max_draw_count: u32,
        stride: u32,
    ) -> galaxy_3d_engine::galaxy3d::Result<()>;

    fn dispatch(
        &mut self,
        group_count_x: u32,
        group_count_y: u32,
        group_count_z: u32,
    ) -> galaxy_3d_engine::galaxy3d::Result<()>;
}

// Nouveau : Compute pipelines
impl Renderer {
    fn create_compute_pipeline(
        &self,
        desc: ComputePipelineDesc,
    ) -> galaxy_3d_engine::galaxy3d::Result<Arc<dyn RendererComputePipeline>>;
}
```

**Implémentation Vulkan** :

```rust
// Vulkangalaxy_3d_engine::galaxy3d::render::CommandList
fn draw_indexed_indirect_count(&mut self, ...) -> galaxy_3d_engine::galaxy3d::Result<()> {
    unsafe {
        let vk_buffer = downcast_buffer(buffer);
        let vk_count_buffer = downcast_buffer(count_buffer);

        self.device.cmd_draw_indexed_indirect_count(
            self.command_buffer,
            vk_buffer.buffer,
            offset,
            vk_count_buffer.buffer,
            count_offset,
            max_draw_count,
            stride,
        );
    }
    Ok(())
}
```

**Estimation** : 7-10 jours

---

#### Phase 15 : LODs + GPU Skinning (Planifié)

**Objectif** : LODs automatiques + skeletal animation

**LODs** :

```rust
pub struct MeshInfo {
    pub lods: Vec<LodInfo>,
}

pub struct LodInfo {
    pub distance: f32,      // Distance de transition
    pub index_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
}

// Sélection LOD
fn select_lod(distance: f32, mesh: &MeshInfo) -> &LodInfo {
    mesh.lods.iter()
        .find(|lod| distance < lod.distance)
        .unwrap_or(mesh.lods.last().unwrap())
}
```

**GPU Skinning** :

```rust
// Vertex avec bones
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub bone_indices: [u8; 4],
    pub bone_weights: [f32; 4],
}

// Uniform buffer des bones
let bones_buffer = renderer.create_buffer(BufferDesc {
    usage: BufferUsage::UNIFORM,
    data: &skeleton.bone_matrices, // 256 mat4
});

// Bind dans descriptor set
command_list.bind_descriptor_sets(&pipeline, &[
    &bones_descriptor,
]);
```

**Estimation** : 5-7 jours

---

### 8. Recommandations

#### Pour Prototypage (Phase 9-12)

```rust
// Simple mesh individuel
let vertex_buffer = renderer.create_buffer(...);
let index_buffer = renderer.create_buffer(...);

command_list.bind_vertex_buffer(&vertex_buffer, 0);
command_list.bind_index_buffer(&index_buffer, 0);
command_list.draw_indexed(count, 0, 0);
```

**Quand utiliser** :
- ✅ < 1000 objets
- ✅ Prototypage rapide
- ✅ Pas de contrainte FPS

---

#### Pour Production (Phase 13+)

```rust
// Global buffers + indirect drawing
mesh_registry.load_mesh("tree.obj")?;
mesh_registry.load_mesh("rock.obj")?;
// ... 10,000 meshes

// Bind une seule fois
command_list.bind_vertex_buffer(&mesh_registry.global_vertex_buffer, 0);
command_list.bind_index_buffer(&mesh_registry.global_index_buffer, 0);

// Indirect draw (GPU culling)
command_list.draw_indexed_indirect_count(
    &indirect_buffer,
    0,
    &count_buffer,
    0,
    10_000,
);
```

**Quand utiliser** :
- ✅ > 10,000 objets
- ✅ Open world / grandes scènes
- ✅ Optimisation CPU critique

---

#### Tableau Récapitulatif

| Phase | Approche | Objets | CPU Overhead | GPU Culling | FPS (10k objets) |
|-------|----------|--------|--------------|-------------|------------------|
| **9 (actuel)** | Individual buffers | 1,000 | 15 ms | Non | 30 FPS |
| **13** | Global buffers | 10,000 | 3 ms | Non | 60 FPS |
| **14** | Indirect + Culling | 100,000 | 0.1 ms | Oui | 120 FPS |
| **15** | + LODs + Skinning | 100,000+ | 0.1 ms | Oui | 144 FPS |

---

### 9. Références Techniques

#### Concepts

- **Indirect Drawing** : https://www.khronos.org/opengl/wiki/Vertex_Rendering#Indirect_rendering
- **GPU Culling** : "GPU-Driven Rendering Pipelines" (Advances in Real-Time Rendering, SIGGRAPH)
- **Hi-Z Occlusion Culling** : https://interplayoflight.wordpress.com/2017/11/15/experiments-in-gpu-based-occlusion-culling/

#### Vulkan

- `vkCmdDrawIndexedIndirect` : https://registry.khronos.org/vulkan/specs/1.3/man/html/vkCmdDrawIndexedIndirect.html
- `vkCmdDrawIndexedIndirectCount` : https://registry.khronos.org/vulkan/specs/1.3/man/html/vkCmdDrawIndexedIndirectCount.html
- `vkCmdDispatch` : https://registry.khronos.org/vulkan/specs/1.3/man/html/vkCmdDispatch.html

#### Implémentations AAA

- **Unreal Engine 5 Nanite** : GPU-driven culling, indirect drawing, virtual geometry
- **Unity DOTS** : ECS + GPU culling + indirect rendering
- **Assassin's Creed Valhalla** : 500k+ objects with GPU culling
- **Fortnite** : Indirect drawing for foliage (millions of instances)

#### GDC Talks

- "GPU-Driven Rendering Pipelines" (2015, Ubisoft)
- "Destiny's Multithreaded Rendering Architecture" (2015, Bungie)
- "The Rendering of Horizon Zero Dawn" (2017, Guerrilla Games)

---

## 🔍 Internal Logging System

### ⚠️ Règle importante

**Tous les messages de log doivent être en anglais**, peu importe la langue du code ou des commentaires.

### Objectif

Créer un système de logging interne au moteur 3D, invisible pour l'utilisateur final mais permettant de remplacer le logger par défaut.

### Architecture

#### 1. **Trait `Logger`** - Interface de logging

```rust
pub trait Logger: Send + Sync {
    fn log(&self, entry: &LogEntry);
}

pub struct LogEntry {
    pub severity: LogSeverity,
    pub timestamp: SystemTime,
    pub source: String,        // "Vulkan", "Engine", "Renderer"
    pub message: String,
    pub file: Option<&'static str>,  // Pour log détaillé
    pub line: Option<u32>,           // Pour log détaillé
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogSeverity {
    Trace,   // Très verbeux (désactivé par défaut)
    Debug,   // Debug info
    Info,    // Informations importantes
    Warn,    // Avertissements
    Error,   // Erreurs critiques
}
```

#### 2. **API dans `galaxy3d::Engine`**

**API Publique** (utilisateur) :
```rust
impl Engine {
    /// Remplacer le logger par défaut
    pub fn set_logger<L: Logger + 'static>(logger: L);

    /// Revenir au logger par défaut (console avec couleurs)
    pub fn reset_logger();
}
```

**API Interne** (moteur uniquement - `pub(crate)`) :
```rust
impl Engine {
    /// Log simple (sans file/line)
    pub(crate) fn log(severity: LogSeverity, source: &str, message: String);

    /// Log détaillé (avec file/line)
    pub(crate) fn log_detailed(
        severity: LogSeverity,
        source: &str,
        message: String,
        file: &'static str,
        line: u32
    );
}
```

#### 3. **Macros** - Raccourcis pour le moteur

```rust
// Log simple
engine_trace!("source", "message");
engine_debug!("source", "message");
engine_info!("source", "message");
engine_warn!("source", "message");
engine_error!("source", "message");

// Log détaillé (avec file!() et line!())
engine_trace_detailed!("source", "message");
engine_debug_detailed!("source", "message");
engine_info_detailed!("source", "message");
engine_warn_detailed!("source", "message");
engine_error_detailed!("source", "message");
```

#### 4. **DefaultLogger** - Logger par défaut

Le logger par défaut utilise `println!()` avec la crate `colored` pour afficher dans la console :

- **Trace** → Gris/Cyan pâle
- **Debug** → Cyan
- **Info** → Vert
- **Warn** → Jaune
- **Error** → Rouge (gras)

**Formats** :
- Simple : `[timestamp] [SEVERITY] [Source] Message`
- Détaillé : `[timestamp] [SEVERITY] [Source] Message (file.rs:line)`

---

### Intégration Vulkan - Redirection Debug Messenger

Le debug messenger Vulkan capture les messages de validation et les **redirige** vers notre système de logging :

```rust
// Dans galaxy_3d_engine_renderer_vulkan/src/debug.rs

unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let data = &*callback_data;
    let message = CStr::from_ptr(data.p_message).to_string_lossy();
    let message_id = CStr::from_ptr(data.p_message_id_name).to_string_lossy();

    // Conversion Vulkan → LogSeverity
    let log_severity = match severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => LogSeverity::Error,
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => LogSeverity::Warn,
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => LogSeverity::Info,
        _ => LogSeverity::Trace,  // VERBOSE → TRACE
    };

    // Redirection vers notre système de log
    Engine::log_detailed(
        log_severity,
        "Vulkan",
        format!("[{}] {}", message_id, message),
        file!(),
        line!()
    );

    vk::FALSE
}
```

**Résultat** : Tous les messages Vulkan passent par le `Logger` actuel → possibilité de les rediriger vers n'importe quel backend.

---

### Exemples d'Utilisation

#### 1. Utilisation par défaut (console)

```rust
fn main() {
    galaxy3d::Engine::initialize().unwrap();

    // Le logger par défaut est déjà actif
    // Tous les logs du moteur s'affichent dans la console avec couleurs

    // ... code de l'application ...
}
```

#### 2. Logger personnalisé - Écriture dans fichier

```rust
use galaxy_3d_engine::galaxy3d;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

struct FileLogger {
    file: Mutex<std::fs::File>,
}

impl FileLogger {
    fn new(path: &str) -> Self {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("Failed to open log file");

        Self { file: Mutex::new(file) }
    }
}

impl galaxy3d::log::Logger for FileLogger {
    fn log(&self, entry: &galaxy3d::log::LogEntry) {
        let mut file = self.file.lock().unwrap();

        let log_line = if let (Some(file), Some(line)) = (entry.file, entry.line) {
            format!(
                "[{:?}] [{:?}] [{}] {} ({}:{})\n",
                entry.timestamp, entry.severity, entry.source,
                entry.message, file, line
            )
        } else {
            format!(
                "[{:?}] [{:?}] [{}] {}\n",
                entry.timestamp, entry.severity, entry.source, entry.message
            )
        };

        file.write_all(log_line.as_bytes()).ok();
    }
}

fn main() {
    galaxy3d::Engine::initialize().unwrap();

    // Remplacer le logger par défaut
    let file_logger = FileLogger::new("galaxy3d_engine.log");
    galaxy3d::Engine::set_logger(file_logger);

    // Maintenant tous les logs vont dans le fichier

    // ... code de l'application ...
}
```

#### 3. Logger réseau (JSON sur UDP)

```rust
use std::net::UdpSocket;

struct NetworkLogger {
    socket: UdpSocket,
    server_addr: String,
}

impl galaxy3d::log::Logger for NetworkLogger {
    fn log(&self, entry: &galaxy3d::log::LogEntry) {
        let json = format!(
            r#"{{"severity":"{}","source":"{}","message":"{}"}}"#,
            format!("{:?}", entry.severity),
            entry.source,
            entry.message
        );

        self.socket.send_to(json.as_bytes(), &self.server_addr).ok();
    }
}
```

---

### Bénéfices

✅ **Transparence** : L'utilisateur n'a pas besoin de s'occuper du logging sauf s'il veut personnaliser
✅ **Flexibilité** : Possibilité de rediriger vers fichier, réseau, base de données, etc.
✅ **Uniformité** : Tous les logs (Engine, Vulkan, futurs backends) utilisent le même système
✅ **Thread-safe** : `RwLock` permet le logging concurrent depuis plusieurs threads
✅ **Redirection Vulkan** : Les messages de validation Vulkan sont intégrés au système

---

### Exemples Réels d'Utilisation dans le Moteur

#### 1. Logs dans `Engine::create_renderer()` et `Engine::destroy_renderer()`

```rust
// galaxy_3d_engine/src/engine.rs

pub fn create_renderer<R: Renderer + 'static>(renderer: R) -> Result<()> {
    let arc_renderer: Arc<Mutex<dyn Renderer>> = Arc::new(Mutex::new(renderer));
    Self::register_renderer(arc_renderer)?;

    // Log successful creation
    crate::engine_info!("galaxy3d::Engine", "Renderer singleton created successfully");

    Ok(())
}

pub fn destroy_renderer() -> Result<()> {
    let state = ENGINE_STATE.get()
        .ok_or_else(|| Error::InitializationFailed("Engine not initialized".to_string()))?;

    let mut lock = state.renderer.write()
        .map_err(|_| Error::BackendError("Renderer lock poisoned".to_string()))?;

    *lock = None;

    // Log successful destruction
    crate::engine_info!("galaxy3d::Engine", "Renderer singleton destroyed");

    Ok(())
}
```

**Sortie console** :
```
[2026-01-31 17:17:42.073] [INFO ] [galaxy3d::Engine] Renderer singleton created successfully
[2026-01-31 17:18:25.341] [INFO ] [galaxy3d::Engine] Renderer singleton destroyed
```

#### 2. Logs dans le Vulkan Debug Messenger

```rust
// galaxy_3d_engine_renderer_vulkan/src/debug.rs

unsafe extern "system" fn vulkan_debug_callback(...) -> vk::Bool32 {
    // Map Vulkan severity to Engine log severity
    let log_severity = if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        LogSeverity::Error
    } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        LogSeverity::Warn
    } else if message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        LogSeverity::Info
    } else {
        LogSeverity::Trace
    };

    // Format message
    let log_message = format!(
        "[{}]{} {}: {}",
        type_str, repeat_indicator, message_id_name, message
    );

    // Log using Engine logging system
    // Only ERROR severity includes file:line information
    if log_severity == LogSeverity::Error {
        Engine::log_detailed(
            log_severity,
            "galaxy3d::vulkan::DebugMessenger",
            log_message.clone(),
            file!(),
            line!()
        );
    } else {
        Engine::log(
            log_severity,
            "galaxy3d::vulkan::DebugMessenger",
            log_message.clone()
        );
    }

    vk::FALSE
}
```

#### 3. Logs dans le rapport de statistiques Vulkan

```rust
// galaxy_3d_engine_renderer_vulkan/src/debug.rs

pub fn print_validation_stats_report() {
    let stats = get_validation_stats();

    if stats.total() == 0 {
        engine_info!("galaxy3d::vulkan::ValidationStats", "No validation messages");
        return;
    }

    engine_info!("galaxy3d::vulkan::ValidationStats", "=== Validation Statistics Report ===");

    if stats.errors > 0 {
        engine_error!("galaxy3d::vulkan::ValidationStats", "Errors: {}", stats.errors);
    }
    if stats.warnings > 0 {
        engine_warn!("galaxy3d::vulkan::ValidationStats", "Warnings: {}", stats.warnings);
    }
    if stats.info > 0 {
        engine_info!("galaxy3d::vulkan::ValidationStats", "Info: {}", stats.info);
    }
    if stats.verbose > 0 {
        engine_trace!("galaxy3d::vulkan::ValidationStats", "Verbose: {}", stats.verbose);
    }

    engine_info!("galaxy3d::vulkan::ValidationStats", "Total: {}", stats.total());
    engine_info!("galaxy3d::vulkan::ValidationStats", "{} message(s) appeared multiple times", duplicate_count);
    engine_info!("galaxy3d::vulkan::ValidationStats", "====================================");
}
```

**Sortie console** :
```
[2026-01-31 17:18:30.120] [INFO ] [galaxy3d::vulkan::ValidationStats] === Validation Statistics Report ===
[2026-01-31 17:18:30.121] [ERROR] [galaxy3d::vulkan::ValidationStats] Errors: 2 (debug.rs:132)
[2026-01-31 17:18:30.121] [WARN ] [galaxy3d::vulkan::ValidationStats] Warnings: 5
[2026-01-31 17:18:30.122] [INFO ] [galaxy3d::vulkan::ValidationStats] Info: 128
[2026-01-31 17:18:30.122] [TRACE] [galaxy3d::vulkan::ValidationStats] Verbose: 42
[2026-01-31 17:18:30.123] [INFO ] [galaxy3d::vulkan::ValidationStats] Total: 177
[2026-01-31 17:18:30.123] [INFO ] [galaxy3d::vulkan::ValidationStats] 12 message(s) appeared multiple times
[2026-01-31 17:18:30.124] [INFO ] [galaxy3d::vulkan::ValidationStats] ====================================
```

#### 4. Log d'erreur critique avec break-on-error

```rust
// galaxy_3d_engine_renderer_vulkan/src/debug.rs

// Break on error if configured (for debugger attachment)
if config.break_on_error
    && message_severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR)
{
    engine_error!(
        "galaxy3d::vulkan::DebugMessenger",
        "BREAK ON VALIDATION ERROR - Aborting execution | Context: {} [{}] | Message: {}",
        message_id_name,
        type_str,
        message
    );
    std::process::abort();
}
```

**Sortie console** :
```
[2026-01-31 17:18:35.234] [ERROR] [galaxy3d::vulkan::DebugMessenger] BREAK ON VALIDATION ERROR - Aborting execution | Context: VUID-vkCmdDraw-None-02699 [Validation] | Message: Invalid pipeline state (debug.rs:350)
```

#### Notes Importantes

- ⚠️ **Seul `engine_error!` inclut file:line automatiquement** (via `Engine::log_detailed()`)
- ✅ Les autres macros (`engine_info!`, `engine_warn!`, `engine_trace!`, `engine_debug!`) utilisent `Engine::log()` sans file:line
- ✅ Le source doit toujours suivre le format `"galaxy3d::module::SubModule"` pour une hiérarchie claire
- ✅ Tous les messages doivent être en **anglais**

---

## 🪵 Phase 9 - Logging System (Completed ✅)

### Overview

Le système de logging de Galaxy3D Engine permet aux utilisateurs d'intercepter et de router les logs internes du moteur via un trait `Logger` personnalisable.

**Composants** :
- **Logger Trait** : Interface publique pour implémenter des loggers personnalisés
- **DefaultLogger** : Implémentation par défaut (console avec couleurs + horodatage)
- **Macros engine_*** : Macros internes au moteur (masquées de l'API publique)
- **TracingLogger** : Exemple d'implémentation utilisant `tracing` (dans la démo)

---

### 1. Logger Trait (API Publique)

**Fichier** : `galaxy_3d_engine/src/log.rs`

```rust
/// Logging severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSeverity {
    Trace,   // Verbose debugging information
    Debug,   // Detailed debug information
    Info,    // Informational messages
    Warn,    // Warning messages
    Error,   // Error messages
}

/// Log entry with metadata
pub struct LogEntry<'a> {
    pub severity: LogSeverity,
    pub source: &'a str,    // e.g., "galaxy3d::vulkan::Renderer"
    pub message: &'a str,
    pub file: Option<&'a str>,   // File path (only for errors)
    pub line: Option<u32>,       // Line number (only for errors)
}

/// Logger trait - Implement this to create custom loggers
pub trait Logger: Send + Sync {
    fn log(&self, entry: &LogEntry);
}
```

**Installation d'un logger personnalisé** :
```rust
// Remplacer le DefaultLogger par un logger personnalisé
let my_logger = MyCustomLogger::new()?;
galaxy3d::Engine::set_logger(my_logger);
```

---

### 2. DefaultLogger (Implémentation par Défaut)

**Comportement** :
- Sortie console avec **couleurs** (via crate `colored`)
- **Horodatage** avec précision millisecondes (via crate `chrono`)
- Format : `[timestamp] [SEVERITY] [source] message (file:line)`

**Exemple de sortie** :
```
[2026-01-31 17:18:30.120] [INFO ] [galaxy3d::vulkan::Renderer] Vulkan renderer initialized
[2026-01-31 17:18:30.234] [ERROR] [galaxy3d::vulkan::Swapchain] Failed to acquire image (vulkan_swapchain.rs:142)
[2026-01-31 17:18:30.456] [WARN ] [galaxy3d::vulkan::ValidationLayer] Performance warning detected
```

**Couleurs** :
- 🟢 `TRACE` : Bright Black (gris)
- 🔵 `DEBUG` : Blue
- ⚪ `INFO` : White
- 🟡 `WARN` : Yellow
- 🔴 `ERROR` : Bright Red

---

### 3. Macros engine_* (Internes au Moteur)

**Fichier** : `galaxy_3d_engine/src/log.rs`

**Macros disponibles** (usage interne uniquement) :
```rust
engine_trace!("galaxy3d::module", "Verbose debug: {}", value);
engine_debug!("galaxy3d::module", "Debug info: {}", value);
engine_info!("galaxy3d::module", "Informational: {}", value);
engine_warn!("galaxy3d::module", "Warning: {}", value);
engine_error!("galaxy3d::module", "Error: {}", value);  // Inclut file:line automatiquement
```

**Caractéristiques** :
- ✅ Marquées `#[doc(hidden)]` → **Cachées de la documentation publique**
- ✅ Toujours `#[macro_export]` → Accessibles dans les crates internes (e.g., `galaxy_3d_engine_renderer_vulkan`)
- ✅ NON ré-exportées dans `galaxy3d::log` → Invisibles pour les utilisateurs
- ⚠️ **Seul `engine_error!`** appelle `Engine::log_detailed()` avec file:line

**Implémentation** :
```rust
// engine_info! - Pas de file:line
#[doc(hidden)]
#[macro_export]
macro_rules! engine_info {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log(
            $crate::galaxy3d::log::LogSeverity::Info,
            $source,
            format!($($arg)*)
        )
    };
}

// engine_error! - Avec file:line automatique
#[doc(hidden)]
#[macro_export]
macro_rules! engine_error {
    ($source:expr, $($arg:tt)*) => {
        $crate::galaxy3d::Engine::log_detailed(
            $crate::galaxy3d::log::LogSeverity::Error,
            $source,
            format!($($arg)*),
            file!(),
            line!()
        )
    };
}
```

**Exports dans `lib.rs`** :
```rust
// galaxy_3d_engine/src/lib.rs
pub mod galaxy3d {
    pub mod log {
        // ✅ Exporte les types publics
        pub use crate::log::{Logger, LogEntry, LogSeverity, DefaultLogger};

        // ❌ NE PAS exporter les macros (internes uniquement)
        // Les macros restent accessibles via #[macro_export] pour les crates internes
    }
}
```

---

### 4. TracingLogger (Exemple dans la Démo)

**Fichier** : `Games/galaxy3d_demo/src/tracing_logger.rs`

Exemple d'implémentation du trait `Logger` utilisant l'écosystème `tracing` pour router les logs vers :
- **Console** : Logs colorés via `tracing-subscriber`
- **Fichier** : Logs horodatés sans couleur (avec `chrono`)

**Dépendances (Cargo.toml)** :
```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "ansi"] }
chrono = "0.4"
```

**Implémentation** :
```rust
use galaxy_3d_engine::galaxy3d::log::{Logger, LogEntry, LogSeverity};
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use tracing::Level;

pub struct TracingLogger {
    file: Mutex<File>,
}

impl TracingLogger {
    pub fn new(log_path: &str) -> std::io::Result<Self> {
        // Crée/tronque le fichier log
        let file = File::create(log_path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl Logger for TracingLogger {
    fn log(&self, entry: &LogEntry) {
        // 1. Convertir LogSeverity → tracing::Level
        let level = match entry.severity {
            LogSeverity::Trace => Level::TRACE,
            LogSeverity::Debug => Level::DEBUG,
            LogSeverity::Info => Level::INFO,
            LogSeverity::Warn => Level::WARN,
            LogSeverity::Error => Level::ERROR,
        };

        // 2. Formater le message avec source module (et file:line si disponible)
        let full_message = if let (Some(file), Some(line)) = (entry.file, entry.line) {
            format!("[{}] {} ({}:{})", entry.source, entry.message, file, line)
        } else {
            format!("[{}] {}", entry.source, entry.message)
        };

        // 3. Logger via tracing (console avec couleurs)
        match level {
            Level::TRACE => tracing::trace!("{}", full_message),
            Level::DEBUG => tracing::debug!("{}", full_message),
            Level::INFO => tracing::info!("{}", full_message),
            Level::WARN => tracing::warn!("{}", full_message),
            Level::ERROR => tracing::error!("{}", full_message),
        }

        // 4. Écrire dans le fichier (sans couleurs, avec timestamp)
        if let Ok(mut file) = self.file.lock() {
            let severity_str = match entry.severity {
                LogSeverity::Trace => "TRACE",
                LogSeverity::Debug => "DEBUG",
                LogSeverity::Info => "INFO ",
                LogSeverity::Warn => "WARN ",
                LogSeverity::Error => "ERROR",
            };

            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");

            let log_line = if let (Some(file_path), Some(line)) = (entry.file, entry.line) {
                format!("[{}] [{}] [{}] {} ({}:{})\n",
                    timestamp, severity_str, entry.source, entry.message, file_path, line)
            } else {
                format!("[{}] [{}] [{}] {}\n",
                    timestamp, severity_str, entry.source, entry.message)
            };

            let _ = file.write_all(log_line.as_bytes());
        }
    }
}
```

**Utilisation dans main.rs** :
```rust
fn main() {
    // 1. Initialiser tracing-subscriber (console)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    // 2. Initialiser le moteur 3D
    galaxy3d::Engine::initialize()?;

    // 3. Installer TracingLogger pour remplacer DefaultLogger
    if let Ok(tracing_logger) = TracingLogger::new("galaxy3d_demo.log") {
        galaxy3d::Engine::set_logger(tracing_logger);
    }

    // 4. Tous les logs du moteur seront routés vers tracing + fichier
    // ...
}
```

**Sortie console (via tracing-subscriber)** :
```
2026-01-31T17:18:30.120Z  INFO tracing_logger: [galaxy3d::vulkan::Renderer] Vulkan renderer initialized
2026-01-31T17:18:30.234Z ERROR tracing_logger: [galaxy3d::vulkan::Swapchain] Failed to acquire image (vulkan_swapchain.rs:142)
```

**Sortie fichier (`galaxy3d_demo.log`)** :
```
[2026-01-31 17:18:30.120] [INFO ] [galaxy3d::vulkan::Renderer] Vulkan renderer initialized
[2026-01-31 17:18:30.234] [ERROR] [galaxy3d::vulkan::Swapchain] Failed to acquire image (vulkan_swapchain.rs:142)
```

---

### Notes Importantes

**Séparation des responsabilités** :
- 🔒 **Macros `engine_*`** : Usage **interne** au moteur (renderer Vulkan, systèmes internes)
  - Cachées via `#[doc(hidden)]`
  - Non ré-exportées dans l'API publique
  - Accessibles aux crates internes via `#[macro_export]`

- 🌐 **Trait `Logger`** : Interface **publique** pour utilisateurs
  - Permet de capturer les logs du moteur
  - Implémentations personnalisées (tracing, slog, log4rs, etc.)
  - Exemple `TracingLogger` fourni dans la démo

**Règles de logging** :
- ✅ Tous les messages en **anglais**
- ✅ Source format : `"galaxy3d::module::SubModule"`
- ✅ Seul `engine_error!` inclut file:line automatiquement
- ✅ DefaultLogger utilise `colored` + `chrono`

**Fichiers modifiés** :
- `galaxy_3d_engine/src/log.rs` : Ajout `#[doc(hidden)]` aux macros
- `galaxy_3d_engine/src/lib.rs` : Suppression ré-export macros dans `galaxy3d::log`
- `Games/galaxy3d_demo/src/tracing_logger.rs` : Exemple TracingLogger
- `Games/galaxy3d_demo/src/main.rs` : Utilisation TracingLogger

---

## 🔷 resource::Mesh - Design Notes (2026-02-04)

**Status**: Structure de base implémentée ✅
- Hiérarchie 4 niveaux : Mesh > MeshEntry > MeshLOD > SubMesh
- MeshDesc passe raw data (Vec<u8>), ResourceManager crée les buffers GPU
- Validation automatique des offsets submesh vs buffer sizes
- Calcul automatique vertex/index counts depuis data length et stride
- Logging info à la création et suppression

### Vision Architecture

```
resource::Mesh  →  Stockage structuré de render::Buffer (niveau GPU/ressource)
                   - Données GPU brutes
                   - Hiérarchie : Mesh (groupe) > MeshEntry > LOD > SubMesh
                   - PAS de concepts de scène (materials, AABB, LOD selection logic)
                   - Lifetime étendu (ressource réutilisable)

scene::Mesh     →  Instance dans la scène (futur)
                   - Référence resource::Mesh
                   - Matériaux, AABB, logique de sélection LOD, transforms, etc.
                   - Lifetime = durée de la scène ou moins
```

### Hiérarchie à 4 niveaux

```
resource::Mesh (groupe)
├── name: "characters"
├── vertex_buffer: Arc<render::Buffer>       (partagé par tous)
├── index_buffer: Option<Arc<render::Buffer>> (partagé par tous)
├── vertex_layout: VertexLayout              (partagé par tous)
├── index_type: IndexType                    (partagé par tous)
│
└── meshes: HashMap<String, MeshEntry>
    │
    ├── "hero" → MeshEntry
    │   └── lods: Vec<MeshLOD>
    │       ├── [0] → MeshLOD (LOD0 - plus détaillé)
    │       │   └── submeshes: HashMap<String, SubMesh>
    │       │       ├── "body"  → SubMesh { offsets, topology, renderer_ref }
    │       │       ├── "armor" → SubMesh { ... }
    │       │       └── "cape"  → SubMesh { ... }
    │       │
    │       ├── [1] → MeshLOD (LOD1)
    │       │   └── submeshes: HashMap<String, SubMesh>
    │       │       ├── "body"  → SubMesh { ... }
    │       │       └── "armor" → SubMesh { ... }  // cape supprimée
    │       │
    │       └── [2] → MeshLOD (LOD2 - moins détaillé)
    │           └── submeshes: HashMap<String, SubMesh>
    │               └── "body"  → SubMesh { ... }  // tout fusionné
    │
    └── "enemy_grunt" → MeshEntry
        └── lods: Vec<MeshLOD>
            └── ...
```

**Accès** : `mesh.submesh("hero", 0, "body")` → Option<&SubMesh>

### Décisions Finales

| Aspect | Décision | Justification |
|--------|----------|---------------|
| **VertexLayout** | Par groupe (Mesh) | Tous les SubMesh partagent le même buffer → même stride |
| **IndexType** | Par groupe (Mesh) | Un seul index buffer partagé |
| **Validation offsets** | Oui | Erreurs détectées tôt, debug facilité |
| **Total counts** | Stockés dans Mesh | Pour validation, pas de calcul à chaque accès |
| **Nom SubMesh** | Obligatoire | Lookup par nom dans HashMap |
| **Topology** | Par SubMesh | Flexibilité (triangles, lignes, points) |
| **VertexFormat par SubMesh** | Non nécessaire | Le pipeline décide quels attributs lire |
| **Renderer ref** | Dans chaque SubMesh | Comme pour les regions de Texture |
| **Création** | Via ResourceManager uniquement | Cohérence avec resource::Texture |
| **Modifications post-création** | Dans Mesh, appelées par ResourceManager | Évite duplication de code |

### Pas de VertexFormat par SubMesh - Explication

Le buffer contient toujours toutes les données (position + normal + UV + ...).

```
Buffer stride = 32 bytes : [position: 12B][normal: 12B][uv: 8B]
```

**Pipeline LOD0** (lit tout) :
```rust
vertex_input_attributes: [
    { location: 0, offset: 0,  format: Vec3 },  // position
    { location: 1, offset: 12, format: Vec3 },  // normal
    { location: 2, offset: 24, format: Vec2 },  // uv
]
stride: 32
```

**Pipeline LOD2** (simplifié, lit seulement position) :
```rust
vertex_input_attributes: [
    { location: 0, offset: 0, format: Vec3 },  // position seulement
]
stride: 32  // MÊME stride, bytes normal/UV ignorés
```

→ Le choix "utiliser UV ou pas" est une décision de **rendu** (scene), pas de **ressource**.

### LODs dans le même buffer

```
Vertex Buffer:
[---- LOD0 (1000 verts) ----][---- LOD1 (500 verts) ----][---- LOD2 (200 verts) ----]
offset=0                     offset=1000                  offset=1500

Index Buffer:
[---- LOD0 indices ----][---- LOD1 indices ----][---- LOD2 indices ----]
```

La **sélection** du LOD (quelle distance, quel screen-size) est gérée par `scene::Mesh`.

### Structures Rust

```rust
// ===== Dans renderer/pipeline.rs =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    U16,
    U32,
}

impl IndexType {
    pub fn size_bytes(&self) -> u32 {
        match self {
            IndexType::U16 => 2,
            IndexType::U32 => 4,
        }
    }
}

// ===== Dans resource/mesh.rs =====

/// SubMesh - plus petit élément drawable
pub struct SubMesh {
    renderer: Arc<Mutex<dyn Renderer>>,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub topology: PrimitiveTopology,
}

/// MeshLOD - niveau de détail contenant des submeshes
pub struct MeshLOD {
    submeshes: HashMap<String, SubMesh>,
}

/// MeshEntry - un mesh nommé avec ses LODs
pub struct MeshEntry {
    lods: Vec<MeshLOD>,
}

/// Mesh - groupe de meshes partageant les mêmes buffers
pub struct Mesh {
    name: String,
    renderer: Arc<Mutex<dyn Renderer>>,
    vertex_buffer: Arc<dyn Buffer>,
    index_buffer: Option<Arc<dyn Buffer>>,
    vertex_layout: VertexLayout,
    index_type: IndexType,
    total_vertex_count: u32,
    total_index_count: u32,
    meshes: HashMap<String, MeshEntry>,
}

// ===== Descriptors pour création =====

pub struct SubMeshDesc {
    pub name: String,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub topology: PrimitiveTopology,
}

pub struct MeshLODDesc {
    pub lod_index: usize,
    pub submeshes: Vec<SubMeshDesc>,
}

pub struct MeshEntryDesc {
    pub name: String,
    pub lods: Vec<MeshLODDesc>,
}

pub struct MeshDesc {
    /// Raw vertex data (bytes, interleaved according to vertex_layout)
    pub vertex_data: Vec<u8>,
    /// Raw index data (optional, None for non-indexed meshes)
    pub index_data: Option<Vec<u8>>,
    /// Vertex layout description (defines stride for vertex count calculation)
    pub vertex_layout: VertexLayout,
    /// Index type (U16 or U32, defines stride for index count calculation)
    pub index_type: IndexType,
    /// Initial mesh entries (can be empty, add later via add_mesh_entry)
    pub meshes: Vec<MeshEntryDesc>,
}

// Note: total_vertex_count and total_index_count are computed automatically
// from data length and layout stride by ResourceManager::create_mesh()
```

### API ResourceManager

```rust
impl ResourceManager {
    // Création
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<Arc<Mesh>>;

    // Accès
    pub fn mesh(&self, name: &str) -> Option<&Arc<Mesh>>;
    pub fn remove_mesh(&mut self, name: &str) -> bool;
    pub fn mesh_count(&self) -> usize;

    // Modification (délègue à Mesh)
    pub fn add_mesh_entry(&mut self, mesh_name: &str, entry: MeshEntryDesc) -> Result<()>;
    pub fn add_mesh_lod(&mut self, mesh_name: &str, entry_name: &str, lod: MeshLODDesc) -> Result<()>;
    pub fn add_submesh(&mut self, mesh_name: &str, entry_name: &str, lod: usize, submesh: SubMeshDesc) -> Result<()>;
}
```

### API Mesh (méthodes de modification)

```rust
impl Mesh {
    // Accès
    pub fn submesh(&self, mesh: &str, lod: usize, submesh: &str) -> Option<&SubMesh>;
    pub fn mesh(&self, name: &str) -> Option<&MeshEntry>;
    pub fn mesh_names(&self) -> Vec<&str>;

    // Modification (utilisées par ResourceManager et directement)
    pub fn add_mesh_entry(&mut self, entry: MeshEntryDesc) -> Result<()>;
    pub fn add_mesh_lod(&mut self, entry_name: &str, lod: MeshLODDesc) -> Result<()>;
    pub fn add_submesh(&mut self, entry_name: &str, lod: usize, submesh: SubMeshDesc) -> Result<()>;
}
```

### Comparaison avec Moteurs Modernes

| Aspect | Unreal | Unity | Godot | Galaxy3D |
|--------|--------|-------|-------|----------|
| Vertex buffer | Multi-streams | Interleaved | Interleaved | Interleaved |
| Index obligatoire | ~100% | ~100% | ~100% | Optionnel |
| LOD storage | Par LOD | Séparé (LODGroup) | Séparé | Même buffer |
| Hiérarchie | StaticMesh > LOD > Section | Mesh > SubMesh | Mesh > Surface | Mesh > Entry > LOD > SubMesh |

### Limites Vulkan/D3D12 (référence)

| Limite | Valeur typique |
|--------|---------------|
| Buffer max | 2 GB |
| Max vertices (stride 32B) | ~67 millions |
| Pratique par mesh | 1k - 150k vertices |
| Index u16 | max 65 535 vertices |
| Index u32 | max ~4 milliards |

### Décision finale : Indexed vs Non-indexed

**Choix** : Un seul type `Mesh` avec `index_buffer: Option<Arc<dyn Buffer>>`

**Justification** :
- 99% des meshes sont indexés
- Simplicité : un seul type à gérer
- Différence minime avec Texture (les textures ont des métadonnées fondamentalement différentes)
- SubMesh unifié : `index_offset`/`index_count` ignorés si non-indexé

### Implémentation (2026-02-04) ✅

**Fichiers créés/modifiés** :
- `galaxy_3d_engine/src/renderer/pipeline.rs` : Ajout `IndexType` enum
- `galaxy_3d_engine/src/resource/mesh.rs` : Nouveau fichier avec `Mesh`, `MeshEntry`, `MeshLOD`, `SubMesh` + descriptors
- `galaxy_3d_engine/src/resource/mod.rs` : Export du module mesh
- `galaxy_3d_engine/src/resource/resource_manager.rs` : Gestion des Mesh

**API publique** :
```rust
// Création via ResourceManager
let mesh = resource_manager.create_mesh("characters".to_string(), MeshDesc { ... })?;

// Accès
let mesh = resource_manager.mesh("characters")?;
let submesh = mesh.submesh("hero", 0, "body")?;

// Modification
resource_manager.add_mesh_entry("characters", MeshEntryDesc { ... })?;
resource_manager.add_mesh_lod("characters", "hero", MeshLODDesc { ... })?;
resource_manager.add_submesh("characters", "hero", 1, SubMeshDesc { ... })?;
```

---

## Phase 14 : Système de Mipmaps pour Textures

### Objectif

Implémenter le support des mipmaps pour les textures avec trois modes : aucun mipmap, génération automatique GPU, ou mipmaps manuels fournis par l'utilisateur.

### Philosophie et Décisions

**Problématique** : Comment gérer les mipmaps pour améliorer la qualité visuelle et les performances ?

**Solutions envisagées** :
1. ❌ Toujours générer des mipmaps → Surcoût inutile pour certaines textures
2. ❌ Seulement support manuel → Complexe pour l'utilisateur
3. ✅ **Trois modes flexibles** → Laisse le choix à l'utilisateur

**Mode retenu** : Enum `MipmapMode` avec trois variantes
- `None` : Pas de mipmaps (1 seul niveau)
- `Generate { max_levels: Option<u32> }` : Génération automatique GPU
- `Manual(ManualMipmapData)` : Mipmaps fournis par l'utilisateur

**Structure `ManualMipmapData`** :
- `Single(Vec<Vec<u8>>)` : Mêmes mipmaps pour toutes les couches (SimpleTexture, AtlasTexture)
- `Layers(Vec<LayerMipmapData>)` : Mipmaps par couche (ArrayTexture)

### Implémentation (2026-02-06) ✅

**Architecture Vulkan** :
- **Génération GPU** : Utilise `vkCmdBlitImage` avec filtre LINEAR
  - Boucle sur les niveaux 1..N
  - Transition source → TRANSFER_SRC_OPTIMAL
  - Blit du niveau précédent vers le niveau actuel (downsampling)
  - Transition source → SHADER_READ_ONLY_OPTIMAL
  - Gère toutes les array layers en un seul blit

- **Upload Manuel** :
  - Crée des staging buffers pour chaque niveau de mipmap
  - Pattern : BufferCreateInfo → allocate → bind → copy data
  - Pour `Single` : upload vers toutes les array layers
  - Pour `Layers` : upload vers la couche spécifique
  - Toutes les transitions vers SHADER_READ_ONLY à la fin

**Fichiers modifiés** :
- `galaxy_3d_engine/src/renderer/texture.rs` : Ajout `MipmapMode`, `ManualMipmapData`, `LayerMipmapData`
- `galaxy_3d_engine/src/resource/texture.rs` : Validations des mipmaps dans `from_desc()`
- `galaxy_3d_engine_renderer_vulkan/src/vulkan.rs` : Implémentation GPU et upload manuel

**API publique** :
```rust
// Texture avec mipmaps générés automatiquement
TextureDesc {
    mipmap: MipmapMode::Generate { max_levels: None }, // Génère tous les niveaux
    // ... autres paramètres
}

// Texture avec mipmaps manuels
TextureDesc {
    mipmap: MipmapMode::Manual(ManualMipmapData::Single(vec![
        level1_data, // Half resolution
        level2_data, // Quarter resolution
        // ...
    ])),
    // ...
}

// Pas de mipmaps
TextureDesc {
    mipmap: MipmapMode::None, // Par défaut
    // ...
}
```

---

## Phase 15 : Unified Texture System (Refactoring)

### Objectif

Simplifier l'architecture des textures en remplaçant 3 types distincts (SimpleTexture, AtlasTexture, ArrayTexture) par un seul type unifié avec support des layers et atlas regions.

### Philosophie et Décisions

**Problématique** : Trois types de textures séparés créent de la duplication et limitent la flexibilité (impossible d'avoir des atlas dans ArrayTexture).

**Ancienne architecture** (obsolète) :
```rust
trait Texture { ... }
struct SimpleTexture { ... }   // 1 texture 2D
struct AtlasTexture { ... }    // 1 texture 2D + régions
struct ArrayTexture { ... }    // N layers nommés
```

**Nouvelle architecture** (actuelle) :
```rust
struct Texture {
    renderer_texture: Arc<dyn RendererTexture>,
    descriptor_set: Arc<dyn DescriptorSet>,
    layers: Vec<TextureLayer>,        // 1+ layers
    layer_names: HashMap<String, usize>,
}

struct TextureLayer {
    name: String,
    layer_index: u32,
    regions: Vec<AtlasRegion>,        // Optionnel (atlas)
    region_names: HashMap<String, usize>,
}
```

**Types de textures** :
- **Simple texture** : array_layers=1, 1 seul layer, régions atlas optionnelles
- **Indexed texture** : array_layers>1, N layers, chaque layer avec régions optionnelles

**Avantages** :
- ✅ Simplification : 1 type au lieu de 3
- ✅ Flexibilité : Atlas possible dans les indexed textures
- ✅ Cohérence : Même API pour tous les types
- ✅ Extensibilité : Facile d'ajouter des features aux layers

### Validations (9 checks dans from_desc())

1. **Simple texture** : exactement 1 layer avec index 0
2. **Indexed texture** : peut être créée vide (layers ajoutés plus tard)
3. **Layer indices** : tous < array_layers
4. **Noms de layer** : pas de doublons
5. **Indices de layer** : pas de doublons
6. **Régions atlas** : dans les bounds de la texture
7. **Noms de région** : pas de doublons par layer
8. **Données mipmap** : indices de layer valides
9. **Données de layer** : taille correcte selon dimensions et format

### Implémentation (2026-02-06) ✅

**API publique** :
```rust
// Création via ResourceManager (une seule méthode)
let texture = resource_manager.create_texture("sprites".to_string(), TextureDesc {
    renderer: renderer.clone(),
    texture: render::TextureDesc {
        width: 1024,
        height: 1024,
        format: TextureFormat::R8G8B8A8_SRGB,
        array_layers: 2,  // Indexed texture
        mipmap: MipmapMode::Generate { max_levels: None },
        // ...
    },
    layers: vec![
        LayerDesc {
            name: "layer0".to_string(),
            layer_index: 0,
            data: Some(layer0_pixels),
            regions: vec![
                AtlasRegionDesc {
                    name: "player".to_string(),
                    region: AtlasRegion { x: 0, y: 0, width: 64, height: 64 },
                },
            ],
        },
    ],
})?;

// Accès aux layers
let layer = texture.layer(0)?;                    // Par index
let layer = texture.layer_by_name("layer0")?;   // Par nom
let index = texture.layer_index_by_name("layer0")?;  // Obtenir l'index

// Accès aux régions atlas
let region = texture.region("layer0", "player")?;  // Convenience
let region = layer.region_by_name("player")?;      // Par nom
let region = layer.region(0)?;                     // Par index

// Modification (indexed textures)
resource_manager.add_texture_layer("sprites", LayerDesc { ... })?;
resource_manager.add_texture_region("sprites", "layer0", AtlasRegionDesc { ... })?;

// Info
let is_simple = texture.is_simple();      // array_layers == 1
let is_indexed = texture.is_indexed();    // array_layers > 1
let is_atlas = layer.is_atlas();          // regions.len() > 0
```

**ResourceManager simplifié** :
```rust
// Avant : 3 méthodes
create_simple_texture(name, SimpleTextureDesc)
create_atlas_texture(name, AtlasTextureDesc)
create_array_texture(name, ArrayTextureDesc)

// Maintenant : 1 méthode
create_texture(name, TextureDesc)
```

**Fichiers modifiés** :
- `galaxy_3d_engine/src/resource/texture.rs` : Réécriture complète avec nouvelle architecture
- `galaxy_3d_engine/src/resource/resource_manager.rs` : Simplifié avec une seule méthode
- `galaxy_3d_engine/src/resource/mod.rs` : Exports mis à jour

**Migration** :
- ❌ Suppression de `SimpleTexture`, `AtlasTexture`, `ArrayTexture`
- ❌ Suppression du trait `resource::Texture` (un seul type concret)
- ✅ Type unifié `Texture` avec layers et régions

---

## Phase 16 : Resource Pipeline System

### Objectif

Créer un système de resource::Pipeline pour regrouper des pipelines GPU sous forme de variantes nommées, suivant le même pattern Vec+HashMap que les textures et meshes.

### Philosophie et Décisions

**Problématique** : Comment organiser les pipelines GPU de manière flexible tout en permettant un accès rapide ?

**Concept de variantes** :
- Un `Pipeline` regroupe des configurations de pipeline liées (ex: "mesh" pipeline)
- Chaque variante représente une configuration spécifique (ex: "static", "animated", "transparent")
- L'utilisateur décide de l'organisation des variantes (responsabilité utilisateur)

**Architecture retenue** : Pattern Vec+HashMap avec variantes
```rust
struct Pipeline {
    renderer: Arc<Mutex<dyn Renderer>>,  // Pour add_variant()
    variants: Vec<PipelineVariant>,
    variant_names: HashMap<String, usize>,
}

struct PipelineVariant {
    name: String,
    renderer_pipeline: Arc<dyn RendererPipeline>,
}
```

**Caractéristiques clés** :
- ✅ Peut être créé vide (pas de variantes requises)
- ✅ Stocke le renderer pour `add_variant()` ultérieur
- ✅ Pattern cohérent avec Texture et Mesh
- ✅ Descriptors et render pass gérés plus tard (focus sur le squelette)

### Validations (1 check dans from_desc())

1. **Noms de variantes** : pas de doublons

### Implémentation (2026-02-06) ✅

**API publique** :
```rust
// Création via ResourceManager
let pipeline = resource_manager.create_pipeline("mesh".to_string(), PipelineDesc {
    renderer: renderer.clone(),
    variants: vec![
        PipelineVariantDesc {
            name: "static".to_string(),
            pipeline: render::PipelineDesc {
                vertex_shader: vs,
                fragment_shader: fs,
                // ... autres paramètres pipeline
            },
        },
        PipelineVariantDesc {
            name: "animated".to_string(),
            pipeline: render::PipelineDesc { /* ... */ },
        },
    ],
})?;

// Peut être créé vide
let pipeline = resource_manager.create_pipeline("custom".to_string(), PipelineDesc {
    renderer: renderer.clone(),
    variants: vec![],  // Vide, variantes ajoutées plus tard
})?;

// Accès aux variantes
let variant = pipeline.variant(0)?;                      // Par index
let variant = pipeline.variant_by_name("static")?;      // Par nom
let index = pipeline.variant_index("static")?;          // Obtenir l'index

// Info
let count = pipeline.variant_count();

// Modification
resource_manager.add_pipeline_variant("mesh", PipelineVariantDesc {
    name: "wireframe".to_string(),
    pipeline: render::PipelineDesc { /* ... */ },
})?;
```

**ResourceManager** :
```rust
// Création (retourne Arc<Pipeline>)
create_pipeline(name: String, desc: PipelineDesc) -> Result<Arc<Pipeline>>

// Accès
pipeline(name: &str) -> Option<&Arc<Pipeline>>
remove_pipeline(name: &str) -> bool
pipeline_count() -> usize

// Modification
add_pipeline_variant(pipeline_name: &str, desc: PipelineVariantDesc) -> Result<u32>
```

**Fichiers créés** :
- `galaxy_3d_engine/src/resource/pipeline.rs` (150 lignes)
  - Structure `Pipeline` avec renderer stocké
  - Structure `PipelineVariant`
  - Descripteurs `PipelineDesc` et `PipelineVariantDesc`
  - Méthode `from_desc()` avec validation
  - API : `variant()`, `variant_by_name()`, `variant_index()`, `variant_count()`, `add_variant()`

**Fichiers modifiés** :
- `galaxy_3d_engine/src/resource/resource_manager.rs` : Ajout HashMap<String, Arc<Pipeline>> et méthodes
- `galaxy_3d_engine/src/resource/mod.rs` : Exports Pipeline, PipelineVariant, PipelineDesc, PipelineVariantDesc

**Détails d'implémentation** :
- `Pipeline` stocke le renderer pour permettre `add_variant()` sans passer le renderer en paramètre
- `add_variant()` utilise `self.renderer` pour créer le GPU pipeline
- `ResourceManager::add_pipeline_variant()` appelle simplement `Pipeline::add_variant()` (pas de redondance)
- Pattern `Arc::get_mut()` pour la modification sécurisée

**Extension future** :
- Ajout de descriptor sets au niveau variante
- Support de render pass configuration
- Gestion de pipeline cache

---

## 🧪 Tests Unitaires et Tests d'Intégration

### Philosophie de Test

Le projet Galaxy3D Engine utilise une approche pragmatique des tests :
- **Tests unitaires** pour la logique métier (ResourceManager, structures de données)
- **Tests d'intégration** pour les workflows complets (Engine lifecycle)
- **Tests GPU** pour le backend Vulkan (marqués `#[ignore]`)

### Différence entre Tests Unitaires et Tests d'Intégration

#### Tests Unitaires (`src/`)

**Emplacement** : Dans le même module que le code testé (via `#[cfg(test)]` + `#[path]`)

**Caractéristiques** :
- Testent **une seule unité** de code (fonction, méthode, struct)
- Accès au **code privé** (`pub(crate)`, fonctions privées)
- **Rapides** (pas de dépendances externes)
- **Nombreux** (centaines)

**Exemple** :
```rust
// src/resource/texture.rs
struct InternalCache { }  // Privé

pub struct Texture { }

#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;
```

```rust
// src/resource/texture_tests.rs
use super::*;

#[test]
fn test_texture_creation() {
    let texture = Texture::new(/* ... */);
    assert_eq!(texture.width, 1024);
}

#[test]
fn test_internal_cache() {
    // ✅ Peut tester le code privé !
    let cache = InternalCache::new();
    assert!(cache.is_empty());
}
```

#### Tests d'Intégration (`tests/`)

**Emplacement** : Dossier `tests/` à la racine du crate

**Caractéristiques** :
- Testent **plusieurs modules ensemble**
- Accès **uniquement à l'API publique**
- **Plus lents** (setup complet)
- **Moins nombreux** (dizaines)
- Chaque fichier = **crate séparé**

**Exemple** :
```rust
// tests/resource_manager_integration_test.rs
use galaxy_3d_engine::{Engine, resource::*};

#[test]
fn test_full_workflow() {
    Engine::initialize().unwrap();
    Engine::create_resource_manager().unwrap();

    let rm = Engine::resource_manager().unwrap();
    // Test du workflow complet...

    Engine::shutdown();
}
```

#### Règles de Visibilité en Rust

**Point clé** : En Rust, la hiérarchie des modules définit la visibilité.

```
crate
└── resource
    ├── texture (privé : InternalCache)
    │   └── tests ← ✅ Peut accéder à InternalCache (sous-module)
    │
    └── tests/
        └── texture_tests ← ❌ Ne peut PAS accéder à InternalCache (module cousin)
```

**Pourquoi `tests/` ne peut pas accéder au code privé ?**
- Le dossier `tests/` contient des **crates séparés**
- C'est comme si vous utilisiez le crate depuis un projet externe
- Seule l'API publique (`pub`) est accessible

**Pourquoi `#[cfg(test)] mod tests` peut accéder au code privé ?**
- Le module `tests` est un **sous-module** du module parent
- Il fait partie du même module logique
- Il a accès à tout (public + privé)

### Structure Recommandée

#### Pour le Moteur (galaxy_3d_engine)

```
galaxy_3d_engine/
└── src/
    ├── resource/
    │   ├── mod.rs
    │   │
    │   ├── texture.rs              ← Code
    │   ├── texture_tests.rs        ← Tests unitaires
    │   │
    │   ├── mesh.rs                 ← Code
    │   ├── mesh_tests.rs           ← Tests unitaires
    │   │
    │   ├── pipeline.rs             ← Code
    │   ├── pipeline_tests.rs       ← Tests unitaires
    │   │
    │   ├── resource_manager.rs     ← Code
    │   └── resource_manager_tests.rs ← Tests unitaires
    │
    └── renderer/
        ├── command_list.rs
        └── command_list_tests.rs
```

**Déclaration dans chaque fichier** :
```rust
// À la fin de texture.rs
#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;
```

#### Pour le Backend Vulkan (galaxy_3d_engine_renderer_vulkan)

```
galaxy_3d_engine_renderer_vulkan/
└── src/
    ├── vulkan_command_list.rs
    ├── vulkan_command_list_tests.rs  ← Tests avec GPU (#[ignore])
    │
    ├── vulkan_buffer.rs
    ├── vulkan_buffer_tests.rs        ← Tests avec GPU (#[ignore])
    │
    └── vulkan_texture.rs
        └── vulkan_texture_tests.rs   ← Tests avec GPU (#[ignore])
```

**Tous les tests backend nécessitent GPU** :
```rust
// vulkan_command_list_tests.rs
use super::*;

#[test]
#[ignore]  // Nécessite GPU réel
fn test_bind_index_buffer() {
    let (device, ctx) = create_test_context();
    // Test avec Vulkan réel...
}
```

### Types de Tests

#### Tests Sans GPU (Moteur)

**Ce qui peut être testé** :
- ✅ Structures de données (TextureDesc, MeshDesc, PipelineDesc)
- ✅ Validations de paramètres
- ✅ Calculs mathématiques
- ✅ Logique du ResourceManager
- ✅ State tracking
- ✅ Conversions de types

**Exemple** :
```rust
#[test]
fn test_submesh_offsets() {
    let submesh = SubMeshDesc {
        vertex_offset: 4,
        vertex_count: 4,
        index_offset: 6,
        index_count: 6,
        topology: PrimitiveTopology::TriangleList,
    };

    assert_eq!(submesh.vertex_offset, 4);
    assert_eq!(submesh.index_count, 6);
}
```

#### Tests Avec GPU (Backend)

**Ce qui nécessite GPU** :
- Création de buffers Vulkan
- Création de textures Vulkan
- Command list recording/submission
- Validation layers Vulkan

**Tous marqués avec `#[ignore]`** :
```rust
#[test]
#[ignore]  // Lancé avec: cargo test -- --ignored
fn test_vulkan_buffer_creation() {
    let (device, ctx) = create_test_context();
    let buffer = Buffer::new(&ctx, /* ... */).unwrap();
    assert_ne!(buffer.buffer, vk::Buffer::null());
}
```

### Commandes Cargo Test

```bash
# Tests normaux (sans GPU)
cargo test

# Tests d'un module spécifique
cargo test resource

# Tests d'une fonction spécifique
cargo test test_texture_creation

# Tests avec GPU uniquement
cargo test -- --ignored

# Tous les tests (avec et sans GPU)
cargo test -- --include-ignored

# Afficher les println! même si le test passe
cargo test -- --show-output

# Tests en séquentiel (important pour singletons)
cargo test -- --test-threads=1
```

### Macros d'Assertion

```rust
// Égalité
assert_eq!(2 + 2, 4);
assert_ne!(2 + 2, 5);

// Booléen
assert!(true);
assert!(!false, "Message si échec");

// Résultats
let result: Result<i32, &str> = Ok(42);
assert!(result.is_ok());
assert_eq!(result.unwrap(), 42);

// Option
let value: Option<i32> = Some(42);
assert!(value.is_some());
assert_eq!(value.unwrap(), 42);

// Panic attendu
#[test]
#[should_panic(expected = "Width must be > 0")]
fn test_invalid_dimensions() {
    let desc = TextureDesc { width: 0, /* ... */ };
}
```

### Pyramide de Tests

```
        /\
       /  \      ← Tests Manuels (rares)
      /----\
     / E2E  \    ← Tests GPU (#[ignore]) (quelques-uns)
    /--------\
   /  INTÉG.  \  ← Tests d'Intégration (dizaines)
  /------------\
 /   UNITAIRES  \ ← Tests Unitaires (centaines)
/________________\
```

**Pour Galaxy 3D Engine** :
1. **70-80%** : Tests unitaires (moteur, sans GPU)
2. **15-20%** : Tests d'intégration (workflows)
3. **5-10%** : Tests GPU (backend Vulkan, avec `#[ignore]`)

### Crates Utiles

```toml
[dev-dependencies]
serial_test = "3.0"   # Tests séquentiels (pour singletons)
```

**Exemple avec `serial_test`** :
```rust
use serial_test::serial;

#[test]
#[serial]  // Force l'exécution séquentielle
fn test_engine_singleton_1() {
    Engine::reset_for_testing();
    Engine::initialize().unwrap();
    // Test...
    Engine::shutdown();
}

#[test]
#[serial]  // Ne s'exécute PAS en parallèle
fn test_engine_singleton_2() {
    Engine::reset_for_testing();
    Engine::initialize().unwrap();
    // Test...
    Engine::shutdown();
}
```

### Décision : Option 1 pour le Backend

**Choix retenu** : Tests avec GPU uniquement pour le backend Vulkan

**Raisons** :
- Backend petit (13 fichiers)
- Peu de logique métier (surtout appels Vulkan)
- Refactorer pour tests = over-engineering
- Tests réels avec GPU = validation complète

**Garantie** :
- ✅ **ZÉRO refactoring** du code de production
- ✅ Seul ajout : 3 lignes `#[cfg(test)]` à la fin de chaque fichier
- ✅ Logique métier **inchangée**

---

## 📚 References

- [Vulkan Tutorial](https://vulkan-tutorial.com/)
- [Ash Documentation](https://docs.rs/ash/)
- [gpu-allocator Documentation](https://docs.rs/gpu-allocator/)
- [Vulkan Specification](https://registry.khronos.org/vulkan/specs/1.3/)
