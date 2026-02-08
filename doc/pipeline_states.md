# Pipeline GPU : Les Fixed-Function States

> **Document technique Galaxy3D** - Guide complet des paramètres d'un pipeline de rendu
>
> Ce document explique **tous les états** (states) qu'un pipeline GPU contient,
> ce à quoi ils servent, et pourquoi ils sont importants.

---

## Table des matières

1. [C'est quoi un Pipeline GPU ?](#1-cest-quoi-un-pipeline-gpu-)
2. [Vue d'ensemble des états](#2-vue-densemble-des-états)
3. [Rasterization State](#3-rasterization-state)
4. [Depth State (Profondeur)](#4-depth-state-profondeur)
5. [Stencil State (Masque)](#5-stencil-state-masque)
6. [Color Blend State (Mélange de couleurs)](#6-color-blend-state-mélange-de-couleurs)
7. [Multisample State (Anti-aliasing)](#7-multisample-state-anti-aliasing)
8. [Récapitulatif : paramètres d'une passe de pipeline](#8-récapitulatif--paramètres-dune-passe-de-pipeline)
9. [Ce qui existe déjà dans Galaxy3D](#9-ce-qui-existe-déjà-dans-galaxy3d)
10. [Priorité d'implémentation](#10-priorité-dimplémentation)

---

## 1. C'est quoi un Pipeline GPU ?

Imagine une **chaîne de montage** dans une usine. La matière première (les vertices 3D)
entre d'un côté, et le produit fini (les pixels à l'écran) sort de l'autre.

Cette chaîne de montage a **deux types de postes** :

- **Les postes programmables** : ce sont les **shaders**. Tu écris le code qui s'y exécute.
  - Le **vertex shader** transforme chaque sommet (position, rotation, projection).
  - Le **fragment shader** calcule la couleur de chaque pixel.

- **Les postes configurables** : ce sont les **fixed-function states**. Tu ne peux pas
  les programmer, mais tu peux régler leurs paramètres, comme les boutons d'une machine.

Ce document parle uniquement des **postes configurables** (les "boutons de la machine").

### Analogie simple

Pense à un imprimeur qui produit des affiches :

| Étape | Pipeline GPU | Analogie imprimeur |
|-------|-------------|-------------------|
| Vertex Shader | Transforme les sommets 3D | Dessine le plan de l'affiche |
| Rasterization | Convertit les triangles en pixels | Découpe le plan en grille de points |
| Fragment Shader | Calcule la couleur de chaque pixel | Choisit la couleur de chaque point |
| Depth Test | Détermine ce qui est devant/derrière | Vérifie quel calque est au-dessus |
| Blending | Mélange avec ce qui est déjà dessiné | Superpose les calques transparents |
| Output | Écrit dans l'image finale | Imprime l'affiche |

---

## 2. Vue d'ensemble des états

Voici **tous les blocs d'état** qu'un pipeline GPU contient :

```
Pipeline GPU (une "passe")
│
├── Shaders (programmable)
│   ├── Vertex Shader
│   └── Fragment Shader
│
├── Input Assembly (déjà supporté)
│   ├── Topology (TriangleList, LineList, PointList...)
│   └── Vertex Layout (format des vertices)
│
├── Rasterization State          ← Section 3
│   ├── Cull Mode
│   ├── Front Face
│   ├── Polygon Mode
│   └── Depth Bias
│
├── Depth/Stencil State          ← Sections 4 et 5
│   ├── Depth Test
│   ├── Depth Write
│   ├── Depth Compare Op
│   ├── Stencil Test
│   └── Stencil Ops
│
├── Color Blend State            ← Section 6
│   ├── Blend Enable
│   ├── Blend Factors (src, dst)
│   ├── Blend Op
│   └── Color Write Mask
│
├── Multisample State            ← Section 7
│   ├── Sample Count
│   └── Alpha to Coverage
│
└── Descriptors (déjà supporté)
    ├── Push Constants
    └── Descriptor Set Layouts
```

---

## 3. Rasterization State

### C'est quoi ?

La rasterization, c'est l'étape qui transforme un **triangle 3D** (3 sommets) en une
**liste de pixels** à colorier. Le Rasterization State contrôle **comment** cette
conversion se fait.

### Les paramètres

#### 3.1 Cull Mode (Suppression de faces)

Chaque triangle a **deux faces** : une face avant (front) et une face arrière (back).
Dans la vraie vie, si tu regardes un mur, tu ne vois que le côté qui te fait face.
L'autre côté est invisible. Le GPU peut faire pareil pour gagner du temps.

| Valeur | Signification | Quand l'utiliser |
|--------|--------------|------------------|
| `None` | Dessiner les deux faces | Objets fins (feuilles, tissu, verre) |
| `Back` | Supprimer la face arrière | **Le plus courant** : objets solides (murs, personnages) |
| `Front` | Supprimer la face avant | Technique spéciale : outline par "inverted hull" |

**Pourquoi c'est important ?**

Sans culling, le GPU dessine **le double** de triangles pour rien. Sur un personnage de
50 000 triangles, ça veut dire 50 000 triangles gaspillés par frame. Multiplié par des
centaines d'objets, ça fait une grosse différence.

**Exemple concret - Outline par inverted hull (2 passes) :**
- Passe 0 : Cull `Back` (normal) → dessine l'objet normalement
- Passe 1 : Cull `Front` (inversé) + vertices agrandis → dessine seulement "l'envers"
  de l'objet, légèrement plus grand → ça crée un contour visible autour de l'objet

#### 3.2 Front Face (Sens de la face avant)

Comment le GPU détermine quelle face est "l'avant" ? Il regarde l'**ordre des sommets**
du triangle vu depuis la caméra :

| Valeur | Signification |
|--------|--------------|
| `CounterClockwise` | Sens anti-horaire = face avant (**convention standard**) |
| `Clockwise` | Sens horaire = face avant |

```
    A                A
   / \              / \
  /   \            /   \
 B-----C          C-----B

 Counter-          Clockwise
 Clockwise         (sens horaire)
 (sens anti-
  horaire)
```

En pratique, **on ne change presque jamais** ce paramètre. La convention standard est
`CounterClockwise` (OpenGL, glTF, la plupart des outils 3D).

#### 3.3 Polygon Mode (Mode de remplissage)

Comment le GPU dessine un triangle :

| Valeur | Signification | Utilisation |
|--------|--------------|-------------|
| `Fill` | Remplir le triangle | **Le mode normal** |
| `Line` | Dessiner seulement les arêtes | Mode wireframe (debug) |
| `Point` | Dessiner seulement les sommets | Debug, particules |

```
    Fill              Line              Point
   ██████            /\                  .
  ████████          /  \               .   .
 ██████████        /____\             .     .
```

#### 3.4 Depth Bias (Décalage de profondeur)

Un décalage artificiel ajouté à la profondeur de chaque fragment. Ça résout un problème
très spécifique appelé **"z-fighting"** ou **"shadow acne"**.

**Le problème :**

Imagine que tu dessines une ombre sur un sol. L'ombre est exactement à la même profondeur
que le sol. Le GPU ne sait pas lequel est devant → les pixels "scintillent" entre les
deux, créant un motif moche (comme des moirés).

```
Sans depth bias :               Avec depth bias :
┌────────────────┐              ┌────────────────┐
│ ░█░█░█░█░ acne │              │ ████████ clean │
│ █░█░█░█░█      │              │ ████████       │
│ ░█░█░█░█░      │              │ ████████       │
└────────────────┘              └────────────────┘
```

**Les paramètres :**

| Paramètre | Rôle |
|-----------|------|
| `enable` | Activer le depth bias |
| `constant_factor` | Décalage constant (en unités de profondeur) |
| `slope_factor` | Décalage proportionnel à la pente du triangle |
| `clamp` | Valeur maximale du décalage |

**Quand l'utiliser ?**
- Shadow mapping (obligatoire pour éviter le shadow acne)
- Decals (textures projetées sur les surfaces)
- Tout ce qui est coplanaire (deux surfaces au même endroit)

---

## 4. Depth State (Profondeur)

### C'est quoi ?

Le **depth buffer** (ou Z-buffer) est une image invisible qui stocke, pour chaque pixel,
la **distance** entre la caméra et l'objet le plus proche qui a été dessiné.

Le Depth State contrôle comment le GPU utilise cette information pour déterminer
**ce qui est devant et ce qui est derrière**.

### Pourquoi c'est critique ?

**Sans depth test, pas de 3D correcte.** Les objets se dessinent dans l'ordre où tu les
envoies au GPU, pas dans l'ordre de profondeur. Un objet lointain peut apparaître devant
un objet proche.

```
Sans depth test :               Avec depth test :
┌────────────────┐              ┌────────────────┐
│    ┌──┐        │              │    ┌──┐        │
│ ┌──┤  │ bsol   │              │ ┌──┤  │        │
│ │  │  │ devant │              │ │  └──┘ correct│
│ │  └──┘ fond ? │              │ │     │ le cube│
│ └─────┘        │              │ └─────┘ devant │
└────────────────┘              └────────────────┘
```

### Les paramètres

#### 4.1 Depth Test Enable

| Valeur | Signification |
|--------|--------------|
| `true` | Compare la profondeur du fragment avec le depth buffer |
| `false` | Pas de comparaison → tout passe (dessine par-dessus tout) |

**Quand désactiver ?** Pour les éléments UI/HUD qui doivent toujours être visibles,
ou pour le post-processing (effets plein écran).

#### 4.2 Depth Write Enable

| Valeur | Signification |
|--------|--------------|
| `true` | Si le test passe, mettre à jour le depth buffer avec la nouvelle profondeur |
| `false` | Lire le depth buffer mais ne pas le modifier |

**Pourquoi séparer "test" et "write" ?**

C'est le cas clé des **objets transparents** :

- Un objet opaque doit **tester ET écrire** (profondeur normale)
- Un objet transparent doit **tester mais PAS écrire**

Pourquoi ? Si un verre transparent écrit sa profondeur, les objets derrière le verre
seraient "cachés" dans le depth buffer, et ne pourraient plus être dessinés. On veut
que le verre soit devant les objets opaques (test = ON), mais qu'il n'empêche pas les
autres transparents de se dessiner derrière (write = OFF).

```
Scène : mur ← verre ← caméra

Depth write ON pour le verre :
  Mur dessiné ✓ (depth = 10)
  Verre dessiné ✓ (depth = 5, écrit dans depth buffer)
  Objet derrière le verre ? ✗ BLOQUÉ (depth = 8 > 5)

Depth write OFF pour le verre :
  Mur dessiné ✓ (depth = 10)
  Verre dessiné ✓ (depth = 5, mais N'ÉCRIT PAS)
  Objet derrière le verre ? ✓ VISIBLE (depth = 8 < 10, compare avec le mur)
```

#### 4.3 Depth Compare Op (Opérateur de comparaison)

La question que le GPU se pose : "Est-ce que ce nouveau fragment passe le test ?"

| Opérateur | Condition | Usage typique |
|-----------|-----------|---------------|
| `Less` | nouveau < existant | **Le plus courant** : le plus proche gagne |
| `LessOrEqual` | nouveau <= existant | Skybox (dessinée à profondeur max) |
| `Greater` | nouveau > existant | Rendu inversé (le plus loin gagne) |
| `GreaterOrEqual` | nouveau >= existant | Rare |
| `Equal` | nouveau == existant | Decals (dessiner exactement sur une surface) |
| `NotEqual` | nouveau != existant | Effets spéciaux |
| `Always` | toujours vrai | Comme si depth test était désactivé |
| `Never` | toujours faux | Rien ne passe (debug) |

**Combinaisons courantes :**

| Type d'objet | Depth Test | Depth Write | Compare |
|-------------|-----------|-------------|---------|
| Objet opaque | ON | ON | `Less` |
| Objet transparent | ON | **OFF** | `Less` |
| Skybox | ON | OFF | `LessOrEqual` |
| Decal | ON | OFF | `Equal` |
| UI / HUD | OFF | OFF | - |
| Post-processing | OFF | OFF | - |

---

## 5. Stencil State (Masque)

### C'est quoi ?

Le **stencil buffer** est un autre buffer invisible (comme le depth buffer), mais au lieu
de stocker une profondeur, il stocke un **nombre entier de 0 à 255** par pixel.

C'est comme un **pochoir** (stencil en anglais) : tu peux marquer des zones de l'écran,
puis décider de ne dessiner que dans les zones marquées (ou non marquées).

### Analogie

Imagine que tu peins un mur :

1. Tu colles un ruban de masquage (masking tape) sur les bords des fenêtres
2. Tu peins le mur
3. Tu enlèves le ruban → les fenêtres sont propres

Le stencil buffer, c'est le ruban de masquage numérique. Tu "marques" des pixels
(en écrivant des valeurs dans le stencil), puis tu dessines seulement là où le marquage
correspond à ce que tu veux.

### Les paramètres

#### 5.1 Stencil Test Enable

| Valeur | Signification |
|--------|--------------|
| `true` | Comparer la valeur du stencil avec une référence |
| `false` | Pas de test stencil (le plus courant) |

#### 5.2 Compare Op, Reference, Masks

Comme le depth test, le stencil compare une **valeur de référence** avec la **valeur
dans le stencil buffer** :

| Paramètre | Rôle |
|-----------|------|
| `compare_op` | Opérateur (Equal, NotEqual, Less, Always...) |
| `reference` | La valeur à comparer (0-255) |
| `compare_mask` | Masque de bits pour la comparaison |
| `write_mask` | Masque de bits pour l'écriture |

**Formule :** `(reference & compare_mask) compare_op (stencil_value & compare_mask)`

#### 5.3 Stencil Operations

"Que faire avec la valeur du stencil selon le résultat des tests ?"

Il y a **3 situations** :

| Situation | Paramètre | Quand ? |
|-----------|-----------|---------|
| Stencil test échoue | `fail_op` | Le pixel ne passe pas le test stencil |
| Stencil OK, depth échoue | `depth_fail_op` | Stencil OK mais profondeur échoue |
| Tout passe | `pass_op` | Les deux tests sont OK |

Les **opérations possibles** :

| Opération | Effet |
|-----------|-------|
| `Keep` | Ne rien changer |
| `Zero` | Mettre à 0 |
| `Replace` | Remplacer par la valeur de référence |
| `IncrementClamp` | +1 (plafonné à 255) |
| `DecrementClamp` | -1 (plancher à 0) |
| `Invert` | Inverser les bits |
| `IncrementWrap` | +1 (revient à 0 après 255) |
| `DecrementWrap` | -1 (revient à 255 après 0) |

#### 5.4 Front et Back

Le stencil peut avoir des paramètres **différents** pour la face avant et la face arrière
d'un triangle. C'est utile pour les shadow volumes (technique d'ombres).

### Exemples concrets

#### Exemple 1 : Outline / Silhouette

```
Passe 0 - Écrire le masque :
  Dessiner l'objet normalement
  stencil pass_op = Replace, reference = 1
  → Le stencil vaut 1 partout où l'objet est dessiné

Passe 1 - Dessiner l'outline :
  Dessiner l'objet agrandi (scale +5%)
  stencil compare_op = NotEqual, reference = 1
  → Ne dessine QUE là où le stencil N'EST PAS 1
  → Ça dessine seulement la bordure qui dépasse = outline !
```

```
Stencil buffer :         Résultat :
┌────────────────┐       ┌────────────────┐
│    00000000    │       │                │
│  001111110000  │       │  ██████████    │
│  011111111000  │       │  █        █    │
│  011111111000  │       │  █  objet █    │
│  001111110000  │       │  █        █    │
│    00000000    │       │  ██████████    │
└────────────────┘       └── outline ─────┘
```

#### Exemple 2 : Miroir / Portail

```
Passe 0 - Marquer la zone du miroir :
  Dessiner le quad du miroir
  stencil pass_op = Replace, reference = 1
  color_write_mask = NONE (on n'écrit pas de couleur, juste le stencil)

Passe 1 - Dessiner le monde reflété :
  stencil compare_op = Equal, reference = 1
  → Le monde reflété ne se dessine QUE dans la zone du miroir
```

#### Exemple 3 : X-Ray / Silhouette à travers les murs

```
Passe 0 - Dessiner l'objet normalement :
  depth_test = ON, depth_write = ON
  stencil pass_op = Replace, reference = 1

Passe 1 - Dessiner la silhouette derrière les murs :
  depth_compare = Greater (ne dessiner QUE si l'objet est DERRIÈRE quelque chose)
  stencil compare_op = NotEqual, reference = 1 (ne pas redessiner sur l'objet normal)
  → Dessine une couleur unie là où l'objet est caché
```

---

## 6. Color Blend State (Mélange de couleurs)

### C'est quoi ?

Quand le GPU écrit un pixel, il peut **mélanger** la nouvelle couleur (source) avec la
couleur déjà présente dans l'image (destination). C'est le **blending**.

Sans blending, chaque pixel écrase complètement le précédent. Avec blending, les couleurs
se combinent selon une formule mathématique.

### La formule

```
Couleur finale = (srcFactor × srcColor)  [op]  (dstFactor × dstColor)
Alpha   finale = (srcAlphaFactor × srcAlpha) [op] (dstAlphaFactor × dstAlpha)
```

Où :
- `srcColor` = la couleur du fragment qui vient d'être calculée par le shader
- `dstColor` = la couleur qui est déjà dans le framebuffer (l'image)
- `srcFactor` / `dstFactor` = des multiplicateurs
- `op` = une opération mathématique (Add, Subtract, etc.)

### Les paramètres

#### 6.1 Blend Enable

| Valeur | Signification |
|--------|--------------|
| `true` | Mélanger les couleurs avec la formule |
| `false` | Écraser complètement (le plus courant pour les objets opaques) |

#### 6.2 Blend Factors (Multiplicateurs)

Les facteurs les plus importants :

| Facteur | Valeur | Explication simple |
|---------|--------|-------------------|
| `Zero` | 0 | Annule complètement cette composante |
| `One` | 1 | Prend la composante telle quelle |
| `SrcAlpha` | alpha du fragment | Pondère par la transparence du fragment |
| `OneMinusSrcAlpha` | 1 - alpha du fragment | L'inverse : pondère par l'opacité |
| `DstColor` | couleur du framebuffer | Multiplie par ce qui est déjà dessiné |
| `SrcColor` | couleur du fragment | Multiplie par sa propre couleur |
| `OneMinusSrcColor` | 1 - couleur du fragment | Inverse de la couleur source |
| `DstAlpha` | alpha du framebuffer | Pondère par l'alpha du framebuffer |
| `OneMinusDstAlpha` | 1 - alpha du framebuffer | Inverse |
| `ConstantColor` | couleur constante | Une couleur fixée en dehors du shader |

#### 6.3 Blend Op (Opération)

| Opération | Formule | Usage |
|-----------|---------|-------|
| `Add` | src + dst | **Le plus courant** |
| `Subtract` | src - dst | Effets de soustraction |
| `ReverseSubtract` | dst - src | Effets inversés |
| `Min` | min(src, dst) | Minimum des deux |
| `Max` | max(src, dst) | Maximum des deux |

### Modes de blending courants

#### Alpha Blend (transparence classique)

```
srcFactor = SrcAlpha
dstFactor = OneMinusSrcAlpha
op = Add

Formule : final = (alpha × src) + ((1-alpha) × dst)
```

C'est la transparence que tout le monde connaît. Si alpha = 0.5, on mélange 50/50.
Si alpha = 0.0, le fragment est invisible. Si alpha = 1.0, il est opaque.

```
Exemple avec alpha = 0.3 (30% opaque) :
  Source (rouge)  : (1.0, 0.0, 0.0) × 0.3 = (0.3, 0.0, 0.0)
  Dest (bleu)     : (0.0, 0.0, 1.0) × 0.7 = (0.0, 0.0, 0.7)
  Résultat        : (0.3, 0.0, 0.7) → violet tirant vers le bleu
```

#### Additive (lumière, feu, laser, particules)

```
srcFactor = One
dstFactor = One
op = Add

Formule : final = src + dst
```

Les couleurs s'**additionnent**. Parfait pour tout ce qui émet de la lumière, car
la lumière s'additionne dans la vraie vie.

```
Exemple :
  Source (orange)  : (1.0, 0.5, 0.0)
  Dest (bleu)      : (0.0, 0.0, 0.8)
  Résultat         : (1.0, 0.5, 0.8) → plus lumineux !
```

Note : le résultat peut dépasser 1.0 (clampé automatiquement par le GPU).
C'est pour ça que les effets additifs **éclaircissent toujours** l'image.

#### Multiplicative (ombres, assombrissement)

```
srcFactor = DstColor
dstFactor = Zero
op = Add

Formule : final = src × dst
```

Chaque composante de la source **multiplie** la destination. Le résultat est toujours
**plus sombre** (car on multiplie par des valeurs entre 0 et 1).

```
Exemple (ombre à 50%) :
  Source (gris)  : (0.5, 0.5, 0.5)
  Dest (vert)    : (0.0, 0.8, 0.2)
  Résultat       : (0.0, 0.4, 0.1) → le vert est assombri de moitié
```

#### Screen (éclaircissement doux)

```
srcFactor = One
dstFactor = OneMinusSrcColor
op = Add

Formule : final = src + dst × (1 - src)
```

L'inverse du mode multiplicatif. Le résultat est toujours **plus clair**, mais de
façon plus douce que l'additif (ne sature pas autant).

#### Pre-multiplied Alpha (UI, textures de qualité)

```
srcFactor = One
dstFactor = OneMinusSrcAlpha
op = Add

Formule : final = src + dst × (1 - srcAlpha)
```

Comme l'alpha blend classique, mais la couleur source est **déjà multipliée par son
alpha** dans la texture. C'est le format utilisé par la plupart des frameworks UI et
les systèmes de particules modernes, car il gère mieux les bords et le filtrage.

#### 6.4 Color Write Mask

Contrôle **quels canaux** de couleur sont écrits dans le framebuffer :

| Masque | Canaux écrits | Usage |
|--------|--------------|-------|
| `RGBA` | Tous | **Le plus courant** |
| `RGB` | Rouge, Vert, Bleu (pas alpha) | Garder l'alpha existant |
| `A` | Alpha seulement | Préparer un masque alpha |
| `None` | Rien du tout | Écrire seulement dans depth/stencil |

**Exemple :** la passe 0 d'un miroir dessine le quad du miroir avec `color_write = None`
→ pas de couleur visible, mais le stencil est mis à jour.

---

## 7. Multisample State (Anti-aliasing)

### C'est quoi ?

Le **multisampling** (MSAA) est une technique d'anti-aliasing qui prend **plusieurs
échantillons par pixel** pour adoucir les bords des triangles.

### Le problème sans anti-aliasing

```
Sans MSAA (1 sample) :          Avec MSAA 4x :
┌──────────────────┐            ┌──────────────────┐
│ ████             │            │ ████▓            │
│ ██████           │            │ ██████▓          │
│ ████████         │            │ ████████▓        │
│ ██████████       │            │ ██████████▓      │
│ bords en escalier│            │ bords lisses     │
└──────────────────┘            └──────────────────┘
```

Les bords des triangles ont des "marches d'escalier" (aliasing). Le MSAA réduit
cet effet en vérifiant la couverture à plusieurs points dans chaque pixel.

### Les paramètres

#### 7.1 Sample Count

| Valeur | Échantillons par pixel | Qualité | Coût mémoire |
|--------|----------------------|---------|--------------|
| `1` | 1 (pas de MSAA) | Base | ×1 |
| `2` | 2 | Léger lissage | ×2 |
| `4` | 4 | **Bon compromis** | ×4 |
| `8` | 8 | Très lisse | ×8 |

**Le plus utilisé** : 4x MSAA. C'est le compromis qualité/performance habituel.
Au-delà de 8x, le gain visuel est négligeable.

#### 7.2 Alpha to Coverage

Un mode spécial qui convertit l'alpha du fragment en **masque de couverture** MSAA.

**À quoi ça sert ?** Au feuillage (herbe, arbres, grillages).

Le problème : le feuillage utilise des textures avec alpha (alpha-test / alpha-cutoff)
pour créer des formes complexes à partir de simples quads. Mais les bords du cutoff
sont aliasés (en escalier).

```
Sans alpha to coverage :        Avec alpha to coverage :
  Alpha < 0.5 → pixel OFF        Alpha = 0.3 → 1/4 des samples ON
  Alpha >= 0.5 → pixel ON        Alpha = 0.7 → 3/4 des samples ON
  → bords durs                   → bords progressifs
```

**C'est très utile** mais ne fonctionne qu'avec le MSAA actif.

---

## 8. Récapitulatif : paramètres d'une passe de pipeline

Voici **tous les paramètres** qu'une passe (`PipelinePassDesc`) pourrait contenir,
organisés par bloc :

```
PipelinePassDesc
│
├── pipeline: PipelineDesc (render)
│   │
│   ├── SHADERS (existant)
│   │   ├── vertex_shader
│   │   └── fragment_shader
│   │
│   ├── INPUT (existant)
│   │   ├── vertex_layout
│   │   └── topology
│   │
│   ├── RASTERIZATION STATE
│   │   ├── cull_mode          : CullMode (None, Front, Back)
│   │   ├── front_face         : FrontFace (CounterClockwise, Clockwise)
│   │   ├── polygon_mode       : PolygonMode (Fill, Line, Point)
│   │   └── depth_bias         : Option<DepthBias> { constant, slope, clamp }
│   │
│   ├── DEPTH STATE
│   │   ├── depth_test_enable  : bool
│   │   ├── depth_write_enable : bool
│   │   └── depth_compare_op   : CompareOp (Less, Greater, Equal, Always...)
│   │
│   ├── STENCIL STATE
│   │   ├── stencil_test_enable : bool
│   │   ├── front              : StencilOpState { fail, pass, depth_fail, compare, ref, masks }
│   │   └── back               : StencilOpState { ... }
│   │
│   ├── COLOR BLEND STATE
│   │   ├── blend_enable        : bool (existant : enable_blending)
│   │   ├── src_color_factor    : BlendFactor
│   │   ├── dst_color_factor    : BlendFactor
│   │   ├── color_blend_op      : BlendOp
│   │   ├── src_alpha_factor    : BlendFactor
│   │   ├── dst_alpha_factor    : BlendFactor
│   │   ├── alpha_blend_op      : BlendOp
│   │   └── color_write_mask    : ColorWriteMask (R, G, B, A)
│   │
│   ├── MULTISAMPLE STATE
│   │   ├── sample_count        : SampleCount (1, 2, 4, 8)
│   │   └── alpha_to_coverage   : bool
│   │
│   └── DESCRIPTORS (existant)
│       ├── push_constant_ranges
│       └── descriptor_set_layouts
```

---

## 9. Ce qui existe déjà dans Galaxy3D

| Bloc | État | Détail |
|------|------|--------|
| Shaders | OK | vertex + fragment |
| Input Assembly | OK | topology + vertex_layout |
| Push Constants | OK | push_constant_ranges |
| Descriptor Sets | OK | descriptor_set_layouts |
| Blend Enable | Partiel | `enable_blending` (bool) → mode alpha blend hardcodé |
| Rasterization | Hardcodé | CullMode=None, Fill, CCW, pas de depth bias |
| Depth/Stencil | Absent | Pas du tout implémenté |
| Multisample | Hardcodé | 1 sample, pas d'alpha to coverage |

---

## 10. Priorité d'implémentation

Voici l'ordre recommandé pour ajouter ces états, du plus urgent au moins urgent :

### Priorité 1 - Critique (nécessaire pour le rendu 3D)

| État | Pourquoi |
|------|----------|
| **Depth test/write/compare** | Sans ça, impossible de rendre correctement en 3D |
| **Cull mode** | Double le travail GPU si absent |

### Priorité 2 - Haute (nécessaire pour les effets visuels courants)

| État | Pourquoi |
|------|----------|
| **Blend factors configurables** | Additif, multiplicatif, etc. pour particules/effets |
| **Color write mask** | Nécessaire pour le stencil-only pass |
| **Front face** | Nécessaire si on importe des modèles avec convention différente |

### Priorité 3 - Moyenne (nécessaire pour les effets avancés)

| État | Pourquoi |
|------|----------|
| **Stencil state** | Outlines, miroirs, portails, shadow volumes |
| **Depth bias** | Shadow mapping, decals |
| **Polygon mode** | Debug wireframe |

### Priorité 4 - Basse (amélioration de qualité)

| État | Pourquoi |
|------|----------|
| **MSAA sample count** | Anti-aliasing matériel |
| **Alpha to coverage** | Feuillage de qualité |

---

> **Note :** Ce document décrit l'état cible. L'implémentation se fera progressivement
> en commençant par les priorités les plus hautes.
