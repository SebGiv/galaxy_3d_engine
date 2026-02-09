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
8. [États Fixes vs Dynamiques](#8-états-fixes-vs-dynamiques)
9. [Récapitulatif : paramètres d'une passe de pipeline](#9-récapitulatif--paramètres-dune-passe-de-pipeline)
10. [Paramètres complets d'un pipeline Vulkan](#10-paramètres-complets-dun-pipeline-vulkan)
11. [Ce qui existe déjà dans Galaxy3D](#11-ce-qui-existe-déjà-dans-galaxy3d)
12. [Proposition d'implémentation pour Galaxy3D](#12-proposition-dimplémentation-pour-galaxy3d)
13. [Approche progressive d'implémentation](#13-approche-progressive-dimplémentation)
14. [Priorité d'implémentation des états](#14-priorité-dimplémentation-des-états)

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

## 8. États Fixes vs Dynamiques

### Le principe fondamental

Quand tu crées un pipeline GPU, **tous les états sont compilés** en un seul objet
binaire optimisé par le driver. Après création, tu ne peux plus rien changer. C'est
comme graver un CD : une fois gravé, c'est fini.

```
Pipeline A (compilé) :
  ├── Vertex Shader : model_vert.spv
  ├── Fragment Shader : model_frag.spv
  ├── Depth Test : ON
  ├── Cull Mode : Back
  └── Blend : OFF

  → Objet binaire figé. Impossible de modifier le depth test.

Si tu veux le même pipeline SANS depth test :
  → Tu dois créer un Pipeline B distinct.
```

**Pourquoi ?** Pour la performance. Le GPU a besoin de savoir à l'avance exactement
quelle configuration utiliser. En compilant tout en un bloc, le driver peut pré-calculer
le chemin optimal dans le hardware.

### L'exception : les Dynamic States

Vulkan permet de déclarer certains paramètres comme **dynamiques** à la création du
pipeline. Ces paramètres ne sont pas compilés — ils sont définis au moment du rendu
par des commandes.

```
Création du pipeline :
  ├── cullMode : Back                    ← FIXE (compilé, optimisé)
  ├── depthTestEnable : true             ← FIXE (compilé, optimisé)
  ├── depthBias : ???                    ← DYNAMIQUE (défini au rendu)
  ├── viewport : ???                     ← DYNAMIQUE (défini au rendu)
  └── dynamicStates : [Viewport, DepthBias]

Au moment du rendu :
  cmd.set_viewport(...)      ← Obligatoire, pas de valeur par défaut
  cmd.set_depth_bias(2.0, 0.0, 1.5)  ← Obligatoire aussi
```

### Fixe vs Dynamique : le compromis

| | Fixe | Dynamique |
|---|------|-----------|
| **Performance** | Micro-optimisé par le driver | Légèrement moins optimisé |
| **Flexibilité** | Changer = recompiler un pipeline | Changer = une simple commande |
| **Nombre de pipelines** | Plus de combinaisons = plus de pipelines | Moins de pipelines nécessaires |

Le vrai coût n'est pas la micro-optimisation, c'est le **changement de pipeline** entre
deux draw calls. Si rendre un paramètre dynamique permet d'utiliser 1 pipeline au lieu
de 5, on gagne plus qu'on ne perd.

```
Stratégie A (tout fixe) :
  Draw objet 1 → Pipeline A (cull=Back, depth=ON)
  Draw objet 2 → Pipeline B (cull=Back, depth=OFF)    ← changement !
  Draw objet 3 → Pipeline A                           ← changement !
  = 2 changements de pipeline (coûteux)

Stratégie B (depth dynamique) :
  Draw objet 1 → Pipeline X + cmd.set(depth=ON)
  Draw objet 2 → cmd.set(depth=OFF)                   ← simple commande
  Draw objet 3 → cmd.set(depth=ON)                    ← simple commande
  = 0 changement de pipeline
```

### Dynamic States disponibles en Vulkan

#### Vulkan 1.0 (disponibles partout)

| Dynamic State | Commande | Utilité |
|--------------|----------|---------|
| Viewport | `set_viewport` | Zone de rendu (quasi obligatoire) |
| Scissor | `set_scissor` | Rectangle de clipping (quasi obligatoire) |
| LineWidth | `set_line_width` | Épaisseur des lignes |
| DepthBias | `set_depth_bias` | Valeurs du bias (constant, slope, clamp) |
| BlendConstants | `set_blend_constants` | Couleur pour ConstantColor blend factor |
| DepthBounds | `set_depth_bounds` | Min/max du depth bounds test |
| StencilCompareMask | `set_stencil_compare_mask` | Masque de comparaison stencil |
| StencilWriteMask | `set_stencil_write_mask` | Masque d'écriture stencil |
| StencilReference | `set_stencil_reference` | Valeur de référence stencil |

#### Vulkan 1.3 (nécessite Vulkan 1.3)

| Dynamic State | Commande | Utilité |
|--------------|----------|---------|
| CullMode | `set_cull_mode` | Changer le culling sans recompiler |
| FrontFace | `set_front_face` | Changer le sens de la face avant |
| PrimitiveTopology | `set_primitive_topology` | Changer triangle/line/point |
| DepthTestEnable | `set_depth_test_enable` | Activer/désactiver le depth test |
| DepthWriteEnable | `set_depth_write_enable` | Activer/désactiver le depth write |
| DepthCompareOp | `set_depth_compare_op` | Changer l'opérateur de comparaison |
| StencilTestEnable | `set_stencil_test_enable` | Activer/désactiver le stencil |
| StencilOp | `set_stencil_op` | Changer les opérations stencil |
| RasterizerDiscardEnable | `set_rasterizer_discard` | Skip la rasterization |
| DepthBiasEnable | `set_depth_bias_enable` | Activer/désactiver le depth bias |

#### Extensions récentes (VK_EXT_extended_dynamic_state3)

| Dynamic State | Utilité |
|--------------|---------|
| PolygonMode | Fill/Line/Point dynamique |
| RasterizationSamples | Sample count MSAA dynamique |
| ColorBlendEnable | Blending on/off dynamique |
| ColorBlendEquation | Factors + op dynamiques |
| ColorWriteMask | Masque R/G/B/A dynamique |
| AlphaToCoverageEnable | Alpha to coverage dynamique |

### Résumé : ce qui est toujours fixe, ce qui peut être dynamique

| Catégorie | Toujours FIXE | Dynamique possible |
|-----------|--------------|-------------------|
| **Shaders** | Toujours fixe | Jamais dynamique |
| **Pipeline Layout** | Toujours fixe | Jamais dynamique |
| **Render Pass** | Toujours fixe | Jamais dynamique |
| **Vertex Input** | Par défaut fixe | Dynamique (extensions récentes) |
| **Viewport/Scissor** | Par défaut fixe | Dynamique depuis Vulkan 1.0 |
| **Rasterization** | Par défaut fixe | Tout dynamique (Vulkan 1.3+) |
| **Depth/Stencil** | Par défaut fixe | Tout dynamique (Vulkan 1.3) |
| **Color Blend** | Par défaut fixe | Tout dynamique (extensions récentes) |
| **Multisample** | Par défaut fixe | Tout dynamique (extensions récentes) |

Avec les extensions les plus récentes, quasi tout peut être dynamique sauf les shaders,
le pipeline layout et le render pass. Mais en pratique, la plupart des moteurs ne
rendent dynamiques que viewport, scissor, et quelques paramètres ciblés.

### Note sur le depth bias

Le depth bias a une particularité : on peut séparer **l'activation** et **les valeurs**.

- `depth_bias_enable` (ON/OFF) → fixe dans le pipeline (ou dynamique en Vulkan 1.3)
- `constant_factor`, `slope_factor`, `clamp` → dynamiques depuis Vulkan 1.0

Cela permet de créer un seul pipeline avec depth bias activé, et d'ajuster les valeurs
au moment du rendu selon la scène (distance de la lumière, taille de la shadow map...).

---

## 9. Récapitulatif : paramètres d'une passe de pipeline

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

## 10. Paramètres complets d'un pipeline Vulkan

Liste exhaustive de **tous les paramètres fixes** d'un pipeline graphique Vulkan :

### Input Assembly (existant dans Galaxy3D)

| Paramètre | Valeurs |
|-----------|---------|
| `topology` | PointList, LineList, LineStrip, TriangleList, TriangleStrip, TriangleFan, PatchList |
| `primitiveRestartEnable` | true / false |

### Vertex Input (existant dans Galaxy3D)

| Paramètre | Description |
|-----------|-------------|
| `vertexBindingDescriptions` | Liste des bindings (stride, input rate) |
| `vertexAttributeDescriptions` | Liste des attributs (location, format, offset) |

### Rasterization

| Paramètre | Valeurs | Défaut recommandé |
|-----------|---------|-------------------|
| `cullMode` | None, Front, Back, FrontAndBack | Back |
| `frontFace` | CounterClockwise, Clockwise | CounterClockwise |
| `polygonMode` | Fill, Line, Point | Fill |
| `depthBiasEnable` | true / false | false |
| `depthBiasConstantFactor` | f32 | 0.0 |
| `depthBiasClamp` | f32 | 0.0 |
| `depthBiasSlopeFactor` | f32 | 0.0 |
| `lineWidth` | f32 | 1.0 |
| `depthClampEnable` | true / false | false |
| `rasterizerDiscardEnable` | true / false | false |

### Depth/Stencil

| Paramètre | Valeurs | Défaut recommandé |
|-----------|---------|-------------------|
| `depthTestEnable` | true / false | true |
| `depthWriteEnable` | true / false | true |
| `depthCompareOp` | Never, Less, Equal, LessOrEqual, Greater, NotEqual, GreaterOrEqual, Always | Less |
| `depthBoundsTestEnable` | true / false | false |
| `stencilTestEnable` | true / false | false |
| `front/back.failOp` | Keep, Zero, Replace, IncrementAndClamp, DecrementAndClamp, Invert, IncrementAndWrap, DecrementAndWrap | Keep |
| `front/back.passOp` | (idem) | Keep |
| `front/back.depthFailOp` | (idem) | Keep |
| `front/back.compareOp` | (comme depthCompareOp) | Always |
| `front/back.compareMask` | u32 | 0xFF |
| `front/back.writeMask` | u32 | 0xFF |
| `front/back.reference` | u32 | 0 |

### Color Blend (par attachment)

| Paramètre | Valeurs | Défaut recommandé |
|-----------|---------|-------------------|
| `blendEnable` | true / false | false |
| `srcColorBlendFactor` | Zero, One, SrcColor, OneMinusSrcColor, DstColor, OneMinusDstColor, SrcAlpha, OneMinusSrcAlpha, DstAlpha, OneMinusDstAlpha, ConstantColor, OneMinusConstantColor, SrcAlphaSaturate | SrcAlpha |
| `dstColorBlendFactor` | (idem) | OneMinusSrcAlpha |
| `colorBlendOp` | Add, Subtract, ReverseSubtract, Min, Max | Add |
| `srcAlphaBlendFactor` | (idem) | One |
| `dstAlphaBlendFactor` | (idem) | Zero |
| `alphaBlendOp` | Add, Subtract, ReverseSubtract, Min, Max | Add |
| `colorWriteMask` | combinaison R, G, B, A | RGBA |

### Multisample

| Paramètre | Valeurs | Défaut recommandé |
|-----------|---------|-------------------|
| `rasterizationSamples` | 1, 2, 4, 8, 16, 32, 64 | 1 |
| `sampleShadingEnable` | true / false | false |
| `alphaToCoverageEnable` | true / false | false |
| `alphaToOneEnable` | true / false | false |

### Toujours fixes (jamais dynamiques)

| Paramètre | Description |
|-----------|-------------|
| Shaders | Vertex, Fragment (+ optionnels : Tessellation, Geometry) |
| Pipeline Layout | Descriptor set layouts + push constant ranges |
| Render Pass | Compatible render pass + subpass index |

---

## 11. Ce qui existe déjà dans Galaxy3D

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

Le `render::PipelineDesc` actuel :

```rust
pub struct PipelineDesc {
    pub vertex_shader: Arc<dyn Shader>,
    pub fragment_shader: Arc<dyn Shader>,
    pub vertex_layout: VertexLayout,
    pub topology: PrimitiveTopology,
    pub push_constant_ranges: Vec<PushConstantRange>,
    pub descriptor_set_layouts: Vec<u64>,
    pub enable_blending: bool,           // ← seul état configurable
}
```

Tout le reste est hardcodé dans le renderer Vulkan (`create_pipeline()`).

---

## 12. Proposition d'implémentation pour Galaxy3D

### Architecture : structs par bloc avec `Default`

Chaque bloc d'état est représenté par une struct dédiée, avec des valeurs par défaut
sensées. Ainsi, le code existant reste simple et les nouveaux paramètres sont
opt-in.

```rust
// ===== RASTERIZATION STATE =====

pub struct RasterizationState {
    pub cull_mode: CullMode,           // Default: Back
    pub front_face: FrontFace,         // Default: CounterClockwise
    pub polygon_mode: PolygonMode,     // Default: Fill
    pub depth_bias: Option<DepthBias>, // Default: None (désactivé)
}

// ===== DEPTH/STENCIL STATE =====

pub struct DepthStencilState {
    pub depth_test_enable: bool,       // Default: true
    pub depth_write_enable: bool,      // Default: true
    pub depth_compare_op: CompareOp,   // Default: Less
    pub stencil_test_enable: bool,     // Default: false
    pub front: StencilOpState,         // Default: Keep/Keep/Keep/Always
    pub back: StencilOpState,          // Default: Keep/Keep/Keep/Always
}

// ===== COLOR BLEND STATE =====

pub struct ColorBlendState {
    pub blend_enable: bool,            // Default: false (opaque)
    pub src_color_factor: BlendFactor, // Default: SrcAlpha
    pub dst_color_factor: BlendFactor, // Default: OneMinusSrcAlpha
    pub color_blend_op: BlendOp,       // Default: Add
    pub src_alpha_factor: BlendFactor, // Default: One
    pub dst_alpha_factor: BlendFactor, // Default: Zero
    pub alpha_blend_op: BlendOp,       // Default: Add
    pub color_write_mask: ColorWriteMask, // Default: RGBA
}

// ===== MULTISAMPLE STATE =====

pub struct MultisampleState {
    pub sample_count: SampleCount,     // Default: 1 (pas de MSAA)
    pub alpha_to_coverage: bool,       // Default: false
}
```

### Le `PipelineDesc` cible

```rust
pub struct PipelineDesc {
    // --- Existant (inchangé) ---
    pub vertex_shader: Arc<dyn Shader>,
    pub fragment_shader: Arc<dyn Shader>,
    pub vertex_layout: VertexLayout,
    pub topology: PrimitiveTopology,
    pub push_constant_ranges: Vec<PushConstantRange>,
    pub descriptor_set_layouts: Vec<u64>,

    // --- Nouveau (remplace enable_blending) ---
    pub rasterization: RasterizationState,    // cull, front face, polygon mode, depth bias
    pub depth_stencil: DepthStencilState,     // depth test/write, stencil
    pub color_blend: ColorBlendState,         // blending, color write mask
    pub multisample: MultisampleState,        // MSAA, alpha to coverage
}
```

`enable_blending: bool` disparaît, remplacé par `color_blend.blend_enable` et tous
les facteurs/opérations associés.

### Utilisation avec `..Default::default()`

Grâce aux `impl Default`, la syntaxe Rust permet de ne spécifier que ce qui change :

```rust
// Pipeline opaque classique (toutes les valeurs par défaut sont parfaites)
rasterization: RasterizationState::default(),   // cull=Back, Fill, CCW
depth_stencil: DepthStencilState::default(),    // depth ON, write ON, Less
color_blend: ColorBlendState::default(),        // blend OFF, write RGBA
multisample: MultisampleState::default(),       // 1 sample

// Pipeline transparent (un seul champ change par bloc)
depth_stencil: DepthStencilState {
    depth_write_enable: false,                  // seul changement
    ..Default::default()
},
color_blend: ColorBlendState {
    blend_enable: true,                         // les factors sont déjà en alpha blend
    ..Default::default()
},

// Pipeline additif (particules feu)
color_blend: ColorBlendState {
    blend_enable: true,
    src_color_factor: BlendFactor::One,
    dst_color_factor: BlendFactor::One,
    ..Default::default()
},

// Pipeline outline passe 1 (cull front + stencil)
rasterization: RasterizationState {
    cull_mode: CullMode::Front,
    ..Default::default()
},
depth_stencil: DepthStencilState {
    stencil_test_enable: true,
    front: StencilOpState {
        compare_op: CompareOp::NotEqual,
        reference: 1,
        ..Default::default()
    },
    ..Default::default()
},
```

### Impact sur `resource::PipelinePassDesc`

Aucun changement nécessaire au niveau resource. Le `PipelinePassDesc` wrapper
simplement le `render::PipelineDesc`, qui contient déjà tout :

```rust
pub struct PipelinePassDesc {
    pub pipeline: RenderPipelineDesc,  // inchangé, contient les nouveaux blocs
}
```

---

## 13. Approche progressive d'implémentation

### Étape 1 — Fixed-function states (maintenant)

Ajouter les enums, structs et `Default` dans `render::pipeline.rs`, modifier
`PipelineDesc`, adapter le renderer Vulkan pour utiliser les vraies valeurs au lieu
du hardcodé. Viewport et Scissor restent les seuls dynamic states (déjà le cas).

| Tâche | Détail |
|-------|--------|
| Ajouter les types | Enums (CullMode, CompareOp, BlendFactor...) + Structs (RasterizationState...) |
| Modifier `PipelineDesc` | Remplacer `enable_blending` par les 4 blocs d'état |
| Adapter le renderer Vulkan | `create_pipeline()` utilise les valeurs du desc au lieu du hardcodé |
| Mettre à jour les tests + démo | Migration vers la nouvelle API |

### Étape 2 — Dynamic states basiques (plus tard, Vulkan 1.0)

Ajouter les dynamic states les plus utiles qui ne nécessitent que Vulkan 1.0 :

| Dynamic State | Commande dans CommandList | Pourquoi |
|--------------|--------------------------|---------|
| Stencil Reference | `set_stencil_reference()` | Changer la valeur de référence stencil entre les draw calls |
| Stencil Masks | `set_stencil_compare_mask()`, `set_stencil_write_mask()` | Flexibilité du stencil |
| Depth Bias Values | `set_depth_bias()` | Ajuster le bias selon la scène (shadow maps) |
| Blend Constants | `set_blend_constants()` | Pour le ConstantColor blend factor |

### Étape 3 — Extended dynamic states (bien plus tard, Vulkan 1.3)

Quand l'optimisation par réduction du nombre de pipelines deviendra un besoin :

| Dynamic State | Pourquoi |
|--------------|---------|
| CullMode | Éviter un pipeline séparé juste pour changer le culling |
| DepthTestEnable / DepthWriteEnable | Opaque vs transparent avec le même pipeline |
| DepthCompareOp | Changer Less/LessOrEqual/Equal dynamiquement |
| StencilTestEnable / StencilOp | Activer le stencil ponctuellement |

---

## 14. Priorité d'implémentation des états

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
> selon l'approche décrite en section 13.
