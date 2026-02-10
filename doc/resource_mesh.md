# resource::Mesh — Design Document

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-10
> **Statut** : Design (non implémenté)
> **Prérequis** : resource::Material (non implémenté)
> **Voir aussi** : [materials_and_passes.md](materials_and_passes.md), [pipeline_data_binding.md](pipeline_data_binding.md)

---

## Table des matières

1. [Motivation](#1-motivation)
2. [Position dans l'architecture](#2-position-dans-larchitecture)
3. [Rappel : resource::Geometry](#3-rappel--resourcegeometry)
4. [Design de resource::Mesh](#4-design-de-resourcemesh)
5. [Structure détaillée](#5-structure-détaillée)
6. [Relation avec resource::Pipeline (variants)](#6-relation-avec-resourcepipeline-variants)
7. [Gestion des LODs et materials](#7-gestion-des-lods-et-materials)
8. [Pattern helper : mapping par nom de submesh](#8-pattern-helper--mapping-par-nom-de-submesh)
9. [Utilisation dans le ResourceManager](#9-utilisation-dans-le-resourcemanager)
10. [Exemples d'utilisation](#10-exemples-dutilisation)
11. [Perspectives : Scene](#11-perspectives--scene)

---

## 1. Motivation

Aujourd'hui, pour afficher un objet, le code utilisateur doit manuellement :

1. Récupérer la Geometry et ses buffers
2. Récupérer chaque submesh et ses paramètres de draw
3. Récupérer le Pipeline et le binder
4. Récupérer les Textures et binder les descriptor sets
5. Pousser les push constants
6. Émettre les draw calls

Ce processus est **entièrement manuel**. Il n'y a aucune abstraction qui relie la forme (Geometry) à l'apparence (Material/Pipeline).

**resource::Mesh** comble ce manque : c'est une ressource qui associe une GeometryMesh à des Materials, formant un **objet prêt à dessiner**.

---

## 2. Position dans l'architecture

```
Ressources fondamentales (données brutes) :
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│ resource::Geometry│  │resource::Texture  │  │resource::Pipeline │
│                  │  │                  │  │                  │
│ Vertices, indices│  │ Images GPU       │  │ Shaders, states  │
│ Meshes, LODs     │  │ Layers, regions  │  │ Variants, passes │
│ SubMeshes        │  │                  │  │                  │
└──────────────────┘  └──────────────────┘  └──────────────────┘
         │                     │                     │
         │                     ▼                     │
         │            ┌──────────────────┐           │
         │            │resource::Material │◄──────────┘
         │            │                  │
         │            │ Textures + params│
         │            │ → réf. Pipeline  │
         │            └──────────────────┘
         │                     │
         ▼                     ▼
    ┌─────────────────────────────────┐
    │        resource::Mesh           │
    │                                 │
    │  GeometryMesh + Material/submesh│
    │  = "objet prêt à dessiner"     │
    └─────────────────────────────────┘
                   │
                   ▼
          ┌─────────────────┐
          │   Scene (futur) │
          │                 │
          │ Objets 3D avec  │
          │ transform, etc. │
          └─────────────────┘
```

**Séparation des responsabilités :**

| Resource | Responsabilité | Qui la définit |
|----------|---------------|----------------|
| **Geometry** | Forme (vertices, indices, submeshes, LODs) | Le modélisateur 3D |
| **Texture** | Images GPU | L'artiste textures |
| **Pipeline** | Programme GPU (shaders + render states + variants) | Le programmeur graphique |
| **Material** | Apparence (textures + paramètres + réf. Pipeline) | L'artiste matériaux |
| **Mesh** | Forme + apparence combinées (GeometryMesh + Materials) | L'assembleur (artiste ou outil) |

---

## 3. Rappel : resource::Geometry

Une Geometry contient des données géométriques brutes, organisées hiérarchiquement :

```
Geometry "character"
├── vertex_buffer (GPU)
├── index_buffer (GPU)
├── GeometryMesh "body"
│   ├── GeometryLOD 0 (haute qualité)
│   │   ├── GeometrySubMesh "head"   (offset/count dans les buffers)
│   │   ├── GeometrySubMesh "torso"
│   │   └── GeometrySubMesh "legs"
│   ├── GeometryLOD 1 (qualité moyenne)
│   │   ├── GeometrySubMesh "head"
│   │   └── GeometrySubMesh "body"   (torso+legs fusionnés)
│   └── GeometryLOD 2 (basse qualité)
│       └── GeometrySubMesh "whole"  (tout fusionné)
└── GeometryMesh "weapon"
    └── GeometryLOD 0
        └── GeometrySubMesh "blade"
```

**Important** : la Geometry ne contient aucune information visuelle. Pas de couleur, pas de texture, pas de shader.

---

## 4. Design de resource::Mesh

### Principe fondamental

Un `resource::Mesh` est une **GeometryMesh habillée** : chaque submesh de chaque LOD est explicitement associé à un Material.

### Principes de design

1. **Explicite** : chaque submesh a **son** material, déclaré sans ambiguïté
2. **Libre** : aucune contrainte de nommage imposée par le système
3. **Par LOD** : chaque LOD déclare ses propres associations submesh/material (les LODs peuvent avoir des submeshes différents)
4. **Material-agnostic au niveau Pipeline** : le Material connaît son Pipeline, le Mesh ne connaît pas les variants

---

## 5. Structure détaillée

```rust
/// A renderable mesh: geometry + materials per submesh
pub struct Mesh {
    geometry: Arc<Geometry>,
    geometry_mesh_name: String,     // which GeometryMesh within the Geometry
    pipeline: Arc<Pipeline>,        // the Pipeline family (shared by all materials)
    lods: Vec<MeshLOD>,
}

/// Material assignments for a specific LOD level
pub struct MeshLOD {
    /// Each entry maps a GeometrySubMesh to a Material
    /// Order matches the submesh order in the corresponding GeometryLOD
    submesh_materials: Vec<SubMeshMaterial>,
}

/// A submesh paired with its material
pub struct SubMeshMaterial {
    submesh_name: String,           // references a GeometrySubMesh by name
    material: Arc<Material>,
}
```

### Descripteur de création

```rust
pub struct MeshDesc {
    pub geometry: Arc<Geometry>,
    pub geometry_mesh_name: String,
    pub pipeline: Arc<Pipeline>,
    pub lods: Vec<MeshLODDesc>,
}

pub struct MeshLODDesc {
    pub lod_index: usize,
    pub submesh_materials: Vec<SubMeshMaterialDesc>,
}

pub struct SubMeshMaterialDesc {
    pub submesh_name: String,
    pub material: Arc<Material>,
}
```

---

## 6. Relation avec resource::Pipeline (variants)

### Le Material ne connaît pas les variants

Un Material référence un Pipeline (la "famille de shaders"). Il ne choisit **pas** de variant.

```
Material "red_metal"
  → Pipeline "PBR"          (pas "PBR variant 0" ni "PBR variant 1")
  → Texture albedo: "red_metal_albedo"
  → Texture normal: "red_metal_normal"
  → params: roughness=0.3, metalness=0.9
```

### Le Mesh non plus

Le Mesh combine Geometry + Materials. Il n'a pas conscience des variants.

### Qui choisit le variant ?

C'est le **moteur de rendu** (futur système Scene/Renderer) qui, au moment du draw, sélectionne le variant approprié selon le contexte :

| Contexte de l'objet | Variant sélectionné |
|---------------------|---------------------|
| Objet statique | variant "static" (vertex shader simple) |
| Objet animé (skinning) | variant "skinned" (vertex shader avec bones) |
| Instanciation GPU | variant "instanced" (vertex shader avec instance data) |
| Shadow pass | variant "shadow" (fragment shader minimal) |

**Résultat** : un artiste crée **un** Material "red_metal". Ce même Material fonctionne sur un objet statique ET un objet animé, sans duplication.

---

## 7. Gestion des LODs et materials

### Chaque LOD déclare ses propres associations

Chaque LOD peut avoir des submeshes différents (nombre, noms, granularité). Le mapping material est donc **explicitement déclaré par LOD** :

```rust
// LOD 0 : 3 submeshes, 3 materials
MeshLODDesc {
    lod_index: 0,
    submesh_materials: vec![
        SubMeshMaterialDesc { submesh_name: "head",  material: skin.clone() },
        SubMeshMaterialDesc { submesh_name: "torso", material: armor.clone() },
        SubMeshMaterialDesc { submesh_name: "legs",  material: pants.clone() },
    ],
}

// LOD 1 : 2 submeshes (legs fusionnés dans body)
MeshLODDesc {
    lod_index: 1,
    submesh_materials: vec![
        SubMeshMaterialDesc { submesh_name: "head", material: skin.clone() },
        SubMeshMaterialDesc { submesh_name: "body", material: armor.clone() },
    ],
}

// LOD 2 : 1 submesh (tout fusionné, material simplifié)
MeshLODDesc {
    lod_index: 2,
    submesh_materials: vec![
        SubMeshMaterialDesc { submesh_name: "whole", material: simplified.clone() },
    ],
}
```

### Pourquoi pas un mapping partagé automatique ?

On aurait pu mapper les materials par nom de submesh dans un HashMap unique (tous les LODs partagent le même mapping). Mais cette approche :

- **Force des conventions de nommage** sur les submeshes
- **Réduit la liberté** : impossible d'avoir deux submeshes de même nom dans des LODs différents avec des materials différents
- **Introduit de l'implicite** là où l'explicite est préférable

Le design explicite par LOD est légèrement plus verbeux dans le cas commun (même materials partout), mais **sans aucune contrainte cachée**.

---

## 8. Pattern helper : mapping par nom de submesh

Pour le cas commun (99%) où tous les LODs partagent les mêmes materials, un **helper de construction** peut être fourni. C'est un outil de convenience, **pas** le design de base.

### Concept

```rust
/// Helper: build a Mesh by mapping submesh names to materials.
/// All submeshes with the same name across all LODs get the same material.
/// Submesh names not found in the map are skipped (or error).
pub fn mesh_from_name_mapping(
    geometry: Arc<Geometry>,
    geometry_mesh_name: &str,
    pipeline: Arc<Pipeline>,
    name_to_material: &HashMap<String, Arc<Material>>,
) -> Result<Mesh> { ... }
```

### Utilisation

```rust
let mapping = HashMap::from([
    ("head".to_string(),  skin.clone()),
    ("torso".to_string(), armor.clone()),
    ("legs".to_string(),  pants.clone()),
]);

// Construit automatiquement les MeshLODs pour tous les LODs
// en cherchant chaque submesh_name dans le mapping
let mesh = mesh_from_name_mapping(
    character_geom,
    "body",
    pbr_pipeline,
    &mapping,
)?;
```

### Ce que fait le helper

1. Parcourt chaque LOD de la GeometryMesh ciblée
2. Pour chaque submesh, cherche son nom dans le mapping
3. Construit les `MeshLODDesc` automatiquement
4. Si un nom de submesh n'est pas dans le mapping → erreur (ou skip configurable)

**Important** : ce helper est un **outil utilisateur**, pas une partie du design de `resource::Mesh`. L'utilisateur reste libre de construire ses `MeshDesc` manuellement pour les cas spéciaux.

---

## 9. Utilisation dans le ResourceManager

```rust
// ResourceManager
pub struct ResourceManager {
    textures: HashMap<String, Arc<Texture>>,
    geometries: HashMap<String, Arc<Geometry>>,
    pipelines: HashMap<String, Arc<Pipeline>>,
    materials: HashMap<String, Arc<Material>>,   // NEW
    meshes: HashMap<String, Arc<Mesh>>,          // NEW
}

impl ResourceManager {
    // Creation
    pub fn create_material(&mut self, name: String, desc: MaterialDesc) -> Result<Arc<Material>>;
    pub fn create_mesh(&mut self, name: String, desc: MeshDesc) -> Result<Arc<Mesh>>;

    // Access
    pub fn material(&self, name: &str) -> Option<&Arc<Material>>;
    pub fn mesh(&self, name: &str) -> Option<&Arc<Mesh>>;

    // Removal
    pub fn remove_material(&mut self, name: &str) -> bool;
    pub fn remove_mesh(&mut self, name: &str) -> bool;

    // Count
    pub fn material_count(&self) -> usize;
    pub fn mesh_count(&self) -> usize;
}
```

---

## 10. Exemples d'utilisation

### Cas simple : un quad texturé

```rust
// 1. Create Geometry
let quad_geom = rm.create_geometry("quad", GeometryDesc { ... })?;

// 2. Create Pipeline
let pbr_pipeline = rm.create_pipeline("PBR", PipelineDesc { ... })?;

// 3. Create Texture
let wood_tex = rm.create_texture("wood", TextureDesc { ... })?;

// 4. Create Material
let wood_mat = rm.create_material("wood_floor", MaterialDesc {
    pipeline: pbr_pipeline.clone(),
    textures: vec![("albedo", wood_tex.clone())],
    params: vec![("roughness", 0.7), ("metalness", 0.0)],
})?;

// 5. Create Mesh (geometry + material)
let quad_mesh = rm.create_mesh("floor_tile", MeshDesc {
    geometry: quad_geom.clone(),
    geometry_mesh_name: "default".to_string(),
    pipeline: pbr_pipeline.clone(),
    lods: vec![
        MeshLODDesc {
            lod_index: 0,
            submesh_materials: vec![
                SubMeshMaterialDesc {
                    submesh_name: "main".to_string(),
                    material: wood_mat.clone(),
                },
            ],
        },
    ],
})?;
```

### Cas avancé : personnage toon avec LODs différents

```rust
// Materials
let toon_skin    = rm.create_material("toon_skin", ...)?;
let toon_eyes    = rm.create_material("toon_eyes", ...)?;    // emissive, animated UVs
let toon_armor   = rm.create_material("toon_armor", ...)?;
let toon_outline = rm.create_material("toon_outline", ...)?;  // backface, thick lines
let toon_simple  = rm.create_material("toon_simple", ...)?;   // baked atlas

// Mesh with per-LOD material assignments
let character = rm.create_mesh("hero", MeshDesc {
    geometry: character_geom.clone(),
    geometry_mesh_name: "body".to_string(),
    pipeline: toon_pipeline.clone(),
    lods: vec![
        // LOD 0: full detail (4 submeshes, 4 materials)
        MeshLODDesc {
            lod_index: 0,
            submesh_materials: vec![
                SubMeshMaterialDesc { submesh_name: "head".into(),    material: toon_skin.clone() },
                SubMeshMaterialDesc { submesh_name: "eyes".into(),    material: toon_eyes.clone() },
                SubMeshMaterialDesc { submesh_name: "body".into(),    material: toon_armor.clone() },
                SubMeshMaterialDesc { submesh_name: "outline".into(), material: toon_outline.clone() },
            ],
        },
        // LOD 1: medium (2 submeshes, eyes merged, no outline)
        MeshLODDesc {
            lod_index: 1,
            submesh_materials: vec![
                SubMeshMaterialDesc { submesh_name: "head".into(), material: toon_skin.clone() },
                SubMeshMaterialDesc { submesh_name: "body".into(), material: toon_armor.clone() },
            ],
        },
        // LOD 2: low (1 submesh, everything baked)
        MeshLODDesc {
            lod_index: 2,
            submesh_materials: vec![
                SubMeshMaterialDesc { submesh_name: "whole".into(), material: toon_simple.clone() },
            ],
        },
    ],
})?;
```

---

## 11. Perspectives : Scene

`resource::Mesh` reste une **ressource** — une définition réutilisable stockée dans le ResourceManager.

L'étape suivante sera le système **Scene**, qui représente une scène 3D concrète. Dans la Scene, chaque objet affichable référencera les resources du ResourceManager :

```
Scene
├── SceneObject "hero_1"
│   ├── transform: position, rotation, scale
│   ├── mesh: → resource::Mesh "hero"
│   └── (autres composants : physics, animation, ...)
├── SceneObject "floor"
│   ├── transform: ...
│   ├── mesh: → resource::Mesh "floor_tile"
│   └── ...
└── ...
```

Le système Scene sera responsable de :
- Choisir le LOD approprié (distance à la caméra)
- Sélectionner le variant du Pipeline (statique/animé/instancié)
- Émettre les draw calls avec les bons bindings

Mais ça, c'est pour plus tard. `resource::Mesh` prépare le terrain en fournissant des objets "prêts à dessiner" que la Scene n'aura qu'à orchestrer.
