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
4. [Descriptor Sets en profondeur](#4-descriptor-sets-en-profondeur)
5. [Push Constants — données ultra-rapides](#5-push-constants--données-ultra-rapides)
6. [Uniform Buffers (UBO) — données structurées](#6-uniform-buffers-ubo--données-structurées)
7. [Textures et Samplers](#7-textures-et-samplers)
   - 7.1 [Qu'est-ce qu'un Sampler ?](#71-quest-ce-quun-sampler-)
   - 7.2 [Paramètres d'un Sampler](#72-paramètres-dun-sampler)
   - 7.3 [Samplers prédéfinis Galaxy3D](#73-samplers-prédéfinis-galaxy3d)
   - 7.4 [État actuel dans Galaxy3D](#74-état-actuel-dans-galaxy3d)
   - 7.5 [Binding dans les Descriptor Sets](#75-binding-dans-les-descriptor-sets)
   - 7.6 [Textures manquantes](#76-textures-manquantes)
8. [Le Material comme pont entre Pipeline et données](#8-le-material-comme-pont-entre-pipeline-et-données)
9. [Flux de rendu complet avec binding](#9-flux-de-rendu-complet-avec-binding)
10. [Bonnes pratiques des Descriptor Sets](#10-bonnes-pratiques-des-descriptor-sets)
11. [Erreurs classiques à éviter](#11-erreurs-classiques-à-éviter)
12. [Approche des moteurs modernes](#12-approche-des-moteurs-modernes)
13. [Stratégie Galaxy3D](#13-stratégie-galaxy3d)
14. [Glossaire](#14-glossaire)
15. [BindingGroup — abstraction Galaxy3D](#15-bindinggroup--abstraction-galaxy3d)
   - 15.1 [Concept et origine](#151-concept-et-origine)
   - 15.2 [Design choisi (Option B)](#152-design-choisi-option-b)
   - 15.3 [Différences avec un Descriptor Set brut](#153-différences-avec-un-descriptor-set-brut)
   - 15.4 [Relation avec les slots de Pipeline](#154-relation-avec-les-slots-de-pipeline)
   - 15.5 [Ce qui reste en dehors du BindingGroup](#155-ce-qui-reste-en-dehors-du-bindinggroup)
   - 15.6 [API envisagée](#156-api-envisagée)
16. [Réflexion SPIR-V — mapping nom → binding](#16-réflexion-spir-v--mapping-nom--binding)
   - 16.1 [Problématique](#161-problématique)
   - 16.2 [Comment SPIR-V préserve les noms](#162-comment-spir-v-préserve-les-noms)
   - 16.3 [Choix architectural : Option B (réflexion à la création du Pipeline)](#163-choix-architectural--option-b-réflexion-à-la-création-du-pipeline)
   - 16.4 [Structures de données](#164-structures-de-données)
   - 16.5 [Flux d'implémentation](#165-flux-dimplémentation)
   - 16.6 [Lien avec Material](#166-lien-avec-material)
   - 16.7 [Dépendance spirv-reflect](#167-dépendance-spirv-reflect)

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

## 4. Descriptor Sets en profondeur

### 4.1 Définition et analogies

Un **Descriptor Set** est un objet Vulkan qui regroupe un ensemble de **bindings** (liaisons) entre des ressources GPU (buffers, textures, samplers) et des emplacements numérotés que les shaders peuvent lire.

C'est le mécanisme par lequel le CPU dit au GPU : "pour ce draw call, voici les données que le shader doit utiliser".

#### Analogie : le formulaire

```
Descriptor Set Layout = formulaire vierge (structure)
  ┌─────────────────────────────────────────────┐
  │ Champ 0 : [type: Uniform Buffer]            │
  │ Champ 1 : [type: Combined Image Sampler]    │
  │ Champ 2 : [type: Combined Image Sampler]    │
  └─────────────────────────────────────────────┘

Descriptor Set = formulaire rempli (données concrètes)
  ┌─────────────────────────────────────────────┐
  │ Champ 0 : UBO_material_brick                │
  │ Champ 1 : texture_albedo_brick              │
  │ Champ 2 : texture_normal_brick              │
  └─────────────────────────────────────────────┘
```

Un Layout est une **structure vide** (quels types à quels emplacements).
Un Descriptor Set est une **instance concrète** de ce layout avec de vraies ressources.

### 4.2 Descriptor Set Layout — le blueprint

Un layout décrit la **forme** d'un descriptor set sans contenir de données :

```
DescriptorSetLayout {
    bindings: [
        { binding: 0, type: UNIFORM_BUFFER,          count: 1, stages: VERTEX|FRAGMENT },
        { binding: 1, type: COMBINED_IMAGE_SAMPLER,   count: 1, stages: FRAGMENT },
        { binding: 2, type: COMBINED_IMAGE_SAMPLER,   count: 1, stages: FRAGMENT },
        { binding: 3, type: COMBINED_IMAGE_SAMPLER,   count: 1, stages: FRAGMENT },
    ]
}
```

Chaque binding spécifie :
- **binding** : numéro de l'emplacement (correspond au `layout(binding = N)` dans le shader)
- **type** : quel genre de ressource (UBO, texture+sampler, storage buffer, etc.)
- **count** : nombre de descriptors à cet emplacement (>1 pour les tableaux de textures)
- **stages** : quels étages du shader y accèdent (vertex, fragment, compute, etc.)

Le layout est créé **une seule fois**, au moment de la création du Pipeline. Il fait partie du Pipeline Layout :

```
Pipeline Layout = [
    Set 0 Layout : per-frame (caméra, lumières)
    Set 1 Layout : per-material (paramètres, textures)
    Set 2 Layout : per-object (optionnel)
]

Pipeline = Shaders + Pipeline Layout + States (blend, depth, rasterizer)
```

Le même layout peut être partagé entre plusieurs pipelines si leur structure de binding est identique.

### 4.3 Relations Layout ↔ Descriptor Set ↔ Pipeline

#### Layout → Descriptor Set (1-N)

Un layout peut servir de modèle pour **N** descriptor sets différents :

```
Layout_PBR_Material :
  binding 0 : UBO
  binding 1 : texture
  binding 2 : texture

  ──────────────────────────────────────
  │                                    │
  ▼                                    ▼
DS_brick                           DS_metal
  binding 0 : UBO_brick             binding 0 : UBO_metal
  binding 1 : tex_brick_albedo      binding 1 : tex_metal_albedo
  binding 2 : tex_brick_normal      binding 2 : tex_metal_normal
```

Le nombre de descriptor sets n'est pas lié au nombre de layouts, mais au nombre de **combinaisons uniques de ressources**.

#### Layout → Pipeline (N-N)

Plusieurs pipelines peuvent utiliser le **même** layout s'ils attendent la même structure de données :

```
Layout_PBR_Material ◄──── Pipeline_PBR_Opaque
                    ◄──── Pipeline_PBR_Translucent
                    ◄──── Pipeline_PBR_Wireframe

Layout_Unlit       ◄──── Pipeline_Unlit_Color
                   ◄──── Pipeline_Unlit_Debug
```

#### Descriptor Set → Pipeline au draw time (N-N)

Au moment du draw, n'importe quel descriptor set peut être bindé à n'importe quel pipeline, **à condition que leurs layouts soient compatibles** (même structure) :

```
Pipeline_PBR_Opaque + DS_brick     → Draw mur
Pipeline_PBR_Opaque + DS_metal     → Draw armure
Pipeline_PBR_Translucent + DS_glass → Draw vitre
```

#### Exemple concret

```
Textures : T1 (brique), T2 (métal), T3 (verre)
Pipelines : P1 (opaque), P2 (translucide), P3 (wireframe), P4 (shadow)

Layouts nécessaires :
  L1 = [UBO + 2 textures]    ← utilisé par P1, P2, P3
  L2 = [UBO seul]            ← utilisé par P4 (shadow n'a pas besoin de textures)

Descriptor Sets nécessaires :
  DS1 = L1 rempli avec [UBO_brick, T1_albedo, T1_normal]
  DS2 = L1 rempli avec [UBO_metal, T2_albedo, T2_normal]
  DS3 = L1 rempli avec [UBO_glass, T3_albedo, T3_normal]
  DS4 = L2 rempli avec [UBO_generic]

Draw calls possibles :
  P1 + DS1 → brique opaque
  P1 + DS2 → métal opaque
  P2 + DS3 → verre translucide
  P3 + DS1 → brique wireframe
  P4 + DS4 → shadow pass (tous objets)
```

### 4.4 Contenu d'un Descriptor Set

Un descriptor set **ne contient pas que des textures**. Il peut contenir :

| Type Vulkan | Description | Exemple |
|---|---|---|
| `UNIFORM_BUFFER` | Buffer de données structurées (lecture seule shader) | Matrices caméra, paramètres matériaux (roughness, metallic, couleur) |
| `COMBINED_IMAGE_SAMPLER` | Texture + mode de filtrage | Albedo, normal map, roughness map |
| `STORAGE_BUFFER` | Buffer lecture/écriture (compute shaders) | Données de particules, résultats de compute |
| `SAMPLED_IMAGE` | Texture seule (sans sampler) | Utilisé avec samplers séparés |
| `SAMPLER` | Sampler seul (sans texture) | Filtrage bilinéaire, trilinéaire, anisotropique |
| `STORAGE_IMAGE` | Image lecture/écriture (compute shaders) | Post-processing, génération procédurale |
| `UNIFORM_BUFFER_DYNAMIC` | UBO avec offset dynamique | Plusieurs objets dans un même gros buffer |
| `INPUT_ATTACHMENT` | Framebuffer attachment en lecture | Deferred shading (lire le G-buffer) |

#### Distinction importante : la Roughness

La **roughness** (ou tout paramètre scalaire du matériau) n'est pas directement dans le descriptor set. Elle est dans un **Uniform Buffer**, et c'est le buffer qui est bindé dans le descriptor set :

```
Descriptor Set (Set 1 — per-material)
  ├── binding 0 : Uniform Buffer ──→ { roughness: 0.5, metallic: 0.8, base_color: [1,0,0,1] }
  ├── binding 1 : Texture ──→ albedo.png
  ├── binding 2 : Texture ──→ normal.png
  └── binding 3 : Sampler ──→ linear_repeat
```

Deux matériaux avec les **mêmes textures** mais des **roughness différentes** ont besoin de :
- Deux UBOs différents (un avec roughness=0.5, un avec roughness=0.8)
- Donc deux descriptor sets différents (car ils pointent vers des UBOs différents)

### 4.5 Organisation par fréquence de changement

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

#### Pourquoi cet ordre ?

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

#### Impact sur les performances

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

### 4.6 Cycle de vie complet

#### 1. Création du Descriptor Pool

Avant de pouvoir allouer des descriptor sets, il faut créer un pool :

```
Descriptor Pool :
  max_sets: 1000
  pool_sizes: [
    { type: UNIFORM_BUFFER,          count: 500 },
    { type: COMBINED_IMAGE_SAMPLER,  count: 2000 },
  ]
```

Le pool pré-alloue la mémoire. C'est analogue à un allocateur de mémoire spécialisé.

#### 2. Allocation du Descriptor Set

```
descriptor_set = pool.allocate(layout)
```

L'allocation est rapide (pas d'appel GPU, juste de la gestion mémoire côté driver).

#### 3. Écriture (Write)

C'est ici qu'on remplit le descriptor set avec les ressources concrètes :

```
vkUpdateDescriptorSets(device, [
    WriteDescriptorSet {
        dst_set: descriptor_set,
        dst_binding: 0,
        descriptor_type: UNIFORM_BUFFER,
        buffer_info: { buffer: ubo_material, offset: 0, range: 64 },
    },
    WriteDescriptorSet {
        dst_set: descriptor_set,
        dst_binding: 1,
        descriptor_type: COMBINED_IMAGE_SAMPLER,
        image_info: { sampler: linear_sampler, image_view: albedo_view, layout: SHADER_READ_ONLY },
    },
])
```

**IMPORTANT** : L'écriture doit être faite **avant** que le descriptor set soit utilisé par le GPU, et **pas pendant** qu'il est utilisé.

#### 4. Bind pendant le rendu

```
vkCmdBindDescriptorSets(command_buffer, GRAPHICS, pipeline_layout, set_index=1, [descriptor_set])
```

Le bind est très rapide — c'est juste un changement de pointeur dans le command buffer.

#### 5. Réutilisation

Le descriptor set reste valide tant que :
- Le pool n'est pas détruit
- Les ressources qu'il référence ne sont pas détruites
- Il n'est pas en cours d'utilisation par le GPU au moment d'une écriture

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

### 7.1 Qu'est-ce qu'un Sampler ?

Un **Sampler** et une **Texture** sont deux objets GPU **fondamentalement distincts** :

| Concept | Analogie | Ce que c'est |
|---------|----------|-------------|
| **Texture** | Une **photo** | Les données brutes : pixels (texels), dimensions, format (RGBA8, BC7…), niveaux de mipmap |
| **Sampler** | Les **réglages de l'appareil photo** | Comment on **lit** la texture : filtrage, mode de répétition, niveau de détail |

Quand un shader exécute `texture(sampler2D, uv)`, il y a en réalité **deux objets** qui collaborent :
- La **Texture** (`VkImage` + `VkImageView`) → les données pixel
- Le **Sampler** (`VkSampler`) → les règles de lecture

#### Pourquoi séparer les deux ?

**1. Réutilisation** — Un même sampler peut servir pour des centaines de textures. Si toutes les textures diffuses utilisent du filtrage linéaire + repeat, un seul objet `VkSampler` suffit pour toutes.

**2. Combinaisons** — Une même texture peut être lue avec des samplers différents selon le contexte :

```
Texture terrain.png :
  + Sampler REPEAT     → le terrain se répète à l'infini sur le sol
  + Sampler CLAMP      → la même texture en preview dans l'éditeur (pas de répétition)
  + Sampler NEAREST    → debug mode (voir les texels individuels)

Même image, trois comportements de lecture différents.
```

**3. Performance** — Un `VkSampler` est un objet extrêmement léger (~quelques octets de configuration GPU). En avoir 3 à 5 pour tout le moteur est la norme. Alors qu'une texture peut peser des centaines de Mo en mémoire GPU.

**4. Séparation des responsabilités** — La texture ne devrait pas savoir *comment* elle sera lue. Un artiste crée une image ; le moteur décide comment l'échantillonner selon le contexte (filtrage anisotropique sur un sol, filtrage nearest pour du pixel art, etc.).

### 7.2 Paramètres d'un Sampler

Un sampler configure quatre aspects de la lecture :

#### Filtrage (comment interpoler entre les texels)

| Mode | Comportement | Usage typique |
|------|-------------|---------------|
| `NEAREST` | Retourne le texel le plus proche, aucune interpolation | Pixel art, lookup tables, données brutes |
| `LINEAR` | Interpolation bilinéaire entre les 4 texels voisins | Rendu 3D général, textures réalistes |

Le filtrage s'applique en deux situations :
- **Magnification** (`magFilter`) : quand la texture est affichée plus grande qu'elle n'est (on zoom)
- **Minification** (`minFilter`) : quand la texture est affichée plus petite qu'elle n'est (on dézoom)

#### Mode d'adressage (que faire quand UV sort de [0, 1])

| Mode | Comportement | Usage typique |
|------|-------------|---------------|
| `REPEAT` | La texture se répète en boucle (carrelage) | Sol, murs, terrain |
| `MIRRORED_REPEAT` | Se répète en miroir (évite les coutures) | Textures procédurales |
| `CLAMP_TO_EDGE` | Le bord de la texture se prolonge à l'infini | UI, shadow maps, environment maps |
| `CLAMP_TO_BORDER` | Couleur fixe (noir, blanc, transparent) en dehors | Effets spéciaux, masques |

Le mode d'adressage est configurable indépendamment pour U, V, et W (3D).

#### Mipmapping (niveaux de détail)

Les mipmaps sont des versions pré-calculées de la texture à des résolutions décroissantes (1024→512→256→128→…→1). Le sampler contrôle :

| Paramètre | Description |
|-----------|-------------|
| `mipmapMode` | `NEAREST` (pas d'interpolation entre niveaux) ou `LINEAR` (interpolation trilinéaire) |
| `minLod` | Niveau de mipmap minimum utilisable (0 = pleine résolution) |
| `maxLod` | Niveau de mipmap maximum utilisable |
| `mipLodBias` | Décalage du LOD (positif = plus flou, négatif = plus net) |

**Filtrage trilinéaire** = `minFilter: LINEAR` + `magFilter: LINEAR` + `mipmapMode: LINEAR`. C'est le mode le plus courant pour un rendu de qualité.

#### Anisotropie (qualité en angle rasant)

Le filtrage anisotropique améliore la qualité quand une surface est vue en angle rasant (sol, route, mur en perspective). Sans anisotropie, ces surfaces deviennent floues.

| Niveau | Qualité | Coût GPU |
|--------|---------|----------|
| 1x (désactivé) | Flou en angle rasant | Nul |
| 4x | Bon compromis | Faible |
| 8x | Très bon | Modéré |
| 16x | Maximum | Légèrement plus élevé |

Sur les GPU modernes, le coût du filtrage anisotropique 16x est quasi négligeable. C'est activé par défaut dans la plupart des moteurs.

### 7.3 Samplers prédéfinis Galaxy3D

Un moteur 3D n'a besoin que de quelques samplers couvrant les cas d'usage courants :

| Sampler | Filtrage | Adressage | Anisotropie | Usage typique |
|---------|----------|-----------|-------------|---------------|
| `LinearRepeat` | LINEAR + trilinéaire | REPEAT | 16x | Textures diffuse, normal maps, terrain |
| `LinearClamp` | LINEAR + trilinéaire | CLAMP_TO_EDGE | 16x | UI, shadow maps, environment maps, post-process |
| `NearestRepeat` | NEAREST | REPEAT | Désactivé | Pixel art, données brutes, voxels |
| `NearestClamp` | NEAREST | CLAMP_TO_EDGE | Désactivé | Lookup tables, textures de bruit, indices |
| `Shadow` | LINEAR | CLAMP_TO_BORDER (blanc) | Désactivé | Shadow maps avec comparaison de profondeur |

Ces samplers sont créés **une seule fois** à l'initialisation du moteur et réutilisés pour toutes les textures qui en ont besoin. Cinq objets `VkSampler` suffisent pour couvrir la grande majorité des situations.

### 7.4 État actuel dans Galaxy3D

**Ce qui existe** :

Un seul sampler partagé dans le backend Vulkan (`VulkanRenderer::texture_sampler`), créé à l'initialisation avec :
- Filtrage : LINEAR (mag et min)
- Mipmapping : LINEAR
- Adressage : REPEAT (U, V, W)
- Anisotropie : activée (16x)

Ce sampler est utilisé pour toutes les textures du moteur sans distinction.

**Ce qui n'existe pas encore** :

- Aucun `renderer::Sampler` dans la couche abstraite du renderer
- Aucune possibilité de créer des samplers avec des réglages différents
- Aucune association sampler ↔ texture configurable côté utilisateur

À terme, le renderer devra exposer les samplers prédéfinis (section 7.3) et permettre au Material de spécifier quel sampler utiliser pour chaque texture.

### 7.5 Binding dans les Descriptor Sets

Les textures sont bindées dans les descriptor sets, typiquement dans le Set 1 (per-material). En Vulkan, le type `COMBINED_IMAGE_SAMPLER` associe une texture **ET** un sampler dans un même binding :

```
Descriptor Set 1 :
  binding 0 : UBO MaterialData
  binding 1 : Combined Image Sampler → diffuse texture + LinearRepeat
  binding 2 : Combined Image Sampler → normal map     + LinearRepeat
  binding 3 : Combined Image Sampler → roughness map  + LinearRepeat
  binding 4 : Combined Image Sampler → emission map   + LinearRepeat
```

Deux textures dans le même descriptor set peuvent utiliser des samplers différents :

```
Descriptor Set (post-process) :
  binding 0 : Combined Image Sampler → color buffer   + LinearClamp   ← pas de repeat
  binding 1 : Combined Image Sampler → depth buffer   + NearestClamp  ← pas d'interpolation
  binding 2 : Combined Image Sampler → noise texture  + NearestRepeat ← repeat sans filtrage
```

### 7.6 Textures manquantes

Un matériau ne fournit pas toujours toutes les textures. Solutions courantes :

| Approche | Description |
|----------|-------------|
| **Texture par défaut** | Binder une texture 1×1 blanche/noire/flat quand la texture est absente |
| **Flag dans le UBO** | Un booléen `has_normal_map` dans le MaterialData |
| **Specialization constants** | Variantes shader compilées avec/sans certaines textures |

**Recommandation** : Texture par défaut. Créer au démarrage du moteur :

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

## 10. Bonnes pratiques des Descriptor Sets

### Un Descriptor Set par combinaison unique de ressources

```
CORRECT :
  DS1 → [UBO_brick, tex_brick]        ← pour tous les objets "brick"
  DS2 → [UBO_metal, tex_metal]        ← pour tous les objets "metal"
  DS3 → [UBO_wood, tex_wood]          ← pour tous les objets "wood"

INCORRECT :
  DS1 → [UBO_shared, tex_brick]       ← on change UBO_shared entre les draws
  (danger : race condition GPU/CPU)
```

### Ne jamais muter un Descriptor Set en cours d'utilisation

Si le GPU est en train de lire un descriptor set (draw call soumis mais pas terminé), **ne pas écrire dedans**. Solutions :

- **Double/triple buffering** : avoir N copies du DS (une par frame en vol)
- **Allocation par frame** : allouer de nouveaux DS à chaque frame, libérer les anciens quand le GPU a fini

### Pré-construire les Descriptor Sets

Ne pas créer de descriptor sets pendant la boucle de draw. Les construire à l'avance :
- Au chargement du matériau
- Quand un matériau est modifié

### Trier les draws par pipeline puis par descriptor set

```
OPTIMAL :
  Bind Pipeline A
    Bind DS1 → Draw, Draw, Draw
    Bind DS2 → Draw, Draw
  Bind Pipeline B
    Bind DS3 → Draw

SOUS-OPTIMAL :
  Bind Pipeline A, Bind DS1 → Draw
  Bind Pipeline B, Bind DS3 → Draw     ← changement de pipeline inutile
  Bind Pipeline A, Bind DS2 → Draw     ← re-changement
  Bind Pipeline A, Bind DS1 → Draw     ← re-bind inutile
```

Le changement de **pipeline est coûteux** (~microseconde). Le changement de **descriptor set est quasi gratuit** (~nanosecondes).

### Organiser par fréquence de changement

```
Set 0 : Per-Frame    → bindé 1 fois par frame
Set 1 : Per-Material → bindé 1 fois par matériau
Set 2 : Per-Object   → bindé 1 fois par objet (si push constants insuffisantes)
```

Quand on bind le Set 1, le Set 0 reste bindé (il n'est pas invalidé).

---

## 11. Erreurs classiques à éviter

### Mettre le Descriptor Set dans la Texture

```
MAUVAIS :
  resource::Texture {
      renderer_texture: Arc<dyn Texture>,
      descriptor_set: Arc<dyn DescriptorSet>,  ← NON
  }

POURQUOI : Une même texture peut apparaître dans plusieurs descriptor sets
avec des bindings différents (binding 1 dans un DS, binding 3 dans un autre)
ou combinée avec des ressources différentes (UBO différent, sampler différent).
Le descriptor set n'appartient pas à la texture.
```

### Muter un buffer bindé dans un DS en cours d'utilisation

```
MAUVAIS :
  Bind DS1 → [UBO_params, tex_albedo]
  Draw submesh_1                          ← GPU commence à lire UBO_params
  UBO_params.roughness = 0.8             ← DANGER : GPU lit peut-être encore l'ancienne valeur
  Draw submesh_2                          ← données indéterminées

CORRECT :
  DS1 → [UBO_params_A(roughness=0.5), tex_albedo]
  DS2 → [UBO_params_B(roughness=0.8), tex_albedo]
  Bind DS1 → Draw submesh_1              ← GPU lit UBO_params_A
  Bind DS2 → Draw submesh_2              ← GPU lit UBO_params_B (buffer séparé)
```

### Un seul gros Descriptor Set pour tout

```
MAUVAIS :
  DS_tout = [UBO_camera, UBO_lights, UBO_material, tex_albedo, tex_normal, UBO_object]
  → Chaque changement d'objet/matériau nécessite un nouveau DS complet

CORRECT :
  DS_frame    = [UBO_camera, UBO_lights]                    ← 1 par frame
  DS_material = [UBO_material, tex_albedo, tex_normal]      ← 1 par matériau
  Push constants = model_matrix                              ← par objet
```

### Allouer des Descriptor Sets dans la boucle de rendu

```
MAUVAIS :
  for each object:
      let ds = pool.allocate(layout)         ← allocation à chaque frame pour chaque objet
      write_descriptor_set(ds, resources)     ← écriture à chaque frame
      cmd.bind(ds)
      cmd.draw()

CORRECT :
  // Au chargement :
  for each material:
      material.ds = pool.allocate(layout)
      write_descriptor_set(material.ds, material.resources)

  // Au rendu :
  for each object:
      cmd.bind(object.material.ds)           ← simple bind, pas d'allocation
      cmd.draw()
```

---

## 12. Approche des moteurs modernes

### Unity

Unity cache complètement les descriptor sets derrière le concept de **Material properties** :

```
Couche 1 — Utilisateur (C#) :
  material.SetTexture("_MainTex", brickTexture);
  material.SetFloat("_Roughness", 0.5f);
  → L'utilisateur ne voit jamais de descriptor set, layout, binding

Couche 2 — Shader Property System (C++ natif) :
  → Chaque property a un identifiant numérique (hash du nom)
  → Le système résout : nom → slot/binding dans le shader compilé
  → Table de correspondance compilée à partir des #pragma et annotations

Couche 3 — Backend Vulkan (C++ natif) :
  → Hash de la combinaison {textures + UBO contents}
  → Lookup dans un cache global de descriptor sets
  → Si trouvé : réutilise le DS existant
  → Si pas trouvé : alloue un nouveau DS depuis le pool, écrit les ressources, cache-le
  → Bind le DS au moment du draw

Résultat :
  L'utilisateur manipule des "propriétés de matériau" (SetFloat, SetTexture)
  Le backend Vulkan gère automatiquement l'allocation/cache/réutilisation des DS
  90%+ des DS sont réutilisés frame après frame (cache hit)
```

### Unreal Engine 5

UE5 pré-calcule les bindings dans un système de commandes de draw :

```
Couche 1 — Utilisateur (Blueprint/C++) :
  Material Instance → hérite d'un Master Material
  → Paramètres modifiables : couleur, roughness, textures
  → L'utilisateur ne voit jamais de descriptor set

Couche 2 — FMeshDrawCommand (C++) :
  → Au moment du chargement (pas au rendu !) :
     → Pré-compile toutes les infos de draw dans un FMeshDrawCommand
     → Contient : pipeline state, vertex buffers, toutes les références de ressources
  → C'est un objet immuable prêt à être soumis

Couche 3 — RHI (Rendering Hardware Interface) :
  → API abstraite (SetShaderTexture, SetShaderUniformBuffer)
  → Accumule les appels de binding

Couche 4 — FVulkanRHI (backend Vulkan) :
  → Hash de la combinaison accumulée de ressources
  → Cache global de descriptor sets
  → Allocateur par pool avec reset par frame

Organisation des descriptor sets UE5 :
  Set 0 : Global      → données partagées par toute la frame
  Set 1 : Per-Pass    → données spécifiques à une passe de rendu
  Set 2 : Per-Material → textures et paramètres du matériau
  Set 3 : Per-Object  → matrices de transformation, bones
```

### Points communs des moteurs modernes

1. **L'utilisateur ne voit jamais les descriptor sets** — il manipule des matériaux avec des propriétés nommées
2. **Hash + Cache** — les DS sont identifiés par un hash de leur contenu, et réutilisés si déjà existants
3. **Organisation par fréquence** — les sets sont numérotés du moins fréquemment changé au plus fréquemment changé
4. **Pré-construction** — les DS sont construits au chargement, pas pendant le rendu

### Pourquoi cette complexité ?

Cette complexité existe pour gérer des scènes avec **des milliers de matériaux** et **des dizaines de milliers d'objets**. À cette échelle :

- Allouer un DS par matériau naïvement = explosion mémoire
- Les cacher par hash = ~90% de réutilisation
- Les trier par pipeline = moins de state changes GPU = gros gain de performance

**Pour Galaxy3D** (scènes de taille modeste), cette complexité n'est pas nécessaire dans un premier temps. Une approche directe est préférable.

---

## 13. Stratégie Galaxy3D

### Approche choisie : un DS par material × submesh

Chaque submesh dans un RenderInstance possède son propre descriptor set, pré-construit :

```
RenderInstance (mesh "character")
├── RenderSubMesh 0 (torse)
│   ├── Pipeline : PBR_opaque
│   └── DescriptorSet → [UBO{roughness=0.3}, tex_body, tex_normal_body]
├── RenderSubMesh 1 (yeux)
│   ├── Pipeline : PBR_translucent
│   └── DescriptorSet → [UBO{roughness=0.1}, tex_eyes, tex_normal_eyes]
└── RenderSubMesh 2 (armure)
    ├── Pipeline : PBR_opaque
    └── DescriptorSet → [UBO{roughness=0.9}, tex_armor, tex_normal_armor]
```

### Rendu simplifié

```rust
// Tri par pipeline pour minimiser les changements de pipeline
let sorted_submeshes = sort_by_pipeline(scene.all_submeshes());

for submesh in sorted_submeshes {
    cmd.bind_pipeline(submesh.pipeline);              // changé seulement si différent
    cmd.bind_descriptor_set(submesh.descriptor_set);  // quasi gratuit
    cmd.push_constants(submesh.model_matrix);         // ultra rapide
    cmd.draw_indexed(submesh.index_count, submesh.index_offset, 0);
}
```

### Pourquoi cette approche

| Avantage | Explication |
|---|---|
| **Simple** | Pas de hash, pas de cache, pas de résolution dynamique |
| **Sûr** | Chaque draw a ses propres données, pas de race condition |
| **Performant** | Le bind de DS est quasi gratuit, le tri par pipeline minimise les vrais coûts |
| **Évolutif** | On peut ajouter un cache par hash plus tard si nécessaire |

### Où vit le Descriptor Set ?

```
PAS dans resource::Texture     → une texture peut être dans plusieurs DS
PAS dans resource::Pipeline    → un pipeline peut être utilisé avec plusieurs DS
→ DANS le RenderInstance/RenderSubMesh → c'est la combinaison concrète material+pipeline+mesh
```

Le Material (quand il sera implémenté) sera responsable de **créer** le descriptor set à partir de ses textures et paramètres. Le RenderInstance **stockera** ce descriptor set pour l'utiliser au rendu.

### Ce qui existe déjà

| Fonctionnalité | Status | Localisation |
|----------------|--------|-------------|
| Push constants | Implémenté | `cmd_push_constants()` dans CommandList |
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

### Évolution future

Si Galaxy3D doit un jour gérer des milliers de matériaux, on pourra ajouter un cache :

```
Phase 1 (actuel)  : DS par material × submesh, construction directe
Phase 2 (futur)   : Cache de DS par hash de contenu (comme Unity/UE5)
Phase 3 (lointain) : Bindless textures (un seul énorme DS avec toutes les textures)
```

---

## 14. Glossaire

| Terme | Définition |
|---|---|
| **Descriptor** | Un seul binding dans un descriptor set (une texture, un buffer, etc.) |
| **Descriptor Set** | Groupe de descriptors qui sont bindés ensemble au GPU |
| **Descriptor Set Layout** | Blueprint décrivant la structure d'un descriptor set (types et bindings) |
| **Descriptor Pool** | Allocateur spécialisé pour les descriptor sets |
| **Pipeline Layout** | Ensemble de descriptor set layouts + push constant ranges, définissant l'interface complète du pipeline |
| **Binding** | Numéro d'emplacement dans un descriptor set (correspond à `layout(binding=N)` dans le shader) |
| **Set** | Numéro du descriptor set dans le pipeline layout (correspond à `layout(set=N)` dans le shader) |
| **UBO** | Uniform Buffer Object — buffer de données structurées en lecture seule pour les shaders |
| **SSBO** | Shader Storage Buffer Object — buffer lecture/écriture pour compute shaders |
| **Combined Image Sampler** | Texture + sampler combinés en un seul descriptor |
| **Push Constants** | Petites données (≤128 octets) écrites directement dans le command buffer, sans descriptor set |
| **Sampler** | Objet GPU définissant comment lire une texture : filtrage, mode d'adressage, mipmapping, anisotropie. Séparé de la texture elle-même |
| **BindingGroup** | Abstraction Galaxy3D au-dessus du Descriptor Set Vulkan, inspirée de WebGPU. Immuable après création, layout déduit du Pipeline, pool géré en interne |
| **Anisotropie** | Filtrage améliorant la qualité des textures vues en angle rasant. Niveaux : 1x (off), 4x, 8x, 16x (max) |
| **Filtrage bilinéaire** | Interpolation entre les 4 texels voisins (`LINEAR`). Trilinéaire = bilinéaire + interpolation entre niveaux de mipmap |

---

## 15. BindingGroup — abstraction Galaxy3D

### 15.1 Concept et origine

Le terme **Descriptor Set** est un concept Vulkan bas niveau dont le nom n'est pas particulièrement parlant. L'API **WebGPU** a introduit le concept équivalent sous le nom de **BindingGroup** (ou GPUBindGroup), qui exprime mieux la nature de l'objet : un **groupe de bindings** prêt à être attaché au pipeline.

Galaxy3D adopte ce nom et cette philosophie : l'utilisateur du moteur manipule des **BindingGroup**, jamais des Descriptor Sets directement. Sous le capot, un BindingGroup **est** un `VkDescriptorSet`, mais encapsulé dans une abstraction plus ergonomique.

```
                    Utilisateur Galaxy3D
                          │
                    BindingGroup API
                          │
                 ┌────────┴────────┐
                 │   renderer::    │
                 │  BindingGroup   │  ← trait abstrait
                 └────────┬────────┘
                          │
                 ┌────────┴────────┐
                 │   VulkanBinding │
                 │     Group       │  ← implémentation Vulkan
                 │ (= VkDescriptor│
                 │      Set)       │
                 └─────────────────┘
```

### 15.2 Design choisi (Option B)

Galaxy3D utilise le design **"vrai BindingGroup"** où le layout est déduit du Pipeline, inspiré de WebGPU :

1. **Le Pipeline stocke ses layouts** — À la création du Pipeline, les `VkDescriptorSetLayout` sont créés et stockés en interne. L'utilisateur ne les voit jamais.

2. **Création par le Pipeline** — Pour créer un BindingGroup, on passe par le Pipeline qui connaît le layout attendu :

```
// L'utilisateur ne manipule jamais de layout directement
let bg = pipeline.create_binding_group(set_index, &[
    Binding::UniformBuffer(ubo_camera),
    Binding::CombinedImageSampler(tex_albedo, sampler_linear),
    Binding::CombinedImageSampler(tex_normal, sampler_linear),
]);
```

3. **Immuable après création** — Une fois créé, le BindingGroup ne peut plus être modifié. Pour changer les ressources, on en crée un nouveau. Cette contrainte élimine toute race condition GPU/CPU.

4. **Pool géré en interne** — Le `VkDescriptorPool` est créé et géré par le `VulkanRenderer`. L'utilisateur n'a pas à se soucier de l'allocation, du dimensionnement, ni de la libération du pool. C'est déjà le cas dans le backend actuel (`VulkanRenderer::descriptor_pool`).

### 15.3 Différences avec un Descriptor Set brut

| Aspect | Descriptor Set Vulkan brut | BindingGroup Galaxy3D |
|--------|---------------------------|----------------------|
| **Layout** | L'utilisateur doit créer un `VkDescriptorSetLayout` explicitement | Layout déduit du Pipeline, invisible pour l'utilisateur |
| **Pool** | L'utilisateur doit créer et dimensionner un `VkDescriptorPool` | Pool géré en interne par le renderer |
| **Allocation** | `vkAllocateDescriptorSets` explicite | Allocation automatique à la création du BindingGroup |
| **Écriture** | `vkUpdateDescriptorSets` séparé après allocation | Écriture en une seule étape à la construction |
| **Mutabilité** | Modifiable via `vkUpdateDescriptorSets` (dangereux si en cours d'utilisation) | Immuable après création — sûr par design |
| **Création** | 3 étapes : allocate → write → utilise | 1 étape : `pipeline.create_binding_group(...)` |
| **Sous le capot** | C'est un `VkDescriptorSet` | C'est exactement un `VkDescriptorSet` — aucune différence de performance |

### 15.4 Relation avec les slots de Pipeline

Un Pipeline possède **plusieurs slots** (ou "tiroirs") numérotés, chacun associé à un layout :

```
Pipeline "PBR_forward"
  ┌──────────────────────────────────┐
  │ Slot 0 (set=0) : Layout_PerFrame │ ← caméra, lumières
  │ Slot 1 (set=1) : Layout_Material │ ← UBO matériau + textures
  │ Slot 2 (set=2) : Layout_PerObj   │ ← optionnel (si push constants insuffisantes)
  └──────────────────────────────────┘
```

Chaque slot correspond au `set = N` dans le GLSL.

#### Cardinalités

| Relation | Cardinalité | Explication |
|----------|-------------|-------------|
| Slot ↔ Layout (dans un pipeline) | **1-1** | Chaque slot a exactement un layout |
| Layout ↔ Pipelines qui l'utilisent | **1-N** | Plusieurs pipelines peuvent partager le même layout si leur structure est identique |
| Layout ↔ BindingGroups créés depuis ce layout | **1-N** | Un layout sert de modèle pour N BindingGroups avec des ressources différentes |

#### Indépendance des slots

Quand on bind un BindingGroup dans le slot 1, le slot 0 **reste bindé** — il n'est pas invalidé. C'est ce qui permet l'organisation par fréquence :

```
Bind BindingGroup slot 0 (per-frame)         ← 1 fois par frame

  Pour chaque matériau :
    Bind BindingGroup slot 1 (per-material)  ← slot 0 reste bindé

    Pour chaque objet :
      Push constants (per-object)            ← slots 0 et 1 restent bindés
      Draw
```

### 15.5 Ce qui reste en dehors du BindingGroup

Les **Push Constants** sont un mécanisme **complètement séparé** des BindingGroup :

| Aspect | BindingGroup | Push Constants |
|--------|-------------|----------------|
| **Contenu** | UBOs, textures, samplers, storage buffers | Scalaires, matrices (≤128 octets) |
| **Mutabilité** | Immuable après création | Modifiable à chaque draw call |
| **Mécanisme GPU** | Pointeur vers de la mémoire GPU | Données copiées directement dans le command buffer |
| **Coût de changement** | Quasi gratuit (changement de pointeur) | Encore plus rapide (pas de pointeur, données inline) |
| **Déclaration shader** | `layout(set=N, binding=M) uniform ...` | `layout(push_constant) uniform ...` |

Les push constants ne passent **pas** par un BindingGroup. Elles sont écrites directement via `cmd_push_constants()`.

#### Exemple Galaxy3D — Roughness en push constant

Si un paramètre comme `roughness` doit changer à chaque draw call (par exemple pour varier entre les submeshes d'un même matériau), on peut le placer dans les push constants plutôt que dans un UBO :

```glsl
layout(push_constant) uniform PushConstants {
    mat4 model;           // 64 octets — transformation de l'objet
    float roughness;      // 4 octets  — rugosité per-draw
    float metallic;       // 4 octets  — métallicité per-draw
    uint object_id;       // 4 octets  — identifiant unique
    // ... jusqu'à 128 octets
};
```

Dans ce cas, le BindingGroup (per-material) contient les textures et les paramètres qui ne changent pas par draw, et les push constants contiennent les valeurs qui varient.

### 15.6 API envisagée

#### Création d'un BindingGroup

```rust
// Le Pipeline expose la création de BindingGroup
// L'utilisateur ne touche jamais aux layouts ni au pool

let bg_per_frame = pipeline.create_binding_group(
    renderer,
    0,  // set index
    &[
        BindingResource::UniformBuffer(ubo_camera),
        BindingResource::UniformBuffer(ubo_lights),
    ],
)?;

let bg_material_brick = pipeline.create_binding_group(
    renderer,
    1,  // set index
    &[
        BindingResource::UniformBuffer(ubo_brick_params),
        BindingResource::CombinedImageSampler(tex_brick_albedo, sampler_linear_repeat),
        BindingResource::CombinedImageSampler(tex_brick_normal, sampler_linear_repeat),
        BindingResource::CombinedImageSampler(tex_brick_roughness, sampler_linear_repeat),
    ],
)?;
```

#### Utilisation au rendu

```rust
// Boucle de rendu
cmd.bind_pipeline(&pipeline);
cmd.bind_binding_group(0, &bg_per_frame);        // per-frame — 1 fois

for material in materials_sorted {
    cmd.bind_binding_group(1, &material.binding_group);  // per-material

    for object in material.objects {
        cmd.push_constants(&PushConstants {
            model: object.transform,
            roughness: object.roughness_override,
            // ...
        });
        cmd.draw_indexed(object.index_count, object.index_offset, 0);
    }
}
```

#### Trait renderer::BindingGroup

```rust
/// A BindingGroup is an immutable set of GPU resource bindings.
/// It maps to a VkDescriptorSet in Vulkan, but the layout and pool
/// are managed internally by the renderer.
pub trait BindingGroup: Send + Sync {
    /// Returns the set index this BindingGroup was created for.
    fn set_index(&self) -> u32;
}

/// Resources that can be bound in a BindingGroup.
pub enum BindingResource<'a> {
    UniformBuffer(&'a dyn Buffer),
    CombinedImageSampler(&'a dyn Texture, &'a dyn Sampler),
    StorageBuffer(&'a dyn Buffer),
    // ... extensible
}
```

---

## 16. Réflexion SPIR-V — mapping nom → binding

> **Statut** : Design validé, non implémenté
> **Date** : 2026-02-15

### 16.1 Problématique

Actuellement, les `BindingGroupLayoutDesc` sont spécifiés **manuellement** lors de la création
d'un pipeline. L'appelant doit connaître à l'avance les numéros de set et de binding :

```rust
// Situation actuelle : tout est hardcodé
BindingGroupLayoutDesc {
    entries: vec![BindingSlotDesc {
        binding: 0,                              // hardcodé
        binding_type: BindingType::CombinedImageSampler,
        count: 1,
        stage_flags: ShaderStageFlags::FRAGMENT,
    }],
}
```

Le problème : si le shader change (ajout d'un binding, réorganisation), le code Rust doit
être mis à jour manuellement. Il n'y a aucun moyen de demander "quel est le binding de
`texSampler` ?" au pipeline.

### 16.2 Comment SPIR-V préserve les noms

Le GLSL compilé en SPIR-V conserve les noms des variables via des instructions `OpName`
dans le bytecode. Ces noms ne sont pas utilisés par le GPU mais sont accessibles par des
outils de réflexion.

Exemple — pour ce shader :

```glsl
layout(set = 0, binding = 0) uniform sampler2DArray texSampler;
```

Le bytecode SPIR-V contient :

| Instruction SPIR-V | Donnée |
|---------------------|--------|
| `OpName` | `"texSampler"` |
| `OpDecorate ... DescriptorSet` | `0` |
| `OpDecorate ... Binding` | `0` |
| Type | `OpTypeSampledImage` (combined image sampler) |

Une bibliothèque de réflexion peut parser ce bytecode et extraire :
`"texSampler" → { set: 0, binding: 0, type: CombinedImageSampler, stage: Fragment }`

**Important** : Vulkan lui-même n'expose **aucune API de réflexion**. Ni `VkPipeline`, ni
`VkShaderModule` ne permettent de récupérer ces métadonnées. Il faut parser le bytecode
SPIR-V directement.

### 16.3 Choix architectural : parsing au Shader, fusion au Pipeline

Quatre options ont été évaluées :

| Option | Point d'insertion | Verdict |
|--------|------------------|---------|
| A — Parsing + merge au `create_pipeline()` | Stocker le bytecode SPIR-V dans Shader, parser au pipeline | Re-parse si shader partagé |
| **B — Parsing au `create_shader()`, merge au `create_pipeline()`** | **Chaque shader stocke sa réflexion (interne backend)** | **✅ Retenu** |
| C — Couche `resource::*` | Parsing avant le renderer | Responsabilité mal placée |
| D — Exposer la réflexion sur le trait `Shader` | Réflexion publique per-shader | Complexifie le trait `Shader` |

**Pourquoi Option B** :

1. **Un shader est souvent partagé** : le même vertex shader sert dans plusieurs pipelines
   (shadow pass, depth pass, color pass...). Avec l'option A, on re-parse le même bytecode
   à chaque `create_pipeline()`. Avec B, on parse **une seule fois** à `create_shader()`
2. **Moins de mémoire** : on jette le bytecode SPIR-V après le parsing, on ne garde que
   les métadonnées compactes (quelques dizaines d'octets vs plusieurs KB de bytecode)
3. **Le parsing appartient au shader** : c'est le shader qui déclare les bindings, pas le
   pipeline. Le pipeline ne fait que combiner deux shaders
4. **Le trait `Shader` reste inchangé** : la réflexion per-shader est `pub(crate)` dans
   `VulkanShader`, seul le trait `Pipeline` expose la réflexion fusionnée publiquement
5. **Séparation** : la couche `resource` n'a pas à manipuler des concepts GPU (sets, bindings)

### 16.4 Structures de données

Structures définies dans `renderer/pipeline.rs` (abstraites, pas Vulkan-spécifiques) :

```rust
/// SPIR-V reflection data for a compiled pipeline.
/// Merges vertex + fragment shader bindings.
pub struct PipelineReflection {
    bindings: Vec<ReflectedBinding>,
    binding_names: HashMap<String, usize>,  // pattern Vec+HashMap
}

/// A single reflected binding extracted from SPIR-V bytecode.
pub struct ReflectedBinding {
    pub name: String,
    pub set: u32,
    pub binding: u32,
    pub binding_type: BindingType,       // réutilise le type existant
    pub stage_flags: ShaderStageFlags,   // Vertex | Fragment | les deux
}
```

Le trait `Pipeline` gagne une méthode :

```rust
pub trait Pipeline: Send + Sync {
    fn binding_group_layout_count(&self) -> u32;
    fn reflection(&self) -> &PipelineReflection;  // nouveau
}
```

Accès par nom ou par index (pattern standard du projet) :

```rust
impl PipelineReflection {
    /// Access by index (hot path, O(1))
    pub fn binding(&self, index: usize) -> Option<&ReflectedBinding>;

    /// Access by name (lookup)
    pub fn binding_by_name(&self, name: &str) -> Option<&ReflectedBinding>;

    /// Name → index resolution
    pub fn binding_index(&self, name: &str) -> Option<usize>;

    /// Total number of reflected bindings
    pub fn binding_count(&self) -> usize;
}
```

### 16.5 Flux d'implémentation

```
create_shader(ShaderDesc { code: &[u8], stage: Vertex, ... })
    │  → créer vk::ShaderModule (comme avant)
    │  → spirq : parser le bytecode SPIR-V
    │  → stocker Vec<ReflectedBinding> dans VulkanShader (pub(crate))
    │  → jeter le bytecode SPIR-V (pas conservé en mémoire)
    ▼

create_pipeline(PipelineDesc { vertex_shader, fragment_shader, ... })
    │  → downcast les deux VulkanShader
    │  → fusionner les Vec<ReflectedBinding> (union, merge stage_flags)
    │  → construire PipelineReflection (Vec + HashMap)
    │  → stocker dans VulkanPipeline
    ▼

Arc<dyn Pipeline>
    │  → pipeline.reflection().binding_by_name("texSampler")
    │     → Some(ReflectedBinding { set: 0, binding: 0, ... })
```

**Fusion des réflexions** : quand un binding apparaît dans les deux shaders (même set +
même binding), les `stage_flags` sont combinés (`Vertex | Fragment`). Si le même
(set, binding) a des types différents entre vertex et fragment → erreur.

### 16.6 Lien avec Material

La réflexion permet au `Material` de résoudre ses noms de slots vers des bindings GPU :

```
resource::Material                     renderer::Pipeline (via réflexion)
  texture_slot "texSampler"  ──────►  name="texSampler" → set=0, binding=0
  param "roughness"          ──────►  name="roughness"  → set=1, binding=0 (UBO member)
```

À terme, le moteur pourrait :
1. Lire les noms des texture slots du Material
2. Les résoudre via `pipeline.reflection().binding_by_name(name)`
3. Créer automatiquement les `BindingGroup` correspondants

Cela éliminerait la spécification manuelle des bindings.

### 16.7 Dépendance spirq

Le crate **`spirq`** (pur Rust, activement maintenu) a été retenu pour la réflexion SPIR-V.

**Pourquoi `spirq`** (et pas `rspirv-reflect`) :

| Critère | `rspirv-reflect` | `spirq` |
|---------|-------------------|---------|
| Bindings (nom, set, binding, type) | Oui | Oui |
| Membres internes UBO/SSBO (noms, types, offsets, strides) | **Non** | **Oui** |
| Pur Rust | Oui | Oui |
| Maintenu activement | Oui | Oui |

`rspirv-reflect` suffirait pour le mapping nom → (set, binding), mais ne permet pas
d'extraire les membres internes des UBO/SSBO. Or cette information sera nécessaire
quand le moteur auto-remplira des uniform buffers à partir des paramètres de Material.
Choisir `spirq` dès maintenant évite une migration future et la dette technique associée.

**Note** : `spirq` définit ses propres `DescriptorType` — une table de conversion vers
les types `ash`/Vulkan est nécessaire (un `match` d'environ 10 lignes, écrit une fois).

**Intégration** :
- `spirq` est ajouté au crate **backend Vulkan** (`galaxy_3d_engine_renderer_vulkan`)
- Le crate core (`galaxy_3d_engine`) définit les structures abstraites (`PipelineReflection`,
  `ReflectedBinding`) sans dépendance à `spirq`

---

## Références

- **Vulkan Specification** : Chapters 14 (Resource Descriptors), Push Constants, Uniform Buffers
- **Vulkan Guide** : [Descriptor Sets](https://vkguide.dev/docs/chapter-4/descriptors/)
- **WebGPU Specification** : [GPUBindGroup](https://www.w3.org/TR/webgpu/#bind-groups) — inspiration directe pour le concept BindingGroup
- **Sascha Willems** : Vulkan descriptor set examples
- Voir aussi : [materials_and_passes.md](materials_and_passes.md) pour l'architecture Material/Pipeline/Passes
- Voir aussi : [rendering_techniques.md](rendering_techniques.md) pour les techniques d'optimisation du rendu
- **SPIR-V Specification** : [Khronos SPIR-V Registry](https://registry.khronos.org/SPIR-V/) — OpName, OpDecorate
- **spirq** : [GitHub](https://github.com/PENGUINLIONG/spirq-rs) — bibliothèque de réflexion SPIR-V pure Rust (retenu pour Galaxy3D)
