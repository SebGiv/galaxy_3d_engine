# Galaxy3DEngine - Usage Guide

Complete guide for using the Galaxy3D rendering engine in your Rust applications.

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [Core Concepts](#core-concepts)
3. [Creating a Renderer](#creating-a-renderer)
4. [Loading Shaders](#loading-shaders)
5. [Creating Buffers](#creating-buffers)
6. [Creating Pipelines](#creating-pipelines)
7. [Rendering Loop](#rendering-loop)
8. [Resource Management](#resource-management)
9. [Error Handling](#error-handling)
10. [Multi-Screen Support](#multi-screen-support)

---

## Quick Start

### Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
galaxy_3d_engine = { path = "../Galaxy3DEngine/galaxy_3d_engine" }
galaxy_3d_engine_renderer_vulkan = { path = "../Galaxy3DEngine/galaxy_3d_engine_renderer_vulkan" }
winit = "0.30"
```

### Minimal Example

```rust
use galaxy_3d_engine::{Renderer, RendererConfig};
use galaxy_3d_engine_renderer_vulkan;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::Window;
use std::sync::{Arc, Mutex};

fn main() {
    // Register Vulkan backend plugin
    galaxy_3d_engine_renderer_vulkan::register();

    // Create window
    let event_loop = EventLoop::new().unwrap();
    let window = Window::default_attributes()
        .with_title("Galaxy3D")
        .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
        .build(&event_loop)
        .unwrap();

    // Create renderer with configuration
    let config = RendererConfig {
        enable_validation: true,
        app_name: "My App".to_string(),
        app_version: (1, 0, 0),
    };

    let renderer = galaxy_3d_engine::renderer_plugin_registry()
        .lock().unwrap()
        .as_ref().unwrap()
        .create_renderer("vulkan", &window, config)
        .expect("Failed to create renderer");

    // Event loop
    event_loop.run(|event, window_target| {
        // Handle events and render
    }).unwrap();
}
```

---

## Core Concepts

### Trait-Based Polymorphism

Galaxy3D uses Rust traits with `Arc<dyn Trait>` to achieve C++-style polymorphism:

- **`Renderer`**: Main interface for creating GPU resources
- **`RendererBuffer`**: GPU buffer (vertex, index, uniform)
- **`RendererShader`**: Compiled shader module
- **`RendererPipeline`**: Graphics pipeline state
- **`RendererFrame`**: Per-frame command recording

### Resource Lifetime

All GPU resources are automatically cleaned up when dropped (RAII pattern). Resources must be kept alive as long as they're used.

### Thread Safety

The `Renderer` is wrapped in `Arc<Mutex<...>>` for thread-safe access. Lock the mutex when creating resources or recording commands.

---

## Creating a Renderer

### Configuration

```rust
use galaxy_3d_engine::RendererConfig;

let config = RendererConfig {
    enable_validation: true,        // Enable Vulkan validation layers
    app_name: "MyApp".to_string(),  // Application name
    app_version: (1, 0, 0),         // Version (major, minor, patch)
};
```

### Backend Registration

```rust
// Register Vulkan backend
galaxy_3d_engine_renderer_vulkan::register();

// Create renderer via plugin registry
let renderer = galaxy_3d_engine::renderer_plugin_registry()
    .lock().unwrap()
    .as_ref().unwrap()
    .create_renderer("vulkan", &window, config)?;
```

---

## Loading Shaders

Shaders must be precompiled to SPIR-V format (use `glslc` from Vulkan SDK).

### Vertex Shader Example (triangle.vert)

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

### Fragment Shader Example (triangle.frag)

```glsl
#version 450

layout(location = 0) in vec3 fragColor;
layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(fragColor, 1.0);
}
```

### Compile Shaders

```bash
glslc triangle.vert -o triangle_vert.spv
glslc triangle.frag -o triangle_frag.spv
```

### Load Shaders in Code

```rust
use galaxy_3d_engine::{ShaderDesc, ShaderStage};

// Load shader bytecode
let vert_spv = include_bytes!("../shaders/triangle_vert.spv");
let frag_spv = include_bytes!("../shaders/triangle_frag.spv");

// Create shader modules
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

## Creating Buffers

### Vertex Data Structure

```rust
#[repr(C)]
struct Vertex {
    pos: [f32; 2],    // Position (x, y)
    color: [f32; 3],  // Color (r, g, b)
}

let vertices = [
    Vertex { pos: [ 0.0, -0.5], color: [1.0, 0.0, 0.0] }, // Top (red)
    Vertex { pos: [ 0.5,  0.5], color: [0.0, 1.0, 0.0] }, // Bottom-right (green)
    Vertex { pos: [-0.5,  0.5], color: [0.0, 0.0, 1.0] }, // Bottom-left (blue)
];
```

### Create Vertex Buffer

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

// Upload vertex data
let vertex_data = unsafe {
    std::slice::from_raw_parts(
        vertices.as_ptr() as *const u8,
        std::mem::size_of_val(&vertices),
    )
};

vertex_buffer.update(0, vertex_data)?;
```

---

## Creating Pipelines

### Define Vertex Layout

```rust
use galaxy_3d_engine::{
    VertexLayout, VertexBinding, VertexAttribute,
    VertexInputRate, Format,
};

let vertex_layout = VertexLayout {
    bindings: vec![
        VertexBinding {
            binding: 0,
            stride: 20,  // 2 floats (pos) + 3 floats (color) = 5 * 4 bytes
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
            format: Format::R32G32B32_SFLOAT,  // vec3 color
            offset: 8,  // After 2 floats (2 * 4 bytes)
        },
    ],
};
```

### Create Graphics Pipeline

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

## Rendering Loop

### Basic Render Loop

```rust
use winit::event::{Event, WindowEvent};
use winit::event_loop::ControlFlow;

event_loop.run(move |event, window_target| {
    match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => {
                // Clean up resources
                renderer.lock().unwrap().wait_idle().ok();
                window_target.exit();
            }
            WindowEvent::RedrawRequested => {
                // Render frame
                if let Ok(mut renderer_guard) = renderer.lock() {
                    if let Ok(frame) = renderer_guard.begin_frame() {
                        // Bind pipeline and buffers
                        frame.bind_pipeline(&pipeline).ok();
                        frame.bind_vertex_buffer(&vertex_buffer, 0).ok();

                        // Draw triangle (3 vertices)
                        frame.draw(3, 0).ok();

                        // End frame
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

### Frame Recording API

```rust
// Begin frame - acquires swapchain image and begins command buffer
let frame = renderer.lock().unwrap().begin_frame()?;

// Bind graphics pipeline
frame.bind_pipeline(&pipeline)?;

// Bind vertex buffer at binding 0
frame.bind_vertex_buffer(&vertex_buffer, 0)?;

// Draw 3 vertices starting at vertex 0
frame.draw(vertex_count: 3, first_vertex: 0)?;

// End frame - submits commands and presents
renderer.lock().unwrap().end_frame(frame)?;
```

---

## Resource Management

### RAII Pattern

Resources are automatically destroyed when dropped:

```rust
{
    let buffer = renderer.lock().unwrap().create_buffer(desc)?;
    // Use buffer...
} // Buffer automatically destroyed here
```

### Explicit Cleanup

For explicit control over cleanup order:

```rust
// Drop resources in reverse creation order
drop(pipeline);
drop(vertex_buffer);
drop(fragment_shader);
drop(vertex_shader);

// Wait for GPU to finish before dropping renderer
renderer.lock().unwrap().wait_idle().ok();
drop(renderer);
```

### Resource Lifetimes

Resources must outlive any frames that use them:

```rust
// CORRECT: Resources created before render loop
let vertex_buffer = create_buffer(...)?;
let pipeline = create_pipeline(...)?;

loop {
    let frame = begin_frame()?;
    frame.bind_pipeline(&pipeline)?;
    frame.bind_vertex_buffer(&vertex_buffer, 0)?;
    end_frame(frame)?;
}

// INCORRECT: Don't create resources inside render loop
loop {
    let vertex_buffer = create_buffer(...)?;  // ❌ Resource dropped too early!
    let frame = begin_frame()?;
    frame.bind_vertex_buffer(&vertex_buffer, 0)?;
    end_frame(frame)?;
}
```

---

## Error Handling

### Result Type

All fallible operations return `RenderResult<T>`:

```rust
use galaxy_3d_engine::RenderResult;

fn create_resources() -> RenderResult<()> {
    let buffer = renderer.lock().unwrap().create_buffer(desc)?;
    let shader = renderer.lock().unwrap().create_shader(desc)?;
    Ok(())
}
```

### Error Types

```rust
use galaxy_3d_engine::RenderError;

match renderer.lock().unwrap().begin_frame() {
    Ok(frame) => {
        // Render...
    }
    Err(RenderError::BackendError(msg)) => {
        eprintln!("Backend error: {}", msg);
    }
    Err(RenderError::InitializationFailed(msg)) => {
        eprintln!("Initialization failed: {}", msg);
    }
    Err(e) => {
        eprintln!("Other error: {:?}", e);
    }
}
```

---

## Multi-Screen Support

### Window Resize Handling

The renderer automatically recreates the swapchain when the window is resized:

```rust
WindowEvent::Resized(new_size) => {
    // Notify renderer of resize
    renderer.lock().unwrap().resize(new_size.width, new_size.height);
}
```

### Multi-Monitor Support

When moving windows between monitors with different resolutions, the swapchain is automatically recreated:

```rust
// Just handle resize events - the renderer does the rest
WindowEvent::Resized(new_size) => {
    if new_size.width > 0 && new_size.height > 0 {
        renderer.lock().unwrap().resize(new_size.width, new_size.height);
    }
}
```

### Frame Skipping During Resize

During swapchain recreation, `begin_frame()` may return an error. Simply skip that frame:

```rust
match renderer.lock().unwrap().begin_frame() {
    Ok(frame) => {
        // Normal rendering
        frame.bind_pipeline(&pipeline)?;
        // ...
        renderer.lock().unwrap().end_frame(frame)?;
    }
    Err(_) => {
        // Swapchain being recreated - skip this frame
    }
}
```

---

## Complete Example

See [`galaxy3d_demo`](../../Games/galaxy3d_demo) for a complete working example that demonstrates:

- Renderer initialization
- Shader loading
- Vertex buffer creation
- Graphics pipeline setup
- Render loop with frame synchronization
- Multi-screen support
- Proper resource cleanup

---

## API Reference

For detailed API documentation, run:

```bash
cargo doc --open -p galaxy_3d_engine
```

---

## Troubleshooting

### Vulkan Validation Errors

Enable validation layers in debug builds:

```rust
let config = RendererConfig {
    enable_validation: cfg!(debug_assertions),  // Only in debug builds
    ..Default::default()
};
```

### Black Screen

- Check that shaders are correctly compiled to SPIR-V
- Verify vertex layout matches shader input
- Ensure triangle winding order (use `CullMode::NONE` for debugging)

### Resource Leaks

- Always call `wait_idle()` before dropping the renderer
- Drop resources in reverse creation order
- Use validation layers to detect unreleased resources

---

## Next Steps

- Explore the [CLAUDE.md](./CLAUDE.md) design document
- Study the [galaxy3d_demo](../../Games/galaxy3d_demo) source code
- Check out the Vulkan Tutorial for advanced rendering techniques
