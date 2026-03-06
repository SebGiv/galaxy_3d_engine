# Render Graph — Design Document

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-11 (mis à jour 2026-02-13)
> **Statut** : Implémenté — DAG structure, RenderTargetKind (Swapchain + Texture), API spécialisées
> **Refactoring planifié** : AccessType — barrières automatiques (voir §11)
> **Prérequis** : SceneManager (implémenté), render::RenderTarget (trait existant), render::Swapchain (trait existant)
> **Voir aussi** : [scene_design.md](scene_design.md), [pipeline_data_binding.md](pipeline_data_binding.md)
> **Note** : Le module a été renommé de `target` à `render_graph`. Le concept de "render graph" (DAG de passes de rendu) remplace la notion initiale de "target manager". Les render targets deviennent des arêtes du graphe.

---

## Table des matières

1. [Problématique](#1-problématique)
2. [Réflexion architecturale](#2-réflexion-architecturale)
3. [Frontière bas niveau / haut niveau](#3-frontière-bas-niveau--haut-niveau)
4. [SceneRenderTarget](#4-scenerendertarget) *(design futur)*
5. [RenderGraphManager](#5-rendergraphmanager)
6. [API de rendu](#6-api-de-rendu)
7. [RenderPass — concept et abstraction](#7-renderpass--concept-et-abstraction)
8. [Exemple d'utilisation](#8-exemple-dutilisation)
9. [Design du render_graph::RenderTarget](#9-design-du-render_graphrendertarget-implémenté--2026-02-13) **(implémenté)**
10. [Itérations futures](#10-itérations-futures)
11. [Refactoring AccessType — Barrières automatiques](#11-refactoring-accesstype--barrières-automatiques-planifié)
12. [Redesign des barrières — Tracking dynamique des layouts](#12-redesign-des-barrières--tracking-dynamique-des-layouts-planifié)
13. [Migration Vulkan 1.3 — Dynamic Rendering + Synchronization2](#13-migration-vulkan-13--dynamic-rendering--synchronization2-planifié)

---

## 1. Problématique

Quand on rend une scène, il faut répondre à une question fondamentale : **où est-ce qu'on dessine ?**

Deux cas :
- **À l'écran** — directement via la swapchain (le cas le plus courant)
- **Dans une texture intermédiaire** — pour shadow maps, reflections, post-processing, minimap, etc.

Le moteur 3D a besoin d'une abstraction qui encapsule cette destination de rendu, indépendamment de la scène qu'on y dessine.

---

## 2. Réflexion architecturale

### Première idée : targets dans le SceneManager

L'idée initiale était de stocker les targets dans le SceneManager (dans un HashMap séparé des scènes), pour éviter de créer un manager supplémentaire.

**Avantage** : un seul manager, moins de boilerplate.

**Problème** : sémantiquement, un target n'est pas une scène. Le SceneManager gère le "quoi dessiner" (les scènes et leurs RenderInstances). Les targets sont le "où dessiner" — un concept fondamentalement différent.

Comparaison avec le ResourceManager : il fonctionne bien parce que tout ce qu'il contient (Geometry, Texture, Pipeline, Material, Mesh) **est** une resource. Il y a cohérence sémantique. Mélanger scènes et targets dans le même manager reviendrait à créer un "SceneAndTargetManager" — signal que deux responsabilités sont confondues.

### Conclusion : manager séparé

Les render graphs sont gérés par un **RenderGraphManager** dédié, séparé du SceneManager. Chaque manager a une responsabilité unique :

| Manager | Responsabilité | Contenu |
|---------|---------------|---------|
| **SceneManager** | "Quoi dessiner" | Scènes nommées (`Arc<Mutex<Scene>>`) |
| **RenderGraphManager** | "Comment rendre" | Render graphs nommés (DAG de passes + targets) |
| **ResourceManager** | "Avec quoi" | Geometry, Texture, Pipeline, Material, Mesh |

### Relation Scene ↔ Target : many-to-many

La relation entre scènes et targets n'est **pas** 1:1 :

```
Scene "game"    ──→  Target "screen"       (vue principale)
Scene "game"    ──→  Target "shadow_map"   (même scène, shadow pass)
Scene "game"    ──→  Target "reflection"   (même scène, reflection probe)

Scene "ui"      ──→  Target "screen"       (overlay sur le même écran)
Scene "minimap" ──→  Target "screen"       (coin de l'écran)
```

Une même scène peut être rendue vers **plusieurs targets** (vue principale + ombres + reflets). Un même target (l'écran) peut recevoir le rendu de **plusieurs scènes** (jeu + UI + minimap).

C'est l'appelant (le moteur de jeu) qui décide ces combinaisons — pas le moteur 3D.

---

## 3. Frontière bas niveau / haut niveau

### Ce que le moteur 3D (Galaxy3D) fournit

Des **primitives** de rendu. Il ne décide pas quoi rendre où — il exécute ce qu'on lui demande.

```
┌─────────────────────────────────────────────────────────┐
│              MOTEUR DE JEU (futur, haut niveau)         │
│                                                         │
│  "Je veux rendre ma scène 'game' avec ma caméra        │
│   principale sur l'écran, puis ma scène 'ui'           │
│   avec une caméra ortho par-dessus"                     │
│                                                         │
│  → Décide l'ordre et la composition                     │
│  → Gère les caméras (position, FOV, projection)         │
│  → Appelle le moteur 3D N fois par frame                │
│                                                         │
│  Pseudo-code :                                          │
│    engine3d.begin_frame()                               │
│    engine3d.render(scene_game, target_shadow, light_vp) │
│    engine3d.render(scene_game, target_screen, cam_main) │
│    engine3d.render(scene_ui, target_screen, cam_ortho)  │
│    engine3d.end_frame()                                 │
└────────────────────────┬────────────────────────────────┘
                         │  appelle
                         ▼
┌─────────────────────────────────────────────────────────┐
│              MOTEUR 3D (Galaxy3D, bas niveau)            │
│                                                         │
│  Fournit 4 primitives :                                 │
│                                                         │
│  1. SceneManager       → gestion des scènes     ✅     │
│  2. RenderGraphManager → gestion des render graphs       │
│  3. render()           → rendre une scène sur un target │
│  4. begin/end_frame()  → lifecycle de frame             │
│                                                         │
│  → Ne DÉCIDE PAS quoi rendre où                         │
│  → EXÉCUTE ce qu'on lui demande                         │
│  → CACHE les détails GPU (RenderPass, Swapchain, etc.)  │
└─────────────────────────────────────────────────────────┘
```

### Tableau de répartition

| Concept | Moteur 3D (bas niveau) | Moteur de Jeu (haut niveau) |
|---------|----------------------|---------------------------|
| **Scene** | Stocke les RenderInstances | Décide quels objets y mettre |
| **Target** | Crée/gère les surfaces GPU | Décide quels targets créer |
| **Camera** | Reçoit un `Mat4` (view_projection) | Calcule le Mat4 depuis position/rotation/FOV |
| **render()** | Exécute : bind, draw, submit | Décide : quelle scene + quel target + quelle camera |
| **Ordre de rendu** | Exécute dans l'ordre des appels | Décide l'ordre (shadows avant couleur, UI en dernier) |
| **RenderPass** | Crée automatiquement, **invisible** | N'existe pas à ce niveau |
| **Swapchain** | Gère en interne (begin/end_frame) | N'existe pas à ce niveau |
| **Composition** | Pas son rôle | FrameComposer / RenderGraph (si besoin) |

---

## 4. SceneRenderTarget

### Structure

```rust
/// A named render target — screen or offscreen texture
///
/// Encapsulates the GPU surface and its associated render pass.
/// The render pass is created automatically — the user never
/// manipulates it directly.
pub struct SceneRenderTarget {
    /// Underlying GPU render target surface
    surface: Arc<dyn RenderTarget>,

    /// Auto-created render pass (internal, hidden from user)
    render_pass: Arc<dyn RenderPass>,

    /// Clear color (RGBA)
    clear_color: [f32; 4],

    /// Clear depth value
    clear_depth: f32,

    /// Viewport (defaults to full target size)
    viewport: Viewport,
}
```

### Deux types de targets

#### Target écran (Screen)

Wraps la swapchain. L'image change à chaque frame (`acquire_next_image`), mais le SceneRenderTarget abstrait ce mécanisme.

```rust
target_manager.create_screen_target("screen", &renderer_name)?;
```

En interne :
- Récupère le swapchain via le renderer
- Crée un render pass compatible avec le format swapchain
- Le `surface` est mis à jour à chaque `begin_frame()` via `acquire_next_image()`

#### Target texture (Offscreen)

Pour shadow maps, reflections, post-processing, render-to-texture.

```rust
target_manager.create_texture_target("shadow_map", TextureTargetDesc {
    width: 1024,
    height: 1024,
    format: TextureFormat::D32Float,
})?;
```

En interne :
- Crée un `RenderTarget` via `Renderer::create_render_target()`
- Crée un render pass compatible
- Le `surface` est fixe (ne change pas entre les frames)

### Pourquoi le RenderPass est caché

Le RenderPass est un concept Vulkan obligatoire (en OpenGL, il n'existe pas explicitement). Il décrit :
- Le format des attachments (couleur, depth)
- Les opérations de chargement/stockage (clear, load, store, don't care)
- Les dépendances entre subpasses

C'est un **détail d'implémentation GPU** que le moteur 3D doit abstraire. Quand l'utilisateur crée un target, le moteur crée automatiquement un render pass compatible. L'utilisateur n'a jamais besoin de savoir que ça existe.

---

## 5. RenderGraphManager

### Structure (implémenté)

```rust
/// Manages named render graphs
///
/// Singleton managed by Engine (same pattern as SceneManager, ResourceManager).
pub struct RenderGraphManager {
    render_graphs: HashMap<String, RenderGraph>,
}
```

### API (implémenté)

```rust
impl RenderGraphManager {
    pub fn new() -> Self;
    pub fn create_render_graph(&mut self, name: &str) -> Result<&RenderGraph>;
    pub fn render_graph(&self, name: &str) -> Option<&RenderGraph>;
    pub fn render_graph_mut(&mut self, name: &str) -> Option<&mut RenderGraph>;
    pub fn remove_render_graph(&mut self, name: &str) -> Option<RenderGraph>;
    pub fn render_graph_count(&self) -> usize;
    pub fn render_graph_names(&self) -> Vec<&str>;
    pub fn clear(&mut self);
}
```

### Lifecycle dans Engine (implémenté)

```rust
// Création
Engine::create_render_graph_manager()?;

// Accès
let rgm = Engine::render_graph_manager()?;

// Destruction (avant SceneManager, ResourceManager, Renderers)
Engine::destroy_render_graph_manager()?;
```

Ordre de destruction :
1. RenderGraphManager (les graphs référencent des objets GPU)
2. SceneManager (les scènes référencent des resources)
3. ResourceManager (les resources référencent des objets GPU)
4. Renderers (les objets GPU)

---

## 6. API de rendu

### Primitive : render()

```rust
/// Render a scene to a target
///
/// This is THE core rendering primitive.
/// The caller (game engine) invokes this N times per frame,
/// once for each scene/target/camera combination.
pub fn render(
    scene: &Scene,
    target: &SceneRenderTarget,
    view_projection: Mat4,       // Phase 1: just a matrix
                                  // Phase 2: Camera struct
) -> Result<()>
```

En interne, cette fonction :
1. `cmd.begin_render_pass(target.render_pass, target.surface, clear_values)`
2. `cmd.set_viewport(target.viewport)`
3. Pour chaque RenderInstance visible dans la scène :
   - `cmd.bind_vertex_buffer(instance.vertex_buffer)`
   - `cmd.bind_index_buffer(instance.index_buffer)` (si indexé)
   - Pour chaque submesh du LOD 0 :
     - `cmd.bind_pipeline(submesh.passes[0])`
     - `cmd.bind_descriptor_sets(submesh.descriptor_sets)`
     - `cmd.push_constants(view_projection + instance.world_matrix + submesh.params)`
     - `cmd.draw_indexed(...)` ou `cmd.draw(...)`
4. `cmd.end_render_pass()`

### Frame lifecycle

```rust
/// Begin a new frame
///
/// For screen targets: acquires the next swapchain image.
/// Must be called before any render() call.
pub fn begin_frame() -> Result<()>

/// End the current frame
///
/// Submits all recorded commands and presents screen targets.
/// Must be called after all render() calls.
pub fn end_frame() -> Result<()>
```

### Boucle de rendu typique (vue du consommateur)

```
begin_frame()

render(scene_game,    target_shadow,  light_view_projection)
render(scene_game,    target_screen,  camera_view_projection)
render(scene_ui,      target_screen,  ortho_projection)

end_frame()
```

Le moteur 3D exécute chaque `render()` dans l'ordre reçu. C'est l'appelant qui contrôle la composition.

---

## 7. RenderPass — concept et abstraction

### Qu'est-ce qu'un RenderPass ?

En Vulkan, avant de dessiner quoi que ce soit, il faut déclarer un RenderPass qui décrit :

- **Attachments** : les surfaces sur lesquelles on dessine (couleur, depth, stencil)
- **Load operation** : que faire au début (Clear = effacer, Load = garder le contenu, DontCare)
- **Store operation** : que faire à la fin (Store = sauvegarder, DontCare = jeter)
- **Subpasses** : les étapes de rendu internes (Phase 1 : un seul subpass)

Exemple concret :
```
RenderPass pour un target écran :
  Attachment 0 : couleur RGBA8
    loadOp  = Clear (effacer avec clear_color)
    storeOp = Store (garder le résultat pour la présentation)
  Subpass 0 : utilise attachment 0 en sortie couleur
```

### Pourquoi c'est caché dans Galaxy3D

- En OpenGL, le concept n'existe pas — le driver le gère implicitement
- En Vulkan, c'est obligatoire mais c'est de la configuration bas niveau
- Le consommateur du moteur 3D n'a pas à savoir que ça existe
- Le render pass est entièrement déterminé par le format du target

**Règle** : quand un `SceneRenderTarget` est créé, le moteur crée automatiquement un render pass compatible. L'utilisateur ne manipule jamais de render pass directement.

---

## 8. Exemple d'utilisation

```rust
// === Setup (une fois) ===

// Créer les managers
Engine::initialize()?;
let renderer = VulkanRenderer::new(&window, Config::default())?;
Engine::create_renderer("main", renderer)?;
Engine::create_resource_manager()?;
Engine::create_scene_manager()?;
Engine::create_render_graph_manager()?;

// Créer un render graph et un target écran (futur)
{
    let rgm = Engine::render_graph_manager()?;
    let mut rgm = rgm.lock().unwrap();
    let graph = rgm.create_render_graph("main")?;
    // TODO: ajouter des passes et targets au graph
}

// Créer une scène et y ajouter des objets
let scene_arc = {
    let sm = Engine::scene_manager()?;
    let mut sm = sm.lock().unwrap();
    sm.create_scene("game")?
};

{
    let mut scene = scene_arc.lock().unwrap();
    let key = scene.create_render_instance(&mesh, world_matrix, aabb, 0)?;
}

// === Boucle de rendu (chaque frame) ===

loop {
    Engine::begin_frame()?;

    // Le moteur de jeu calcule la view-projection matrix
    let vp = projection * view;

    // Rendre la scène sur l'écran
    {
        let scene = scene_arc.lock().unwrap();
        let rgm = Engine::render_graph_manager()?;
        let rgm = rgm.lock().unwrap();
        let graph = rgm.render_graph("main").unwrap();

        // TODO: Engine::render(&scene, graph, vp)?;
    }

    Engine::end_frame()?;
}

// === Cleanup ===
Engine::destroy_render_graph_manager()?;
Engine::destroy_scene_manager()?;
Engine::destroy_resource_manager()?;
Engine::destroy_renderer("main")?;
Engine::shutdown();
```

---

## 9. Design du render_graph::RenderTarget (implémenté — 2026-02-13)

### Contexte

Le render graph est un DAG où les **nodes** sont des `render_graph::RenderPass` et les **edges** sont des `render_graph::RenderTarget`. Ces types haut niveau ne sont pas à confondre avec les types bas niveau `renderer::RenderPass` et `renderer::RenderTarget` du backend GPU.

**Principe clé** : le graph stocke des **descripteurs** (resource::Texture, renderer::Swapchain), pas des vues GPU résolues. La résolution GPU (`renderer.create_render_target_view()`) se fait à l'**exécution**, pas à la construction du graph.

### Implémenté

#### Structure du DAG

```rust
pub struct RenderGraph {
    passes: Vec<RenderPass>,              // nodes
    pass_names: HashMap<String, usize>,
    targets: Vec<RenderTarget>,           // edges
    target_names: HashMap<String, usize>,
}
```

API : `add_pass()`, `add_swapchain_target()`, `add_texture_target()`, `set_output()`, `set_input()`, accesseurs par index/nom.

#### render_graph::RenderTarget — deux types de targets

Un render target du graph est soit un **Swapchain** (l'écran) soit une **Texture** (offscreen).

```rust
pub struct TextureTargetView {
    pub texture: Arc<resource::Texture>,  // resource-level, NOT GPU view
    pub layer: u32,
    pub mip_level: u32,
}

pub enum RenderTargetKind {
    /// L'écran — stocke la référence au swapchain.
    /// Résolu chaque frame via swapchain.acquire_next_image()
    Swapchain(Arc<Mutex<dyn renderer::Swapchain>>),
    /// Une texture — stocke la resource::Texture + layer + mip.
    /// Résolu à l'exécution via renderer.create_render_target_view()
    Texture(TextureTargetView),
}

pub struct RenderTarget {
    kind: RenderTargetKind,
    written_by: Option<usize>,  // single writer constraint
}
```

#### Deux méthodes de création spécialisées

```rust
impl RenderGraph {
    /// Swapchain target — prend la référence au swapchain
    pub fn add_swapchain_target(
        &mut self,
        name: &str,
        swapchain: Arc<Mutex<dyn renderer::Swapchain>>,
    ) -> Result<usize>;

    /// Texture target — prend la resource::Texture + coordonnées
    /// Pas de renderer en paramètre : la résolution GPU est différée
    pub fn add_texture_target(
        &mut self,
        name: &str,
        texture: Arc<resource::Texture>,
        layer: u32,
        mip_level: u32,
    ) -> Result<usize>;
}
```

#### Renderer::create_render_target_view (implémenté)

Méthode du trait `Renderer` pour créer un `renderer::RenderTarget` (vue GPU bas niveau) à partir d'une `renderer::Texture` existante :

```rust
fn create_render_target_view(
    &self,
    texture: &dyn Texture,
    layer: u32,
    mip_level: u32,
) -> Result<Arc<dyn RenderTarget>>;
```

- Crée un `VkImageView` (Vulkan) ciblant un layer/mip spécifique de l'image existante
- La texture doit avoir un usage compatible (`RenderTarget`, `SampledAndRenderTarget`, ou `DepthStencil`)
- Le `renderer::RenderTarget` retourné possède l'ImageView mais **pas** l'image (qui reste dans la Texture)
- Implémenté dans VulkanRenderer et MockRenderer
- **Appelé à l'exécution du graph**, pas à la construction

#### Exécution (à implémenter)

```
Pour chaque pass du graph (ordre topologique) :
  Pour chaque output target :
    match target.kind() {
        Swapchain(swapchain) => {
            // Résolu dynamiquement chaque frame
            let (idx, rt) = swapchain.lock().acquire_next_image()?;
            command_list.begin_render_pass(&render_pass, &rt, &clear)?;
        }
        Texture(view) => {
            // Résolu ici via le renderer
            let gpu_tex = view.texture.renderer_texture();
            let rt = renderer.create_render_target_view(
                gpu_tex.as_ref(), view.layer, view.mip_level)?;
            command_list.begin_render_pass(&render_pass, &rt, &clear)?;
        }
    }
```

#### Usage complet

```rust
// 1. Créer la texture avec usage compatible
let shadow_tex = rm.create_texture("shadow_map", TextureDesc {
    texture: RenderTextureDesc {
        width: 2048, height: 2048,
        format: TextureFormat::D32_FLOAT,
        usage: TextureUsage::DepthStencil,
        ..
    },
    ..
})?;

// 2. Construire le graph
let graph = rgm.create_render_graph("main")?;

// Swapchain target — on passe la référence au swapchain
graph.add_swapchain_target("screen", swapchain.clone())?;

// Texture target — on passe la resource::Texture, résolution GPU différée
graph.add_texture_target("shadow_map", shadow_tex.clone(), 0, 0)?;

// Passes et connexions
graph.add_pass("shadow")?;
graph.add_pass("geometry")?;
graph.set_output("shadow", "shadow_map")?;
graph.set_input("geometry", "shadow_map")?;
graph.set_output("geometry", "screen")?;
```

### Points en suspens

| Question | Statut |
|----------|--------|
| Résolution GPU des texture targets (`create_render_target_view`) | Se fait à l'exécution du graph — pas encore implémenté |
| `renderer::RenderPass` auto-créé par target | Reporté — sera traité lors de l'implémentation de l'exécution |
| Exécution du graph (tri topologique + command list) | Reporté — prochaine étape majeure |
| Cache des `renderer::RenderTarget` views (éviter de recréer chaque frame) | Reporté — optimisation future |

---

## 10. Itérations futures

### Priorité haute

| Item | Description | Impact |
|------|-------------|--------|
| **Camera** | Structure Camera (position, rotation, FOV, near/far, projection type) | Remplace le `Mat4` brut dans `render()` |
| **Depth buffer** | Ajouter un depth attachment aux targets | Nécessaire pour le Z-test (objets qui se masquent) |
| **Target resize** | Recréer les targets quand la fenêtre est redimensionnée | Synchronisé avec `Swapchain::recreate()` |

### Priorité moyenne

| Item | Description | Impact |
|------|-------------|--------|
| ~~**Clear mode configurable**~~ | ~~Clear / Load / DontCare par target~~ | **Implémenté** (2026-02-15) — `TargetOps` enum (Color / DepthStencil), per-target clear values, LoadOp/StoreOp indépendants pour depth et stencil |
| **Multi-attachment** | Targets avec couleur + depth + stencil | Nécessaire pour le rendu 3D correct |
| **Viewport configurable** | Viewport sur la Camera (pas sur le render_graph::RenderPass). Split-screen = plusieurs cameras avec viewports différents dans un seul pass GPU | Voir [camera_design.md](camera_design.md) |
| **Resource pooling** | Pool de textures/framebuffers réutilisables au resize | Évite de recréer tout le render graph au resize — approche standard des moteurs modernes (Unreal RDG, Frostbite Frame Graph). Actuellement le graph est entièrement recréé, ce qui est acceptable tant que le nombre de passes/targets reste faible |

### Priorité haute (nouveau)

| Item | Description | Impact |
|------|-------------|--------|
| **Render Graph — AccessType & barrières** | Système d'access types remplaçant `set_input`/`set_output`, génération automatique de barrières et transitions de layout | **Bloquant** pour le multi-pass (HDR + tonemap, shadow maps, post-processing). Voir **§11** pour le design complet |

### Priorité basse

| Item | Description | Impact |
|------|-------------|--------|
| **Render-to-texture puis sampling** | Utiliser un target texture comme source dans un material | Dépend du refactoring AccessType (§11) |
| **MSAA targets** | Targets avec multisampling | Anti-aliasing |
| **MRT (Multiple Render Targets)** | Écrire dans plusieurs textures en un seul pass | Deferred rendering (G-buffer) |

---

## 11. Refactoring AccessType — Barrières automatiques (planifié)

> **Date** : 2026-03-05
> **Statut** : Implémenté (API + types) — **barrières statiques remplacées par §12** (tracking dynamique)
> **Motivation** : Le multi-pass rendering (ex: HDR scene → tonemap) nécessite des transitions
> de layout et des barrières mémoire entre passes. L'API actuelle (`set_input`/`set_output`)
> ne porte pas assez d'information pour les générer.
> **Références** : Frostbite Frame Graph (GDC 2017), Unreal Engine 5 RDG (`ERHIAccess`), Granite (Maister)

### 11.1 Problème actuel

Le `compile()` hardcode `final_layout: ColorAttachment` pour tous les color targets et `DepthStencilAttachment` pour les depth targets. Il n'y a **aucune barrière** entre les passes (pas de `pipeline_barrier` dans le trait `CommandList`).

Quand un target est **écrit** par un pass (color attachment) puis **lu** par un autre (texture sampling), le GPU a besoin de :

1. **Transition de layout** : `ColorAttachment` → `ShaderReadOnly`
2. **Barrière mémoire** : flush le cache couleur, invalider le cache texture
3. **Synchronisation** : attendre que le pass writer finisse avant que le reader commence

Sans ces barrières, le second pass lit des données invalides ou dans le mauvais layout. Avec un seul pass, ce n'est pas visible car tout est écrit/lu dans le même layout.

### 11.2 Solution : système d'AccessType

Chaque pass déclare **comment** il utilise chaque resource via un `AccessType`. Le compilateur déduit automatiquement les layouts, stages, access masks et génère les barrières nécessaires.

#### AccessType (render_graph)

```rust
/// How a pass accesses a resource.
/// Determines layout, pipeline stage, and access mask automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// Color attachment write (render pass output)
    ColorAttachmentWrite,
    /// Color attachment read (e.g. blending with existing content)
    ColorAttachmentRead,
    /// Depth/stencil write
    DepthStencilWrite,
    /// Depth/stencil read-only (e.g. depth testing without writing)
    DepthStencilReadOnly,
    /// Fragment shader sampling (texture read)
    FragmentShaderRead,
    /// Vertex shader sampling (e.g. displacement maps)
    VertexShaderRead,
    /// Compute shader read (storage buffer / image)
    ComputeRead,
    /// Compute shader write (storage buffer / image)
    ComputeWrite,
    /// Transfer source (copy, blit)
    TransferRead,
    /// Transfer destination (copy, blit)
    TransferWrite,
    /// Ray tracing acceleration structure read
    RayTracingRead,
}
```

#### Table de mapping AccessType → (PipelineStage, AccessMask, ImageLayout)

| AccessType | PipelineStage | AccessMask | ImageLayout |
|------------|--------------|------------|-------------|
| ColorAttachmentWrite | ColorAttachmentOutput | ColorAttachmentWrite | ColorAttachment |
| ColorAttachmentRead | ColorAttachmentOutput | ColorAttachmentRead | ColorAttachment |
| DepthStencilWrite | EarlyFragmentTests \| LateFragmentTests | DepthStencilWrite | DepthStencilAttachment |
| DepthStencilReadOnly | EarlyFragmentTests \| LateFragmentTests | DepthStencilRead | DepthStencilReadOnly |
| FragmentShaderRead | FragmentShader | ShaderRead | ShaderReadOnly |
| VertexShaderRead | VertexShader | ShaderRead | ShaderReadOnly |
| ComputeRead | ComputeShader | ShaderRead | ShaderReadOnly |
| ComputeWrite | ComputeShader | ShaderWrite | General |
| TransferRead | Transfer | TransferRead | TransferSrc |
| TransferWrite | Transfer | TransferWrite | TransferDst |
| RayTracingRead | RayTracingShader | ShaderRead | ShaderReadOnly |

Cette table est **statique** — chaque `AccessType` se résout en exactement un triplet (stage, access, layout). Le compilateur l'utilise pour générer les barrières.

### 11.3 Nouvelle API du RenderGraph

#### `add_access()` remplace `set_input()` / `set_output()`

```rust
/// Per-resource access declaration for a pass.
pub struct ResourceAccess {
    pub target_id: usize,
    pub access_type: AccessType,
}

impl RenderGraph {
    /// Declare how a pass accesses a target.
    /// Replaces set_input() / set_output().
    pub fn add_access(
        &mut self,
        pass: &str,
        target: &str,
        access: AccessType,
    ) -> Result<()>;
}
```

#### RenderPass modifié

```rust
pub struct RenderPass {
    // ...
    accesses: Vec<ResourceAccess>,  // remplace inputs: Vec<usize> + outputs: Vec<usize>
    barriers: Vec<ImageMemoryBarrier>,  // rempli par compile()
    // ...
}
```

#### Exemple d'utilisation — single pass (migration directe)

```rust
// AVANT
graph.set_output("scene", "color")?;
graph.set_output("scene", "depth")?;

// APRÈS
graph.add_access("scene", "color", AccessType::ColorAttachmentWrite)?;
graph.add_access("scene", "depth", AccessType::DepthStencilWrite)?;
```

#### Exemple d'utilisation — multi-pass (HDR + tonemap)

```rust
// Scene pass: writes HDR color + depth
graph.add_access("scene",   "hdr_color", AccessType::ColorAttachmentWrite)?;
graph.add_access("scene",   "depth",     AccessType::DepthStencilWrite)?;

// Tonemap pass: reads HDR color as texture, writes LDR to screen
graph.add_access("tonemap", "hdr_color", AccessType::FragmentShaderRead)?;
graph.add_access("tonemap", "screen",    AccessType::ColorAttachmentWrite)?;
```

Le compilateur voit que `hdr_color` passe de `ColorAttachmentWrite` (scene) à `FragmentShaderRead` (tonemap) et génère automatiquement la barrière :
- Layout transition : `ColorAttachment` → `ShaderReadOnly`
- Src stage : `ColorAttachmentOutput`, src access : `ColorAttachmentWrite`
- Dst stage : `FragmentShader`, dst access : `ShaderRead`

### 11.4 Algorithme de compile() — génération de barrières

```
Pour chaque target T :
  accesses = [(pass_i, access_type_i)] trié par ordre topologique
  Pour chaque paire consécutive (prev, curr) :
    Si prev.access_type nécessite une transition vers curr.access_type :
      → Générer un ImageMemoryBarrier {
          src_stage:  map(prev.access_type).stage,
          dst_stage:  map(curr.access_type).stage,
          src_access: map(prev.access_type).access_mask,
          dst_access: map(curr.access_type).access_mask,
          old_layout: map(prev.access_type).layout,
          new_layout: map(curr.access_type).layout,
          texture:    T.texture,
          layer, mip_level, layer_count, mip_count
        }
      → Attacher cette barrière au début de curr.pass

Le final_layout de chaque target dans le render pass est déterminé
par le DERNIER access de ce target dans la timeline.
```

### 11.5 Algorithme de execute() — émission des barrières

```
Pour chaque pass dans l'ordre topologique :
  Si pass.barriers n'est pas vide :
    cmd.pipeline_barrier(&pass.barriers)
  cmd.begin_render_pass(...)
  pass.action.execute(cmd)
  cmd.end_render_pass()
```

### 11.6 Types à ajouter côté graphics_device

#### PipelineStageFlags

```rust
bitflags! {
    pub struct PipelineStageFlags: u32 {
        const TOP_OF_PIPE             = 0x0001;
        const VERTEX_SHADER           = 0x0008;
        const FRAGMENT_SHADER         = 0x0080;
        const EARLY_FRAGMENT_TESTS    = 0x0100;
        const LATE_FRAGMENT_TESTS     = 0x0200;
        const COLOR_ATTACHMENT_OUTPUT = 0x0400;
        const COMPUTE_SHADER          = 0x0800;
        const TRANSFER                = 0x1000;
        const BOTTOM_OF_PIPE          = 0x2000;
        const RAY_TRACING_SHADER      = 0x00200000;
    }
}
```

#### AccessFlags

```rust
bitflags! {
    pub struct AccessFlags: u32 {
        const SHADER_READ            = 0x0020;
        const SHADER_WRITE           = 0x0040;
        const COLOR_ATTACHMENT_READ  = 0x0080;
        const COLOR_ATTACHMENT_WRITE = 0x0100;
        const DEPTH_STENCIL_READ     = 0x0200;
        const DEPTH_STENCIL_WRITE    = 0x0400;
        const TRANSFER_READ          = 0x0800;
        const TRANSFER_WRITE         = 0x1000;
    }
}
```

#### ImageMemoryBarrier

```rust
/// Describes a pipeline barrier for image layout transition and memory synchronization.
pub struct ImageMemoryBarrier {
    pub src_stage: PipelineStageFlags,
    pub dst_stage: PipelineStageFlags,
    pub src_access: AccessFlags,
    pub dst_access: AccessFlags,
    pub old_layout: ImageLayout,
    pub new_layout: ImageLayout,
    pub texture: Arc<dyn Texture>,
    pub layer: u32,
    pub mip_level: u32,
    pub layer_count: u32,
    pub mip_count: u32,
}
```

#### CommandList — nouvelle méthode

```rust
trait CommandList {
    /// Insert a pipeline barrier (layout transitions + memory synchronization).
    fn pipeline_barrier(&mut self, barriers: &[ImageMemoryBarrier]) -> Result<()>;
    // ... existing methods ...
}
```

#### ImageLayout — nouveaux variants

```rust
pub enum ImageLayout {
    Undefined,
    General,                // ← nouveau (compute storage images)
    ColorAttachment,
    DepthStencilAttachment,
    DepthStencilReadOnly,   // ← nouveau (depth read-only optimization)
    ShaderReadOnly,
    TransferSrc,
    TransferDst,
    PresentSrc,
}
```

### 11.7 Fichiers impactés (moteur)

| Fichier | Modification |
|---------|-------------|
| `graphics_device/render_pass.rs` | `ImageLayout` : +`General`, +`DepthStencilReadOnly` |
| `graphics_device/mod.rs` | Nouveaux types : `PipelineStageFlags`, `AccessFlags`, `ImageMemoryBarrier` |
| `graphics_device/command_list.rs` | Trait `CommandList` : +`pipeline_barrier()` |
| `render_graph/mod.rs` | Nouveaux types : `AccessType`, `ResourceAccess` |
| `render_graph/render_pass.rs` | `inputs`/`outputs` → `accesses: Vec<ResourceAccess>`, +`barriers: Vec<ImageMemoryBarrier>` |
| `render_graph/render_target.rs` | Supprimer `written_by` (déduit des accesses) |
| `render_graph/render_graph.rs` | `add_access()` remplace `set_input()`/`set_output()`, `compile()` génère les barrières, `execute()` les émet |
| `vulkan.rs` (renderer) | Impl `pipeline_barrier()` via `vkCmdPipelineBarrier` |
| `mock.rs` (renderer) | Impl `pipeline_barrier()` (no-op) |

### 11.8 Migration des demos

Toutes les demos existantes utilisent `set_output()` / `set_input()`. La migration est mécanique :

| Avant | Après |
|-------|-------|
| `set_output("pass", "color")` | `add_access("pass", "color", AccessType::ColorAttachmentWrite)` |
| `set_output("pass", "depth")` | `add_access("pass", "depth", AccessType::DepthStencilWrite)` |
| `set_input("pass", "texture")` | `add_access("pass", "texture", AccessType::FragmentShaderRead)` |

Impact estimé : ~2-3 lignes modifiées par demo, 6 demos au total.

---

## 12. Redesign des barrières — Tracking dynamique des layouts (planifié)

> **Date** : 2026-03-06
> **Statut** : Design approuvé — en attente d'implémentation
> **Motivation** : Le design §11 (barrières statiques à compile-time) a un bug fondamental :
> le render pass `final_layout` et les barrières explicites tentent de transiter le même image layout,
> causant des erreurs de validation Vulkan. Ce redesign résout le problème de manière générique et définitive.
> **Références** : Granite engine (Themaister), Frostbite Frame Graph (GDC 2017), UE5 RDG

### 12.1 Diagnostic du problème

Le §11 génère deux types de transitions de layout qui entrent en conflit :

1. **Transitions implicites** via le render pass : `compile()` calcule un `final_layout` via `last_access_layout()`,
   qui regarde le **dernier** accès du target dans tout le graphe. Le render pass transite automatiquement
   `initial_layout → final_layout`.

2. **Transitions explicites** via `vkCmdPipelineBarrier` : `generate_barriers()` émet des barrières
   entre chaque paire d'accès consécutifs.

**Exemple concret** (HDR → tonemap) :

```
Pass "scene"   : ColorAttachmentWrite sur HDR → layout = ColorAttachment
Pass "tonemap" : FragmentShaderRead sur HDR   → layout = ShaderReadOnly
```

- `last_access_layout()` retourne `ShaderReadOnly` (dernier accès = tonemap)
- Le render pass "scene" est créé avec `final_layout = ShaderReadOnly`
- → Le render pass transite HDR de `ColorAttachment` → `ShaderReadOnly`
- Puis `generate_barriers()` émet une barrière `ColorAttachment` → `ShaderReadOnly` avant "tonemap"
- → **Conflit** : l'image est déjà en `ShaderReadOnly`, la barrière attend `ColorAttachment`

Ce n'est pas un bug isolé — c'est un **conflit architectural** entre deux mécanismes de transition.

### 12.2 Solution : barrières dynamiques + render pass sans transition

**Principe fondamental** (validé par Granite, Frostbite, UE5) :

> Les render passes ne font **aucune** transition de layout.
> `initialLayout = finalLayout = layout propre de l'attachment`.
> Toutes les transitions sont faites par des `vkCmdPipelineBarrier` explicites,
> générées **dynamiquement à l'exécution**.

Ce principe élimine toute ambiguïté : un seul mécanisme gère les transitions (les barrières explicites),
et le render pass se contente d'utiliser l'image dans le layout qu'elle a déjà.

### 12.3 Tracking de l'état par target

Chaque `RenderTarget` stocke l'état GPU courant de l'image :

```rust
pub struct RenderTarget {
    pub name: String,
    pub texture: Arc<resource::Texture>,
    pub layer: u32,
    pub mip: u32,
    pub clear_color: Option<[f32; 4]>,
    pub view: Arc<dyn render::TextureView>,

    // Dynamic state — tracked per-frame at execution time
    pub current_layout: ImageLayout,        // starts Undefined
    pub current_stage: PipelineStageFlags,   // starts TOP_OF_PIPE
    pub current_access: AccessFlags,         // starts NONE
}
```

- **Frame 1** : `current_layout = Undefined` (image jamais utilisée)
- **Frame 2+** : `current_layout` = layout du dernier accès de la frame précédente
- Mis à jour **après** chaque barrière émise

### 12.4 Émission des barrières à l'exécution

Les barrières ne sont plus générées dans `compile()` (statique) mais dans `execute()` (dynamique),
juste avant chaque pass :

```rust
fn emit_barrier(
    cmd: &dyn CommandBuffer,
    target: &mut RenderTarget,
    access: &ResourceAccess,
) -> Result<()> {
    let required = access.access_type.info();
    let old_layout = target.current_layout;
    let new_layout = required.layout;

    // Skip if layout already correct and access is read-only
    if old_layout == new_layout && !access.access_type.is_write() {
        return Ok(());
    }

    // Source stage/access: from tracked state
    let (src_stage, src_access) = if old_layout == ImageLayout::Undefined {
        // First frame: no previous access to wait on
        (PipelineStageFlags::TOP_OF_PIPE, AccessFlags::NONE)
    } else {
        (target.current_stage, target.current_access)
    };

    cmd.pipeline_barrier(PipelineBarrierDesc {
        src_stage,
        dst_stage: required.stage,
        src_access,
        dst_access: required.access,
        old_layout,
        new_layout,
        texture: target.texture.graphics_device_texture(),
        layer: target.layer,
        mip: target.mip,
    })?;

    // Update tracked state
    target.current_layout = new_layout;
    target.current_stage = required.stage;
    target.current_access = required.access;

    Ok(())
}
```

#### Boucle d'exécution

```rust
pub fn execute(&mut self, cmd: &dyn CommandBuffer, frame_index: usize) -> Result<()> {
    for pass in &self.compiled_passes {
        // 1. Emit barriers for all resources accessed by this pass
        for access in &pass.accesses {
            let target = &mut self.targets[access.target_id];
            Self::emit_barrier(cmd, target, access)?;
        }

        // 2. Begin render pass (initial_layout == final_layout — no transition)
        cmd.begin_render_pass(&pass.render_pass_desc[frame_index])?;

        // 3. Execute pass action
        if let Some(action) = &pass.action {
            action.execute(cmd)?;
        }

        // 4. End render pass
        cmd.end_render_pass()?;
    }
    Ok(())
}
```

### 12.5 Modifications de compile()

`compile()` est **simplifié** :

- **Supprimé** : `generate_barriers()` — les barrières sont maintenant dynamiques
- **Supprimé** : `last_access_layout()` — n'a plus de raison d'être
- **Modifié** : les render passes sont créés avec `initial_layout = final_layout = layout de l'attachment`

```rust
// Pour chaque attachment d'un pass :
let access_info = access.access_type.info();
let attachment_layout = access_info.layout;

RenderPassAttachment {
    format: ...,
    load_op: if target.clear_color.is_some() { LoadOp::Clear } else { LoadOp::Load },
    store_op: StoreOp::Store,
    initial_layout: attachment_layout,   // NO transition
    final_layout: attachment_layout,     // NO transition
}
```

Le render pass ne fait plus aucune transition — il utilise l'image dans le layout
où la barrière l'a placée.

### 12.6 Cas couverts

| Cas | Comportement |
|-----|-------------|
| **Frame 1** (layout UNDEFINED) | Barrière `UNDEFINED → X`, `TOP_OF_PIPE → stage requis` |
| **Frame 2+** (layout connu) | Barrière `previous_layout → X` avec les bons stages |
| **LoadOp::Clear** | OK : `initial_layout = attachment_layout`, la barrière a déjà transité |
| **LoadOp::Load** | OK : la barrière préserve le contenu (`old_layout` correct) |
| **Même target lu par 2 passes** | Skip si layout identique + read-only |
| **Write → Read** | Barrière correcte : `ColorAttachment → ShaderReadOnly` |
| **Read → Write** | Barrière correcte : `ShaderReadOnly → ColorAttachment` |
| **Compute passes** | Même logique : `General` pour write, `ShaderReadOnly` pour read |
| **Multi-frame buffering** | `current_layout` est par-target, chaque target a son propre état |
| **Plusieurs graphs** | Chaque graph a ses propres targets avec leur propre état |

### 12.7 Comparaison avec les moteurs de référence

| Moteur | Approche | Similitude avec notre design |
|--------|----------|------------------------------|
| **Granite** (Themaister) | `current_layout` par image, barrières dynamiques, flush/invalidate model | Très proche — même tracking dynamique, même skip read→read |
| **Frostbite** (Frame Graph) | Rebuilt chaque frame, barrières calculées à compile(), aliasing force UNDEFINED | Notre tracking survit entre frames (plus efficient pour graphes statiques) |
| **UE5 RDG** | `ERHIAccess` enum, split barriers, async compute | Même concept d'access type, mais plus avancé (split barriers) |

**Recommandation explicite de Themaister** (créateur de Granite) :

> *"Do NOT use render pass initial/final layout for transitions.
> Use `initialLayout = layout = finalLayout` and handle all transitions
> with `vkCmdPipelineBarrier`."*

C'est exactement ce que ce redesign implémente.

### 12.8 Fichiers impactés

| Fichier | Modification |
|---------|-------------|
| `render_graph/render_graph.rs` | `RenderTarget` +3 champs tracking, `compile()` simplifié (render pass sans transition), `execute()` émet les barrières dynamiquement, suppression de `generate_barriers()` et `last_access_layout()` |
| `render_graph/access_type.rs` | Aucun changement (déjà correct) |
| `graphics_device/graphics_device.rs` | Vérifier que `pipeline_barrier()` existe dans le trait `CommandBuffer` |
| Backend Vulkan | Vérifier l'implémentation de `pipeline_barrier()` |

### 12.9 Optimisations futures (non bloquantes)

| Optimisation | Description | Priorité |
|-------------|-------------|----------|
| **Discard optimization** | Si `LoadOp::Clear` ou `DontCare`, utiliser `old_layout = UNDEFINED` (le driver peut skipper le copy-on-transition) | Moyenne |
| **Batch barriers** | Regrouper toutes les barrières d'un pass en un seul `vkCmdPipelineBarrier` | Moyenne |
| **Split barriers** | Émettre la partie "release" après le pass source et la partie "acquire" avant le pass destination (overlap GPU) | Basse |
| **Async compute** | Barrières cross-queue avec semaphores | Basse |

---

## 13. Migration Vulkan 1.3 — Dynamic Rendering + Synchronization2 (planifié)

> **Date** : 2026-03-06
> **Statut** : Réflexion — en attente de décision
> **Motivation** : Vulkan 1.3 promeut deux extensions majeures dans le core qui simplifient
> considérablement le backend Vulkan et résolvent structurellement plusieurs problèmes
> rencontrés avec le design actuel (render pass compatibility, layout transitions, barrières).
> **Prérequis** : §12 (tracking dynamique des layouts) — le modèle de barrières §12 est la
> marche intermédiaire naturelle vers cette migration.

### 13.1 VK_KHR_dynamic_rendering

#### Problème actuel

En Vulkan 1.0-1.2, il faut créer deux objets statiques avant de pouvoir dessiner :

- **VkRenderPass** — décrit les attachments, formats, load/store ops, transitions de layout, subpasses
- **VkFramebuffer** — lie un VkRenderPass à des VkImageView concrètes

Ces objets sont lourds à gérer : cache, compatibilité render pass/pipeline, recréation au resize,
et surtout les transitions de layout implicites via `initialLayout`/`finalLayout` qui sont la cause
directe du bug §12.

#### Solution : dynamic rendering

Avec `VK_KHR_dynamic_rendering` (Vulkan 1.3 core), les deux objets disparaissent.
Tout est spécifié inline dans le command buffer :

```c
VkRenderingAttachmentInfo colorAttachment = {
    .imageView   = myColorImageView,
    .imageLayout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
    .loadOp      = VK_ATTACHMENT_LOAD_OP_CLEAR,
    .storeOp     = VK_ATTACHMENT_STORE_OP_STORE,
    .clearValue  = { .color = {0,0,0,1} },
};

VkRenderingInfo renderingInfo = {
    .renderArea           = { {0,0}, {width, height} },
    .layerCount           = 1,
    .colorAttachmentCount = 1,
    .pColorAttachments    = &colorAttachment,
    .pDepthAttachment     = &depthAttachment,
};

vkCmdBeginRendering(cmd, &renderingInfo);
// ... draw calls ...
vkCmdEndRendering(cmd);
```

**Aucune transition de layout** — toutes les transitions sont faites par `vkCmdPipelineBarrier`
(ou `vkCmdPipelineBarrier2` avec synchronization2).

#### Pipelines sans VkRenderPass

Les pipelines déclarent leurs formats directement via `VkPipelineRenderingCreateInfo` :

```c
VkPipelineRenderingCreateInfo renderingCreateInfo = {
    .colorAttachmentCount    = 1,
    .pColorAttachmentFormats = &colorFormat,
    .depthAttachmentFormat   = depthFormat,
};
pipelineCreateInfo.pNext = &renderingCreateInfo;
pipelineCreateInfo.renderPass = VK_NULL_HANDLE;
```

Plus de "pipeline compatible avec un VkRenderPass" — le pipeline connaît ses formats, point.

#### Subpasses

Dynamic rendering **n'a pas de subpasses**. En pratique, quasiment aucun moteur moderne ne les
utilise — les subpasses étaient conçues pour le tile-based rendering (mobile), mais les drivers
desktop les ignorent. Pour le tile-based mobile, Vulkan 1.3 a `VK_KHR_dynamic_rendering_local_read`.

#### Adoption par les moteurs

- **Godot 4** — dynamic rendering par défaut
- **Unreal Engine 5** — support depuis UE 5.1
- **id Tech** (Doom Eternal) — early adopter
- **Source 2** (Valve) — migré
- **Granite** (Themaister) — supporte les deux, recommande dynamic rendering

### 13.2 VK_KHR_synchronization2

#### Problème avec la synchro Vulkan 1.0

L'API de synchronisation originale est confuse et error-prone :

- **32 bits** pour les bitmasks de stages/access → à court de bits (ray tracing, mesh shaders)
- **Stages globaux** : `vkCmdPipelineBarrier` partage `srcStageMask`/`dstStageMask` entre toutes
  les barrières d'un même appel → le driver doit prendre l'union, réduisant le parallélisme GPU
- **`TOP_OF_PIPE`/`BOTTOM_OF_PIPE`** : sémantique piégeuse et mal comprise
- **Access mask vague** : `VK_ACCESS_SHADER_READ_BIT` ne distingue pas sampled/storage/uniform
- **Combinaisons invalides** acceptées silencieusement (ex: vertex shader + color attachment write)

#### Solution : synchronization2

`VK_KHR_synchronization2` (Vulkan 1.3 core) résout tous ces problèmes :

| Aspect | Vulkan 1.0 | Vulkan 1.3 (Synchronization2) |
|--------|-----------|-------------------------------|
| **Bitmask** | 32 bits | 64 bits (`VkPipelineStageFlagBits2`) |
| **Stages par barrière** | Globaux (partagés) | Par barrière (indépendants) |
| **"Pas de dépendance"** | `TOP_OF_PIPE` (confus) | `NONE` (explicite) |
| **Access shader** | `SHADER_READ` (vague) | `SHADER_SAMPLED_READ`, `SHADER_STORAGE_READ` (précis) |
| **Layouts simplifiés** | 9+ layouts spécifiques | +`READ_ONLY_OPTIMAL`, +`ATTACHMENT_OPTIMAL` |
| **Fonction** | `vkCmdPipelineBarrier` | `vkCmdPipelineBarrier2` |

Chaque barrière porte ses propres stages, permettant au driver d'optimiser indépendamment :

```c
VkImageMemoryBarrier2 barriers[2] = {
    {   // Barrière 1 : ses propres stages
        .srcStageMask  = VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
        .srcAccessMask = VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT,
        .dstStageMask  = VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT,
        .dstAccessMask = VK_ACCESS_2_SHADER_SAMPLED_READ_BIT,
        // ...
    },
    {   // Barrière 2 : stages DIFFÉRENTS — le driver optimise séparément
        .srcStageMask  = VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
        .srcAccessMask = VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
        .dstStageMask  = VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT,
        .dstAccessMask = VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_READ_BIT,
        // ...
    },
};
VkDependencyInfo depInfo = {
    .imageMemoryBarrierCount = 2,
    .pImageMemoryBarriers    = barriers,
};
vkCmdPipelineBarrier2(cmd, &depInfo);
```

### 13.3 Impact sur l'architecture du moteur

#### Abstractions moteur préservées

Les abstractions `graphics_device::RenderPass` et `graphics_device::FrameBuffer` restent dans le moteur.
Elles portent de l'information **sémantique universelle** (attachments, formats, load/store ops, dimensions)
qui est nécessaire indépendamment de l'API GPU :

| Couche | Rôle | Vulkan 1.0-1.2 | Vulkan 1.3 (dynamic rendering) |
|--------|------|----------------|-------------------------------|
| `graphics_device::RenderPass` | Descripteur sémantique | Backend crée un `VkRenderPass` | Backend stocke un descripteur léger (struct Rust) |
| `graphics_device::FrameBuffer` | Binding des ImageView | Backend crée un `VkFramebuffer` | Backend stocke les ImageView (struct Rust) |
| `cmd.begin_render_pass(rp, fb)` | Exécution | `vkCmdBeginRenderPass(...)` | `vkCmdBeginRendering(...)` — construit inline depuis les données du descripteur |

Le trait `GraphicsDevice` et le trait `CommandBuffer` ne changent **pas**.
Seule l'implémentation Vulkan change en interne.

#### Barrières internalisées dans le backend

Le concept de "barrière" est un détail d'implémentation GPU qui varie selon l'API :

| API | Synchronisation |
|-----|----------------|
| **Vulkan** | `vkCmdPipelineBarrier2` — layout transitions + memory barriers explicites |
| **D3D12** | `ResourceBarrier` — resource state transitions |
| **Metal** | Rien — le driver gère automatiquement |

Le moteur ne devrait pas exposer de notion de barrière. À la place :

1. Le **render graph** déclare les accès via `AccessType` (information sémantique universelle)
2. Le **render graph** passe ces infos au `begin_render_pass()` du `CommandBuffer`
3. Le **backend** gère la synchronisation en interne :
   - Track le layout/state courant de chaque image
   - Émet les barrières/transitions nécessaires avant le render pass
   - Met à jour son état interne

```rust
// Trait CommandBuffer — pas de pipeline_barrier()
trait CommandBuffer {
    /// Begin a render pass.
    /// The backend handles all synchronization internally
    /// based on the access declarations.
    fn begin_render_pass(
        &self,
        desc: &RenderPassDesc,
        accesses: &[ResourceAccess],
    ) -> Result<()>;

    fn end_render_pass(&self) -> Result<()>;
}
```

Le tracking du layout sort du `RenderTarget` (render graph) et va dans le backend :

```rust
// Backend Vulkan — état interne, invisible du moteur
struct VulkanCommandBuffer {
    image_states: HashMap<VkImage, ImageState>,
}

struct ImageState {
    layout: vk::ImageLayout,
    stage: vk::PipelineStageFlags2,
    access: vk::AccessFlags2,
}
```

#### Bénéfices

1. **Le moteur reste API-agnostic** — pas de concept Vulkan dans l'abstraction
2. **Le render graph est simplifié** — il déclare les accès, le backend se débrouille
3. **Le backend Vulkan est simplifié** — plus de cache VkRenderPass, plus de VkFramebuffer, plus de compatibilité render pass/pipeline
4. **Portable** — un backend Metal n'a qu'à ignorer les barrières (le driver gère), un backend D3D12 émet ses propres `ResourceBarrier`
5. **Le pipeline déclare ses formats via `PipelineDesc`** — cohérent avec `VkPipelineRenderingCreateInfo`

### 13.4 Relation avec §12

Le §12 (tracking dynamique des layouts) est la **marche intermédiaire** vers cette migration :

1. **§12 maintenant** : on utilise encore VkRenderPass/VkFramebuffer, mais on force
   `initial_layout = final_layout` (pas de transition via render pass) et on fait tout
   avec `vkCmdPipelineBarrier`. Le tracking est dans le render graph.

2. **§13 ensuite** : on remplace `vkCmdBeginRenderPass` par `vkCmdBeginRendering`,
   on supprime la création de VkRenderPass/VkFramebuffer, on déplace le tracking dans
   le backend, et on utilise `vkCmdPipelineBarrier2`. Le modèle de barrières reste identique.

Implémenter §12 d'abord permet de valider le modèle de barrières dynamiques avant de
migrer l'API Vulkan. La migration §13 sera quasi-triviale une fois §12 en place.

### 13.5 Fichiers impactés (backend Vulkan uniquement)

| Fichier | Modification |
|---------|-------------|
| `vulkan.rs` | `create_render_pass()` → no-op (stocke un descripteur), `create_frame_buffer()` → no-op (stocke les ImageView), `begin_render_pass()` → `vkCmdBeginRendering` + barrières internes via `vkCmdPipelineBarrier2`, `create_pipeline()` → `VkPipelineRenderingCreateInfo` au lieu de VkRenderPass |
| `graphics_device.rs` (trait) | `begin_render_pass()` prend `&[ResourceAccess]` en paramètre additionnel, suppression de `pipeline_barrier()` si exposé |
| `render_graph/render_graph.rs` | Suppression du tracking de layout (déplacé dans le backend), `execute()` passe les accesses à `begin_render_pass()` |

**Le moteur (render graph, abstractions) ne change quasiment pas.** L'essentiel du travail est dans le backend Vulkan.
