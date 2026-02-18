# Render Graph — Design Document

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-11 (mis à jour 2026-02-13)
> **Statut** : Implémenté — DAG structure, RenderTargetKind (Swapchain + Texture), API spécialisées
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

### Priorité basse

| Item | Description | Impact |
|------|-------------|--------|
| **Render-to-texture puis sampling** | Utiliser un target texture comme source dans un material | Post-processing, reflections, shadow maps |
| **MSAA targets** | Targets avec multisampling | Anti-aliasing |
| **MRT (Multiple Render Targets)** | Écrire dans plusieurs textures en un seul pass | Deferred rendering (G-buffer) |
| **Render Graph** | Graphe de dépendances entre passes de rendu (base implémentée) | Optimisation automatique de l'ordre et des barrières |
