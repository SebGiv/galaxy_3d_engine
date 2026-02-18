# Camera — Design Notes

> **Statut** : À implémenter
> **Date** : 2026-02-17

---

## Concept

Camera bas niveau pour le moteur 3D. C'est un **conteneur de données passif** — elle ne
calcule rien elle-même. L'appelant (game engine) calcule toutes les valeurs et les pose
dans la Camera.

Le moteur de jeu (ECS, scene graph) ajoutera des helpers haut niveau par-dessus
(look_at, follow, auto-resize projection, etc.).

## Structure

La Camera est un struct standalone, **pas** stockée dans la Scene.
C'est un outil fourni par le moteur, possédé et piloté par l'appelant.

```rust
/// Low-level camera. A passive data container — computes nothing.
/// The caller is responsible for computing and setting all fields.
#[derive(Debug, Clone)]
pub struct Camera {
    view_matrix: Mat4,            // position + rotation (inverse de la transform)
    projection_matrix: Mat4,      // perspective ou orthographique
    frustum: Frustum,             // 6 plans pour le culling (calculé par l'appelant)
    viewport: Viewport,           // zone d'affichage dans le render target
    scissor: Option<Rect2D>,      // clip rectangle (None = identique au viewport)
}
```

### Philosophie bas niveau

L'appelant calcule **tout** côté haut niveau et passe les résultats à la Camera :

```
Game Engine (haut niveau)                    Camera (bas niveau)
─────────────────────────                    ──────────────────────
position + rotation           → calcule →    view_matrix: Mat4
fov + near + far + ratio      → calcule →    projection_matrix: Mat4
view * projection             → calcule →    frustum: Frustum
taille fenêtre / layout       → calcule →    viewport: Viewport
```

Quand un paramètre haut niveau change (ex: le joueur bouge), le game engine
recalcule les valeurs bas niveau concernées et les passe à la Camera via les setters.
La Camera ne sait pas et ne se soucie pas de ce qui a changé ni pourquoi.

### Frustum

Le Frustum est un struct standalone (6 plans). Le moteur fournit un utilitaire pour
l'extraire d'une matrice VP, mais l'appelant peut aussi le construire autrement :

```rust
/// Six frustum planes for culling.
/// Each plane is (A, B, C, D) where Ax + By + Cz + D = 0.
/// Normal (A, B, C) points inward (toward the visible volume).
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    pub planes: [Vec4; 6],  // left, right, bottom, top, near, far
}

impl Frustum {
    /// Utility: extract planes from a view-projection matrix.
    /// Works for both perspective and orthographic projections.
    pub fn from_view_projection(vp: &Mat4) -> Self { ... }

    /// Test if an AABB intersects this frustum.
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool { ... }
}
```

`Frustum::from_view_projection()` est un **utilitaire**, pas un mécanisme interne.
L'appelant l'utilise s'il veut, ou calcule le frustum autrement.

---

## Viewport et Scissor

### Pourquoi le viewport est sur la Camera

Le viewport et le scissor sont de l'**état dynamique** du command buffer (`cmd.set_viewport()`,
`cmd.set_scissor()`). Ce ne sont **pas** des objets GPU — juste des géométries (x, y, width, height)
passées à des commandes Vulkan.

Le `render_graph::RenderPass` ne se soucie pas du viewport : il définit un render target
(le "où dessiner") et des attachments. Le viewport, lui, est intimement lié à la camera :

- Le **viewport** détermine l'aspect ratio → l'aspect ratio conditionne la **projection matrix**
- Changer le viewport sans ajuster la projection déforme l'image
- Un render_graph::RenderPass peut contenir **plusieurs cameras** avec des viewports différents (split-screen)

Résultat : le viewport appartient naturellement à la Camera.

### Viewport — qu'est-ce que c'est ?

Le viewport transforme les coordonnées NDC (-1..1) en coordonnées pixel dans le render target.

```
NDC (-1..1)  ──viewport──→  Pixels dans le render target
```

Défini par : `(x, y, width, height, min_depth, max_depth)` — réutilise `renderer::Viewport`.

Analogie : un projecteur (viewport) projette le film (la scène 3D) sur une partie de l'écran.
Changer le viewport = déplacer/redimensionner le projecteur.

### Scissor — qu'est-ce que c'est ?

Le scissor clippe (masque) tous les pixels en dehors d'un rectangle.

```
Pixels après rasterization  ──scissor──→  Seuls les pixels dans le rectangle passent
```

Défini par : `(x, y, width, height)` — réutilise `renderer::Rect2D`.

Analogie : un cache en carton posé devant l'écran. Tout ce qui est en dehors du trou est masqué.

### Viewport vs Scissor

| | Viewport | Scissor |
|---|---------|---------|
| **Effet** | Transforme (scale + position) | Masque (clip) |
| **Impact** | Déforme/redimensionne l'image | Coupe sans déformer |
| **Par défaut** | Tout le render target | Tout le render target |

### Usages courants du scissor dans les moteurs modernes

- **UI** : clipper un enfant qui dépasse son parent (scroll view, popup)
- **Deferred lighting** : optimiser les lights en ne rastérisant que leur bounding rect
- **Shadow atlas** : rendre chaque shadow map dans sa zone de l'atlas
- **VR** : isoler chaque œil dans sa moitié de texture

En v1, `scissor` sera `None` (= même rectangle que le viewport).

---

## Split-screen

Plusieurs cameras avec des viewports différents dans un **seul** GPU render pass :

```
Un seul begin_render_pass() / end_render_pass() :

  cmd.set_viewport(left_half)
  cmd.set_scissor(left_half)
  draw(scene, camera_player1)

  cmd.set_viewport(right_half)
  cmd.set_scissor(right_half)
  draw(scene, camera_player2)
```

Un render_graph::RenderPass = un render target, pas un viewport.
Le split-screen n'a pas besoin de render passes supplémentaires.

Le post-processing, lui, travaille sur le render target **complet** (viewport = full image).
Il ne connaît pas le split-screen.

---

## RenderView et Frustum Culling

### Concept

`Scene::frustum_cull()` produit un **RenderView** — le résultat du culling pour une camera donnée :

```rust
/// Result of frustum culling. Ephemeral — lives for one frame.
/// Created by Scene::frustum_cull().
#[derive(Debug, Clone)]
pub struct RenderView {
    camera: Camera,                 // snapshot de la camera au moment du culling
    visible_instances: Vec<usize>,  // indices into Scene::render_instances
}

impl RenderView {
    pub fn camera(&self) -> &Camera { &self.camera }
    pub fn visible_instances(&self) -> &[usize] { &self.visible_instances }
}
```

Propriétés :
- **Immuable** une fois créé (on cull, on consomme le résultat)
- **Éphémère** (pas de `Arc`, pas de `Mutex`, vit une frame)
- **Partageable** entre plusieurs passes (l'appelant le passe où il veut)

### Production

`Scene::frustum_cull()` crée le RenderView. C'est le seul point de création :

```rust
impl Scene {
    pub fn frustum_cull(&self, camera: &Camera) -> RenderView { ... }
}
```

### V1 — Squelette sans culling

La méthode existe mais retourne **tous** les objets de la scène :

```rust
impl Scene {
    pub fn frustum_cull(&self, camera: &Camera) -> RenderView {
        let visible_instances = (0..self.render_instance_count()).collect();
        RenderView::new(camera.clone(), visible_instances)
    }
}
```

L'appelant utilise déjà le workflow complet (cull → RenderView → pass).
Quand le vrai frustum culling sera implémenté, seul l'intérieur de cette méthode change.

### Consommation

```rust
impl Scene {
    /// Draw visible instances from a RenderView into a command list.
    pub fn draw(&self, view: &RenderView, cmd: &mut dyn CommandList) -> Result<()> { ... }
}
```

---

## Workflow complet

### Setup (une fois, à la construction du graph)

```rust
// Slot partagé : pont entre l'appelant et la closure du PassAction
// Arc<Mutex<...>> permet au même RenderView d'être lu par plusieurs passes
let view_slot: Arc<Mutex<Option<RenderView>>> = Arc::new(Mutex::new(None));

// La PassAction capture le slot (clone de l'Arc, pas des données)
graph.set_action(geometry_pass, Box::new(CustomAction::new({
    let scene = scene_arc.clone();    // clone de l'Arc, pas de la Scene
    let slot = view_slot.clone();     // clone de l'Arc, pas du RenderView
    move |cmd| {
        let scene = scene.lock().unwrap();
        let slot = slot.lock().unwrap();
        if let Some(view) = slot.as_ref() {
            scene.draw(view, cmd)?;
        }
        Ok(())
    }
})))?;
```

### Chaque frame

```rust
// 1. Le culling fabrique le RenderView
let view = scene_arc.lock().unwrap().frustum_cull(&camera);

// 2. On le met dans le slot (accessible par toutes les passes qui partagent ce slot)
*view_slot.lock().unwrap() = Some(view);

// 3. Le graph exécute → la closure lit le slot → scene.draw()
graph.execute(|cmd| Ok(()))?;
```

---

## Frontière bas niveau / haut niveau

La Camera et le RenderView sont des **outils** fournis par le moteur, pas des ressources gérées.

| Concept | Le moteur fournit | Le moteur stocke/gère |
|---------|-------------------|----------------------|
| Scene | La struct + RenderInstances | Oui (SceneManager) |
| RenderGraph | Le DAG + compile/execute | Oui (RenderGraphManager) |
| Resources | Geometry, Texture, Pipeline... | Oui (ResourceManager) |
| **Camera** | **La struct (conteneur passif)** | **Non** |
| **Frustum** | **La struct + `from_view_projection()` utilitaire** | **Non** |
| **RenderView** | **La struct (créée par `Scene::frustum_cull()`)** | **Non** |

```
┌──────────────────────────────────────────────────┐
│           GAME ENGINE (haut niveau, futur)         │
│                                                    │
│  Possède les cameras                               │
│  Calcule view, projection, frustum, viewport       │
│  Passe les valeurs à la Camera bas niveau          │
│  Appelle scene.frustum_cull() → RenderView         │
│  Assigne les RenderViews aux passes via les slots  │
└────────────────────┬─────────────────────────────┘
                     │  utilise
                     ▼
┌──────────────────────────────────────────────────┐
│           MOTEUR 3D (bas niveau, Galaxy3D)         │
│                                                    │
│  Fournit : Camera, Frustum, RenderView             │
│  Scene::frustum_cull() crée le RenderView          │
│  Scene::draw() consomme le RenderView              │
│  Ne calcule rien dans la Camera — conteneur passif │
└──────────────────────────────────────────────────┘
```

---

## Interaction avec le Render Graph

Le `render_graph::RenderPass` ne connaît **ni les cameras, ni les scènes, ni les RenderViews**.
La connexion se fait via le `PassAction` (callback sur le pass). Le slot `Arc<Mutex<Option<RenderView>>>`
fait le pont entre l'appelant (qui produit le RenderView) et la closure (qui le consomme).

---

## API Camera

```rust
impl Camera {
    // Construction
    fn new(view: Mat4, projection: Mat4, frustum: Frustum, viewport: Viewport) -> Self;

    // Getters
    fn view_matrix(&self) -> &Mat4;
    fn projection_matrix(&self) -> &Mat4;
    fn view_projection_matrix(&self) -> Mat4;       // projection * view
    fn frustum(&self) -> &Frustum;
    fn viewport(&self) -> &Viewport;
    fn scissor(&self) -> Option<&Rect2D>;
    fn effective_scissor(&self) -> Rect2D;           // scissor or viewport as Rect2D

    // Setters — store, compute nothing
    fn set_view(&mut self, matrix: Mat4);
    fn set_projection(&mut self, matrix: Mat4);
    fn set_frustum(&mut self, frustum: Frustum);
    fn set_viewport(&mut self, viewport: Viewport);
    fn set_scissor(&mut self, scissor: Option<Rect2D>);
}
```

Pas de `look_at()`, `set_fov()`, `update_frustum()`, etc. — c'est le rôle du game engine.

---

## Module

```
src/camera/
├── mod.rs              // exports: Camera, Frustum, RenderView
├── camera.rs           // Camera struct + getters/setters
├── frustum.rs          // Frustum struct + from_view_projection() + intersects_aabb()
└── render_view.rs      // RenderView struct + accessors
```

Le culling (`Scene::frustum_cull()`) vit dans le module `scene/`, pas dans `camera/`.
Le module `scene` dépend de `camera` (import Camera, RenderView), pas l'inverse.
