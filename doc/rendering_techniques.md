# Techniques de Rendu - Moteurs 3D Modernes

> **Projet** : Galaxy3D Engine
> **Date** : 2026-02-08
> **Objectif** : Répertorier et expliquer les techniques d'optimisation du rendu utilisées dans les moteurs 3D modernes.

---

## Table des matières

1. [Le problème fondamental](#1-le-problème-fondamental)
2. [Sort Keys (Clés de tri)](#2-sort-keys-clés-de-tri)
3. [Command Buckets (Seaux de commandes)](#3-command-buckets-seaux-de-commandes)
4. [Mesh Draw Commands + Caching](#4-mesh-draw-commands--caching)
5. [SRP Batcher (Unity)](#5-srp-batcher-unity)
6. [Bindless Rendering](#6-bindless-rendering)
7. [GPU-Driven Rendering / Indirect Rendering](#7-gpu-driven-rendering--indirect-rendering)
8. [Render Graph / Frame Graph](#8-render-graph--frame-graph)
9. [Frustum Culling & Occlusion Culling](#9-frustum-culling--occlusion-culling)
10. [Camera-Relative Rendering](#10-camera-relative-rendering)
11. [LOD & Streaming (Monde ouvert)](#11-lod--streaming-monde-ouvert)
12. [Multi-caméra et Multi-scène](#12-multi-caméra-et-multi-scène)
13. [Comparaison des moteurs](#13-comparaison-des-moteurs)
14. [Recommandations pour Galaxy3D](#14-recommandations-pour-galaxy3d)

---

## 1. Le problème fondamental

### Le coût des changements d'état GPU

Le GPU fonctionne comme une chaîne de montage. Chaque fois qu'on change un élément de la configuration (pipeline, texture, buffer), le GPU doit **reconfigurer sa chaîne**, ce qui prend du temps. Ce temps perdu s'appelle un **state change** (changement d'état).

Hiérarchie des coûts (du plus coûteux au moins coûteux) :

| Changement | Coût relatif | Ce que ça fait |
|-----------|-------------|---------------|
| Render Pass / Render Target | Très élevé | Changer la "toile" sur laquelle on dessine |
| Pipeline (shader) | Élevé | Changer le programme de dessin du GPU |
| Descriptor Set (textures) | Moyen | Changer les images/données accessibles au shader |
| Vertex / Index Buffer | Faible | Changer la géométrie à dessiner |
| Push Constants | Très faible | Changer quelques paramètres (position, couleur...) |

### L'objectif

Organiser les appels de dessin (draw calls) pour **minimiser les changements d'état coûteux**. Au lieu de dessiner objet par objet (ce qui change constamment de pipeline/texture/mesh), on regroupe les dessins par état partagé.

### Exemple concret

Scène avec 6 objets utilisant 2 pipelines et 2 textures :

```
NAÏF (18 changements d'état) :
  Bind Pipeline A → Bind Texture 1 → Draw Obj1
  Bind Pipeline B → Bind Texture 2 → Draw Obj2
  Bind Pipeline A → Bind Texture 2 → Draw Obj3   ← re-bind Pipeline A !
  Bind Pipeline B → Bind Texture 1 → Draw Obj4
  Bind Pipeline A → Bind Texture 1 → Draw Obj5   ← re-bind encore !
  Bind Pipeline B → Bind Texture 2 → Draw Obj6

OPTIMISÉ (8 changements d'état) :
  Bind Pipeline A
    Bind Texture 1 → Draw Obj1, Draw Obj5
    Bind Texture 2 → Draw Obj3
  Bind Pipeline B
    Bind Texture 1 → Draw Obj4
    Bind Texture 2 → Draw Obj2, Draw Obj6
```

Résultat : **2 changements de pipeline au lieu de 6**, et les textures sont aussi mieux groupées.

---

## 2. Sort Keys (Clés de tri)

### Principe

Encoder toutes les priorités de tri dans **un seul entier 64 bits**. Chaque ressource (pipeline, texture, mesh) possède un identifiant numérique. On combine ces IDs par décalage de bits dans un `u64` :

```
Sort Key (u64) :
┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ Pass     │ Pipeline │ Texture  │ Mesh     │ Depth    │
│ 4 bits   │ 12 bits  │ 16 bits  │ 16 bits  │ 16 bits  │
└──────────┴──────────┴──────────┴──────────┴──────────┘
   MSB (plus prioritaire)                LSB (moins prioritaire)
```

Trier les draw calls revient alors à **trier un tableau de `u64`**, ce que le CPU fait extrêmement rapidement (radix sort, O(n)).

### Encodage pour objets opaques vs transparents

L'ordre de tri change selon le type d'objet :

- **Opaques** : tri front-to-back (les objets proches d'abord) pour profiter du early-Z du GPU, qui rejette automatiquement les pixels cachés derrière des objets déjà dessinés.
- **Transparents** : tri back-to-front (les objets lointains d'abord) car la transparence nécessite de dessiner l'arrière-plan avant le premier plan pour que le mélange de couleurs soit correct.

```
Opaques :   Pipeline (MSB) → Texture → Depth croissant (LSB)
Transparents : Depth décroissant (MSB) → Pipeline → Texture (LSB)
```

### Performances

- Trier 100 000 `u64` : ~1-2 ms avec un radix sort
- Trier 10 000 `u64` : ~0.1 ms
- Les `u64` sont cache-friendly (données contigües en mémoire)
- Comparaisons = simples comparaisons entières, pas d'indirection

### Plage d'IDs

La plage est limitée par le nombre de bits alloués (ex: 12 bits = 4096 pipelines max), mais en pratique :
- Un jeu AAA a entre 50 et 200 pipelines → 12 bits suffisent largement
- Les IDs représentent les ressources **visibles** après culling, pas toutes les ressources du jeu

### Utilisé par

- **BGFX** (moteur open source) : sort keys 64 bits pures
- **Unity** (built-in renderer) : sort keys pour l'ordre de rendu
- **Godot 4** : sort keys + buckets

---

## 3. Command Buckets (Seaux de commandes)

### Principe

Au lieu de trier un tableau, on **répartit directement** les draw calls dans des conteneurs pré-organisés au moment de leur soumission :

```
Bucket Pipeline 0 :
  ├── Bucket Texture 0 : [draw, draw, draw]
  ├── Bucket Texture 3 : [draw, draw]
  └── Bucket Texture 7 : [draw]

Bucket Pipeline 1 :
  ├── Bucket Texture 1 : [draw, draw, draw, draw]
  └── Bucket Texture 5 : [draw]
```

Pour rendre, on itère les buckets dans l'ordre. **Pas de tri nécessaire** — le classement est fait à l'insertion.

### Avantages

- Insertion en O(1) — le coût est réparti sur chaque objet au lieu d'un tri global
- Pas de phase de tri explicite
- Simple à implémenter et à débugger
- Prédictible en termes de performances

### Inconvénients

- Structure mémoire plus complexe (HashMap imbriquées ou tableaux d'indices)
- Moins flexible que les sort keys pour changer l'ordre de priorité dynamiquement

### Utilisé par

- **Destiny** (Bungie) — technique présentée par Natalya Tatarchuk à la GDC
- **id Tech** (Doom, Quake) — command buffers triés par état

---

## 4. Mesh Draw Commands + Caching

### Principe (technique Unreal Engine)

Nom officiel : **Mesh Draw Pipeline** (introduit dans UE 4.22).

Chaque combinaison unique (pipeline + vertex buffer + textures + paramètres) est encapsulée dans un **Mesh Draw Command** (MDC). Ces commandes sont **mises en cache** et réutilisées d'une frame à l'autre.

### Fonctionnement

```
Frame N :
  1. Construction des MDC pour les objets visibles
  2. Mise en cache des MDC
  3. Tri des MDC (sort keys sur les MDC, pas sur les objets)
  4. Soumission au GPU

Frame N+1 :
  1. Réutilisation du cache (95% des MDC sont identiques)
  2. Mise à jour uniquement des MDC modifiés (objet déplacé, matériau changé)
  3. Tri incrémental (le tableau est déjà quasi-trié → insertion sort quasi O(n))
  4. Soumission au GPU
```

### Cohérence temporelle

D'une frame à l'autre, très peu de choses changent :
- La caméra bouge légèrement
- Quelques objets entrent/sortent du frustum
- Rares changements de matériaux

Le cache permet d'éviter de reconstruire et re-trier des milliers de commandes chaque frame. Seules les commandes **invalides** sont recalculées.

### Utilisé par

- **Unreal Engine 4.22+** : Mesh Draw Pipeline
- **Unreal Engine 5** : évolution avec GPU Scene

---

## 5. SRP Batcher (Unity)

### Principe

Nom officiel : **Scriptable Render Pipeline Batcher** (Unity 2019+).

Au lieu de regrouper par matériau identique (comme l'ancien batching de Unity), le SRP Batcher regroupe par **shader variant identique**.

### Différence clé

```
Ancien système :
  Même matériau (même shader + mêmes textures + mêmes paramètres) → batch
  → Très restrictif, peu d'objets sont identiques

SRP Batcher :
  Même shader variant (même shader, paramètres différents OK) → batch
  → Beaucoup plus de regroupements possibles
```

### Comment ça marche

- Les propriétés de chaque matériau (couleur, tiling, etc.) sont stockées dans un **buffer GPU persistant** (CBUFFER)
- Changer de matériau ne nécessite plus de re-upload CPU → GPU
- Le GPU lit directement les propriétés depuis le buffer via un offset
- Tant que le shader ne change pas, les draw calls s'enchaînent sans coût de changement d'état

### Utilisé par

- **Unity URP** (Universal Render Pipeline)
- **Unity HDRP** (High Definition Render Pipeline)

---

## 6. Bindless Rendering

### Principe

Technique GPU moderne qui **élimine le besoin de binder les textures** individuellement avant chaque draw call.

### Fonctionnement classique vs bindless

```
CLASSIQUE :
  Bind Texture "brique" → Draw mur
  Bind Texture "herbe"  → Draw sol      ← changement d'état !
  Bind Texture "bois"   → Draw porte    ← changement d'état !

BINDLESS :
  Toutes les textures sont dans un grand tableau GPU
  Le shader reçoit juste un index (entier) :

  Draw mur   (texture_index = 0)
  Draw sol   (texture_index = 1)   ← pas de changement d'état !
  Draw porte (texture_index = 2)   ← pas de changement d'état !
```

### Impact

- Le tri par texture devient **inutile** — changer de texture ne coûte plus rien
- La sort key peut se simplifier : on ne garde que pipeline + depth
- Réduit drastiquement le nombre de descriptor set binds

### Prérequis API

| API | Feature requise |
|-----|----------------|
| Vulkan | `descriptorIndexing` (Vulkan 1.2+) |
| DirectX 12 | Bindless / SM 6.6 |
| OpenGL | `GL_ARB_bindless_texture` |

### Utilisé par

- **Unreal Engine 5**
- **Frostbite** (EA/DICE)
- **id Tech 7** (Doom Eternal)
- La plupart des moteurs AAA récents

---

## 7. GPU-Driven Rendering / Indirect Rendering

### Principe

Déplacer le tri, le culling et la génération des draw calls **sur le GPU** via des compute shaders. Le CPU ne fait presque plus rien.

### Pipeline classique vs GPU-driven

```
CLASSIQUE (CPU-driven) :
  CPU : frustum cull → tri → build draw calls → submit
  GPU : exécute les draw calls

GPU-DRIVEN :
  CPU : upload la scène entière une fois → "GPU, fais le rendu"
  GPU : [compute] frustum cull → occlusion cull → LOD select → tri
        [compute] génère les draw calls dans un buffer
        [draw]    exécute les draw calls auto-générés
```

### Multi-Draw Indirect (MDI)

Fonctionnalité clé : le GPU lit les paramètres de dessin depuis un buffer au lieu de recevoir des commandes individuelles du CPU.

```
// Le CPU envoie UN SEUL appel :
vkCmdDrawIndexedIndirect(cmd, draw_buffer, offset, draw_count, stride);

// Le GPU lit draw_count commandes depuis draw_buffer :
// { vertex_count, instance_count, first_index, vertex_offset, first_instance }
// { vertex_count, instance_count, first_index, vertex_offset, first_instance }
// ...
```

Avec `vkCmdDrawIndexedIndirectCount`, le GPU décide même **combien** de draw calls exécuter (le résultat du culling GPU).

### Nanite (Unreal Engine 5)

L'implémentation la plus avancée de GPU-driven rendering :
- La géométrie est découpée en **clusters** de ~128 triangles
- Un compute shader fait le culling et la sélection de LOD **par cluster**
- Les clusters visibles sont rendus via un mécanisme de rasterisation software/hardware hybride
- Capable de gérer des **milliards de triangles** en temps réel

### Utilisé par

- **Unreal Engine 5** (Nanite + GPU Scene)
- **Frostbite** (Battlefield, FIFA)
- **id Tech 7** (Doom Eternal)

---

## 8. Render Graph / Frame Graph

### Principe

Décrire la frame entière comme un **graphe de dépendances** entre les passes de rendu. Le moteur compile ce graphe et en déduit l'ordre optimal, les transitions de ressources, et le pooling mémoire.

### Structure

```
                    ┌─────────────┐
                    │ Shadow Map  │
                    │ (depth only)│
                    └──────┬──────┘
                           │ (produit une texture de profondeur)
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ G-Buffer     │  │ G-Buffer     │  │ G-Buffer     │
│ (géométrie   │  │ (terrain)    │  │ (végétation) │
│  opaque)     │  │              │  │              │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       └────────┬────────┘─────────────────┘
                ▼
        ┌──────────────┐
        │ Lighting     │  (lit les G-Buffers + shadow map)
        └──────┬───────┘
               ▼
        ┌──────────────┐
        │ Transparent  │  (dessiné back-to-front)
        └──────┬───────┘
               ▼
        ┌──────────────┐
        │ Post-Process │  (bloom, anti-aliasing, tone mapping)
        └──────────────┘
```

### Ce que le moteur en déduit

- **Ordre des passes** : les shadow maps avant le lighting, le G-Buffer avant le lighting, etc.
- **Transitions de ressources** : quand une texture passe de "write" (render target) à "read" (input du shader suivant), le moteur insère automatiquement les barrières de synchronisation GPU
- **Pooling mémoire** : deux render targets qui ne sont jamais utilisés en même temps peuvent partager la même mémoire GPU (aliasing)
- **Parallélisme** : les passes indépendantes (ex: les 3 G-Buffers ci-dessus) peuvent être exécutées en parallèle sur le GPU

### Relation avec le tri des draw calls

Le render graph organise les **passes**. Le tri des draw calls (sort keys, buckets, etc.) se fait **à l'intérieur** de chaque passe, sur des sous-ensembles plus petits et donc plus rapides à trier.

### Utilisé par

- **Frostbite** (EA/DICE) — inventeur du terme "Frame Graph" (GDC 2017)
- **Unreal Engine 5** — "RDG" (Render Dependency Graph)
- **Unity HDRP** — Render Graph API

---

## 9. Frustum Culling & Occlusion Culling

### Frustum Culling

Éliminer les objets **en dehors du champ de vision** de la caméra avant tout rendu.

La caméra définit un volume en forme de pyramide tronquée (frustum). Tout objet dont la bounding box (boîte englobante) est entièrement à l'extérieur de ce volume est éliminé.

```
            Near plane
               ┌───────┐
              /    ★    /     ★ = visible (dans le frustum)
             / caméra  /
            /         /
           /    ●    /        ● = visible
          /         /
         └─────────┘
          Far plane

  ✗                           ✗ = éliminé (hors frustum)
```

Résultat typique : **70-80% des objets éliminés** avant même de commencer le tri.

### Occlusion Culling

Éliminer les objets **cachés derrière d'autres objets**, même s'ils sont dans le frustum.

Techniques principales :
- **Hardware Occlusion Queries** : le GPU teste si un objet serait visible, le CPU lit le résultat à la frame suivante (1 frame de latence)
- **Hierarchical Z-Buffer (Hi-Z)** : on crée une version mip-mappée du depth buffer, et on teste les bounding boxes contre cette pyramide de profondeur
- **Software Occlusion** : un rasterizer CPU simplifié dessine les gros occluders et teste les autres objets contre ce depth buffer logiciel (technique utilisée par Frostbite)

### Ordre des opérations

```
Objets totaux de la scène : 100 000
        │
        ▼ Frustum Culling (CPU ou GPU)
        │
Objets dans le frustum : 20 000
        │
        ▼ Occlusion Culling (CPU ou GPU)
        │
Objets réellement visibles : 5 000
        │
        ▼ Tri / Sort Keys / Buckets
        │
Draw calls optimisés : 5 000 (mais avec un minimum de state changes)
```

---

## 10. Camera-Relative Rendering

### Le problème

Un `f32` a ~7 chiffres significatifs. À grande distance de l'origine (0, 0, 0), la précision diminue :

| Distance de l'origine | Précision |
|----------------------|-----------|
| 0 m | ~0.0000001 m |
| 1 000 m | ~0.0001 m |
| 10 000 m | ~0.001 m |
| 100 000 m | ~0.01 m (jittering visible) |
| 1 000 000 m | ~0.1 m |

### Solutions

#### Camera-Relative Rendering

Au lieu d'envoyer au GPU les positions absolues des objets, on soustrait la position de la caméra **avant** de construire la matrice de transformation :

```
position_relative = position_objet - position_camera   // toujours petit
matrice_model = translate(position_relative)
MVP = projection × view_at_origin × matrice_model
```

Le GPU travaille toujours avec des nombres proches de zéro → précision maximale.

#### Origin Rebasing

Quand le joueur dépasse un seuil de distance, on soustrait un offset à **tous** les objets du monde pour recentrer l'univers autour du joueur. Transparent pour le gameplay.

Utilisé par : Unreal Engine (`UWorld::SetNewWorldOrigin`)

#### Coordonnées en double précision (f64)

Stocker les positions monde en `f64` (15 chiffres significatifs) et convertir en `f32` relatif à la caméra au moment du rendu.

Utilisé par : Unreal Engine 5 (Large World Coordinates), Star Citizen, KSP 2

#### Coordonnées segmentées

Séparer la position en une partie entière (chunk, `i32`) et une partie locale (`f32`) :

```
WorldPosition {
    chunk: IVec3,   // dans quel chunk (entier, pas de perte)
    local: Vec3,    // position dans le chunk (toujours petit, toujours précis)
}
```

Utilisé par : Minecraft

---

## 11. LOD & Streaming (Monde ouvert)

### Level of Detail (LOD)

Chaque objet existe en plusieurs versions de complexité décroissante. Le moteur choisit la version selon la distance à la caméra :

```
Très proche   →  Mesh haute résolution (10 000 triangles)
Proche        →  Mesh moyenne résolution (2 000 triangles)
Moyen         →  Mesh basse résolution (500 triangles)
Loin          →  Imposteur 2D (billboard, 2 triangles)
Très loin     →  Invisible (culled)
```

### Streaming de monde

Le monde est découpé en **cellules/chunks**. Seules les cellules proches du joueur sont chargées en mémoire :

```
┌───┬───┬───┬───┬───┐
│   │ L │ L │ L │   │   L = LOD bas
├───┼───┼───┼───┼───┤
│ L │ M │ M │ M │ L │   M = LOD moyen
├───┼───┼───┼───┼───┤
│ L │ M │ ★ │ M │ L │   ★ = Joueur (LOD max)
├───┼───┼───┼───┼───┤
│ L │ M │ M │ M │ L │
├───┼───┼───┼───┼───┤
│   │ L │ L │ L │   │   (vide) = pas en mémoire
└───┴───┴───┴───┴───┘
```

Le chargement/déchargement se fait en arrière-plan sur des threads dédiés, sans écran de chargement.

### Utilisé par

- **Unreal Engine 5** : World Partition (grille automatique) + Nanite (LOD par cluster GPU)
- **Unity** : Addressables + scènes additives
- **Tous les jeux monde ouvert** : GTA, Zelda, Elden Ring, etc.

---

## 12. Multi-caméra et Multi-scène

### Principe

Chaque caméra dessine sur son propre **render target** (sa "toile"). Un render pass est une unité atomique — tous les bindings (pipeline, textures, buffers) doivent être ré-établis pour chaque render pass.

### Hiérarchie

```
Render Target / Caméra      ← le plus coûteux
  └── Render Pass
        └── Pipeline         ← re-bind obligatoire par render pass
              └── Textures
                    └── Buffers
                          └── Push Constants
```

### Ordre de rendu multi-caméra

Les caméras qui produisent des textures utilisées par d'autres caméras doivent être rendues **en premier** (dépendances) :

```
1. Caméra miroir     → render target "reflet"    (rendu en premier)
2. Caméra minimap    → render target "minimap"    (rendu ensuite)
3. Caméra principale → render target "écran"      (utilise "reflet" et "minimap" comme textures)
```

### Couches de visibilité (Layer Masks)

Même dans un monde unifié, chaque caméra peut filtrer les objets via des masques :
- Caméra principale : couches "monde" + "personnages" + "effets"
- Caméra UI : couche "interface" uniquement
- Caméra minimap : couche "monde" uniquement (vue de dessus, sans effets)

---

## 13. Comparaison des moteurs

### Techniques utilisées par moteur

| Moteur | Sort Keys | Buckets | MDC Cache | SRP Batcher | Bindless | GPU-Driven | Render Graph |
|--------|:---------:|:-------:|:---------:|:-----------:|:--------:|:----------:|:------------:|
| **Unreal 5** | ✓ (interne) | | ✓ | | ✓ | ✓ (Nanite) | ✓ (RDG) |
| **Unity URP** | ✓ | | | ✓ | | | |
| **Unity HDRP** | ✓ | | | ✓ | | | ✓ |
| **Frostbite** | | ✓ | | | ✓ | ✓ | ✓ (Frame Graph) |
| **id Tech 7** | | ✓ | | | ✓ | ✓ | |
| **BGFX** | ✓ (pur) | | | | | | |
| **Godot 4** | ✓ | ✓ | | | | | |

### Évolution historique

```
2005  Sort Keys simples (tout sur CPU)
        │
2010  + Command Buckets (Destiny, id Tech)
        │
2015  + Frame Graph (Frostbite, GDC 2017)
      + Mesh Draw Commands Caching (Unreal 4.22)
        │
2018  + SRP Batcher (Unity 2019)
      + Bindless Rendering (rend le tri par texture obsolète)
        │
2021  + GPU-Driven Rendering (Nanite / UE5)
      + GPU Scene (toute la scène sur le GPU)
        │
2024  Tendance : le CPU ne trie presque plus rien,
      le GPU gère culling + tri + génération des draw calls
```

---

## 14. Recommandations pour Galaxy3D

### Phase 1 — Fondations

- **Sort Keys 64 bits** : simple, efficace, suffisant pour la majorité des scènes
- **Frustum Culling** : indispensable, élimine 70-80% des objets
- **Camera-Relative Rendering** : une soustraction avant le calcul MVP, évite le jittering

### Phase 2 — Optimisations

- **Command Buckets** : alternative ou complément aux sort keys pour les scènes complexes
- **Caching des commandes** : exploiter la cohérence temporelle entre frames
- **LOD** basique : 2-3 niveaux de détail par mesh

### Phase 3 — Avancé

- **Render Graph** : orchestrer les passes de rendu et automatiser les transitions de ressources
- **Bindless Rendering** : exploiter `descriptorIndexing` de Vulkan 1.2
- **GPU-Driven Culling** : compute shaders pour le frustum/occlusion culling
- **Multi-Draw Indirect** : `vkCmdDrawIndexedIndirect` pour réduire les appels CPU→GPU

### Phase 4 — AAA

- **GPU-Driven Rendering complet** : la scène entière gérée par le GPU
- **Streaming de monde** : chargement/déchargement dynamique de cellules
- **Coordonnées f64** : support des grands mondes (Large World Coordinates)

---

## Références

- **GDC 2017 — Frostbite** : "FrameGraph: Extensible Rendering Architecture in Frostbite" (Yuriy O'Donnell)
- **GDC 2015 — Bungie** : "Destiny's Multithreaded Rendering Architecture" (Natalya Tatarchuk)
- **Unreal Engine Docs** : Mesh Draw Pipeline, Nanite, RDG
- **Unity Docs** : SRP Batcher, Render Graph API
- **BGFX** : [github.com/bkaradzic/bgfx](https://github.com/bkaradzic/bgfx) — implémentation de référence sort keys
- **Vulkan Spec** : `vkCmdDrawIndexedIndirect`, `descriptorIndexing`
