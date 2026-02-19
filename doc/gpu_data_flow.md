# GPU Data Flow : Données Per-Frame & Per-Instance

> **Date** : 2026-02-19
> **Statut** : Référence architecturale — à implémenter progressivement

---

## Introduction

Ce document décrit comment les données transitent du CPU vers le GPU dans un moteur 3D moderne,
en s'appuyant sur les pratiques réelles d'Unreal Engine 5, Unity (SRP/DOTS), Godot 4 et Wicked Engine.

L'objectif est de servir de référence pour l'évolution de Galaxy3D depuis l'architecture actuelle
(push constants uniquement) vers une architecture UBO + SSBO scalable.

---

## Niveaux de fréquence de mise à jour

Les données envoyées au GPU sont organisées par **fréquence de changement** :

| Niveau | Fréquence | Mécanisme GPU | Contenu |
|--------|-----------|---------------|---------|
| **Per-frame** | 1× par frame | UBO global | View, Projection, caméra, temps, lumières |
| **Per-material** | Change au switch de matériau | SSBO ou descriptor set | Paramètres matériaux, textures |
| **Per-instance** | 1× par draw call | SSBO indexé | World matrix, données d'instance |
| **Per-draw** | 1× par draw call | Push constants | Index dans le SSBO (4-8 bytes) |

---

## Per-Frame : UBO Global

Bindé **une seule fois** au début de la frame. Contient toutes les données globales.

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `view` | mat4 | 64 B | Matrice vue (caméra) |
| `projection` | mat4 | 64 B | Matrice projection |
| `view_projection` | mat4 | 64 B | Précalculé CPU : `proj × view` (1 seule multiplication par frame) |
| `prev_view_projection` | mat4 | 64 B | VP de la frame N-1 (motion vectors, TAA) |
| `inv_view` | mat4 | 64 B | Inverse de la vue (reconstruction position world depuis depth) |
| `inv_projection` | mat4 | 64 B | Inverse de la projection |
| `camera_position` | vec4 | 16 B | Position de la caméra en world space (spéculaire, fog) |
| `camera_direction` | vec4 | 16 B | Direction de vue (effets atmosphériques) |
| `time` | vec4 | 16 B | x=temps, y=sin(temps), z=delta_time, w=frame_count |
| `sun_direction` | vec4 | 16 B | Direction de la lumière directionnelle principale |
| `sun_color` | vec4 | 16 B | Couleur + intensité du soleil |
| `viewport_size` | vec4 | 16 B | x=width, y=height, z=1/width, w=1/height |

**Total** : ~480 B — une seule copie par frame.

---

## Per-Instance : SSBO

Les moteurs modernes séparent les données per-instance en **plusieurs SSBOs**
pour optimiser la cohérence de cache GPU. Le GPU lit les buffers par cache lines
(~128 B) : si un compute shader de culling ne lit que 20 B par instance,
autant ne pas charger 200 B inutiles.

### SSBO Transforms

Lu par **tous les shaders** (vertex, fragment, compute culling).

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `world` | float3x4 | 48 B | Matrice LocalToWorld (la dernière ligne `[0,0,0,1]` est implicite) |
| `prev_world` | float3x4 | 48 B | World de la frame N-1 (motion vectors, TAA, motion blur) |
| `world_to_local` | float3x4 | 48 B | Inverse de world (éclairage, décals, calculs en object space) |

**Total** : ~144 B/instance

**Pourquoi pas de MVP ?** La world matrix ne change que quand l'objet bouge.
Stocker le MVP forcerait à recalculer et ré-uploader les données de **toutes** les instances
à chaque frame, même celles qui n'ont pas bougé, juste parce que la caméra a tourné.
Le VP est dans l'UBO global et la multiplication `VP × worldPos` est triviale pour le GPU.

**Pourquoi pas de Normal Matrix ?** Voir la section dédiée ci-dessous.

### SSBO Material Refs

Lu par les shaders de rendu (forward/deferred).

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `material_index` | uint | 4 B | Index dans le SSBO de matériaux |
| `tint_color` | vec4 | 16 B | Couleur de modulation per-instance |
| `alpha` | float | 4 B | Opacité (fade-in/fade-out, dissolution) |
| `uv_offset_scale` | vec4 | 16 B | Animation UV (eau, écrans, scrolling) |
| `emission_scale` | float | 4 B | Intensité émission per-instance |

**Total** : ~44 B/instance (padded à 48)

### SSBO Visibility / Culling

Lu par le **compute shader de culling** et le système de LOD.

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `bounding_sphere` | vec4 | 16 B | Centre xyz + rayon w (world space) |
| `flags` | uint | 4 B | Bitfield : visible, shadow_caster, shadow_receiver, static |
| `lod_bias` | float | 4 B | Ajustement LOD per-instance |
| `object_id` | uint | 4 B | Picking souris, outlining, stencil |
| `layer_mask` | uint | 4 B | Rendu sélectif (UI, personnages, décor) |

**Total** : ~32 B/instance

### SSBO Bones (skinning uniquement)

Utilisé uniquement par les objets animés (personnages, créatures).

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `bone_matrices` | mat4[] | 64 B × N | N = nombre d'os (typiquement 64–256) |
| `prev_bone_matrices` | mat4[] | 64 B × N | Motion blur sur personnages animés |

Indexé via un `bone_offset + bone_count` stocké dans le SSBO transforms.

---

## Per-Draw : Push Constants

Les push constants sont réduites au **strict minimum** dans un pipeline moderne :

| Donnée | Type | Taille | Description |
|--------|------|--------|-------------|
| `instance_id` | uint | 4 B | Index dans le SSBO d'instances |
| `material_id` | uint | 4 B | Index dans le SSBO de matériaux (optionnel si dans SSBO material refs) |

**Total** : 4–8 B par draw call.

Les push constants sont le mécanisme le plus rapide (zéro allocation GPU,
écriture directe dans le command buffer), mais limités à 128–256 B selon le GPU.
Les utiliser uniquement pour des index.

---

## Normal Matrix : Ne pas stocker

Aucun moteur moderne ne stocke la normal matrix per-instance. Voici pourquoi et comment
ils gèrent les normales selon les cas.

### Rappel mathématique

La "normal matrix" correcte est `transpose(inverse(mat3(world)))`.
Elle est nécessaire pour que les normales restent **perpendiculaires** à la surface
après transformation.

### Cas 1 : Translation + Rotation (pas de scale)

`mat3(world)` est orthogonale → `transpose(inverse(M)) = M`.
La normal matrix EST `mat3(world)`. Rien à calculer.

```glsl
vec3 n = normalize(mat3(world) * normal);
```

### Cas 2 : Scale uniforme (même facteur X, Y, Z)

Le `normalize()` élimine le facteur scalaire. `mat3(world)` + `normalize()` suffit.

```glsl
vec3 n = normalize(mat3(world) * normal);  // le normalize corrige le scale
```

### Cas 3 : Scale non-uniforme (facteurs différents X, Y, Z)

C'est le **seul** cas problématique. `mat3(world)` déforme les normales :
la surface est étirée dans une direction, mais la normale doit rester perpendiculaire.

**Solution : utiliser `world_to_local`** (déjà dans le SSBO pour d'autres raisons) :

```glsl
// world_to_local est dans le SSBO — pas de inverse() dans le shader
vec3 n = normalize(transpose(mat3(world_to_local)) * normal);
```

`transpose()` est quasi gratuit (changement d'indexation mémoire).

### Ce que font les moteurs réels

| Moteur | Stocke normal matrix ? | Stocke world_to_local ? | Méthode shader |
|--------|----------------------|------------------------|----------------|
| **Unreal Engine 5** | Non | Oui (`WorldToLocal` dans FPrimitiveSceneData) | `transpose(mat3(WorldToLocal))` |
| **Unity** | Non | Oui (`unity_WorldToObject` dans cbuffer) | `transpose(mat3(WorldToObject))` |
| **Wicked Engine** | Non | Non | `mat3(world)` + normalize |
| **Godot 4** | Non | Non | `mat3(world)` + normalize |

### Quand recalculer `world_to_local` ?

`world_to_local` n'est **pas recalculé chaque frame**. Il est recalculé côté CPU
**uniquement quand l'objet bouge** (dirty flag). Pour un objet statique,
il est calculé une seule fois à la création.

```
Objet statique (bâtiment, décor) :
  → world calculé 1 fois
  → world_to_local calculé 1 fois
  → SSBO jamais mis à jour pour cet objet

Objet dynamique (personnage, véhicule) :
  → world recalculé quand il bouge
  → world_to_local recalculé en même temps
  → SSBO mis à jour pour cet objet uniquement
```

### Pourquoi `inverse()` n'existe pas en HLSL/Slang

Galaxy3D utilise **Slang** comme langage shader (superset de HLSL, compilé en SPIR-V).
Contrairement à GLSL, ni HLSL ni Slang ne fournissent de fonction `inverse()` intégrée
pour les matrices. Voici pourquoi :

| Langage | `inverse()` built-in ? |
|---------|----------------------|
| **GLSL** | Oui (depuis GLSL 1.40) |
| **HLSL** | Non |
| **Slang** | Non |
| **Metal** | Non |

**Raisons techniques :**

1. **Pas d'instruction hardware** — Les GPUs n'ont aucune instruction dédiée pour
   inverser une matrice. En GLSL, `inverse()` est décomposé par le driver en dizaines
   d'opérations ALU (cofacteurs, déterminant, division). C'est du sucre syntaxique.

2. **Philosophie "coût visible"** — HLSL a été conçu pour ne pas cacher les opérations
   coûteuses. Une inverse 3×3 coûte ~24 multiplications + 1 division. Ne pas la fournir
   force le programmeur à se poser la question : "ai-je vraiment besoin de calculer ça
   dans le shader, ou puis-je précalculer côté CPU ?"

3. **En pratique, on n'inverse jamais côté GPU** — Les moteurs modernes passent
   `world_to_local` déjà précalculé depuis le CPU dans un SSBO. Le calcul est fait
   une seule fois quand l'objet bouge, pas à chaque vertex.

**En Slang**, quand l'inverse est nécessaire dans le shader (ex: normal matrix),
on utilise une fonction par cofacteurs :

```slang
float3x3 inverse3x3(float3x3 m) {
    float a00 = m[0][0], a01 = m[0][1], a02 = m[0][2];
    float a10 = m[1][0], a11 = m[1][1], a12 = m[1][2];
    float a20 = m[2][0], a21 = m[2][1], a22 = m[2][2];

    float det = a00 * (a11 * a22 - a12 * a21)
              - a01 * (a10 * a22 - a12 * a20)
              + a02 * (a10 * a21 - a11 * a20);
    float invDet = 1.0 / det;

    float3x3 r;
    r[0][0] = (a11 * a22 - a12 * a21) * invDet;
    r[0][1] = (a02 * a21 - a01 * a22) * invDet;
    r[0][2] = (a01 * a12 - a02 * a11) * invDet;
    r[1][0] = (a12 * a20 - a10 * a22) * invDet;
    r[1][1] = (a00 * a22 - a02 * a20) * invDet;
    r[1][2] = (a02 * a10 - a00 * a12) * invDet;
    r[2][0] = (a10 * a21 - a11 * a20) * invDet;
    r[2][1] = (a01 * a20 - a00 * a21) * invDet;
    r[2][2] = (a00 * a11 - a01 * a10) * invDet;
    return r;
}

// Utilisation : normal matrix
float3x3 normalMatrix = transpose(inverse3x3(float3x3(world)));
float3 worldNormal = mul(normalMatrix, localNormal);
```

### Recommandation Galaxy3D

| Cas | Méthode | Coût |
|-----|---------|------|
| Sans scale non-uniforme | `normalize(mat3(world) * N)` | Quasi nul |
| Avec scale non-uniforme | `normalize(transpose(mat3(world_to_local)) * N)` | Quasi nul |

Référence : Eric Zhang, *"Stop Using Normal Matrix"*.

---

## Abstraction Shader : Built-in Variables

Les moteurs modernes fournissent des **abstractions** pour que les utilisateurs de shaders
n'aient pas besoin de connaître les détails des buffers (UBO, SSBO, push constants).

### Niveaux d'abstraction (du plus accessible au plus bas niveau)

| Niveau | Public cible | Voit les buffers ? | Exemple |
|--------|-------------|-------------------|---------|
| **Shader Graph** | Artiste | Non | Noeuds visuels (Unreal Material Editor, Unity Shader Graph) |
| **Surface Shader** | Tech artist | Non | Définit albedo/roughness/normal, le moteur gère le reste |
| **Built-in variables** | Shader programmer | Non | `MODEL_MATRIX`, `UnityObjectToWorldNormal()` |
| **Raw shader** | Engine programmer | **Oui** | `layout(set=1, binding=0) buffer Instances {...}` |

### Exemples concrets par moteur

**Godot 4** — langage shader propre avec built-in variables :
```glsl
void vertex() {
    // Variables fournies automatiquement par le moteur :
    VERTEX;            // position locale
    NORMAL;            // normale locale
    MODEL_MATRIX;      // world matrix (vient du SSBO en interne)
    VIEW_MATRIX;       // vient de l'UBO global
    PROJECTION_MATRIX; // vient de l'UBO global
}
void fragment() {
    ALBEDO = texture(my_tex, UV).rgb;
    ROUGHNESS = 0.5;
    // Le moteur gère le PBR lighting automatiquement
}
```

**Unity** — includes HLSL fournis par le moteur :
```hlsl
#include "UnityCG.cginc"  // définit toutes les variables et fonctions

v2f vert(appdata v) {
    o.pos = UnityObjectToClipPos(v.vertex);           // MVP × pos (en interne)
    o.worldNormal = UnityObjectToWorldNormal(v.normal); // normal matrix (en interne)
    o.worldPos = mul(unity_ObjectToWorld, v.vertex).xyz;
}
```

**Unreal Engine** — variables injectées automatiquement :
```hlsl
GetWorldPosition();          // position world du pixel
GetWorldNormal();            // normale world
ResolvedView.ViewProjection; // VP matrix
```

### Projection Galaxy3D

Galaxy3D utilise **Slang** comme langage shader (superset de HLSL, compilé en SPIR-V via `slangc`).
Un fichier `.slang` unique contient vertex et fragment entry points.

Étape pragmatique : fournir un **module Slang** avec les built-in variables :

```slang
import galaxy3d;

// Fonctions disponibles :
float4 G3D_WorldPosition(float3 local_pos);   // world * pos
float3 G3D_WorldNormal(float3 local_normal);  // normale en world space
float4 G3D_ClipPosition(float4 world_pos);    // VP * world_pos

// Variables disponibles :
G3D_WORLD_MATRIX       // float4x4 world (depuis SSBO)
G3D_VIEW_PROJECTION    // float4x4 VP (depuis UBO)
G3D_CAMERA_POSITION    // float3 (depuis UBO)
G3D_TIME               // float (depuis UBO)
```

L'étape Shader Graph est un objectif à long terme (parsing, génération de code, UI).

---

## Shader : Calculs dérivés

Le vertex shader calcule les données dérivées à partir des buffers :

```glsl
// UBO global (per-frame)
layout(set = 0, binding = 0) uniform FrameData {
    mat4 view_projection;
    // ...
};

// SSBO per-instance
layout(set = 1, binding = 0) readonly buffer Instances {
    InstanceData instances[];
};

// Push constant
layout(push_constant) uniform PC {
    uint instance_id;
};

void main() {
    mat4 world = mat4(instances[instance_id].world);  // float3x4 → mat4

    // World position (pour l'éclairage, le fog, les shadows)
    vec4 world_pos = world * vec4(in_position, 1.0);

    // Clip position
    gl_Position = view_projection * world_pos;

    // Normale en world space (cas courant : pas de scale non-uniforme)
    frag_world_normal = mat3(world) * in_normal;

    // Avec scale non-uniforme : utiliser world_to_local (déjà dans le SSBO)
    // mat3 wit = transpose(mat3(instances[instance_id].world_to_local));
    // frag_world_normal = wit * in_normal;
}
```

---

## Double / Triple Buffering

Pour ne pas écrire dans un buffer que le GPU est en train de lire,
on garde **N copies** des SSBOs (une par frame in-flight, typiquement 2 ou 3).

```
Frame 0 → écrit dans SSBO[0], GPU lit SSBO[2] (frame N-2)
Frame 1 → écrit dans SSBO[1], GPU lit SSBO[0] (frame N-1)
Frame 2 → écrit dans SSBO[2], GPU lit SSBO[1] (frame N-1)
```

Seuls les SSBOs mutables (transforms, visibility) ont besoin de buffering.
Les SSBOs statiques (matériaux, bones) peuvent être partagés si non modifiés.

---

## Empreinte mémoire

| 100K instances | Taille |
|----------------|--------|
| SSBO Transforms (×3 buffered) | ~43 MB |
| SSBO Material Refs | ~5 MB |
| SSBO Visibility | ~3 MB |
| UBO Global | ~0.5 KB |
| **Total** | **~51 MB** |

Négligeable pour un GPU avec 8–24 GB de VRAM.

---

## Projection Galaxy3D : État actuel → Cible

| Aspect | Actuel | Cible intermédiaire | Cible AAA |
|--------|--------|---------------------|-----------|
| Per-frame | Push constants (MVP) | UBO global (VP, caméra, lumières) | Idem |
| Per-instance | Push constants (world, params) | SSBO indexé (world, material_id) | SSBO + indirect draw |
| Per-draw | Push constants (132 B) | Push constants (4 B : instance_id) | Rien (gl_InstanceIndex) |
| Culling | CPU (frustum) | CPU (frustum) | GPU compute (frustum + occlusion) |
| Normal matrix | Calculée dans le shader | Calculée dans le shader | Idem |
| Langage shader | **Slang** (.slang → SPIR-V) | Slang | Slang |

L'étape intermédiaire (UBO + SSBO + push constant index) est le sweet spot :
simple à implémenter, scale à des dizaines de milliers d'objets,
et pose les bases pour le GPU-driven rendering.

---

## Références

- **Unreal Engine 5** : `FPrimitiveSceneData` stocke `LocalToWorld` dans un GPU Scene Buffer
- **Unity SRP Batcher** : `unity_ObjectToWorld` dans cbuffer `UnityPerDraw`
- **Unity DOTS BRG** : `objectToWorld` (float3x4 = 48 B) dans SSBO per-batch
- **Godot 4** : `transform` via instance buffer, `scene_data` UBO global
- **Wicked Engine** : `ShaderTransform` (3× float4 = 48 B) dans SSBO, pas de normal matrix
- **Eric Zhang** : *"Stop Using Normal Matrix"* — calculer dans le shader
- **Khronos Vulkan Samples** : *"Constant data in Vulkan"* — comparatif UBO/SSBO/push constants
- **vkguide.dev** : *"GPU Driven Rendering"* — architecture SSBO + indirect draw
