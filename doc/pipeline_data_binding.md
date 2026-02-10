# Binding de Données dans les Pipelines

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-10
> **Objectif** : Documenter comment les données (matrices, couleurs, textures, paramètres) sont transmises aux shaders via les pipelines.
> **Voir aussi** : [materials_and_passes.md](materials_and_passes.md) pour l'architecture Material/Pipeline/Passes.

---

## Table des matières

1. [Vue d'ensemble](#1-vue-densemble)
2. [Les 3 fréquences de mise à jour](#2-les-3-fréquences-de-mise-à-jour)
3. [Les 3 mécanismes GPU (Vulkan)](#3-les-3-mécanismes-gpu-vulkan)
4. [Organisation des Descriptor Sets](#4-organisation-des-descriptor-sets)
5. [Push Constants — données ultra-rapides](#5-push-constants--données-ultra-rapides)
6. [Uniform Buffers (UBO) — données structurées](#6-uniform-buffers-ubo--données-structurées)
7. [Textures et Samplers](#7-textures-et-samplers)
8. [Le Material comme pont entre Pipeline et données](#8-le-material-comme-pont-entre-pipeline-et-données)
9. [Flux de rendu complet avec binding](#9-flux-de-rendu-complet-avec-binding)
10. [Recommandations pour Galaxy3D](#10-recommandations-pour-galaxy3d)

---

## 1. Vue d'ensemble

Un pipeline GPU (vertex shader + fragment shader + configuration) ne peut pas fonctionner seul. Il a besoin de **données** pour travailler :

- **Où est la caméra ?** → matrice de vue
- **Quelle perspective ?** → matrice de projection
- **Où est l'objet ?** → matrice de transformation (model)
- **De quelle couleur ?** → paramètres du matériau
- **Quelle texture ?** → textures bindées
- **Quelles lumières ?** → positions, couleurs, intensités

Ces données doivent être transmises du CPU (code Rust) vers le GPU (shaders GLSL/SPIR-V). C'est le **binding de données**.

### Analogie simple

```
Pipeline = la recette de cuisine (comment préparer le plat)
Données  = les ingrédients (ce qu'on met dans le plat)

Sans ingrédients, la recette ne produit rien.
Deux plats peuvent utiliser la même recette avec des ingrédients différents.
```

---

## 2. Les 3 fréquences de mise à jour

Toutes les données ne changent pas au même rythme. On les classe en **3 fréquences** :

### Per-Frame (une fois par frame)

Données partagées par **tous** les objets dessinés dans la frame. Elles ne changent qu'une seule fois par image.

| Donnée | Type GLSL | Taille | Description |
|--------|-----------|--------|-------------|
| View matrix | mat4 | 64 octets | Position et orientation de la caméra |
| Projection matrix | mat4 | 64 octets | Projection perspective ou orthographique |
| ViewProjection matrix | mat4 | 64 octets | Pré-multiplié (view × proj) — optimisation |
| Camera position | vec3 | 12 octets | Position monde de la caméra (reflets spéculaires) |
| Time | float | 4 octets | Temps écoulé (animations shader) |
| Screen resolution | vec2 | 8 octets | Largeur × hauteur en pixels |
| Ambient light | vec3 | 12 octets | Lumière ambiante globale |
| Light count | int | 4 octets | Nombre de lumières actives |
| Light array | struct[] | variable | Positions, couleurs, intensités, types |

**Exemple GLSL :**

```glsl
// Set 0, Binding 0 — Per-Frame UBO
layout(set = 0, binding = 0) uniform PerFrameData {
    mat4 view;
    mat4 projection;
    mat4 view_projection;
    vec3 camera_position;
    float time;
    vec2 screen_resolution;
    float _padding1;
    float _padding2;
    vec3 ambient_light;
    int light_count;
};

// Set 0, Binding 1 — Lumières
struct Light {
    vec4 position;       // xyz = position, w = type (0=dir, 1=point, 2=spot)
    vec4 color;          // xyz = couleur, w = intensité
    vec4 direction;      // xyz = direction (spot/dir), w = range
    vec4 params;         // x = inner_cone, y = outer_cone, zw = reserved
};

layout(set = 0, binding = 1) uniform LightData {
    Light lights[16];    // max 16 lumières
};
```

### Per-Material (change par matériau)

Données partagées par tous les objets utilisant le **même matériau**. Changent quand on passe d'un matériau à un autre.

| Donnée | Type GLSL | Taille | Description |
|--------|-----------|--------|-------------|
| Diffuse/Albedo color | vec4 | 16 octets | Couleur de base |
| Specular color | vec3 | 12 octets | Couleur des reflets |
| Roughness | float | 4 octets | Rugosité PBR (0 = miroir, 1 = mat) |
| Metallic | float | 4 octets | Métallicité PBR (0 = diélectrique, 1 = métal) |
| Emission | vec3 | 12 octets | Auto-illumination |
| Emission intensity | float | 4 octets | Force de l'émission |
| UV tiling | vec2 | 8 octets | Répétition des UV (scale) |
| UV offset | vec2 | 8 octets | Décalage des UV |
| Alpha cutoff | float | 4 octets | Seuil pour l'alpha test |
| Normal map strength | float | 4 octets | Intensité de la normal map |
| Diffuse texture | sampler2D | — | Texture albedo |
| Normal texture | sampler2D | — | Normal map |
| Roughness texture | sampler2D | — | Roughness/metallic map |
| Emission texture | sampler2D | — | Emission map |

**Exemple GLSL :**

```glsl
// Set 1, Binding 0 — Per-Material UBO
layout(set = 1, binding = 0) uniform MaterialData {
    vec4 base_color;         // rgba
    vec3 specular_color;
    float roughness;
    float metallic;
    float emission_intensity;
    vec2 uv_tiling;
    vec2 uv_offset;
    float alpha_cutoff;
    float normal_strength;
};

// Set 1, Bindings 1-4 — Textures du matériau
layout(set = 1, binding = 1) uniform sampler2D diffuse_map;
layout(set = 1, binding = 2) uniform sampler2D normal_map;
layout(set = 1, binding = 3) uniform sampler2D roughness_metallic_map;
layout(set = 1, binding = 4) uniform sampler2D emission_map;
```

### Per-Object (change par draw call)

Données uniques à **chaque objet** dessiné. Changent à chaque draw call.

| Donnée | Type GLSL | Taille | Description |
|--------|-----------|--------|-------------|
| Model matrix | mat4 | 64 octets | Position/rotation/scale de l'objet dans le monde |
| Normal matrix | mat4 | 64 octets | Inverse transpose de la model matrix (normales) |
| Object ID | uint | 4 octets | Identifiant unique (picking, sélection) |
| Tint color | vec4 | 16 octets | Couleur de teinte per-instance |
| Bone matrices | mat4[] | variable | Matrices d'animation squelettique |

**Exemple — via Push Constants (rapide) :**

```glsl
// Push constants — données per-object ultra-rapides
layout(push_constant) uniform PushConstants {
    mat4 model;          // 64 octets
    // On pourrait ajouter : uint object_id (4 octets), etc.
    // Limite : 128 octets garantis par Vulkan
};
```

**Exemple — via UBO (si les données sont trop grosses pour les push constants) :**

```glsl
// Set 2, Binding 0 — Per-Object UBO (alternative aux push constants)
layout(set = 2, binding = 0) uniform ObjectData {
    mat4 model;
    mat4 normal_matrix;
    uint object_id;
    vec4 tint_color;
};

// Set 2, Binding 1 — Bone matrices (skinning)
layout(set = 2, binding = 1) uniform BoneData {
    mat4 bones[64];       // max 64 bones
};
```

---

## 3. Les 3 mécanismes GPU (Vulkan)

Vulkan offre 3 moyens pour envoyer des données aux shaders, chacun avec ses caractéristiques :

### Push Constants

| Propriété | Valeur |
|-----------|--------|
| **Taille max** | 128 octets garantis (souvent 256 sur les GPU modernes) |
| **Vitesse** | Le plus rapide — pas d'allocation mémoire, directement dans le command buffer |
| **Quand ça change** | À chaque draw call (idéal pour per-object) |
| **Limitation** | Très petit — juste assez pour une mat4 (64 octets) + quelques scalaires |
| **Coût CPU** | Quasi nul — un simple `vkCmdPushConstants` |

**Cas d'usage idéal** : Model matrix, object ID, tint color — tout ce qui change à chaque draw call et tient dans ~128 octets.

**Déjà implémenté** dans Galaxy3D via `cmd_push_constants()`.

### Uniform Buffers (UBO) via Descriptor Sets

| Propriété | Valeur |
|-----------|--------|
| **Taille max** | 64 KB minimum garanti (souvent 256 KB+) |
| **Vitesse** | Rapide — données en GPU memory, accès optimisé |
| **Quand ça change** | Per-frame ou per-material (on met à jour le buffer une fois, puis on bind) |
| **Limitation** | Plus lent à changer que les push constants — bind de descriptor set |
| **Coût CPU** | Upload des données + bind du descriptor set |

**Cas d'usage idéal** : Données de caméra/lumières (per-frame), paramètres du matériau (per-material).

### Textures / Samplers via Descriptor Sets

| Propriété | Valeur |
|-----------|--------|
| **Taille** | Arbitraire (images GPU) |
| **Vitesse** | Accès via texture cache du GPU — très rapide en lecture |
| **Quand ça change** | Per-material |
| **Limitation** | Ne contient que des images (pas des données arbitraires) |

**Cas d'usage idéal** : Textures du matériau (albedo, normal, roughness, emission).

### Tableau comparatif

| Mécanisme | Taille | Vitesse de changement | Usage type |
|-----------|--------|----------------------|------------|
| Push Constants | ~128 octets | Extrêmement rapide | Per-object (model matrix) |
| UBO (Descriptor Set) | ~64 KB | Rapide | Per-frame (camera), Per-material (params) |
| Textures (Descriptor Set) | Illimité | Rapide | Per-material (images) |

---

## 4. Organisation des Descriptor Sets

### Stratégie par fréquence de changement

L'organisation standard (utilisée par la majorité des moteurs modernes) est de répartir les descriptor sets **par fréquence de mise à jour** :

```
Descriptor Set 0 : Per-Frame
  ├── binding 0 : UBO PerFrameData (camera, time, screen...)
  └── binding 1 : UBO LightData (tableau de lumières)

Descriptor Set 1 : Per-Material
  ├── binding 0 : UBO MaterialData (couleurs, roughness, metallic...)
  ├── binding 1 : sampler2D (diffuse texture)
  ├── binding 2 : sampler2D (normal map)
  ├── binding 3 : sampler2D (roughness/metallic map)
  └── binding 4 : sampler2D (emission map)

Descriptor Set 2 : Per-Object (optionnel, si push constants insuffisantes)
  ├── binding 0 : UBO ObjectData (model matrix, normal matrix, object ID)
  └── binding 1 : UBO BoneData (skinning matrices)

Push Constants : Per-Object (rapide, petites données)
  └── model matrix (64 octets)
```

### Pourquoi cet ordre ?

L'avantage est de **minimiser les rebinds** pendant le rendu :

```
Début de frame :
  Bind Set 0 (camera, lumières)              ← 1 seul bind par frame

  Pour chaque matériau :
    Bind Set 1 (paramètres, textures)        ← 1 bind par matériau

    Pour chaque objet avec ce matériau :
      Push constants (model matrix)          ← ultra rapide, par objet
      Draw
```

Le Set 0 n'est bindé **qu'une seule fois par frame**. Le Set 1 n'est rebindé que quand on change de matériau. Seules les push constants changent à chaque draw call.

### Impact sur les performances

```
Sans cette organisation (tout dans un seul set) :
  Frame avec 1000 objets, 50 matériaux :
  → 1000 binds de descriptor set complets (UBO + textures + tout)

Avec cette organisation (sets par fréquence) :
  → 1 bind du Set 0 (per-frame)
  → 50 binds du Set 1 (per-material)
  → 1000 push constants (quasi gratuits)
  → Total : 51 binds + 1000 push constants au lieu de 1000 binds complets
```

---

## 5. Push Constants — données ultra-rapides

### Comment ça fonctionne

Les push constants sont écrites **directement dans le command buffer** Vulkan. Pas besoin de :
- Créer un buffer GPU
- Mapper de la mémoire
- Créer un descriptor set

On appelle simplement `vkCmdPushConstants` avec un pointeur vers les données.

### Layout typique

```
Push Constants (128 octets max garantis) :
  ┌────────────────────────────┐
  │ mat4 model       (64 oct) │  ← transformation de l'objet
  │ mat4 normal_mat  (64 oct) │  ← pour les normales
  └────────────────────────────┘
          = 128 octets (pile la limite garantie)

Variante compacte :
  ┌────────────────────────────┐
  │ mat4 model       (64 oct) │
  │ uint object_id   (4 oct)  │
  │ vec4 tint_color  (16 oct) │
  │ float custom_1   (4 oct)  │
  │ float custom_2   (4 oct)  │
  │ ... (jusqu'à 128 total)   │
  └────────────────────────────┘
```

### Quand préférer les push constants vs un UBO per-object ?

| Critère | Push Constants | UBO Per-Object (Set 2) |
|---------|---------------|----------------------|
| Taille des données | ≤ 128 octets | > 128 octets |
| Besoin de skinning | Non | Oui (64 mat4 = 4 KB) |
| Vitesse | Plus rapide | Légèrement plus lent |
| Complexité code | Simple | Gestion de buffers + descriptors |

**Recommandation** : Utiliser les push constants pour la model matrix et quelques scalaires. Basculer vers un UBO seulement pour le skinning ou des données per-object volumineuses.

---

## 6. Uniform Buffers (UBO) — données structurées

### Création et mise à jour

Un UBO suit ce cycle de vie :

```
1. Création (une seule fois au démarrage ou au chargement) :
   → Allouer un buffer GPU avec BufferUsage::Uniform
   → Créer un descriptor set qui pointe vers ce buffer

2. Mise à jour (per-frame ou quand le matériau change) :
   → Mapper le buffer en mémoire CPU
   → Écrire les données (memcpy)
   → Unmapper

3. Binding (pendant le rendu) :
   → vkCmdBindDescriptorSets avec le set qui contient le UBO

4. Utilisation dans le shader :
   → Le shader accède aux données via le layout uniform
```

### Alignement mémoire (std140)

Les UBO en Vulkan suivent les règles d'alignement **std140**. Attention aux pièges :

```glsl
// ATTENTION — les règles std140 :
layout(std140, set = 0, binding = 0) uniform PerFrameData {
    mat4 view;              // offset 0,   size 64, align 16
    mat4 projection;        // offset 64,  size 64, align 16
    vec3 camera_position;   // offset 128, size 12, align 16
    float time;             // offset 140, size 4,  align 4  ← remplit le vec3
    vec2 screen_resolution; // offset 144, size 8,  align 8
    float _pad1;            // offset 152, padding pour alignment
    float _pad2;            // offset 156, padding pour alignment
    vec3 ambient_light;     // offset 160, size 12, align 16
    int light_count;        // offset 172, size 4,  align 4
};
// Taille totale : 176 octets
```

**Piège classique** : Un `vec3` occupe 12 octets mais s'aligne sur 16. Le `float` qui suit peut se glisser dans le trou de 4 octets, mais pas un `vec2`.

**Astuce** : En Rust, définir la structure avec `#[repr(C)]` et ajouter des paddings explicites :

```rust
#[repr(C)]
struct PerFrameData {
    view: [[f32; 4]; 4],              // mat4
    projection: [[f32; 4]; 4],        // mat4
    camera_position: [f32; 3],        // vec3
    time: f32,                        // float (remplit le vec3)
    screen_resolution: [f32; 2],      // vec2
    _pad: [f32; 2],                   // padding
    ambient_light: [f32; 3],          // vec3
    light_count: i32,                 // int
}
```

---

## 7. Textures et Samplers

### Binding dans les Descriptor Sets

Les textures sont bindées dans les descriptor sets, typiquement dans le Set 1 (per-material) :

```
Descriptor Set 1 :
  binding 0 : UBO MaterialData
  binding 1 : Combined Image Sampler → diffuse texture
  binding 2 : Combined Image Sampler → normal map
  binding 3 : Combined Image Sampler → roughness/metallic map
  binding 4 : Combined Image Sampler → emission map
```

### Textures manquantes

Un matériau ne fournit pas toujours toutes les textures. Solutions courantes :

| Approche | Description |
|----------|-------------|
| **Texture par défaut** | Binder une texture 1×1 blanche/noire/flat quand la texture est absente |
| **Flag dans le UBO** | Un booléen `has_normal_map` dans le MaterialData |
| **Specialization constants** | Variantes shader compilées avec/sans certaines textures |

**Recommandation** : Texture par défaut (le plus simple). Créer au démarrage du moteur :

```
default_white   : 1×1 pixel blanc  (1, 1, 1, 1)  → pour diffuse absent
default_black   : 1×1 pixel noir   (0, 0, 0, 1)  → pour emission absent
default_normal  : 1×1 pixel normal (0.5, 0.5, 1)  → pour normal map absent
default_rough   : 1×1 pixel        (1, 0, 0, 0)   → roughness=1, metallic=0
```

---

## 8. Le Material comme pont entre Pipeline et données

### Le problème

Le Pipeline définit **quelles données** le shader attend (le layout). Mais il ne contient pas les données elles-mêmes. Le Mesh définit la géométrie, mais pas les couleurs/textures. Il faut un objet qui fait le lien :

```
Pipeline "standard_forward" dit :
  "Mon shader attend : un UBO MaterialData au set 1 binding 0,
   une diffuse_map au set 1 binding 1, une normal_map au set 1 binding 2"

Material "brick_wall" répond :
  "Voici les données : base_color=(0.8, 0.7, 0.6), roughness=0.9,
   diffuse_map=brick_texture, normal_map=brick_normals"
```

### Structure du Material

```rust
pub struct Material {
    /// Pipeline associé (technique de rendu)
    pipeline: String,

    /// Textures nommées
    /// "diffuse" → texture brique, "normal" → normal map brique
    textures: HashMap<String, Arc<Texture>>,

    /// Paramètres scalaires/vectoriels
    /// "base_color" → Vec4(0.8, 0.7, 0.6, 1.0), "roughness" → Float(0.9)
    parameters: HashMap<String, MaterialParam>,

    /// Descriptor set GPU pré-construit (per-material, Set 1)
    /// Créé/mis à jour automatiquement à partir des textures et paramètres
    descriptor_set: Option<Arc<dyn RendererDescriptorSet>>,

    /// UBO GPU contenant les paramètres scalaires
    /// Mis à jour quand les paramètres changent
    parameter_buffer: Option<Arc<dyn RendererBuffer>>,
}
```

### Cycle de vie d'un Material

```
1. Création :
   Material::new("brick_wall", "standard_forward")

2. Configuration :
   material.set_texture("diffuse", brick_texture);
   material.set_texture("normal", brick_normals);
   material.set_param("base_color", Vec4(0.8, 0.7, 0.6, 1.0));
   material.set_param("roughness", Float(0.9));

3. Upload GPU (automatique ou explicite) :
   → Crée le UBO MaterialData avec les paramètres
   → Crée le descriptor set avec le UBO + les textures
   → Stocke le descriptor_set dans le Material

4. Utilisation pendant le rendu :
   → Bind le descriptor set (Set 1)
   → Draw
```

### Material vs Material Instance

Pour les gros projets, on distingue parfois :

| Concept | Description | Exemple |
|---------|-------------|---------|
| **Material** | Template partagé (pipeline + textures par défaut) | "PBR_Standard" |
| **Material Instance** | Copie avec paramètres modifiés | "PBR_Standard avec couleur rouge" |

L'avantage : 100 objets rouges et 100 objets bleus partagent le même Material de base, seule la couleur diffère dans les instances. Les textures (lourdes) ne sont pas dupliquées.

**Pour Galaxy3D** : Commencer avec un Material simple (sans la notion d'instance). L'ajouter plus tard si nécessaire.

---

## 9. Flux de rendu complet avec binding

### Rendu d'une frame complète

```
DÉBUT DE FRAME
│
├── 1. Mise à jour du PerFrameData UBO
│     → camera.view, camera.projection, time, lights[]
│     → Upload dans le buffer GPU
│
├── 2. Bind Descriptor Set 0 (per-frame)
│     → Ne sera plus changé jusqu'à la fin de la frame
│
├── 3. Pour chaque matériau (trié par pipeline) :
│     │
│     ├── 3a. Bind le Pipeline GPU (si différent du précédent)
│     │
│     ├── 3b. Bind le Descriptor Set 1 du matériau (UBO + textures)
│     │
│     └── 3c. Pour chaque objet utilisant ce matériau :
│           │
│           ├── Push constants : model_matrix
│           ├── Bind vertex/index buffer du mesh
│           └── Draw call
│
FIN DE FRAME
```

### Exemple concret

```
Scène : 5 objets, 3 matériaux

Objets :
  - Mur1 (mesh=cube, material=brick)
  - Mur2 (mesh=cube, material=brick)
  - Sol   (mesh=plane, material=stone)
  - Perso (mesh=character, material=skin)
  - Arme  (mesh=sword, material=skin)      ← même matériau que Perso

Rendu optimisé (trié par matériau) :

  Bind Set 0 (camera + lights)               ← 1 fois

  Bind Pipeline "standard_forward"           ← 1 fois (même pipeline pour tous)

  Bind Set 1 (material "brick")              ← 1er matériau
    Push model_matrix(Mur1) → Draw
    Push model_matrix(Mur2) → Draw

  Bind Set 1 (material "stone")              ← 2ème matériau
    Push model_matrix(Sol) → Draw

  Bind Set 1 (material "skin")               ← 3ème matériau
    Push model_matrix(Perso) → Draw
    Push model_matrix(Arme) → Draw

Total : 1 bind Set 0, 1 bind pipeline, 3 binds Set 1, 5 push constants, 5 draws
```

---

## 10. Recommandations pour Galaxy3D

### Ce qui existe déjà

| Fonctionnalité | Status | Localisation |
|----------------|--------|-------------|
| Push constants | Implémenté | `cmd_push_constants()` dans CommandList |
| Descriptor sets | Implémenté | `create_descriptor_set()` dans Renderer |
| Buffers (vertex, index, uniform) | Implémenté | `create_buffer()` avec BufferUsage |
| Textures | Implémenté | `create_texture()` + `resource::Texture` |
| Pipeline avec variantes | Implémenté | `resource::Pipeline` |

### Ce qu'il faut ajouter

#### Étape 1 — Textures par défaut

Créer les 4 textures par défaut au démarrage (white, black, flat_normal, default_roughness). Elles serviront quand un matériau ne fournit pas toutes les textures.

#### Étape 2 — PerFrameData UBO

Créer un buffer uniform contenant les données de caméra et de lumières. Mis à jour une fois par frame, bindé dans le Set 0.

```rust
struct PerFrameData {
    view: Mat4,
    projection: Mat4,
    view_projection: Mat4,
    camera_position: Vec3,
    time: f32,
    // ... lumières
}
```

#### Étape 3 — resource::Material

Structure qui associe un pipeline, des textures et des paramètres scalaires. Gère la création du descriptor set GPU (Set 1) à partir de ses données.

```rust
pub struct Material {
    pipeline: String,
    textures: HashMap<String, Arc<Texture>>,
    parameters: HashMap<String, MaterialParam>,
    descriptor_set: Option<Arc<dyn RendererDescriptorSet>>,
    parameter_buffer: Option<Arc<dyn RendererBuffer>>,
}
```

#### Étape 4 — Pipeline Layout dans resource::Pipeline

Le resource::Pipeline doit déclarer **quel layout** il attend, pour que le Material sache comment construire son descriptor set.

```rust
pub struct PipelineLayout {
    /// Noms et types des paramètres attendus dans le UBO
    parameters: Vec<ParameterDesc>,
    /// Noms et slots des textures attendues
    texture_slots: Vec<TextureSlotDesc>,
}

pub struct ParameterDesc {
    name: String,
    param_type: MaterialParamType,
    default_value: MaterialParam,
}

pub struct TextureSlotDesc {
    name: String,           // "diffuse", "normal", etc.
    binding: u32,           // binding dans le descriptor set
    default_texture: DefaultTexture, // White, Black, FlatNormal, etc.
}
```

### Ordre de priorité

```
1. Textures par défaut          → facile, utile immédiatement
2. PerFrameData UBO             → nécessaire pour la caméra/lumières
3. resource::Material simple    → pipeline + textures + paramètres
4. Pipeline Layout              → auto-validation Material ↔ Pipeline
5. Material instances           → optimisation future
```

---

## Références

- **Vulkan Specification** : Push Constants, Descriptor Sets, Uniform Buffers
- **Vulkan Guide — Descriptor Sets** : organisation par fréquence de mise à jour
- **GPU Gems 3, Chapter 2** : "Efficient Resource Management" — stratégies de binding
- Voir aussi : [materials_and_passes.md](materials_and_passes.md) pour l'architecture Material/Pipeline/Passes
- Voir aussi : [rendering_techniques.md](rendering_techniques.md) pour les techniques d'optimisation du rendu
