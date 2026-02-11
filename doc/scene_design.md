# Scene & RenderInstance — Design Document

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-11
> **Statut** : Implémenté
> **Prérequis** : resource::Mesh (implémenté), resource::Material (implémenté), SceneManager (implémenté)
> **Dépendances** : `slotmap = "1"` (index générationnel)
> **Voir aussi** : [resource_mesh.md](resource_mesh.md), [materials_and_passes.md](materials_and_passes.md), [pipeline_data_binding.md](pipeline_data_binding.md)

---

## Table des matières

1. [Vision architecturale](#1-vision-architecturale)
2. [Philosophie moteur 3D vs moteur de jeu](#2-philosophie-moteur-3d-vs-moteur-de-jeu)
3. [Structure de la Scene](#3-structure-de-la-scene)
4. [RenderInstance](#4-renderinstance)
5. [RenderLOD et RenderSubMesh](#5-renderlod-et-rendersubmesh)
6. [AABB — Bounding Box](#6-aabb--bounding-box)
7. [Flags](#7-flags)
8. [Sélection de variant](#8-sélection-de-variant)
9. [Transmission des paramètres au shader](#9-transmission-des-paramètres-au-shader)
10. [Construction : from_mesh()](#10-construction--from_mesh)
11. [Boucle de rendu](#11-boucle-de-rendu)
12. [Itérations futures](#12-itérations-futures)

---

## 1. Vision architecturale

Le Galaxy3D Engine est un moteur de rendu 3D **bas niveau**. Il est conçu pour être utilisé par un moteur de jeu (futur projet) qui gère la logique de haut niveau (ECS, graphe de scène, physique, etc.).

La frontière est nette :

```
┌──────────────────────────────┐
│       MOTEUR DE JEU          │  ECS, nodes, graphe de scène,
│                              │  position (Vec3), rotation (Quat),
│                              │  logique de jeu, IA, physique
│                              │
│    Calcule la world_matrix   │
│    Pousse dans la scène 3D   │
└──────────┬───────────────────┘
           │  world_matrix + resource::Mesh
           ▼
┌──────────────────────────────┐
│       MOTEUR 3D (Galaxy3D)   │  RenderInstances, culling,
│                              │  tri par pipeline/material,
│                              │  batching, draw calls optimisés
│                              │
│    "Affiche-moi tout ça      │
│     le plus vite possible"   │
└──────────────────────────────┘
```

Le moteur 3D ne sait pas ce qu'est un "personnage" ou un "arbre". Il reçoit des objets affichables avec une matrice de transformation et les rend de manière optimisée.

---

## 2. Philosophie moteur 3D vs moteur de jeu

### Ce que le moteur 3D gère

- Affichage optimisé des RenderInstances
- Frustum culling (AABB vs plans de la caméra)
- Tri par pipeline/material pour réduire les state changes
- Multi-pass rendering
- Gestion des LODs (sélection du niveau de détail)
- Lumières et éclairage

### Ce que le moteur de jeu gèrera (futur)

- Graphe de scène (nœuds parent-enfant)
- Position (Vec3) et rotation (Quaternion) de chaque objet
- Calcul de la world_matrix finale (multiplication dans la hiérarchie)
- ECS (Entity Component System)
- Sélection du LOD basée sur la distance caméra

### La frontière

Le moteur de jeu manipule des objets logiques avec des positions/rotations, calcule les matrices finales, puis pousse des `RenderInstance` dans la `Scene` du moteur 3D. Le moteur 3D ne remonte jamais dans la logique du jeu.

---

## 3. Structure de la Scene

```rust
pub struct Scene {
    render_instances: SlotMap<RenderInstanceKey, RenderInstance>,
    // lights: Vec<Light>,    // Phase 2
    // camera: Camera,        // Phase 2
}
```

La Scene utilise un **SlotMap** (index générationnel) pour stocker les RenderInstances :

- **O(1) insert/remove** — pas de compaction ni de décalage d'indices
- **Clés stables** — `RenderInstanceKey` reste valide même après suppression d'autres instances
- **Cache-friendly** — les données restent contiguës en mémoire (itération rapide)
- **Sécurité générationnelle** — chaque clé contient un compteur de génération ; accéder à un slot réutilisé retourne `None` au lieu de données corrompues

`Scene::new()` est `pub(crate)` — une Scene ne peut être créée que via `SceneManager::create_scene()`.

### RenderInstanceKey

```rust
slotmap::new_key_type! { pub struct RenderInstanceKey; }
```

Clé opaque `(index: u32, version: u32)` — 8 bytes, `Copy + Clone + Eq + Hash`. C'est l'identifiant que le moteur de jeu stocke pour accéder, modifier ou supprimer un RenderInstance.

### SceneManager

Le `SceneManager` est un singleton géré par `Engine` (même pattern que `ResourceManager`). Il gère les scènes nommées, permettant plusieurs scènes actives simultanément (scène principale, UI overlay, minimap, etc.).

```rust
pub struct SceneManager {
    scenes: HashMap<String, Arc<Mutex<Scene>>>,
}
```

Les scènes sont wrappées dans `Arc<Mutex<Scene>>` pour un accès thread-safe partagé. `SceneManager::create_scene()` retourne un `Arc<Mutex<Scene>>` que l'appelant peut conserver.

---

## 4. RenderInstance

### Structure

```rust
pub struct RenderInstance {
    /// Shared vertex buffer (from Geometry, shared across all submeshes)
    vertex_buffer: Arc<dyn Buffer>,

    /// Shared index buffer (from Geometry, optional for non-indexed meshes)
    index_buffer: Option<Arc<dyn Buffer>>,

    /// LOD levels (index 0 = most detailed)
    lods: Vec<RenderLOD>,

    /// World transform matrix (final, pre-computed by game engine)
    world_matrix: Mat4,

    /// Bit flags (visibility, shadow casting, etc.)
    flags: u64,

    /// Axis-Aligned Bounding Box in local space
    bounding_box: AABB,

    /// Active pipeline variant index (default: 0)
    variant_index: usize,
}
```

### Pourquoi les buffers sont au niveau instance (pas submesh)

Dans la `Geometry` actuelle, **tous les submeshes partagent les mêmes vertex/index buffers**. Chaque submesh n'est qu'une plage (offset + count) dans ces buffers partagés. Stocker les buffers au niveau instance évite de dupliquer des `Arc` par submesh.

### Pourquoi world_matrix et pas position/rotation

Le moteur 3D est bas niveau. Il reçoit la matrice finale. Le calcul position → rotation → scale → matrice est la responsabilité du moteur de jeu. Cela permet au moteur 3D de rester simple et de ne pas avoir de dépendance sur un système de scène graph.

---

## 5. RenderLOD et RenderSubMesh

### RenderLOD

```rust
pub struct RenderLOD {
    /// Submeshes for this LOD level
    sub_meshes: Vec<RenderSubMesh>,
}
```

### RenderSubMesh

```rust
pub struct RenderSubMesh {
    /// Base vertex offset in the shared vertex buffer
    vertex_offset: u32,

    /// Number of vertices to draw
    vertex_count: u32,

    /// Base index offset in the shared index buffer
    index_offset: u32,

    /// Number of indices to draw (0 if non-indexed)
    index_count: u32,

    /// Primitive topology (TriangleList, LineList, etc.)
    topology: PrimitiveTopology,

    /// Renderer pipelines, one per pass of the selected variant
    passes: Vec<Arc<dyn RendererPipeline>>,

    /// Descriptor sets for texture binding in shaders
    descriptor_sets: Vec<Arc<dyn DescriptorSet>>,

    /// Material parameters for push constants
    params: Vec<(String, ParamValue)>,
}
```

### Origine des données (mapping resource → render)

| Champ RenderSubMesh      | Source                                                  |
|---------------------------|---------------------------------------------------------|
| `vertex_offset`           | `GeometrySubMesh.vertex_offset`                        |
| `vertex_count`            | `GeometrySubMesh.vertex_count`                         |
| `index_offset`            | `GeometrySubMesh.index_offset`                         |
| `index_count`             | `GeometrySubMesh.index_count`                          |
| `topology`                | `GeometrySubMesh.topology`                             |
| `passes`                  | `Material.pipeline → PipelineVariant.passes[*].renderer_pipeline` |
| `descriptor_sets`         | `Material.textures[*].texture → Texture.descriptor_set`|
| `params`                  | `Material.params`                                      |

### Draw call par submesh

Pour un submesh indexé :
```
draw_indexed(index_count, 1, index_offset, vertex_offset, 0)
```

Pour un submesh non-indexé :
```
draw(vertex_count, 1, vertex_offset, 0)
```

---

## 6. AABB — Bounding Box

### Choix : AABB

Les moteurs 3D modernes (Unreal, Unity, Godot) utilisent tous l'AABB comme volume de culling principal :

- **Plus tight** que la sphere pour les objets non-sphériques (bâtiments, véhicules, personnages)
- **Test frustum** : 6 tests point-vs-plan par AABB (rapide)
- **Recalcul world-space** : Transformer les 8 coins par la world_matrix, puis prendre min/max
- **Combinable** : Union de 2 AABB = min/max des composantes

### Structure

```rust
pub struct AABB {
    /// Minimum corner (x, y, z)
    pub min: Vec3,
    /// Maximum corner (x, y, z)
    pub max: Vec3,
}
```

### Stockage : local space

L'AABB est stockée en **local space** dans le RenderInstance. Elle est calculée une seule fois à la création. Pour le frustum culling, on transforme les 8 coins par la `world_matrix` et on recalcule une AABB world englobante.

### Génération de l'AABB

**Situation actuelle** : L'AABB ne peut pas être auto-générée depuis les resources existantes car les vertex data brutes ne sont pas conservées après upload GPU. Le trait `Buffer` n'a pas de `read()`.

**Pour l'instant** : L'AABB est passée en paramètre lors de la construction du RenderInstance.

**Itération future** : Calculer l'AABB pendant la création de `Geometry` (les vertex_data sont encore disponibles à ce moment-là) et la stocker dans `GeometryMesh`. Cela nécessite une modification de `resource::Geometry`.

---

## 7. Flags

```rust
/// Render instance flags (bitfield)
pub const FLAG_VISIBLE: u64       = 1 << 0;
pub const FLAG_CAST_SHADOW: u64   = 1 << 1;
pub const FLAG_RECEIVE_SHADOW: u64 = 1 << 2;
// Bits 3-63 réservés pour extensions futures
```

Le moteur de jeu contrôle la visibilité en modifiant les flags. Le moteur 3D respecte ces flags lors du rendu.

---

## 8. Sélection de variant

### Contexte

Un `resource::Pipeline` peut avoir plusieurs variants (ex: "static", "skinned", "shadow_caster"). Chaque variant a ses propres passes avec des GPU pipelines différentes.

### Phase 1 (actuelle)

Le `variant_index` est stocké dans le RenderInstance. Un seul variant actif par instance. Simple, suffisant pour du Gouraud mono-pass.

```rust
instance.variant_index = 0; // "default"
```

### Phase 2 (itération future)

La boucle de rendu choisit le variant selon le contexte :
- Shadow pass → variant "shadow_caster"
- Color pass → variant par défaut de l'instance
- Le RenderInstance stocke tous les variants disponibles, le rendu sélectionne

---

## 9. Transmission des paramètres au shader

### Phase 1 : Push Constants

Les `ParamValue` du Material sont transmis via `CommandList::push_constants()`.

Avantages :
- Rapide (pas d'allocation GPU)
- Change par draw call
- Adapté aux petites données (128-256 bytes max selon GPU)

Données typiques en push constants :
- `world_matrix` (Mat4, 64 bytes)
- `color` (Vec4, 16 bytes)
- `roughness`, `metallic` (Float, 4 bytes chacun)

### Phase 2 (itération future) : Uniform Buffers

Pour les données plus volumineuses ou partagées entre instances :
- Camera view/projection matrix (partagée par toute la scène)
- Light data (partagée par toute la scène)
- Material data complexes

Nécessitera un système de descriptor sets pour UBO (pas encore implémenté).

### Phase 3 (itération future) : Layout standardisé

Définir un mapping fixe des push constants :
```
Offset 0-63   : world_matrix (Mat4)
Offset 64-79  : color (Vec4)
Offset 80-83  : param_float_0
Offset 84-87  : param_float_1
...
```

---

## 10. Construction : from_mesh() et create_render_instance()

### API publique : Scene::create_render_instance()

```rust
impl Scene {
    pub fn create_render_instance(
        &mut self,
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        variant_index: usize,
    ) -> Result<RenderInstanceKey>
}
```

C'est le point d'entrée pour ajouter un objet à la scène. Retourne un `RenderInstanceKey` stable que l'appelant stocke pour accéder/modifier/supprimer l'instance.

### Construction interne : RenderInstance::from_mesh()

```rust
impl RenderInstance {
    pub(crate) fn from_mesh(
        mesh: &Mesh,
        world_matrix: Mat4,
        bounding_box: AABB,
        variant_index: usize,
    ) -> Result<Self>
}
```

`from_mesh()` est `pub(crate)` — il n'est pas appelable directement par le consommateur. Seul `Scene::create_render_instance()` l'utilise en interne pour construire l'instance puis l'insérer dans le SlotMap.

### Extraction des données

```
mesh.geometry()          → vertex_buffer, index_buffer
mesh.geometry()          → GeometryMesh → GeometryLOD → GeometrySubMesh (offsets, counts, topology)
mesh.lods()              → MeshLOD → SubMesh → material
material.pipeline()      → PipelineVariant[variant_index].passes → Arc<dyn RendererPipeline>
material.textures()      → MaterialTextureSlot → texture.descriptor_set()
material.params()        → Vec<(String, ParamValue)>
```

### Flags par défaut

```rust
flags: FLAG_VISIBLE  // visible par défaut
```

### Itération future : Builder pattern

Si la signature dépasse 4-5 paramètres :

```rust
RenderInstance::builder(&mesh)
    .world_matrix(matrix)
    .bounding_box(aabb)
    .variant_index(0)
    .flags(FLAG_VISIBLE | FLAG_CAST_SHADOW)
    .build()?
```

---

## 11. Boucle de rendu

### Phase 1 — Direct Draw (Gouraud, pas de culling)

```
pour chaque instance avec FLAG_VISIBLE:
    bind vertex_buffer
    bind index_buffer (si présent)
    pour chaque submesh du LOD 0:
        bind pipeline (passes[0])
        bind descriptor_sets
        push_constants(world_matrix + params)
        if index_count > 0:
            draw_indexed(index_count, 1, index_offset, vertex_offset, 0)
        else:
            draw(vertex_count, 1, vertex_offset, 0)
```

### Phase 2 — Frustum Culling

```
calculer frustum planes depuis camera VP matrix
pour chaque instance avec FLAG_VISIBLE:
    world_aabb = transform(instance.bounding_box, instance.world_matrix)
    si world_aabb intersecte frustum:
        ... draw (comme phase 1)
```

### Phase 3 — Tri par pipeline (réduction state changes)

```
trier instances par (pipeline, material, vertex_buffer)
pour chaque groupe de même pipeline:
    bind pipeline une seule fois
    pour chaque instance du groupe:
        ... draw
```

### Phase 4 — Instancing

```
grouper instances identiques (même mesh + material + pipeline)
pour chaque groupe:
    bind une seule fois
    draw_instanced(count = nombre d'instances)
    (matrices dans un storage buffer)
```

---

## 12. Itérations futures

### Priorité haute (nécessaire rapidement)

| Item | Description | Impact |
|------|-------------|--------|
| **AABB auto-calculée** | Calculer l'AABB dans `Geometry::from_desc()` et la stocker dans `GeometryMesh` | Évite de passer l'AABB manuellement |
| **Camera** | Structure Camera (view + projection matrices) dans la Scene | Nécessaire pour le frustum culling et le rendu correct |
| **Lights** | Structure Light (directional, point, spot) dans la Scene | Nécessaire pour un éclairage Gouraud réaliste |
| **Frustum culling** | Test AABB vs frustum planes | Performance : éviter de dessiner les objets hors champ |

### Priorité moyenne (optimisation)

| Item | Description | Impact |
|------|-------------|--------|
| **Tri par pipeline** | Trier les draw calls par pipeline/material | Réduire les state changes GPU |
| **Sélection de variant par la boucle de rendu** | Le rendu choisit le variant selon le pass en cours | Shadow maps, multi-pass rendering |
| **Push constants layout standardisé** | Mapping fixe des offsets | Éviter la sérialisation dynamique |
| **Uniform buffers (UBO)** | Descriptor sets pour données partagées (camera, lights) | Nécessaire pour passer camera/lights au shader |

### Priorité basse (évolution)

| Item | Description | Impact |
|------|-------------|--------|
| **Instancing** | Regrouper les instances identiques en un seul draw call | Performance massive pour scènes répétitives |
| **Builder pattern** | Remplacer `from_mesh()` si trop de paramètres | API plus flexible |
| **LOD auto-selection** | Sélection automatique du LOD selon distance caméra | Qualité vs performance |
| **Virtualisation des submeshes** | Types spécialisés (indexed vs non-indexed) | Éliminer les branches dans la boucle de rendu |
| **Spatial partitioning** | Octree/BVH pour culling hiérarchique | Scènes très larges |
| ~~**Multi-scène**~~ | ~~Scènes multiples simultanées (jeu + UI + minimap)~~ | **Implémenté** — SceneManager avec scènes nommées `Arc<Mutex<Scene>>` |

---

## Annexe A : Flux complet resource → render

```
ResourceManager
├── Geometry "character_geo"
│   ├── vertex_buffer: Arc<dyn Buffer>       ──┐
│   ├── index_buffer: Arc<dyn Buffer>          │
│   └── GeometryMesh "hero"                   │
│       └── GeometryLOD[0]                     │
│           ├── GeometrySubMesh "body"         │  GPU
│           │   ├── vertex_offset: 0           │  Objects
│           │   ├── vertex_count: 1000         │
│           │   ├── index_offset: 0            │
│           │   ├── index_count: 3000          │
│           │   └── topology: TriangleList     │
│           └── GeometrySubMesh "head"         │
│               ├── vertex_offset: 1000        │
│               └── ...                        │
│                                              │
├── Texture "hero_diffuse"                     │
│   ├── renderer_texture: Arc<dyn Texture>     │
│   └── descriptor_set: Arc<dyn DescriptorSet> │
│                                              │
├── Pipeline "gouraud"                         │
│   └── Variant "default"                      │
│       └── Pass 0                             │
│           └── renderer_pipeline: Arc<dyn RendererPipeline>
│                                              │
├── Material "hero_mat"                        │
│   ├── pipeline: Arc<Pipeline> ───────────────┤
│   ├── textures: [("diffuse", hero_diffuse)]  │
│   └── params: [("color", Vec4(1,1,1,1))]    │
│                                              │
└── Mesh "hero"                                │
    ├── geometry: Arc<Geometry> ───────────────┤
    └── lods: [MeshLOD { submeshes: [         │
            SubMesh { submesh_id: 0,           │
                      material: hero_mat }     │
        ]}]                                    │
                                               │
           ┌───────────────────────────────────┘
           │  RenderInstance::from_mesh()
           ▼
    RenderInstance
    ├── vertex_buffer ─────→ Geometry.vertex_buffer
    ├── index_buffer ──────→ Geometry.index_buffer
    ├── lods[0].sub_meshes[0]
    │   ├── vertex_offset: 0      ← GeometrySubMesh
    │   ├── vertex_count: 1000    ← GeometrySubMesh
    │   ├── index_offset: 0       ← GeometrySubMesh
    │   ├── index_count: 3000     ← GeometrySubMesh
    │   ├── topology: TriangleList ← GeometrySubMesh
    │   ├── passes[0] ────────→ Pipeline.variant[0].pass[0].renderer_pipeline
    │   ├── descriptor_sets[0] → Texture.descriptor_set
    │   └── params ────────────→ Material.params
    ├── world_matrix: Mat4         ← fournie par le game engine
    ├── flags: FLAG_VISIBLE        ← par défaut
    ├── bounding_box: AABB         ← fournie (itération: auto-calculée)
    └── variant_index: 0           ← par défaut
```

---

## Annexe B : Exemple d'utilisation (Gouraud + texture + couleur)

```rust
// === Création des resources ===

// 1. Geometry (GPU buffers)
let geometry = rm.create_geometry("cube_geo", GeometryDesc {
    vertex_data: cube_vertices,    // position + normal + uv
    index_data: Some(cube_indices),
    vertex_layout: VertexLayout { /* pos: Vec3, normal: Vec3, uv: Vec2 */ },
    index_type: IndexType::U16,
    meshes: vec![GeometryMeshDesc {
        name: "cube".to_string(),
        lods: vec![GeometryLODDesc {
            submeshes: vec![GeometrySubMeshDesc {
                name: "main".to_string(),
                vertex_offset: 0, vertex_count: 24,
                index_offset: 0, index_count: 36,
                topology: PrimitiveTopology::TriangleList,
            }],
        }],
    }],
})?;

// 2. Texture
let texture = rm.create_texture("checker", TextureDesc {
    texture: RenderTextureDesc { width: 256, height: 256, format: RGBA8, ... },
    layers: vec![LayerDesc { name: "base", data: Some(checker_pixels), ... }],
})?;

// 3. Pipeline (Gouraud shader)
let pipeline = rm.create_pipeline("gouraud", PipelineDesc {
    variants: vec![PipelineVariantDesc {
        name: "default".to_string(),
        passes: vec![PipelinePassDesc { /* vertex + fragment shader Gouraud */ }],
    }],
})?;

// 4. Material
let material = rm.create_material("cube_mat", MaterialDesc {
    pipeline: pipeline.clone(),
    textures: vec![MaterialTextureSlotDesc {
        name: "diffuse".to_string(),
        texture: texture.clone(),
        layer: None, region: None,
    }],
    params: vec![("color".to_string(), ParamValue::Vec4([1.0, 0.5, 0.2, 1.0]))],
})?;

// 5. Mesh
let mesh = rm.create_mesh("cube", MeshDesc {
    geometry: geometry.clone(),
    geometry_mesh: GeometryMeshRef::Name("cube".to_string()),
    lods: vec![MeshLODDesc {
        lod_index: 0,
        submeshes: vec![SubMeshDesc {
            submesh: GeometrySubMeshRef::Name("main".to_string()),
            material: material.clone(),
        }],
    }],
})?;

// === Création de la scène via SceneManager ===

let sm_arc = Engine::scene_manager()?;
let mut sm = sm_arc.lock().unwrap();
let scene_arc = sm.create_scene("main")?;

// === Ajout d'un RenderInstance à la scène ===

let aabb = AABB {
    min: Vec3::new(-0.5, -0.5, -0.5),
    max: Vec3::new(0.5, 0.5, 0.5),
};

let key = {
    let mut scene = scene_arc.lock().unwrap();
    scene.create_render_instance(
        &mesh,
        Mat4::from_translation(Vec3::new(0.0, 1.0, -5.0)),
        aabb,
        0, // variant "default"
    )?
};
// `key` est un RenderInstanceKey — le stocker pour accéder/modifier/supprimer l'instance

// === Accès via la clé ===

{
    let mut scene = scene_arc.lock().unwrap();

    // Lecture
    if let Some(instance) = scene.render_instance(key) {
        println!("LODs: {}", instance.lod_count());
    }

    // Modification
    if let Some(instance) = scene.render_instance_mut(key) {
        instance.set_world_matrix(Mat4::from_translation(Vec3::new(1.0, 2.0, -5.0)));
        instance.set_visible(false);
    }

    // Suppression
    scene.remove_render_instance(key);
}
```
