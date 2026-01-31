# Galaxy3DEngine - Documentation Technique

> **Version** : 0.1.0 (Phase 9 - API Backend-Agnostic Complétée)
> **Dernière mise à jour** : 2026-01-30
> **Statut** : Cœur Prêt pour la Production, Fonctionnalités Avancées Prévues

---

## Table des matières

1. [Résumé exécutif](#résumé-exécutif)
2. [Vue d'ensemble de l'architecture](#vue-densemble-de-larchitecture)
3. [Structure du projet](#structure-du-projet)
4. [Principes de conception fondamentaux](#principes-de-conception-fondamentaux)
5. [Hiérarchie des traits](#hiérarchie-des-traits)
6. [Gestion des ressources](#gestion-des-ressources)
7. [Pipeline de rendu](#pipeline-de-rendu)
8. [Implémentation du backend Vulkan](#implémentation-du-backend-vulkan)
9. [Bibliothèque d'images Galaxy](#bibliothèque-dimages-galaxy)
10. [Application de démonstration](#application-de-démonstration)
11. [Motifs de conception](#motifs-de-conception)
12. [Considérations de performance](#considérations-de-performance)
13. [Sécurité des threads et synchronisation](#sécurité-des-threads-et-synchronisation)
14. [Gestion des erreurs](#gestion-des-erreurs)
15. [Extensibilité future](#extensibilité-future)
16. [Résumé de la référence API](#résumé-de-la-référence-api)

---

## Résumé exécutif

**Galaxy3DEngine** est un moteur de rendu 3D sophistiqué et basé sur les traits, construit en Rust avec une abstraction complète de la plateforme. Il exploite le système de traits de Rust pour découpler l'API de rendu abstraite des implémentations de backend (actuellement Vulkan, avec Direct3D 12 prévu).

### Caractéristiques principales

- **Abstraction multi-API** : Conception basée sur les traits indépendante du backend
- **Abstractions à coût zéro** : Objets traits avec surcharge d'exécution minimale
- **Thread-Safe** : Tous les API sont `Send + Sync`
- **Gestion des ressources RAII** : Nettoyage automatique via le trait Drop
- **Architecture de plugin** : Sélection du backend à l'exécution
- **Validation complète** : Couches de validation Vulkan optionnelles avec statistiques
- **Rendu moderne** : Constantes push, ensembles de descripteurs, cibles de rendu, multi-passe

### Pile technologique

| Composant | Technologie |
|-----------|------------|
| Langage | Rust 2024 Edition |
| API graphique | Vulkan 1.3+ |
| Gestion de fenêtres | winit 0.30 |
| Mémoire GPU | gpu-allocator 0.27 |
| Chargement d'images | Bibliothèque galaxy_image personnalisée |
| Validation | Couches de validation Vulkan |

---

## Vue d'ensemble de l'architecture

### Organisation multi-crates

Le projet est organisé comme un espace de travail Cargo avec des crates spécialisées :

```
Galaxy/
├── Tools/
│   ├── galaxy_3d_engine/           (Racine de l'espace de travail)
│   │   ├── galaxy_3d_engine/       (Traits et types fondamentaux)
│   │   └── galaxy_3d_engine_renderer_vulkan/  (Backend Vulkan)
│   │
│   └── galaxy_image/               (Bibliothèque de chargement d'images)
│
└── Games/
    └── galaxy3d_demo/              (Application de démonstration)
```

### Séparation des préoccupations

1. **galaxy_3d_engine** (Bibliothèque fondamentale)
   - Définit toutes les interfaces de trait public
   - Types indépendants de la plateforme (BufferDesc, TextureDesc, etc.)
   - Système de registre de plugins
   - Types d'erreur

2. **galaxy_3d_engine_renderer_vulkan** (Backend)
   - Implémentations Vulkan concrètes de tous les traits
   - Liaisons Ash pour l'API Vulkan
   - Allocation de mémoire GPU (gpu-allocator)
   - Messager de débogage et validation

3. **galaxy_image** (Bibliothèque d'utilitaires)
   - Chargement/sauvegarde PNG, BMP, JPEG
   - Détection automatique du format
   - Conversion de format de pixels

4. **galaxy3d_demo** (Application)
   - Exemple d'utilisation du moteur
   - Rendu de quad texturé
   - Démontre le chargement et le rendu de textures

### Philosophie de conception

**Principes fondamentaux :**

- **Polymorphisme basé sur les traits** : Toutes les ressources exposées comme `Arc<dyn Trait>` ou `Box<dyn Trait>`
- **Abstraction complète du backend** : Aucun type Vulkan/D3D12 ne fuit dans l'API public
- **Sécurité des types** : Descripteurs de ressources fortement typés
- **Contrôle manuel de la mémoire** : Création de ressources explicite avec nettoyage RAII
- **Sécurité des threads** : Tous les traits exigent `Send + Sync`

---

## Structure du projet

### galaxy_3d_engine (Fondamental)

```
galaxy_3d_engine/src/
├── lib.rs                 # Exportations publiques, registre de plugins
├── engine.rs              # Gestionnaire singleton Galaxy3dEngine
└── renderer/
    ├── mod.rs             # Déclarations de modules
    ├── renderer.rs        # Trait Renderer (interface de fabrique)
    ├── buffer.rs          # Trait RendererBuffer + BufferDesc
    ├── texture.rs         # Trait RendererTexture + TextureDesc
    ├── shader.rs          # Trait RendererShader + ShaderDesc
    ├── pipeline.rs        # Trait RendererPipeline + PipelineDesc
    ├── command_list.rs    # Trait RendererCommandList
    ├── render_target.rs   # Trait RendererRenderTarget
    ├── render_pass.rs     # Trait RendererRenderPass
    ├── swapchain.rs       # Trait RendererSwapchain
    └── descriptor_set.rs  # Trait RendererDescriptorSet
```

### galaxy_3d_engine_renderer_vulkan (Backend Vulkan)

```
galaxy_3d_engine_renderer_vulkan/src/
├── lib.rs                      # Exportations, enregistrement Vulkan
├── debug.rs                    # Couches de validation, messager de débogage
├── vulkan.rs                   # Implémentation VulkanRenderer
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

### galaxy_image (Bibliothèque d'images)

```
galaxy_image/src/
├── lib.rs               # Exportations publiques
├── error.rs             # ImageError, ImageResult
├── component_type.rs    # Types de composant U8, U16, F32
├── pixel_format.rs      # RGB, RGBA, BGR, BGRA, Grayscale
├── image_format.rs      # Énumération de format Png, Bmp, Jpeg
├── image.rs             # Struct Image (largeur, hauteur, données)
├── galaxy_image.rs      # Gestionnaire GalaxyImage (charger/enregistrer)
└── loaders/
    ├── mod.rs           # Trait Loader
    ├── png_loader.rs    # Chargement/sauvegarde PNG
    ├── bmp_loader.rs    # Chargement/sauvegarde BMP
    └── jpeg_loader.rs   # Chargement/sauvegarde JPEG
```

---

## Principes de conception fondamentaux

### 1. Abstraction basée sur les traits

Toutes les ressources sont exposées comme des objets traits pour masquer l'implémentation du backend :

```rust
// API publique (indépendante du backend)
pub trait RendererTexture: Send + Sync {}
pub trait RendererBuffer: Send + Sync {}
pub trait RendererPipeline: Send + Sync {}

// Implémentation du backend (type concret, non exposé)
pub struct VulkanRendererTexture {
    image: vk::Image,
    view: vk::ImageView,
    allocation: Option<Allocation>,
    device: Arc<ash::Device>,
    allocator: Arc<Mutex<Allocator>>,
}

// La fabrique retourne un objet trait
fn create_texture(&mut self, desc: TextureDesc)
    -> RenderResult<Arc<dyn RendererTexture>>
```

**Avantages :**
- Le backend peut être remplacé sans modifier le code utilisateur
- Pas de surcharge de monomorphisation
- Séparation propre de l'interface et de l'implémentation

### 2. Stratégie de pointeur intelligent

| Type de ressource | Propriété | Raison |
|---------------|-----------|--------|
| `Arc<dyn RendererTexture>` | Partagée | Textures utilisées par plusieurs listes de commandes |
| `Arc<dyn RendererBuffer>` | Partagée | Buffers partagés entre les images |
| `Arc<dyn RendererPipeline>` | Partagée | Pipelines réutilisées |
| `Box<dyn RendererCommandList>` | Exclusive | Listes de commandes enregistrées une fois par image |
| `Box<dyn RendererSwapchain>` | Exclusive | Propriétaire unique par fenêtre |

### 3. Gestion des ressources RAII

Toutes les ressources implémentent `Drop` pour le nettoyage automatique :

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

**Ordre de nettoyage :**
1. L'utilisateur supprime la dernière référence `Arc<dyn Trait>`
2. La méthode `Drop::drop()` du type concret est appelée
3. Les ressources GPU sont détruites (vues d'image, images, allocations)
4. Aucun nettoyage manuel requis

### 4. Sécurité des types

Le typage fort empêche les mauvaises utilisations :

```rust
pub enum BufferUsage {
    Vertex,   // Ne peut être lié que comme buffer de vertex
    Index,    // Ne peut être lié que comme buffer d'index
    Uniform,  // Ne peut être lié que comme buffer uniforme
    Storage,  // Ne peut être lié que comme buffer de stockage
}

pub enum TextureUsage {
    Sampled,                  // Échantillonnage par shader uniquement
    RenderTarget,             // Pièce jointe de couleur uniquement
    SampledAndRenderTarget,   // Les deux
    DepthStencil,            // Pièce jointe de profondeur/stencil
}
```

### 5. Sécurité des threads

Tous les traits publics exigent `Send + Sync` :

```rust
pub trait Renderer: Send + Sync { ... }
pub trait RendererTexture: Send + Sync {}
pub trait RendererCommandList: Send + Sync { ... }
```

Le renderer est généralement enveloppé dans `Arc<Mutex<dyn Renderer>>` pour l'accès multi-thread.

---

## Hiérarchie des traits

### Trait fondamental : Renderer

Le trait `Renderer` est l'interface de fabrique principale :

```rust
pub trait Renderer: Send + Sync {
    // Création de ressources
    fn create_texture(&mut self, desc: TextureDesc)
        -> RenderResult<Arc<dyn RendererTexture>>;
    fn create_buffer(&mut self, desc: BufferDesc)
        -> RenderResult<Arc<dyn RendererBuffer>>;
    fn create_shader(&mut self, desc: ShaderDesc)
        -> RenderResult<Arc<dyn RendererShader>>;
    fn create_pipeline(&mut self, desc: PipelineDesc)
        -> RenderResult<Arc<dyn RendererPipeline>>;

    // Infrastructure de rendu
    fn create_command_list(&self)
        -> RenderResult<Box<dyn RendererCommandList>>;
    fn create_render_target(&self, desc: &RendererRenderTargetDesc)
        -> RenderResult<Arc<dyn RendererRenderTarget>>;
    fn create_render_pass(&self, desc: &RendererRenderPassDesc)
        -> RenderResult<Arc<dyn RendererRenderPass>>;
    fn create_swapchain(&self, window: &Window)
        -> RenderResult<Box<dyn RendererSwapchain>>;

    // Gestion des descripteurs
    fn create_descriptor_set_for_texture(&self, texture: &Arc<dyn RendererTexture>)
        -> RenderResult<Arc<dyn RendererDescriptorSet>>;
    fn get_descriptor_set_layout_handle(&self) -> u64;

    // Soumission de commandes
    fn submit(&self, commands: &[&dyn RendererCommandList])
        -> RenderResult<()>;
    fn submit_with_swapchain(&self, commands: &[&dyn RendererCommandList],
                             swapchain: &dyn RendererSwapchain,
                             image_index: u32)
        -> RenderResult<()>;

    // Synchronisation
    fn wait_idle(&self) -> RenderResult<()>;

    // Utilitaires
    fn stats(&self) -> RendererStats;
    fn resize(&mut self, width: u32, height: u32);
}
```

### Traits de ressource

| Trait | Méthodes | Objectif |
|-------|---------|---------|
| **RendererBuffer** | `update(offset, data)` | Buffer GPU (vertex/index/uniform) |
| **RendererTexture** | _(marqueur)_ | Ressource texture GPU |
| **RendererShader** | _(marqueur)_ | Module shader compilé (SPIR-V) |
| **RendererPipeline** | _(marqueur)_ | État du pipeline graphique |
| **RendererDescriptorSet** | _(marqueur)_ | Liaison de ressource (textures, uniformes) |
| **RendererRenderPass** | _(marqueur)_ | Configuration de passe de rendu |
| **RendererRenderTarget** | `width()`, `height()`, `format()` | Destination de rendu |

### Trait RendererCommandList

Interface d'enregistrement de commandes :

```rust
pub trait RendererCommandList: Send + Sync {
    // Cycle de vie du buffer de commandes
    fn begin(&mut self) -> RenderResult<()>;
    fn end(&mut self) -> RenderResult<()>;

    // Gestion de la passe de rendu
    fn begin_render_pass(&mut self,
                         render_pass: &Arc<dyn RendererRenderPass>,
                         render_target: &Arc<dyn RendererRenderTarget>,
                         clear_values: &[ClearValue])
        -> RenderResult<()>;
    fn end_render_pass(&mut self) -> RenderResult<()>;

    // État du pipeline
    fn set_viewport(&mut self, viewport: Viewport) -> RenderResult<()>;
    fn set_scissor(&mut self, scissor: Rect2D) -> RenderResult<()>;
    fn bind_pipeline(&mut self, pipeline: &Arc<dyn RendererPipeline>)
        -> RenderResult<()>;

    // Liaison de ressources
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

    // Dessin
    fn draw(&mut self, vertex_count: u32, first_vertex: u32)
        -> RenderResult<()>;
    fn draw_indexed(&mut self, index_count: u32, first_index: u32, vertex_offset: i32)
        -> RenderResult<()>;
}
```

### Trait RendererSwapchain

Interface de présentation de fenêtre :

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

## Gestion des ressources

### Types de descripteur

#### BufferDesc

```rust
pub struct BufferDesc {
    pub size: u64,
    pub usage: BufferUsage,
}

pub enum BufferUsage {
    Vertex,   // Buffer de vertex
    Index,    // Buffer d'index
    Uniform,  // Buffer uniforme (constant buffer)
    Storage,  // Buffer de stockage (SSBO)
}
```

#### TextureDesc

```rust
pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub usage: TextureUsage,
    pub data: Option<Vec<u8>>,  // Données de pixel initiales
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
    Sampled,                 // Échantillonnage par shader
    RenderTarget,            // Pièce jointe de couleur
    SampledAndRenderTarget,  // Les deux
    DepthStencil,           // Pièce jointe de profondeur/stencil
}
```

#### ShaderDesc

```rust
pub struct ShaderDesc<'a> {
    pub code: &'a [u8],        // Bytecode SPIR-V
    pub stage: ShaderStage,
    pub entry_point: String,   // Généralement "main"
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
    pub descriptor_set_layouts: Vec<u64>,  // vk::DescriptorSetLayout en tant que u64
    pub enable_blending: bool,
}

pub struct VertexLayout {
    pub bindings: Vec<VertexBinding>,
    pub attributes: Vec<VertexAttribute>,
}

pub struct VertexBinding {
    pub binding: u32,
    pub stride: u32,
    pub input_rate: VertexInputRate,  // Vertex ou Instance
}

pub struct VertexAttribute {
    pub location: u32,         // Localisation du shader
    pub binding: u32,          // Lier d'où tirer
    pub format: TextureFormat,
    pub offset: u32,           // Décalage dans le vertex
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
    pub samples: u32,  // 1 = pas de MSAA, 2/4/8 = MSAA
    pub load_op: LoadOp,        // Load, Clear, DontCare
    pub store_op: StoreOp,      // Store, DontCare
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
}
```

### Stratégie d'allocation de mémoire (Vulkan)

**Intégration de GPU Allocator :**

Utilise la crate `gpu-allocator` avec trois types d'emplacement de mémoire :

1. **GpuOnly** (VRAM) - Mémoire locale au dispositif
   - Cibles de rendu
   - Textures
   - Performance optimale
   - Non accessible par CPU

2. **CpuToGpu** (Mappable) - Mémoire visible par l'hôte
   - Buffers de vertex
   - Buffers d'index
   - Buffers uniformes
   - Buffers de staging
   - CPU peut écrire, GPU peut lire

3. **GpuToCpu** (Readback) - Téléchargement depuis GPU
   - Capture de capture d'écran
   - Transfert de données GPU→CPU

**Exemple d'allocation :**

```rust
// Création d'une texture (GpuOnly)
let allocation_info = AllocationCreateDesc {
    name: "texture",
    requirements: image_memory_requirements,
    location: MemoryLocation::GpuOnly,
    linear: false,  // Tiling optimal
    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
};

let allocation = allocator.lock().unwrap()
    .allocate(&allocation_info)
    .map_err(|e| RenderError::OutOfMemory)?;
```

---

## Pipeline de rendu

### Flux de rendu de haut niveau

```
1. INITIALISATION
   ├── Créer Renderer (VulkanRenderer::new)
   ├── Créer Swapchain (renderer.create_swapchain)
   ├── Créer Render Pass (renderer.create_render_pass)
   └── Créer Command Lists (renderer.create_command_list) × 2 pour double buffering

2. CRÉATION DE RESSOURCES
   ├── Charger les textures (renderer.create_texture)
   ├── Créer des ensembles de descripteurs (renderer.create_descriptor_set_for_texture)
   ├── Créer des buffers vertex/index (renderer.create_buffer)
   ├── Compiler les shaders (renderer.create_shader)
   └── Créer des pipelines (renderer.create_pipeline)

3. BOUCLE DE RENDU PRINCIPALE
   Pour chaque image :
   ├── Acquérir une image swapchain
   │   └── (image_index, render_target) = swapchain.acquire_next_image()
   │
   ├── Enregistrer les commandes
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
   ├── Soumettre les commandes
   │   └── renderer.submit_with_swapchain(&[cmd], swapchain, image_index)
   │
   └── Présenter
       └── swapchain.present(image_index)

4. NETTOYAGE (automatique via Drop)
   ├── Drop swapchain (détruit les images, vues, sémaphores)
   ├── Drop command lists (détruit le pool de commandes, les buffers)
   ├── Drop pipelines (détruit le pipeline Vulkan)
   ├── Drop textures/buffers (libère la mémoire GPU)
   └── Drop renderer (détruit le dispositif, l'instance)
```

### Machine d'état de la liste de commandes

```
┌─────────────┐
│   Créée     │
└──────┬──────┘
       │ begin()
       ▼
┌─────────────┐
│ Enregistrement│ ◄─────────┐
└──────┬──────┘           │
       │ begin_render_pass()
       ▼                   │
┌─────────────┐           │
│ Dans la     │           │
│  Passe de   │ ──────────┤
│   Rendu     │  end_render_pass()
└──────┬──────┘
       │ end()
       ▼
┌─────────────┐
│ Exécutable  │ (prêt pour soumettre)
└─────────────┘
```

### Flux de téléchargement de texture

```
1. L'application crée TextureDesc avec les données de pixel
2. Le Renderer crée un buffer de staging (mémoire CpuToGpu)
3. Copier les données de pixel vers le buffer de staging (côté CPU)
4. Créer VkImage avec la mémoire GpuOnly
5. Créer un buffer de commandes pour le transfert :
   a. Barrière : UNDEFINED → TRANSFER_DST_OPTIMAL
   b. Copie : buffer de staging → image
   c. Barrière : TRANSFER_DST_OPTIMAL → SHADER_READ_ONLY_OPTIMAL
6. Soumettre les commandes de transfert
7. Attendre la fin (fence)
8. Détruire le buffer de staging
9. Retourner Arc<dyn RendererTexture>
```

---

## Implémentation du backend Vulkan

### Initialisation de VulkanRenderer

**Étapes :**

1. **Charger la bibliothèque Vulkan**
   - Créer `ash::Entry` (chargeur Vulkan)

2. **Créer une instance**
   - Info sur l'application (nom, version)
   - Extensions requises (KHR_surface, spécifique à la plateforme)
   - Couches de validation optionnelles (VK_LAYER_KHRONOS_validation)

3. **Configurer le messager de débogage** (si validation activée)
   - Configurer le filtre de sévérité
   - Enregistrer la fonction de rappel de débogage
   - Initialiser le suivi des statistiques

4. **Sélectionner le dispositif physique**
   - Interroger la famille de file graphique
   - Interroger la famille de file de présentation
   - Choisir le premier dispositif approprié

5. **Créer un dispositif logique**
   - Activer l'extension de swapchain
   - Créer une file graphique
   - Créer une file de présentation (peut être identique à graphique)

6. **Créer un allocateur GPU**
   - Initialiser `gpu-allocator::Allocator`
   - Configurer les pools pour GpuOnly, CpuToGpu

7. **Créer les primitives de synchronisation**
   - 2 fences (pour double buffering)
   - Pool de descripteurs (1000 ensembles de descripteurs de texture)
   - Sampler de texture globale (filtrage linéaire)
   - Disposition d'ensemble de descripteurs (liaison 0 = COMBINED_IMAGE_SAMPLER)

### Stratégie de synchronisation

**Synchronisation au niveau de la tranche :**

```
Fence[0] ────┐                ┌──── Fence[1]
             │                │
Tranche 0 : ────┴────────────────┘
             Attendre    Soumettre

Tranche 1 : ──────────────┬────────────┬──
                          │            │
                      Attendre sur     Soumettre avec
                      Fence[1]         Fence[0]
```

**Synchronisation du Swapchain :**

```
acquire_next_image()
  └── Signale : image_available_semaphore[current_frame]

submit_with_swapchain()
  ├── Attend : image_available
  └── Signale : render_finished_semaphore[image_index]

present()
  └── Attend : render_finished_semaphore[image_index]
```

### Gestion de l'ensemble de descripteurs

**Disposition globale :**

```rust
Liaison 0 : COMBINED_IMAGE_SAMPLER
  - Type de descripteur : vk::DescriptorType::COMBINED_IMAGE_SAMPLER
  - Nombre de descripteurs : 1
  - Étape du shader : Fragment
```

**Configuration du Sampler :**

- Filtre Mag/Min : LINEAR
- Mode d'adresse : REPEAT
- Anisotropie : Désactivée (max_anisotropy = 1.0)
- Mipmap LOD : 0.0 (pas de mipmaps pour l'instant)

**Pool de descripteurs :**

- Type : COMBINED_IMAGE_SAMPLER
- Ensembles max : 1000
- Permet l'allocation dynamique pendant le rendu

### Création du pipeline

**Configuration d'état :**

1. **Étapes du shader** : Vertex + Fragment avec modules SPIR-V
2. **Entrée de vertex** : Liaisons (foulées) + Attributs (localisations, formats, décalages)
3. **Assemblage d'entrée** : Topologie (TRIANGLE_LIST, LINE_LIST, POINT_LIST)
4. **Viewport** : État dynamique (défini via la liste de commandes)
5. **Rastérisation** : Mode de remplissage (FILL), mode de culling (BACK), face avant (CCW)
6. **Multisample** : Nombre d'échantillons (par défaut 1 = pas de MSAA)
7. **Mélange de couleurs** : Configuration du blending par pièce jointe
8. **État dynamique** : VIEWPORT, SCISSOR
9. **Constantes push** : Plages de données immédiates
10. **Dispositions d'ensemble de descripteurs** : Dispositions de liaison de ressource

**Formule de blending (si activée) :**

```
Résultat = Src * SrcAlpha + Dst * (1 - SrcAlpha)

src_color_blend_factor: SRC_ALPHA
dst_color_blend_factor: ONE_MINUS_SRC_ALPHA
color_blend_op: ADD
```

---

## Bibliothèque d'images Galaxy

### Vue d'ensemble

`galaxy_image` est une bibliothèque légère de chargement/sauvegarde d'images avec détection automatique du format.

**Formats supportés :**

| Format | Extension | Chargement | Sauvegarde | Notes |
|--------|-----------|---------|--------|-------|
| PNG | .png | ✅ | ✅ | Sans perte, support alpha |
| BMP | .bmp | ✅ | ✅ | Pas de compression |
| JPEG | .jpg/.jpeg | ✅ | ✅ | Avec perte, pas d'alpha |

### API

```rust
use galaxy_image::{GalaxyImage, ImageFormat, PixelFormat};

// Charger l'image (format auto-détecté à partir des bytes magiques)
let image = GalaxyImage::load_from_file("texture.png")?;

println!("Image chargée {}x{}", image.width(), image.height());
println!("Format de pixel : {:?}", image.pixel_format());

// Accéder aux données de pixel
let pixels: &[u8] = image.data();

// Enregistrer dans un format différent
GalaxyImage::save_to_file(&image, "output.jpg", ImageFormat::Jpeg)?;
```

### Conversion de format de pixel

**Conversion automatique RGB → RGBA :**

```rust
// Si l'image chargée est RGB (3 octets/pixel)
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
    _ => panic!("Format de pixel non supporté"),
};
```

---

## Application de démonstration

### galaxy3d_demo

**Objectif :** Démontre le chargement de texture et le rendu avec Galaxy3DEngine

**Caractéristiques :**

- Charge 3 textures (PNG, BMP, JPEG)
- Affiche 3 quads texturés côte à côte
- Démontre les ensembles de descripteurs
- Montre la conversion de format de pixels
- Intégration complète de la couche de validation

**Boucle principale :**

```rust
fn render(&mut self) {
    // 1. Acquérir une image de swapchain
    let (image_index, render_target) = self.swapchain
        .as_mut().unwrap()
        .acquire_next_image()
        .unwrap();

    // 2. Obtenir la liste de commandes actuelle (double buffering)
    let cmd = &mut self.command_lists[self.current_frame];

    // 3. Enregistrer les commandes
    cmd.begin().unwrap();
    cmd.begin_render_pass(
        self.render_pass.as_ref().unwrap(),
        &render_target,
        &[ClearValue::Color([0.1, 0.1, 0.1, 1.0])],
    ).unwrap();

    cmd.set_viewport(viewport).unwrap();
    cmd.set_scissor(scissor).unwrap();
    cmd.bind_pipeline(self.pipeline.as_ref().unwrap()).unwrap();

    // Dessiner 3 quads (un pour chaque texture)
    for i in 0..3 {
        cmd.bind_descriptor_sets(
            self.pipeline.as_ref().unwrap(),
            &[&self.descriptor_sets[i]],
        ).unwrap();
        cmd.bind_vertex_buffer(&self.vertex_buffers[i], 0).unwrap();
        cmd.draw(6, 0).unwrap();  // 2 triangles = 6 sommets
    }

    cmd.end_render_pass().unwrap();
    cmd.end().unwrap();

    // 4. Soumettre
    self.renderer.as_ref().unwrap()
        .lock().unwrap()
        .submit_with_swapchain(
            &[cmd.as_ref()],
            self.swapchain.as_ref().unwrap().as_ref(),
            image_index,
        ).unwrap();

    // 5. Présenter
    self.swapchain.as_mut().unwrap()
        .present(image_index)
        .unwrap();

    // 6. Alterner la tranche
    self.current_frame = (self.current_frame + 1) % 2;
}
```

---

## Motifs de conception

### 1. Motif de trait marqueur

**Objectif :** Sécurité des types sans exposer les détails d'implémentation

```rust
pub trait RendererTexture: Send + Sync {}
pub trait RendererShader: Send + Sync {}
pub trait RendererPipeline: Send + Sync {}
```

**Avantages :**
- Empêche la confusion accidentelle du type de ressource
- Permet les ajouts de méthodes futures sans ruptures de compatibilité
- Maintient l'API publique minimale
- Le backend peut ajouter des méthodes via des transtypage unsafe

### 2. Motif de downcast

**Motif :**

```rust
// L'API publique reçoit un objet trait
fn submit_with_swapchain(&self,
                         commands: &[&dyn RendererCommandList],
                         swapchain: &dyn RendererSwapchain,
                         image_index: u32) -> RenderResult<()>

// Le backend transtypes vers le type concret
let vk_cmd = *cmd as *const dyn RendererCommandList
    as *const VulkanRendererCommandList;
let vk_cmd = unsafe { &*vk_cmd };

// Accéder aux membres spécifiques à Vulkan
vk_cmd.command_buffer  // vk::CommandBuffer
```

**Invariant de sécurité :** Le backend crée uniquement des objets traits pour les types concrets correspondants

### 3. Motif de registre de plugin

**Registre global :**

```rust
static RENDERER_REGISTRY: Mutex<Option<RendererPluginRegistry>>
    = Mutex::new(None);

pub fn register_renderer_plugin<F>(name: &'static str, factory: F)
where
    F: Fn(&Window, RendererConfig)
        -> RenderResult<Arc<Mutex<dyn Renderer>>>
        + Send + Sync + 'static
```

**Usage :**

```rust
// Dans l'initialisation de la crate Vulkan
register_renderer_plugin("vulkan", |window, config| {
    Ok(Arc::new(Mutex::new(VulkanRenderer::new(window, config)?)))
});

// Dans l'application
let renderer = renderer_plugin_registry()
    .lock().unwrap()
    .as_mut().unwrap()
    .create_renderer("vulkan", &window, config)?;
```

---

## Considérations de performance

### Stratégie d'allocation

**Ressources pré-allouées :**

- Pool de descripteurs : 1000 ensembles (approprié pour la plupart des scènes)
- Fences d'envoi : 2 (pour double buffering)
- Pools de commandes : Par liste de commandes

**Allocation dynamique :**

- Ensembles de descripteurs : Alloués à la demande depuis le pool
- Textures/Buffers : Alloués via gpu-allocator

### Réutilisation du buffer de commandes

```rust
// Réinitialiser au lieu de recréer
self.device.reset_command_buffer(self.command_buffer, ...);
// Pas de surcharge d'allocation
```

### Réutilisation du sampler

**Sampler global unique :**

Toutes les textures partagent un objet sampler :

```rust
texture_sampler: vk::Sampler,  // Partagé globalement
```

Réduit les changements d'état et la consommation de ressources.

### Barrières de mémoire

**Implicites via les passes de rendu :**

- Les transitions d'attachement se produisent automatiquement
- Pas de barrières manuelles dans l'API publique
- Meilleures opportunités d'optimisation pour les pilotes

---

## Sécurité des threads et synchronisation

### Types thread-safe

Tous les traits publics exigent `Send + Sync` :

```rust
pub trait Renderer: Send + Sync { ... }
pub trait RendererTexture: Send + Sync {}
pub trait RendererCommandList: Send + Sync { ... }
```

### Renderer enveloppé dans Mutex

```rust
Arc<Mutex<dyn Renderer>>  // Accès partagé thread-safe
```

Permet à plusieurs threads de créer des ressources, bien que l'enregistrement des commandes se produise généralement sur le thread de rendu.

### Sécurité des threads de l'allocateur GPU

```rust
allocator: Arc<Mutex<Allocator>>  // Accès synchronisé
```

Toutes les allocations/désallocations sont protégées par mutex.

### Synchronisation CPU-GPU

**Fences (CPU attend GPU) :**

```rust
// Avant de soumettre la tranche N
device.wait_for_fences(&[submit_fences[current_submit_fence]], ...);
device.reset_fences(&[submit_fences[current_submit_fence]]);

// Après soumission
device.queue_submit(..., submit_fences[current_submit_fence]);

current_submit_fence = (current_submit_fence + 1) % 2;
```

**Sémaphores (GPU attend GPU) :**

```rust
// Acquérir attend que l'image soit disponible
acquire_next_image() → signale image_available_semaphore

// Soumettre attend que l'image soit disponible, signale rendu terminé
queue_submit(
    wait: image_available,
    signal: render_finished,
);

// Présenter attend que le rendu soit terminé
present(wait: render_finished);
```

---

## Gestion des erreurs

### Énumération RenderError

```rust
pub enum RenderError {
    BackendError(String),           // Défaillance spécifique au backend
    OutOfMemory,                    // Mémoire GPU épuisée
    InvalidResource(String),        // État/utilisation invalide
    InitializationFailed(String),   // Erreur d'initialisation
}

pub type RenderResult<T> = Result<T, RenderError>;
```

### Propagation des erreurs

Toutes les opérations pouvant échouer retournent `RenderResult<T>` :

```rust
fn create_texture(&mut self, desc: TextureDesc)
    -> RenderResult<Arc<dyn RendererTexture>>;

fn submit(&self, commands: &[&dyn RendererCommandList])
    -> RenderResult<()>;
```

### Intégration de la couche de validation

**Configuration de débogage :**

```rust
pub struct DebugConfig {
    pub severity: DebugSeverity,  // ErrorsOnly, ErrorsAndWarnings, All
    pub output: DebugOutput,      // Console, File("path"), Both("path")
    pub message_filter: DebugMessageFilter,
    pub break_on_error: bool,     // Pause du débogueur sur erreur de validation
    pub panic_on_error: bool,     // Panique sur erreur de validation
    pub enable_stats: bool,       // Suivi des statistiques de validation
}
```

**Suivi des statistiques :**

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

## Extensibilité future

### Caractéristiques prévues (Phase 10+)

**Phase 10-12 : Système de texture avancé**

- Atlas de textures
- Tableaux de textures
- Textures sans liaison (indexation de descripteur)
- Textures virtuelles
- Génération de mipmap (CPU : Lanczos-3, GPU : Box filter)
- Support de conteneur DDS/KTX2
- Compression BC7 (côté CPU)

**Phase 13-15 : Système de maillage avancé**

- Batching de maillage (buffers vertex/index globaux)
- Dessin indirect (vkCmdDrawIndexedIndirect)
- Culling GPU (frustum, occlusion, Hi-Z)
- LODs (Niveaux de détail)
- Skinning GPU (animation squelettique)

**Phase 16+ : Caractéristiques avancées**

- Compute shaders
- Ray tracing (VK_KHR_ray_tracing)
- Enregistrement de commandes multi-thread
- Système render graph
- Système de matériau
- Graphe de scène

### Support multi-backend

**Ajouter Direct3D 12 :**

```rust
// Créer une nouvelle crate : galaxy_3d_engine_renderer_d3d12

// Implémenter tous les traits
pub struct D3D12Renderer { ... }
impl Renderer for D3D12Renderer { ... }

pub struct D3D12RendererTexture { ... }
impl RendererTexture for D3D12RendererTexture {}

// Enregistrer le plugin
register_renderer_plugin("d3d12", |window, config| {
    Ok(Arc::new(Mutex::new(D3D12Renderer::new(window, config)?)))
});
```

**Aucun changement requis dans le code utilisateur :**

```rust
// Sélectionne le backend à l'exécution
let renderer = create_renderer("d3d12", &window, config)?;
```

---

## Résumé de la référence API

### Traits fondamentaux

| Trait | Rôle | Méthodes clés |
|-------|------|-------------|
| `Renderer` | Fabrique/Dispositif | `create_texture`, `create_buffer`, `create_shader`, `create_pipeline`, `create_command_list`, `submit` |
| `RendererCommandList` | Enregistrement de commandes | `begin`, `begin_render_pass`, `bind_pipeline`, `bind_descriptor_sets`, `draw`, `end` |
| `RendererSwapchain` | Présentation | `acquire_next_image`, `present`, `recreate` |
| `RendererBuffer` | Buffer GPU | `update` |
| `RendererTexture` | Texture GPU | _(marqueur)_ |
| `RendererShader` | Module shader | _(marqueur)_ |
| `RendererPipeline` | Pipeline graphique | _(marqueur)_ |
| `RendererDescriptorSet` | Liaison de ressource | _(marqueur)_ |
| `RendererRenderPass` | Config de passe de rendu | _(marqueur)_ |
| `RendererRenderTarget` | Destination de rendu | `width`, `height`, `format` |

### Types de configuration

| Type | Objectif | Champs clés |
|------|---------|------------|
| `RendererConfig` | Configuration du moteur | `enable_validation`, `debug_severity`, `debug_output` |
| `BufferDesc` | Création de buffer | `size`, `usage` (Vertex/Index/Uniform/Storage) |
| `TextureDesc` | Création de texture | `width`, `height`, `format`, `usage`, `data` |
| `ShaderDesc` | Création de shader | `code` (SPIR-V), `stage`, `entry_point` |
| `PipelineDesc` | Création de pipeline | `shaders`, `vertex_layout`, `topology`, `push_constants`, `blending` |
| `RendererRenderPassDesc` | Passe de rendu | `color_attachments`, `depth_attachment` |
| `RendererRenderTargetDesc` | Cible de rendu | `width`, `height`, `format`, `usage`, `samples` |

### Énumérations

| Énumération | Valeurs |
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

**Fin de la documentation technique**
