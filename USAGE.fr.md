# Galaxy3DEngine - Guide d'utilisation

Guide complet pour utiliser le moteur de rendu Galaxy3D dans vos applications Rust.

---

## Table des matières

1. [Démarrage rapide](#démarrage-rapide)
2. [Concepts fondamentaux](#concepts-fondamentaux)
3. [Créer un Renderer](#créer-un-renderer)
4. [Charger les shaders](#charger-les-shaders)
5. [Créer des buffers](#créer-des-buffers)
6. [Créer des pipelines](#créer-des-pipelines)
7. [Boucle de rendu](#boucle-de-rendu)
8. [Gestion des ressources](#gestion-des-ressources)
9. [Gestion des erreurs](#gestion-des-erreurs)
10. [Support multi-écrans](#support-multi-écrans)

---

## Démarrage rapide

### Dépendances

Ajoutez à votre `Cargo.toml` :

```toml
[dependencies]
galaxy_3d_engine = { path = "../Galaxy3DEngine/galaxy_3d_engine" }
galaxy_3d_engine_renderer_vulkan = { path = "../Galaxy3DEngine/galaxy_3d_engine_renderer_vulkan" }
winit = "0.30"
```

### Exemple minimal

```rust
use galaxy_3d_engine::{Renderer, RendererConfig};
use galaxy_3d_engine_renderer_vulkan;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::Window;
use std::sync::{Arc, Mutex};

fn main() {
    // Enregistrer le plugin Vulkan
    

    // Créer la fenêtre
    let event_loop = EventLoop::new().unwrap();
    let window = Window::default_attributes()
        .with_title("Galaxy3D")
        .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
        .build(&event_loop)
        .unwrap();

    // Créer le renderer avec configuration
    let config = RendererConfig {
        enable_validation: true,
        app_name: "Mon App".to_string(),
        app_version: (1, 0, 0),
    };

    let renderer = galaxy_3d_engine::VulkanRenderer::new()
        .lock().unwrap()
        .as_ref().unwrap()
        .create_renderer("vulkan", &window, config)
        .expect("Échec de création du renderer");

    // Boucle d'événements
    event_loop.run(|event, window_target| {
        // Gérer les événements et effectuer le rendu
    }).unwrap();
}
```

---

## Concepts fondamentaux

### Polymorphisme basé sur les traits

Galaxy3D utilise des traits Rust avec `Arc<dyn Trait>` pour obtenir un polymorphisme style C++ :

- **`Renderer`** : Interface principale pour créer les ressources GPU
- **`RendererBuffer`** : Buffer GPU (vertex, index, uniform)
- **`RendererShader`** : Module shader compilé
- **`RendererPipeline`** : État du pipeline graphique
- **`RenderCommandList`** : Enregistrement des commandes par frame

### Durée de vie des ressources

Toutes les ressources GPU sont automatiquement nettoyées lorsqu'elles sont droppées (pattern RAII). Les ressources doivent rester vivantes tant qu'elles sont utilisées.

### Sécurité thread

Le `Renderer` est enveloppé dans `Arc<Mutex<...>>` pour un accès thread-safe. Verrouillez le mutex lors de la création de ressources ou de l'enregistrement de commandes.

---

## Créer un Renderer

### Configuration

```rust
use galaxy_3d_engine::RendererConfig;

let config = RendererConfig {
    enable_validation: true,        // Activer les couches de validation Vulkan
    app_name: "MonApp".to_string(), // Nom de l'application
    app_version: (1, 0, 0),         // Version (majeur, mineur, patch)
};
```

### Enregistrement du backend

```rust
// Enregistrer le backend Vulkan


// Créer le renderer via le registre de plugins
let renderer = galaxy_3d_engine::VulkanRenderer::new()
    .lock().unwrap()
    .as_ref().unwrap()
    .create_renderer("vulkan", &window, config)?;
```

---

## Charger les shaders

Les shaders doivent être précompilés au format SPIR-V (utilisez `glslc` du SDK Vulkan).

### Exemple de vertex shader (triangle.vert)

```glsl
#version 450

layout(location = 0) in vec2 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

void main() {
    gl_Position = vec4(inPosition, 0.0, 1.0);
    fragColor = inColor;
}
```

### Exemple de fragment shader (triangle.frag)

```glsl
#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(fragColor, 1.0);
}
```

### Compiler les shaders

```bash
glslc triangle.vert -o triangle_vert.spv
glslc triangle.frag -o triangle_frag.spv
```

### Charger les shaders dans le code

```rust
use galaxy_3d_engine::{ShaderDesc, ShaderStage};

// Charger le bytecode des shaders
let vert_spv = include_bytes!("../shaders/triangle_vert.spv");
let frag_spv = include_bytes!("../shaders/triangle_frag.spv");

// Créer les modules shader
let mut renderer_guard = renderer.lock().unwrap();

let vertex_shader = renderer_guard.create_shader(ShaderDesc {
    code: vert_spv,
    stage: ShaderStage::Vertex,
    entry_point: "main".to_string(),
})?;

let fragment_shader = renderer_guard.create_shader(ShaderDesc {
    code: frag_spv,
    stage: ShaderStage::Fragment,
    entry_point: "main".to_string(),
})?;
```

---

## Créer des buffers

### Structure de données vertex

```rust
#[repr(C)]
struct Vertex {
    pos: [f32; 2],    // Position (x, y)
    color: [f32; 3],  // Couleur (r, g, b)
}

let vertices = [
    Vertex { pos: [ 0.0, -0.5], color: [1.0, 0.0, 0.0] }, // Haut (rouge)
    Vertex { pos: [ 0.5,  0.5], color: [0.0, 1.0, 0.0] }, // Bas-droite (vert)
    Vertex { pos: [-0.5,  0.5], color: [0.0, 0.0, 1.0] }, // Bas-gauche (bleu)
];
```

### Créer un buffer de vertices

```rust
use galaxy_3d_engine::{BufferDesc, BufferUsage};
use std::sync::Arc;

let buffer_size = std::mem::size_of_val(&vertices) as u64;

let vertex_buffer = {
    let mut renderer_guard = renderer.lock().unwrap();
    renderer_guard.create_buffer(BufferDesc {
        size: buffer_size,
        usage: BufferUsage::Vertex,
    })?
};

// Upload des données vertex
let vertex_data = unsafe {
    std::slice::from_raw_parts(
        vertices.as_ptr() as *const u8,
        std::mem::size_of_val(&vertices),
    )
};

vertex_buffer.update(0, vertex_data)?;
```

---

## Créer des pipelines

### Définir le layout vertex

```rust
use galaxy_3d_engine::{
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, Format,
};

let vertex_layout = VertexLayout {
    bindings: vec![
        VertexBinding {
            binding: 0,
            stride: 20,  // 2 floats (pos) + 3 floats (color) = 5 * 4 octets
            input_rate: VertexInputRate::Vertex,
        },
    ],
    attributes: vec![
        VertexAttribute {
            location: 0,
            binding: 0,
            format: Format::R32G32_SFLOAT,  // vec2 position
            offset: 0,
        },
        VertexAttribute {
            location: 1,
            binding: 0,
            format: Format::R32G32B32_SFLOAT,  // vec3 couleur
            offset: 8,  // Après 2 floats (2 * 4 octets)
        },
    ],
};
```

### Créer un pipeline graphique

```rust
use galaxy_3d_engine::{PipelineDesc, PrimitiveTopology};

let pipeline = {
    let mut renderer_guard = renderer.lock().unwrap();
    renderer_guard.create_pipeline(PipelineDesc {
        vertex_shader: vertex_shader.clone(),
        fragment_shader: fragment_shader.clone(),
        vertex_layout,
        topology: PrimitiveTopology::TriangleList,
    })?
};
```

---

## Boucle de rendu

### Boucle de rendu basique

```rust
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

event_loop.run(move |event, window_target| {
    match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => {
                // Nettoyer les ressources
                renderer.lock().unwrap().wait_idle().ok();
                window_target.exit();
            }
            WindowEvent::RedrawRequested => {
                // Rendu de la frame
                if let Ok(mut renderer_guard) = renderer.lock() {
                    if let Ok(frame) = renderer_guard.begin_frame() {
                        // Lier le pipeline et les buffers
                        frame.bind_pipeline(&pipeline).ok();
                        frame.bind_vertex_buffer(&vertex_buffer, 0).ok();

                        // Dessiner le triangle (3 vertices)
                        frame.draw(3, 0).ok();

                        // Terminer la frame
                        renderer_guard.end_frame(frame).ok();
                    }
                }

                window.request_redraw();
            }
            _ => {}
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    }
}).unwrap();
```

### API d'enregistrement de frame

```rust
// Commencer la frame - acquiert l'image swapchain et commence le command buffer
let frame = renderer.lock().unwrap().begin_frame()?;

// Lier le pipeline graphique
frame.bind_pipeline(&pipeline)?;

// Lier le buffer vertex au binding 0
frame.bind_vertex_buffer(&vertex_buffer, 0)?;

// Dessiner 3 vertices en commençant au vertex 0
frame.draw(vertex_count: 3, first_vertex: 0)?;

// Terminer la frame - soumettre les commandes et présenter
renderer.lock().unwrap().end_frame(frame)?;
```

---

## Gestion des ressources

### Pattern RAII

Les ressources sont automatiquement détruites lorsqu'elles sont droppées :

```rust
{
    let buffer = renderer.lock().unwrap().create_buffer(desc)?;
    // Utiliser le buffer...
} // Buffer automatiquement détruit ici
```

### Nettoyage explicite

Pour un contrôle explicite de l'ordre de nettoyage :

```rust
// Dropper les ressources dans l'ordre inverse de création
drop(pipeline);
drop(vertex_buffer);
drop(fragment_shader);
drop(vertex_shader);

// Attendre que le GPU termine avant de dropper le renderer
renderer.lock().unwrap().wait_idle().ok();
drop(renderer);
```

### Durées de vie des ressources

Les ressources doivent survivre à toutes les frames qui les utilisent :

```rust
// CORRECT : Ressources créées avant la boucle de rendu
let vertex_buffer = create_buffer(...)?;
let pipeline = create_pipeline(...)?;

loop {
    let frame = begin_frame()?;
    frame.bind_pipeline(&pipeline)?;
    frame.bind_vertex_buffer(&vertex_buffer, 0)?;
    end_frame(frame)?;
}

// INCORRECT : Ne pas créer de ressources dans la boucle de rendu
loop {
    let vertex_buffer = create_buffer(...)?;  // ❌ Ressource droppée trop tôt !
    let frame = begin_frame()?;
    frame.bind_vertex_buffer(&vertex_buffer, 0)?;
    end_frame(frame)?;
}
```

---

## Gestion des erreurs

### Type Result

Toutes les opérations faillibles retournent `RenderResult<T>` :

```rust
use galaxy_3d_engine::RenderResult;

fn create_resources() -> RenderResult<()> {
    let buffer = renderer.lock().unwrap().create_buffer(desc)?;
    let shader = renderer.lock().unwrap().create_shader(desc)?;
    Ok(())
}
```

### Types d'erreurs

```rust
use galaxy_3d_engine::RenderError;

match renderer.lock().unwrap().begin_frame() {
    Ok(frame) => {
        // Rendu...
    }
    Err(RenderError::BackendError(msg)) => {
        eprintln!("Erreur backend : {}", msg);
    }
    Err(RenderError::InitializationFailed(msg)) => {
        eprintln!("Échec d'initialisation : {}", msg);
    }
    Err(e) => {
        eprintln!("Autre erreur : {:?}", e);
    }
}
```

---

## Support multi-écrans

### Gestion du redimensionnement

Le renderer recrée automatiquement la swapchain lorsque la fenêtre est redimensionnée :

```rust
WindowEvent::Resized(new_size) => {
    // Notifier le renderer du redimensionnement
    renderer.lock().unwrap().resize(new_size.width, new_size.height);
}
```

### Support multi-moniteurs

Lorsque vous déplacez des fenêtres entre moniteurs avec différentes résolutions, la swapchain est automatiquement recréée :

```rust
// Gérer simplement les événements de redimensionnement - le renderer fait le reste
WindowEvent::Resized(new_size) => {
    if new_size.width > 0 && new_size.height > 0 {
        renderer.lock().unwrap().resize(new_size.width, new_size.height);
    }
}
```

### Saut de frame pendant le redimensionnement

Pendant la récréation de la swapchain, `begin_frame()` peut retourner une erreur. Sautez simplement cette frame :

```rust
match renderer.lock().unwrap().begin_frame() {
    Ok(frame) => {
        // Rendu normal
        frame.bind_pipeline(&pipeline)?;
        // ...
        renderer.lock().unwrap().end_frame(frame)?;
    }
    Err(_) => {
        // Swapchain en cours de récréation - sauter cette frame
    }
}
```

---

## Exemple complet

Voir [`galaxy3d_demo`](../../Games/galaxy3d_demo) pour un exemple complet fonctionnel qui démontre :

- Initialisation du renderer
- Chargement des shaders
- Création de buffers vertex
- Configuration du pipeline graphique
- Boucle de rendu avec synchronisation des frames
- Support multi-écrans
- Nettoyage approprié des ressources

---

## Référence API

Pour la documentation API détaillée, exécutez :

```bash
cargo doc --open -p galaxy_3d_engine
```

---

## Dépannage

### Erreurs de validation Vulkan

Activer les couches de validation en mode debug :

```rust
let config = RendererConfig {
    enable_validation: cfg!(debug_assertions),  // Seulement en debug
    ..Default::default()
};
```

### Écran noir

- Vérifier que les shaders sont correctement compilés en SPIR-V
- Vérifier que le layout vertex correspond aux entrées du shader
- Vérifier l'ordre d'enroulement des triangles (utiliser `CullMode::NONE` pour déboguer)

### Fuites de ressources

- Toujours appeler `wait_idle()` avant de dropper le renderer
- Dropper les ressources dans l'ordre inverse de création
- Utiliser les couches de validation pour détecter les ressources non libérées

---

## Prochaines étapes

- Explorer le document de conception [CLAUDE.md](./CLAUDE.md)
- Étudier le code source de [galaxy3d_demo](../../Games/galaxy3d_demo)
- Consulter le Vulkan Tutorial pour des techniques de rendu avancées
