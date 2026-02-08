# Matériaux, Pipelines et Passes de Rendu

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-08
> **Objectif** : Documenter les concepts de matériaux, pipelines multi-passe, et leur intégration dans le système de rendu.

---

## Table des matières

1. [Concepts fondamentaux](#1-concepts-fondamentaux)
2. [Architecture : Material vs Pipeline vs Passes](#2-architecture--material-vs-pipeline-vs-passes)
3. [Intégration dans Galaxy3D](#3-intégration-dans-galaxy3d)
4. [Pipelines mono-passe (cas courant)](#4-pipelines-mono-passe-cas-courant)
5. [Pipelines multi-passe](#5-pipelines-multi-passe)
6. [Exemples détaillés de pipelines multi-passe](#6-exemples-détaillés-de-pipelines-multi-passe)
7. [Gestion des passes dans le système de rendu](#7-gestion-des-passes-dans-le-système-de-rendu)
8. [Comment les moteurs modernes gèrent ça](#8-comment-les-moteurs-modernes-gèrent-ça)
9. [Recommandations pour Galaxy3D](#9-recommandations-pour-galaxy3d)

---

## 1. Concepts fondamentaux

### Trois niveaux d'abstraction

Le rendu d'un objet 3D implique trois concepts distincts qu'il ne faut pas confondre :

| Concept | Rôle | Qui le définit | Analogie |
|---------|------|---------------|----------|
| **Material** | L'apparence finale souhaitée | L'artiste | "Je veux du verre rouge semi-transparent" |
| **Pipeline** | La technique de rendu (shaders + configuration GPU) | Le programmeur graphique | "Pour faire du verre, il faut 2 passes avec tel blending" |
| **Passe (Pass)** | Une étape de dessin avec un pipeline GPU | Le moteur de rendu | "Bind ce shader, configure ce culling, draw" |

### Ce qu'est un Material

Un matériau est une **description artistique** de l'apparence d'un objet. Il contient :
- Une référence vers un pipeline (la technique de rendu à utiliser)
- Des textures (albedo, normal map, roughness map...)
- Des paramètres scalaires (couleur, opacité, intensité de reflet...)

Le matériau ne contient **aucune information technique** sur les passes de rendu. L'artiste ne sait pas et n'a pas besoin de savoir combien de passes sont nécessaires pour réaliser l'effet visuel qu'il a configuré.

### Ce qu'est un Pipeline (au sens resource::)

Un pipeline au niveau `resource::` est une **technique de rendu complète**. Il encapsule :
- Une ou plusieurs variantes (static, animated, transparent...)
- Pour chaque variante, une ou plusieurs **passes**
- Chaque passe contient un `render::Pipeline` (le pipeline GPU réel)

### Ce qu'est une Passe

Une passe est une **opération de dessin atomique** sur le GPU. Elle correspond à :
- Un `render::Pipeline` bindé (vertex shader + fragment shader + configuration)
- Une configuration de rasterisation (face culling, depth test, blending...)
- Un draw call qui dessine la géométrie de l'objet

Quand un pipeline a plusieurs passes, l'objet est **dessiné plusieurs fois** — une fois par passe, chacune avec un pipeline GPU différent.

---

## 2. Architecture : Material vs Pipeline vs Passes

### Principe de séparation

```
resource::Material (ce que l'artiste voit)
  ├── pipeline: "toon_outline"           // référence vers resource::Pipeline
  ├── textures:
  │   └── "albedo" → Arc<resource::Texture>
  ├── parameters:
  │   ├── "shadow_threshold" → 0.3
  │   ├── "outline_width" → 2.0
  │   └── "outline_color" → (0, 0, 0)
  └── Pas de notion de passe ici.

resource::Pipeline "toon_outline" (ce que le programmeur définit)
  ├── variant "static"
  │   ├── pass 0: render::Pipeline (toon base, cull back)
  │   └── pass 1: render::Pipeline (outline, cull front)
  │
  ├── variant "animated"
  │   ├── pass 0: render::Pipeline (toon base + skinning, cull back)
  │   └── pass 1: render::Pipeline (outline + skinning, cull front)
```

### Pourquoi les passes ne sont JAMAIS dans le Material

Les passes décrivent **comment** dessiner (technique). Le matériau décrit **quoi** dessiner (artistique). Cette séparation est fondamentale :

- **Réutilisabilité** : tous les objets "verre" partagent les mêmes 2 passes. Seuls les paramètres (couleur, opacité) changent via le matériau.
- **Simplicité pour l'artiste** : l'artiste choisit un pipeline ("toon_outline", "glass", "standard") et règle des curseurs. Il ne manipule jamais de passes.
- **Maintenabilité** : si on optimise la technique de rendu du verre (par exemple en fusionnant les 2 passes en 1 grâce à une extension GPU), seul le pipeline change. Aucun matériau à modifier.

C'est l'approche utilisée par Unity (passes dans le Shader, pas dans le Material) et Unreal (le Shading Model du matériau détermine les passes, mais le Material ne les décrit pas).

---

## 3. Intégration dans Galaxy3D

### Architecture existante à 3 niveaux

```
render::    → GPU brut (Pipeline, RenderPass, Buffer, Texture...)
resource::  → Métier (Texture, Mesh, Pipeline avec variantes)
scene::     → Scène (futur : Material, SceneObject...)
```

### Évolution proposée de resource::Pipeline

Le système de variantes existant s'étend naturellement pour supporter les passes multiples :

```rust
// Structure actuelle
pub struct PipelineVariant {
    name: String,
    renderer_pipeline: Arc<dyn RendererPipeline>,  // UN seul pipeline GPU
}

// Évolution proposée
pub struct PipelineVariant {
    name: String,
    passes: Vec<PipelinePass>,  // UN ou PLUSIEURS pipelines GPU
}

pub struct PipelinePass {
    renderer_pipeline: Arc<dyn RendererPipeline>,
    // Configuration spécifique à cette passe si nécessaire
}
```

### Structure du futur resource::Material

```rust
pub struct Material {
    pipeline: String,                              // nom du resource::Pipeline
    textures: HashMap<String, Arc<Texture>>,       // "albedo" → texture
    parameters: HashMap<String, MaterialParam>,    // "roughness" → 0.5
}

pub enum MaterialParam {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
}
```

---

## 4. Pipelines mono-passe (cas courant)

La grande majorité des matériaux (90%+) n'ont besoin que d'une seule passe. En forward rendering, cette passe unique fait tout : transformation des vertices, éclairage, texturing, et écriture dans le framebuffer.

### Standard Forward (PBR ou Phong)

```
Pipeline "standard_forward"
  variant "static":
    pass 0:
      - Vertex shader : transform par MVP
      - Fragment shader : éclairage complet (toutes les lumières)
      - Face culling : back
      - Depth test : activé (lecture + écriture)
      - Blending : désactivé (opaque)
```

Paramètres du matériau : `albedo_texture`, `normal_map`, `roughness`, `metallic`

### Unlit (sans éclairage)

```
Pipeline "unlit"
  variant "static":
    pass 0:
      - Vertex shader : transform par MVP
      - Fragment shader : couleur texture × tint, pas de calcul de lumière
      - Face culling : back
      - Depth test : activé
      - Blending : désactivé
```

Paramètres du matériau : `texture`, `tint_color`

### Transparent simple

```
Pipeline "transparent_simple"
  variant "static":
    pass 0:
      - Vertex shader : transform par MVP
      - Fragment shader : éclairage + alpha
      - Face culling : back
      - Depth test : lecture seule (pas d'écriture)
      - Blending : activé (SrcAlpha, OneMinusSrcAlpha)
```

Paramètres du matériau : `albedo_texture`, `opacity`

---

## 5. Pipelines multi-passe

### Quand faut-il plusieurs passes ?

Un pipeline a besoin de plusieurs passes quand **un seul draw call ne suffit pas** pour produire l'effet visuel souhaité. Cela arrive quand :

- On doit dessiner le même objet avec des **configurations GPU incompatibles** (ex: faces avant ET faces arrière avec des shaders différents)
- On doit dessiner le même objet **plusieurs fois** avec des transformations différentes (ex: couches de fourrure)
- On doit superposer des effets qui nécessitent des **modes de blending différents** sur le même objet

### Important : les passes multi-passe sont rares

En forward rendering, la grande majorité des effets courants sont mono-passe. Les pipelines multi-passe sont réservés à des effets spécifiques. Les moteurs modernes avec deferred rendering réduisent encore plus le besoin de multi-passe par objet, car l'éclairage et beaucoup d'effets sont gérés en post-traitement plein écran.

### Récapitulatif des pipelines multi-passe

| Pipeline | Nb passes | Description courte |
|----------|:---------:|-------------------|
| Toon + Outline (Inverted Hull) | 2 | Cel-shading avec contours |
| Verre / Transparent double-face | 2 | Faces arrière puis faces avant |
| X-Ray / Silhouette | 2 | Vision à travers les obstacles |
| Decal projeté | 2 | Détail superposé sur une surface |
| Force Field / Bouclier | 2 | Champ de force sci-fi |
| Fur Shells (fourrure) | 16-32 | Couches successives de poils |

---

## 6. Exemples détaillés de pipelines multi-passe

### 6.1 Toon Shading avec contours (Inverted Hull)

La technique d'Inverted Hull est la méthode classique pour créer des contours en cel-shading. L'objet est dessiné deux fois : une fois normalement avec un éclairage en paliers, et une fois avec les faces inversées et gonflées pour créer le contour.

#### Passe 0 — Base Toon (faces avant)

```
Configuration GPU :
  - Face culling : Back (on dessine les faces AVANT uniquement)
  - Depth test : activé (lecture + écriture)
  - Blending : désactivé (opaque)

Vertex shader :
  - Transformation classique : position × MVP
  - Passe les normales et UVs au fragment shader

Fragment shader :
  1. Calcule l'éclairage directionnel : dot(normal, light_direction)
  2. Quantifie en paliers au lieu d'un dégradé continu :
     - dot > shadow_threshold  →  base_color (pleine lumière)
     - sinon                   →  shadow_color (ombre)
  3. Applique la texture albedo
  4. Écrit la couleur finale
```

#### Passe 1 — Outline (faces arrière gonflées)

```
Configuration GPU :
  - Face culling : Front (on dessine les faces ARRIÈRE — l'inverse !)
  - Depth test : activé (lecture + écriture)
  - Blending : désactivé

Vertex shader :
  1. Prend chaque vertex
  2. Le déplace le long de sa normale : position += normal * outline_width
     → Le mesh est "gonflé" de quelques millimètres/centimètres
  3. Transforme par MVP

Fragment shader :
  1. Écrit une couleur unie : outline_color
  Pas de texture, pas d'éclairage. Juste une couleur solide.
```

#### Pourquoi ça fonctionne

```
Vue de profil :

Passe 0 (faces avant)        Passe 1 (faces arrière, gonflé)   Résultat combiné
       ╭───╮                        ╭─────╮                       ╭─────╮
      │color│                      █│     │█                      █│color│█
      │color│                      █│     │█                      █│color│█
       ╰───╯                        ╰─────╯                       ╰─────╯

Les faces arrière gonflées dépassent sur les bords de l'objet.
Devant l'objet, elles sont cachées par le depth test (la passe 0 est plus proche).
Sur les bords, elles sont visibles = contour noir.
```

#### Paramètres Material (ce que l'artiste configure)

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `albedo_texture` | Texture | hero_diffuse.png | Texture de base |
| `base_color` | Vec3 | (1.0, 0.8, 0.6) | Couleur en lumière directe |
| `shadow_color` | Vec3 | (0.4, 0.3, 0.3) | Couleur en ombre |
| `shadow_threshold` | Float | 0.3 | Seuil du palier ombre/lumière |
| `outline_width` | Float | 0.02 | Épaisseur du contour (unités monde) |
| `outline_color` | Vec3 | (0.0, 0.0, 0.0) | Couleur du contour |

---

### 6.2 Verre / Transparence double-face

Pour rendre un objet transparent correctement, il faut dessiner les faces arrière (l'intérieur) avant les faces avant (l'extérieur), pour que le mélange de couleurs (blending) soit correct.

#### Passe 0 — Faces arrière (intérieur du verre)

```
Configuration GPU :
  - Face culling : Front (on dessine les faces ARRIÈRE)
  - Depth test : lecture seule (pas d'écriture de profondeur)
  - Blending : activé (SrcAlpha, OneMinusSrcAlpha)

Fragment shader :
  - Couleur teintée : glass_color × opacity
  - Éclairage simplifié (ou pas d'éclairage, selon le rendu souhaité)
```

#### Passe 1 — Faces avant (extérieur du verre)

```
Configuration GPU :
  - Face culling : Back (on dessine les faces AVANT)
  - Depth test : lecture seule (pas d'écriture de profondeur)
  - Blending : activé (SrcAlpha, OneMinusSrcAlpha)

Fragment shader :
  - Couleur teintée : glass_color × opacity
  - Effet Fresnel : les bords sont plus opaques que le centre
    → fresnel = pow(1.0 - dot(view_dir, normal), fresnel_power)
    → alpha_final = mix(opacity, 1.0, fresnel * fresnel_intensity)
```

#### Pourquoi deux passes ?

Avec une seule passe (toutes les faces d'un coup), les faces avant et arrière se battent pour l'ordre de rendu. Certains pixels de l'arrière seraient dessinés après l'avant, ce qui donne un résultat de transparence incohérent. En séparant les passes, on garantit : arrière d'abord, avant ensuite = mélange correct.

#### Paramètres Material

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `glass_color` | Vec3 | (0.8, 0.2, 0.2) | Teinte du verre |
| `opacity` | Float | 0.3 | Opacité de base |
| `fresnel_power` | Float | 3.0 | Intensité de l'effet de bord |
| `fresnel_intensity` | Float | 0.8 | Force du Fresnel |

---

### 6.3 X-Ray / Silhouette à travers les obstacles

Effet utilisé dans les jeux pour voir un personnage ou un objet derrière un mur (allié surligné, objectif visible, etc.).

#### Passe 0 — Rendu normal

```
Configuration GPU :
  - Face culling : Back
  - Depth test : activé (lecture + écriture)
  - Blending : désactivé (opaque)

Shader :
  - Forward classique (éclairage complet, textures, etc.)
```

#### Passe 1 — Silhouette des parties cachées

```
Configuration GPU :
  - Face culling : Back
  - Depth test : GREATER (on ne dessine QUE là où l'objet est DERRIÈRE un obstacle)
  - Depth write : désactivé
  - Blending : activé (additif ou alpha)

Fragment shader :
  - Couleur unie semi-transparente : xray_color × xray_opacity
  - Pas de texture, pas d'éclairage
```

#### Résultat visuel

```
    Mur                    Résultat :
  ┌──────┐                ┌──────┐
  │      │    ●           │      │░░●      ← partie devant le mur : rendu normal
  │      │   /|\    →     │      │░/|\     ← partie derrière le mur : silhouette colorée
  │      │   / \          │      │░/ \
  └──────┘                └──────┘
```

#### Paramètres Material

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `albedo_texture` | Texture | hero.png | Texture de base (passe 0) |
| `xray_color` | Vec3 | (0.2, 0.5, 1.0) | Couleur de la silhouette (bleu) |
| `xray_opacity` | Float | 0.4 | Opacité de la silhouette |

---

### 6.4 Decal projeté (impact de balle, tatouage, marque)

Un decal est une image projetée par-dessus un objet existant. L'objet est d'abord dessiné normalement, puis le decal est superposé avec du blending.

#### Passe 0 — Objet de base

```
Configuration GPU :
  - Forward classique standard
  - Depth test : activé (lecture + écriture)
```

#### Passe 1 — Decal par-dessus

```
Configuration GPU :
  - Depth test : lecture seule (LessEqual), pas d'écriture
  - Depth bias : léger offset négatif (pour éviter le z-fighting)
  - Blending : activé (SrcAlpha, OneMinusSrcAlpha)
  - Face culling : Back

Fragment shader :
  - Projette la texture du decal sur la surface
  - Applique l'opacité du decal
```

#### Paramètres Material

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `base_texture` | Texture | wall.png | Texture de l'objet de base |
| `decal_texture` | Texture | bullet_hole.png | Texture du decal |
| `decal_opacity` | Float | 0.9 | Opacité du decal |
| `decal_scale` | Vec2 | (0.1, 0.1) | Taille du decal |

---

### 6.5 Force Field / Bouclier d'énergie

Effet de champ de force sci-fi avec un aspect brillant sur les bords (effet Fresnel) et une luminosité additionnée à la scène.

#### Passe 0 — Intérieur du bouclier (faces arrière)

```
Configuration GPU :
  - Face culling : Front (faces arrière)
  - Depth test : lecture seule
  - Depth write : désactivé
  - Blending : additif (SrcAlpha, One)

Fragment shader :
  - Fresnel atténué : glow_color × fresnel × 0.5
  - Donne un effet de profondeur interne
```

#### Passe 1 — Extérieur du bouclier (faces avant)

```
Configuration GPU :
  - Face culling : Back (faces avant)
  - Depth test : lecture seule
  - Depth write : désactivé
  - Blending : additif (SrcAlpha, One)

Fragment shader :
  - Fresnel fort : glow_color × pow(fresnel, fresnel_power)
  - Les bords brillent intensément, le centre est quasi-transparent
  - Animation optionnelle : pulse = sin(time * pulse_speed) × 0.5 + 0.5
```

#### Paramètres Material

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `glow_color` | Vec3 | (0.2, 0.6, 1.0) | Couleur du champ de force (bleu) |
| `fresnel_power` | Float | 3.0 | Concentration de la brillance sur les bords |
| `pulse_speed` | Float | 2.0 | Vitesse de pulsation |
| `opacity_base` | Float | 0.1 | Opacité minimale au centre |

---

### 6.6 Fur Shells (fourrure par couches)

Technique qui dessine le même mesh **N fois** (16-32 fois) à des échelles croissantes pour simuler des couches de fourrure.

#### Passe 0 — Peau de base

```
Configuration GPU :
  - Face culling : Back
  - Depth test : activé (lecture + écriture)
  - Blending : désactivé (opaque)

Shader :
  - Forward classique
  - Dessine la surface "nue" sous la fourrure
```

#### Passes 1 à N — Couches de fourrure (shells)

```
Configuration GPU (identique pour chaque couche) :
  - Face culling : Back
  - Depth test : lecture seule
  - Depth write : désactivé
  - Blending : activé (SrcAlpha, OneMinusSrcAlpha)

Vertex shader :
  - Gonfle le mesh : position += normal × (shell_index / total_shells) × fur_length
  - Le shell 1 est juste au-dessus de la peau, le shell N est au bout des poils

Fragment shader :
  - Lit une texture de densité de poils (bruit noir et blanc)
  - Si densité < seuil_pour_cette_couche → discard (pas de poil ici)
  - Sinon → couleur du poil × ombrage
  - Les couches supérieures ont un seuil plus élevé → moins de pixels visibles
    → ça simule des poils qui s'affinent
```

#### Visualisation en coupe

```
Shell 5 (pointes) :    .   .       .         (très peu de pixels)
Shell 4 :             .. . ..     ..
Shell 3 :            ... ....   ....
Shell 2 :           ........   ......
Shell 1 (base) :   ......................
Passe 0 (peau) :  ========================   (surface solide)
```

#### Paramètres Material

| Paramètre | Type | Exemple | Description |
|-----------|------|---------|-------------|
| `fur_density_texture` | Texture | fur_noise.png | Texture de densité des poils |
| `fur_color` | Vec3 | (0.6, 0.4, 0.2) | Couleur de la fourrure |
| `fur_length` | Float | 0.05 | Longueur des poils (unités monde) |
| `fur_density` | Float | 0.8 | Densité globale des poils |
| `num_shells` | Int | 16 | Nombre de couches |
| `fur_gravity` | Vec3 | (0, -0.02, 0) | Direction de gravité sur les poils |

---

## 7. Gestion des passes dans le système de rendu

### Principe fondamental : rendre par passe, pas par objet

Quand la scène contient des objets multi-passe, le système de rendu ne dessine **pas** objet par objet. Il dessine **passe par passe** :

```
INCORRECT (par objet — changements d'état constants) :
  Objet A : pass 0 (pipeline toon) → pass 1 (pipeline outline)
  Objet B : pass 0 (pipeline toon) → pass 1 (pipeline outline)  ← re-bind !
  Objet C : pass 0 (pipeline toon) → pass 1 (pipeline outline)  ← re-bind !

CORRECT (par passe — état stable) :
  Pass 0 : Bind pipeline toon
    Draw Objet A, Draw Objet B, Draw Objet C
  Pass 1 : Bind pipeline outline
    Draw Objet A, Draw Objet B, Draw Objet C
```

### Algorithme de rendu complet

En combinant le tri par état (sort keys) et le multi-passe :

```
Pour chaque pass_index de 0 à max_passes :

    Collecter tous les objets dont le pipeline a une passe à cet index

    Trier ces objets par :
      1. Pipeline GPU (celui de cette passe)
      2. Textures / Descriptor Sets
      3. Vertex / Index Buffer
      4. Profondeur (front-to-back pour opaques, back-to-front pour transparents)

    Pour chaque groupe trié :
      Bind pipeline GPU (si différent du précédent)
      Bind textures (si différentes)
      Bind vertex/index buffer (si différent)
      Push constants (paramètres du matériau)
      Draw
```

### Gestion mixte mono-passe et multi-passe

Dans une scène typique, la majorité des objets sont mono-passe (standard forward) et quelques-uns sont multi-passe (toon, verre...). Le rendu se fait naturellement :

```
Scène exemple :
  500 objets standard forward (1 passe)
  20 objets toon + outline (2 passes)
  5 objets verre (2 passes)

Déroulement du rendu :

Pass 0 (tous les objets qui ont une passe 0 = TOUS) :
  ├── Pipeline "standard_forward" : 500 objets    ← trié par texture/mesh
  ├── Pipeline "toon_base" : 20 objets
  └── Pipeline "glass_back_faces" : 5 objets

Pass 1 (seulement les objets qui ont une passe 1 = 25 objets) :
  ├── Pipeline "toon_outline" : 20 objets
  └── Pipeline "glass_front_faces" : 5 objets

Les 500 objets standard ne participent pas à la passe 1.
```

### Ordre de rendu global

L'ordre de rendu au sein d'une frame respecte des catégories :

```
1. Objets opaques (toutes passes)
   - Triés front-to-back (les plus proches d'abord)
   - Profite du early-Z du GPU pour rejeter les pixels cachés
   - Inclut : standard, toon (passe 0 et 1), unlit, decals...

2. Objets transparents (toutes passes)
   - Triés back-to-front (les plus lointains d'abord)
   - Le blending nécessite de dessiner l'arrière-plan avant le premier plan
   - Inclut : verre (passe 0 et 1), force field, particules...
```

---

## 8. Comment les moteurs modernes gèrent ça

### Unity — Passes dans le Shader

Dans Unity, les passes sont définies dans le **Shader** (équivalent de `resource::Pipeline`), jamais dans le Material :

```
Shader "Custom/ToonOutline" {
    Properties {
        _Color ("Color", Color) = (1,1,1,1)          // paramètre Material
        _OutlineWidth ("Outline Width", Float) = 0.02 // paramètre Material
        _OutlineColor ("Outline Color", Color) = (0,0,0,1)
    }
    SubShader {
        Pass {
            Name "BASE"
            Cull Back
            // ... vertex + fragment shader pour le toon
        }
        Pass {
            Name "OUTLINE"
            Cull Front
            // ... vertex (gonflement) + fragment (couleur unie)
        }
    }
}
```

Le Material ne fait que fournir les valeurs des `Properties`. Le moteur itère les `Pass` automatiquement.

### Unreal Engine — Shading Model

Dans Unreal, le Material déclare un **Shading Model** (Default Lit, Subsurface, Unlit, Two Sided Foliage...). Le renderer sait quelles passes chaque shading model nécessite. Le Material ne décrit jamais explicitement les passes.

```
Material "M_Glass" :
  Shading Model = Translucent
  Blend Mode = Translucent
  Two Sided = true                    // → le moteur sait qu'il faut 2 passes
  Base Color = (0.8, 0.2, 0.2)
  Opacity = 0.3
```

Le moteur interprète `Two Sided + Translucent` et génère automatiquement les 2 passes (faces arrière puis faces avant).

### Point commun

Dans les deux cas, **l'artiste ne manipule jamais les passes**. Il configure des propriétés de haut niveau, et le moteur déduit les passes nécessaires.

---

## 9. Recommandations pour Galaxy3D

### Structure proposée

```
render::Pipeline        → Pipeline GPU brut (1 shader combo + 1 config rasterisation)
resource::Pipeline      → Technique de rendu (variantes × passes de render::Pipeline)
resource::Material      → Apparence (référence pipeline + textures + paramètres)
```

### Évolution du resource::Pipeline existant

Le système de variantes `Vec<PipelineVariant> + HashMap<String, usize>` existant s'étend pour supporter les passes :

```rust
pub struct PipelineVariant {
    name: String,
    passes: Vec<PipelinePass>,           // 1 passe = cas courant, N passes = multi-passe
}

pub struct PipelinePass {
    renderer_pipeline: Arc<dyn RendererPipeline>,
}
```

Un pipeline avec une seule passe (le cas à 90%) reste simple. Les passes multiples sont un cas particulier géré de manière transparente.

### Structure du futur resource::Material

```rust
pub struct Material {
    pipeline: String,                              // nom du resource::Pipeline à utiliser
    textures: HashMap<String, Arc<Texture>>,       // "albedo" → texture
    parameters: HashMap<String, MaterialParam>,    // "roughness" → 0.5
}

pub enum MaterialParam {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
}
```

### Flux de rendu simplifié

```
1. Frustum culling → liste des objets visibles
2. Pour chaque objet : lire son Material → trouver son Pipeline → connaître le nombre de passes
3. Pour chaque pass_index :
     Collecter les objets ayant une passe à cet index
     Trier par pipeline GPU → texture → mesh → profondeur
     Exécuter les draw calls
```

### Priorité d'implémentation

1. **resource::Material** basique (pipeline + textures + paramètres) — le minimum pour associer une apparence à un objet
2. **Extension PipelineVariant** avec `Vec<PipelinePass>` — transformer le champ unique en liste
3. **Pipeline mono-passe** standard forward — couvre 90% des cas
4. **Pipelines multi-passe** (toon outline, glass) — ajoutés selon les besoins

---

## Références

- **Unity ShaderLab** : documentation sur la syntaxe Pass dans les shaders Unity
- **Unreal Shading Models** : documentation sur les Shading Models et leur impact sur les passes de rendu
- **GPU Gems 2, Chapter 19** : "Fur Shell Rendering" — technique de fourrure par couches
- **Real-Time Rendering, 4th Edition** : chapitres sur le multi-pass rendering et les effets de matériaux
- Voir aussi : [rendering_techniques.md](rendering_techniques.md) pour les techniques d'optimisation du rendu (sort keys, GPU-driven, etc.)
