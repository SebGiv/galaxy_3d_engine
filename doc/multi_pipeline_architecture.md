# Multi-Pipeline Architecture — Design Reflexion

> **Date**: 2026-03-09
> **Status**: Reflexion / Not implemented yet

---

## Problem Statement

A single RenderInstance may need to be drawn multiple times per frame with
different pipelines:

- **Forward PBR** — main visual rendering
- **Shadow map** — depth-only shader for shadow casting
- **Outline** — silhouette shader for selected/highlighted objects
- **Depth prepass** — early Z for performance
- **Skinned variants** — same passes but with skinned vertex shaders

The current architecture binds one pipeline per material, which doesn't scale
to multiple passes drawing the same instance with different shaders.

---

## Key Insight: Pipeline = f(pass, material, instance)

A Vulkan pipeline is not a single entity. It is the **combination** of three
independent axes:

| Axis           | Examples                    | Owner           |
|----------------|-----------------------------|-----------------|
| **Pass**       | Forward, Shadow, Outline    | Render Graph    |
| **Surface**    | PBR, Toon, Unlit            | Material        |
| **Vertex**     | Static, Skinned, Instanced  | Mesh / Instance |

Concrete pipeline = `(pass_type x shader_model x vertex_type)`:

```
forward + PBR  + static = pbr.vert         + pbr.frag
forward + PBR  + skinned = pbr_skinned.vert + pbr.frag
shadow  + PBR  + static = depth.vert        + depth.frag
shadow  + PBR  + skinned = depth_skinned.vert + depth.frag
outline + any  + static = outline.vert       + outline.frag
```

---

## Material Redesign

The material should NOT reference a pipeline. It describes a surface:

```rust
struct Material {
    shader_model: ShaderModel,   // PBR, Toon, Unlit
    blend_mode: BlendMode,       // Opaque, AlphaBlend, AlphaTest
    double_sided: bool,

    // Data (textures, parameters)
    textures: Vec<TextureSlot>,
    parameters: Vec<f32>,
}
```

The pipeline is resolved dynamically from the material's properties combined
with the pass and instance context.

---

## Pipeline Cache

Pipelines are derived objects, resolved on demand and cached:

```rust
#[derive(Hash, Eq, PartialEq)]
struct PipelineKey {
    pass_type: PassType,
    shader_model: ShaderModel,
    vertex_type: VertexType,
    blend_mode: BlendMode,
    double_sided: bool,
}

struct PipelineCache {
    cache: HashMap<PipelineKey, Pipeline>,
    shader_library: ShaderLibrary,
}
```

First request for a given key compiles the pipeline. Subsequent requests are
a HashMap lookup. No manual pipeline creation needed.

---

## Two Categories of Passes

### Material-driven passes
The fragment shader comes from the material's shader_model.
- Forward pass (PBR, Toon, Unlit — each material uses its own shader)

### Technique-driven passes
The fragment shader is imposed by the pass (shader_override).
- Shadow map → depth-only for everything
- Outline → outline shader for everything
- Depth prepass → depth-only

Note: the boundary is not always clean. Shadow passes may need to sample
the albedo texture for alpha-tested materials (leaves, fences).

---

## Draw Call Collection (Unreal-inspired)

Each pass collects draw commands:

```
1. FILTER instances by layer_mask
2. RESOLVE pipeline key = (pass, material.shader_model, instance.vertex_type)
3. GET pipeline from cache (create if first time)
4. BUILD DrawCommand { pipeline, material_bindings, geometry, sort_key }
5. SORT by sort_key (minimize GPU state changes)
6. SUBMIT to command list
```

---

## Render Layers (on RenderInstance)

```rust
struct RenderInstance {
    render_layers: u32,  // bitmask
    // ...
}
```

Bit assignments (example):
- Bit 0: VISIBLE (forward pass)
- Bit 1: CAST_SHADOW (shadow pass)
- Bit 2: OUTLINED (outline pass)
- Bit 3: HIGHLIGHTED (mouse hover)

A pass draws an instance if `(instance.render_layers & pass.layer_mask) != 0`.

---

## Current CustomAction Approach

### What works well today
- Maximum flexibility in each callback
- Fine-grained control (when/how to use RenderIndex, multiple draws, etc.)
- Good for prototyping and special passes (tonemap, debug viz)

### What will become problematic
- Each callback reimplements the same pattern: filter → resolve pipeline → sort → draw
- The render graph has no visibility into what passes are doing (can't optimize, validate, parallelize)
- Duplication grows with each new pass type
- Layer filtering is implicit (hidden inside callbacks)

### Proposed evolution
Keep CustomAction for truly special cases (fullscreen effects, debug).
Introduce a SceneDraw action for instance-based passes:

```rust
enum PassAction {
    Custom(callback),               // 100% free, for special cases

    SceneDraw {                     // Dedicated to drawing scene instances
        layer_mask: u32,
        shader_override: Option<ShaderModel>,
        sort_mode: SortMode,        // FrontToBack, BackToFront, ByPipeline
    },
}
```

SceneDraw encapsulates the common pattern. Custom remains for everything else.

**When to make this transition**: when the same filter-resolve-sort-draw
pattern is copy-pasted for the 3rd time across different callbacks.

---

## SceneDrawAction — Detailed Design

### Concept

SceneDrawAction is a new PassAction variant that handles the full
draw-instances-from-scene workflow declaratively. The render graph knows
**what** the pass wants to draw (which layers, which pipeline behavior),
and the engine handles the **how** (filter, resolve, sort, submit).

CustomAction remains for passes that don't draw scene instances (fullscreen
tonemap, debug overlays, compute dispatches, etc.).

### Interface

```rust
enum PassAction {
    /// Fully custom — callback controls everything.
    /// Best for: fullscreen effects, compute, debug, anything non-scene.
    Custom(Box<dyn PassCallback>),

    /// Draws scene instances through the standard pipeline.
    /// Best for: any pass that iterates over visible instances.
    SceneDraw(SceneDrawAction),
}

struct SceneDrawAction {
    /// Which instances participate in this pass.
    layer_mask: u32,

    /// If Some, overrides the material's shader_model for all instances.
    /// Used by technique-driven passes (shadow, depth prepass, outline).
    /// If None, each instance uses its own material's shader_model.
    shader_override: Option<ShaderModel>,

    /// How to sort draw commands before submission.
    sort_mode: SortMode,

    /// Optional: override render states (depth write, blend, cull).
    /// If None, derived from each material's properties.
    render_state_override: Option<RenderStateOverride>,

    /// Optional: additional per-pass data accessible by the shader.
    /// Example: outline color, highlight intensity.
    pass_data: Option<Arc<dyn Any + Send + Sync>>,
}

enum SortMode {
    /// Sort by pipeline first, then material — minimizes GPU state changes.
    /// Best for opaque geometry.
    ByPipeline,

    /// Sort front-to-back — maximizes early-Z rejection.
    /// Best for depth prepass and opaque forward.
    FrontToBack,

    /// Sort back-to-front — required for correct alpha blending.
    /// Best for transparent geometry.
    BackToFront,
}

struct RenderStateOverride {
    depth_write: Option<bool>,
    depth_test: Option<bool>,
    cull_mode: Option<CullMode>,
    blend_mode: Option<BlendMode>,
}
```

### Execution Flow

When the render graph encounters a SceneDrawAction, the engine does:

```
1. FILTER
   instances = scene.visible_instances()
       .filter(|i| (i.render_layers & action.layer_mask) != 0)

2. RESOLVE PIPELINE (per instance)
   for each instance:
       shader_model = action.shader_override
                      .unwrap_or(instance.material.shader_model)
       key = PipelineKey {
           pass_type,
           shader_model,
           vertex_type: instance.vertex_type,
           blend_mode: instance.material.blend_mode,
           double_sided: instance.material.double_sided,
       }
       // Apply render_state_override if present
       pipeline = pipeline_cache.get_or_create(key)

3. BUILD DRAW COMMANDS
   commands.push(DrawCommand {
       sort_key: compute_sort_key(action.sort_mode, pipeline, material, distance),
       pipeline,
       material_bindings,
       geometry,
       instance_offset,
   })

4. SORT
   commands.sort_by_key(|c| c.sort_key)

5. SUBMIT
   for cmd in commands:
       bind_pipeline_if_changed(cmd.pipeline)
       bind_material_if_changed(cmd.material_bindings)
       bind_geometry(cmd.geometry)
       draw_indexed(cmd.index_count, cmd.instance_offset)
```

### What SceneDrawAction gains over CustomAction for scene rendering

| Aspect | CustomAction | SceneDrawAction |
|--------|-------------|-----------------|
| Layer filtering | Implicit in callback | Declarative, visible to render graph |
| Pipeline resolution | Manual in each callback | Automatic via PipelineCache |
| Sort optimization | Manual in each callback | Built-in, configurable |
| State change minimization | Manual | Automatic (sort by pipeline) |
| Render graph introspection | Opaque (can't see inside) | Transparent (layer_mask, sort_mode visible) |
| Parallelization potential | None (black box) | Possible (disjoint layer_masks) |
| Code duplication | Grows with each pass | Zero — single implementation |

### What CustomAction remains better for

- Fullscreen triangle draws (tonemap, post-processing)
- Compute shader dispatches
- Debug visualization with custom logic
- Passes that need fine-grained control over draw order, multiple draws
  of the same instance, conditional drawing, etc.
- Anything that doesn't follow the filter-resolve-sort-draw pattern

### Migration Path

1. **Now**: keep everything as CustomAction. It works.
2. **Trigger**: when the 3rd pass callback duplicates the filter-resolve-sort-draw
   pattern, extract SceneDrawAction.
3. **First candidates**: Forward opaque, Shadow map, Depth prepass — they all
   follow the exact same pattern with different layer_mask and shader_override.
4. **Keep Custom for**: tonemap, debug viz, any future exotic pass.

### Example: a complete render graph with both action types

```rust
render_graph
    .add_pass("shadow", RenderPass {
        action: PassAction::SceneDraw(SceneDrawAction {
            layer_mask: CAST_SHADOW,
            shader_override: Some(ShaderModel::DepthOnly),
            sort_mode: SortMode::FrontToBack,
            render_state_override: None,
            pass_data: None,
        }),
        color_targets: [],
        depth_target: shadow_map,
    })
    .add_pass("depth_pre", RenderPass {
        action: PassAction::SceneDraw(SceneDrawAction {
            layer_mask: VISIBLE,
            shader_override: Some(ShaderModel::DepthOnly),
            sort_mode: SortMode::FrontToBack,
            ..
        }),
        ..
    })
    .add_pass("forward_opaque", RenderPass {
        action: PassAction::SceneDraw(SceneDrawAction {
            layer_mask: VISIBLE,
            shader_override: None,  // use each material's shader
            sort_mode: SortMode::ByPipeline,
            ..
        }),
        ..
    })
    .add_pass("forward_transparent", RenderPass {
        action: PassAction::SceneDraw(SceneDrawAction {
            layer_mask: VISIBLE_TRANSPARENT,
            shader_override: None,
            sort_mode: SortMode::BackToFront,
            ..
        }),
        ..
    })
    .add_pass("outline", RenderPass {
        action: PassAction::SceneDraw(SceneDrawAction {
            layer_mask: OUTLINED,
            shader_override: Some(ShaderModel::Outline),
            sort_mode: SortMode::ByPipeline,
            pass_data: Some(Arc::new(OutlineParams { color: RED, width: 2.0 })),
            ..
        }),
        ..
    })
    .add_pass("tonemap", RenderPass {
        action: PassAction::Custom(tonemap_callback),  // fullscreen triangle
        ..
    });
```

---

## Frame Lifecycle — Update, Visibility, Render

### The Updater is NOT a Render Pass

The current `Updater` writes CPU-side data to GPU buffers (instance transforms,
camera matrices, light data). This is **CPU work**, not GPU draw commands. It
does not belong inside a render pass or the render graph — it runs **before**
the render graph executes.

### Three Phases per Frame

A frame should be split into three sequential phases:

```
Phase 1: UPDATE (CPU, 1×/frame)
    - Animation, physics, game logic
    - Write instance transforms to GPU buffer
    - Write light data to GPU buffer
    - Write global frame data (time, exposure, sun)
    → Produces: up-to-date GPU buffers

Phase 2: VISIBILITY (CPU, 1×/camera)
    - Frustum culling against camera
    - Build RenderIndex (sorted, visible instances)
    - Compute draw-distance, LOD selection
    → Produces: RenderIndex (compact list of what to draw)

Phase 3: RENDER (GPU, render graph)
    - Render graph executes passes in dependency order
    - SceneDrawAction consumes the RenderIndex
    - CustomAction handles fullscreen effects, compute, debug
    → Produces: final frame
```

### Why This Separation Matters

| Concern | Wrong placement | Correct placement |
|---------|----------------|-------------------|
| Write instance transforms | Inside a render pass | Phase 1 (Update) |
| Frustum culling | Inside each pass callback | Phase 2 (Visibility) |
| Sort draw commands | Duplicated in each callback | Phase 3 (SceneDrawAction) |

The Updater (phases 1-2) produces the **data**. The render graph (phase 3)
**consumes** it. Mixing them creates coupling and prevents parallelization.

### Articulation with SceneDrawAction

```
Updater (phases 1-2)         Render Graph (phase 3)
┌─────────────────────┐      ┌──────────────────────────┐
│ update_transforms()  │      │ shadow_pass (SceneDraw)   │
│ update_lights()      │      │   → reads RenderIndex     │
│ update_camera()      │─────>│ forward_pass (SceneDraw)  │
│ compute_visibility() │      │   → reads RenderIndex     │
│ build_render_index() │      │ tonemap_pass (Custom)     │
└─────────────────────┘      │   → fullscreen triangle   │
                              └──────────────────────────┘
```

The RenderIndex is built once and shared across all SceneDrawAction passes.
Each pass filters it further (via layer_mask) but never rebuilds it.

### Future: Multiple Cameras

When multiple cameras are needed (split screen, security cameras, reflections),
the phases split further:

- **Phase 1** (Update) stays 1×/frame — transforms don't depend on camera
- **Phase 2** (Visibility) runs 1×/camera — each camera has its own frustum
- **Phase 3** (Render) runs 1×/camera — each camera has its own render graph

```
update_scene()              // 1×/frame
for camera in cameras:
    compute_visibility()    // 1×/camera → RenderIndex per camera
    render_graph.execute()  // 1×/camera
```

This is why the Updater should eventually be split into `update_scene()`
(frame-global) and `compute_visibility(camera)` (per-camera).

---

## Dynamic Pipeline States (Vulkan 1.3)

### Fixed vs Dynamic — What Goes Where

Vulkan 1.3 promotes many previously-fixed states to dynamic (via
`VK_EXT_extended_dynamic_state`, now core). This drastically reduces the
number of pipeline permutations.

#### Always fixed in the pipeline (cannot or should not be dynamic)

| State | Reason |
|-------|--------|
| Shader stages (vertex, fragment) | GPU compiles the program |
| Pipeline layout (descriptor set layouts + push constants) | CPU↔GPU interface |
| Color/depth attachment formats | GPU needs formats at compile time |
| Multisample count (`rasterizationSamples`) | Affects fragment shader compilation |
| Blend enable / equations / factors | On tile-based GPUs (ARM Mali, Adreno), blending is implemented as instructions injected into the fragment shader — dynamic blend = shader recompilation at submit time (Khronos Issue #1172) |
| Primitive topology | Almost always `TriangleList` — not worth making dynamic |

#### Dynamic (Vulkan 1.3 core, zero cost on desktop GPUs)

| State | Vulkan function |
|-------|----------------|
| Viewport / Scissor | `vkCmdSetViewport` / `vkCmdSetScissor` |
| Cull mode | `vkCmdSetCullMode` |
| Front face | `vkCmdSetFrontFace` |
| Depth test enable | `vkCmdSetDepthTestEnable` |
| Depth write enable | `vkCmdSetDepthWriteEnable` |
| Depth compare op | `vkCmdSetDepthCompareOp` |
| Depth bias enable + values | `vkCmdSetDepthBiasEnable` / `vkCmdSetDepthBias` |
| Stencil test enable | `vkCmdSetStencilTestEnable` |
| Stencil op | `vkCmdSetStencilOp` |

Each `vkCmdSet*` writes ~8-16 bytes into the command buffer. Cost: ~10-30 ns
per call. The driver handles redundant state filtering internally (confirmed
by Mesa source: "checks if the client is setting the same value and doesn't
dirty the state in that case").

**Recommendation**: set unconditionally before each draw, no application-side
dirty tracking. This is what DXVK and modern engines do.

#### Resulting PipelineKey

With maximum dynamic states, the pipeline key shrinks to:

```rust
struct PipelineKey {
    shader_model: ShaderModel,
    vertex_type: VertexType,
    blend_mode: BlendMode,
    color_formats: SmallVec<TextureFormat>,
    depth_format: Option<TextureFormat>,
    sample_count: SampleCount,
}
```

Typical combination count: 3 shaders × 2 vertex types × 3 blend modes
= **18 pipelines** instead of hundreds.

### Complete Dynamic State Catalog

All Vulkan dynamic states relevant to material/pass control. States marked
"Artistic" are defaults set by the content creator in the material. States
marked "Technical" are overrides imposed by a render pass. Some are both.

#### Rasterization

| # | State | Vulkan function | Artistic | Technical |
|---|-------|-----------------|----------|-----------|
| 1 | Cull mode | `vkCmdSetCullMode` | Double-sided foliage, hair | Outline (Front), shadow anti-acne |
| 2 | Front face | `vkCmdSetFrontFace` | Mirrored objects, inverted normals | — |
| 3 | Polygon mode | `vkCmdSetPolygonModeEXT` | Wireframe style, point cloud | Debug wireframe pass |
| 4 | Line width | `vkCmdSetLineWidth` | Wireframe artistic, construction lines | Debug lines |
| 5 | Line rasterization mode | `vkCmdSetLineRasterizationModeEXT` | Smooth (AA), Bresenham (pixel-art) | — |
| 6 | Line stipple enable | `vkCmdSetLineStippleEnableEXT` | Dashed lines (trajectories, outlines) | — |
| 7 | Line stipple | `vkCmdSetLineStippleKHR` | Pattern + repeat factor | — |

#### Depth

| # | State | Vulkan function | Artistic | Technical |
|---|-------|-----------------|----------|-----------|
| 8 | Depth test enable | `vkCmdSetDepthTestEnable` | Skybox, always-on-top overlays | Post-process fullscreen |
| 9 | Depth write enable | `vkCmdSetDepthWriteEnable` | Glass, water, particles (false) | Depth prepass (true), transparent pass (false) |
| 10 | Depth compare op | `vkCmdSetDepthCompareOp` | Decals (Equal) | Reverse-Z (GreaterOrEqual) |
| 11 | Depth bias enable | `vkCmdSetDepthBiasEnable` | Decal material | Shadow maps |
| 12 | Depth bias | `vkCmdSetDepthBias` | Default decal bias | Shadow/decal pass-specific bias |
| 13 | Depth bounds test enable | `vkCmdSetDepthBoundsTestEnable` | — | Local volumetric effects |
| 14 | Depth bounds | `vkCmdSetDepthBounds` | — | Min/max depth for volumetrics |
| 15 | Depth clamp enable | `vkCmdSetDepthClampEnableEXT` | — | Shadow maps (near plane), hair |
| 16 | Depth clip enable | `vkCmdSetDepthClipEnableEXT` | — | Fine clip vs clamp control |

#### Stencil

| # | State | Vulkan function | Artistic | Technical |
|---|-------|-----------------|----------|-----------|
| 17 | Stencil test enable | `vkCmdSetStencilTestEnable` | — | Portals, mirrors, outline, masked zones |
| 18 | Stencil op | `vkCmdSetStencilOp` | — | Portal write (Replace), read (Equal) |
| 19 | Stencil compare mask | `vkCmdSetStencilCompareMask` | — | Separate stencil bit usage per pass |
| 20 | Stencil write mask | `vkCmdSetStencilWriteMask` | — | Different bits per pass |
| 21 | Stencil reference | `vkCmdSetStencilReference` | Material ID (deferred) | Pass ID (portal #1, #2...) |

#### Color output

| # | State | Vulkan function | Artistic | Technical |
|---|-------|-----------------|----------|-----------|
| 22 | Color write mask | `vkCmdSetColorWriteMaskEXT` | — | Depth-only (0000), velocity (RG only) |
| 23 | Color write enable | `vkCmdSetColorWriteEnableEXT` | — | Shadow pass, depth prepass |
| 24 | Blend constants | `vkCmdSetBlendConstants` | Global fade, ghost effect | — |
| 25 | Alpha to coverage enable | `vkCmdSetAlphaToCoverageEnableEXT` | Foliage, grids, hair | Force in MSAA forward pass |

Note: color blend enable, equations, and advanced blend modes are **fixed** in
the pipeline (see "Always fixed" above). They are part of the `PipelineKey` as
`BlendMode` (Opaque, AlphaBlend, Additive, Premultiplied).

#### Not relevant to materials

Viewport/Scissor, RasterizationSamples, SampleMask, SampleLocations,
PrimitiveTopology, PrimitiveRestart, VertexInput, TessellationDomainOrigin,
PatchControlPoints, RasterizationStream, ConservativeRasterization,
ProvokingVertex, DepthClipNegativeOneToOne, RasterizerDiscardEnable,
LogicOp/LogicOpEnable, FragmentShadingRate, all NV-specific states — these
are controlled by the camera, render target, mesh format, or global settings,
not by materials.

### CPU Cost of Dynamic State Per Draw Call

#### Cost anatomy of a single `vkCmdSet*` call

Each call does three things on the CPU:

1. **Function call** — via `vkGetDeviceProcAddr` dispatch table: ~2-5 ns
   (one indirect call). Via loader trampolines: ~5-15 ns (2-3 extra jumps,
   documented by AMD GPUOpen).
2. **Driver body** — set dirty bit + write token into command buffer:
   ~5-10 ns. Linear write into pre-allocated memory, typically L1 cache hit.
3. **Token size** — 8-24 bytes per command depending on parameter count.

**Total per `vkCmdSet*`: ~10-25 ns.**

#### Cost per draw call with ~20 dynamic states

| Operation | Calls | CPU cost | Cmd buffer bytes |
|-----------|-------|----------|-----------------|
| `vkCmdBindPipeline` | 0-1 | ~30-80 ns | ~16-32 B |
| `vkCmdBindDescriptorSets` | 0-1 | ~30-60 ns | ~32-64 B |
| `vkCmdBindVertexBuffers` | 0-1 | ~20-40 ns | ~24-48 B |
| `vkCmdBindIndexBuffer` | 0-1 | ~15-30 ns | ~16-24 B |
| `vkCmdPushConstants` | 1 | ~15-30 ns | ~16-32 B |
| **20 × `vkCmdSet*`** | **20** | **~200-500 ns** | **~200-400 B** |
| `vkCmdDrawIndexed` | 1 | ~15-30 ns | ~24-32 B |
| **Total per draw call** | | **~325-770 ns** | **~330-630 B** |

The 20 `vkCmdSet*` calls represent ~50-65% of the CPU cost of a draw call.

#### Scaling to real workloads

| Draw calls/frame | Total CPU (cmd recording) | % of 16.6 ms (60 FPS) |
|-----------------|--------------------------|----------------------|
| 1,000 | ~0.5 ms | 3% |
| 2,000 | ~1.0 ms | 6% |
| 5,000 | ~2.5 ms | 15% |
| 10,000 | ~5.0 ms | 30% |
| 50,000 | ~25 ms | >100% — bottleneck |

#### AAA reference: Cyberpunk 2077

A RenderDoc frame capture of Cyberpunk 2077 (zhangdoa.com) shows:
- G-Buffer pass: ~400 instanced draw calls
- Total API calls per frame: <10,000 (draws + binds + barriers)
- Estimated real draw calls: **~1,000-3,000 per frame**
- All geometry uses instancing + bindless textures
- Shadow maps update progressively across multiple frames
- The game is GPU-bound, not CPU-bound

With ~2,000 draw calls × 20 dynamic states × ~15 ns = **~0.6 ms** (3.6% of
frame budget). Negligible.

The trend in modern engines is toward **fewer** draw calls (GPU-driven
rendering, multi-draw indirect, mesh shaders), not more. When draw call count
exceeds ~10,000, the solution is `vkCmdDrawIndexedIndirect` (one API call for
N draws), not reducing dynamic states.

### Render State Ownership — Three Levels

Render state is distributed across three levels, following the pattern used
by Unreal (`FMeshPassProcessorRenderState`), Filament (material `.mat` files),
Bevy (`Material::specialize()`), and The Machinery (override blocks).

#### Level 1: Material (artistic intent)

The material carries the **default** render state — these are artistic
choices made by the content creator:

```rust
struct MaterialData {
    // Shader data
    base_color: [f32; 4],
    roughness: f32,
    metallic: f32,
    albedo_index: u32,
    normal_index: u32,

    // Render state defaults (artistic choices)
    blend_mode: BlendMode,    // Opaque, AlphaBlend, Additive
    cull_mode: CullMode,      // Back, Front, None (double-sided)
    depth_write: bool,        // false for transparents
}
```

#### Level 2: Pass (technical overrides)

The render pass can override any material render state for technical reasons:

```rust
struct SceneDrawAction {
    layer_mask: u32,
    shader_override: Option<ShaderModel>,
    sort_mode: SortMode,
    // Overrides — None = use material value
    cull_override: Option<CullMode>,
    depth_write_override: Option<bool>,
    depth_test_override: Option<bool>,
    color_write_override: Option<bool>,
}
```

Examples:

| Pass | Override | Reason |
|------|----------|--------|
| Shadow | `depth_write=true`, `color_write=false`, `shader=DepthOnly` | Depth only |
| Depth prepass | `depth_write=true`, `color_write=false` | Early Z fill |
| Forward opaque | none | Material decides |
| Forward transparent | `depth_write=false`, `sort=BackToFront` | Correct alpha blending |

#### How it works per render pass

The material stores **one** `DynamicRenderState` — its defaults. Each render
pass in the render graph has its own `RenderStateOverrides`. At draw collection
time, the engine merges the two, producing a **resolved** `DynamicRenderState`
per draw command. The same material yields different resolved states depending
on which render pass is being collected:

```
Material "glass" (stored once):
    cull = None, depth_write = false

  Shadow render pass (overrides: depth_write = true)
    → resolved: { cull = None, depth_write = true }

  Forward render pass (no overrides)
    → resolved: { cull = None, depth_write = false }

  Outline render pass (overrides: cull = Front)
    → resolved: { cull = Front, depth_write = false }
```

Each resolved `DynamicRenderState` is copied into the `DrawCommand` for that
pass. When the render graph executes, each render pass submits its own draw
list with the correct resolved states to the command list.

**Key rule**: no Vulkan calls in the engine. The engine builds `DrawCommand`
structs containing `DynamicRenderState` values. The backend reads those values
and translates them into API-specific calls (`vkCmdSetCullMode`, etc.).

#### Level 3: Backend (resolution + submission)

The backend receives the sorted draw list from the engine and submits to Vulkan:

```rust
// 1. RESOLVE — merge material defaults with pass overrides
struct ResolvedDrawState {
    // Dynamic states
    cull_mode: CullMode,
    depth_test: bool,
    depth_write: bool,
    depth_compare: CompareOp,
    front_face: FrontFace,
    stencil_test: bool,
    // Fixed (pipeline key)
    blend_mode: BlendMode,
    // Draw identity
    pipeline_key: PipelineKey,
    object_id: u32,
    geometry: GeometryRef,
}

fn resolve(mat: &MaterialData, pass: &SceneDrawAction) -> ResolvedDrawState {
    ResolvedDrawState {
        cull_mode:   pass.cull_override.unwrap_or(mat.cull_mode),
        depth_write: pass.depth_write_override.unwrap_or(mat.depth_write),
        depth_test:  pass.depth_test_override.unwrap_or(true),
        // ...
    }
}

// 2. SORT — by (pipeline, blend_mode, material, depth)

// 3. SUBMIT — flat loop, no logic
for draw in &sorted_draws {
    if draw.pipeline_key != current_pipeline {
        vkCmdBindPipeline(cmd, cache.get(draw.pipeline_key));
        current_pipeline = draw.pipeline_key;
    }
    vkCmdSetCullMode(cmd, draw.cull_mode);
    vkCmdSetDepthTestEnable(cmd, draw.depth_test);
    vkCmdSetDepthWriteEnable(cmd, draw.depth_write);
    vkCmdSetDepthCompareOp(cmd, draw.depth_compare);
    vkCmdSetFrontFace(cmd, draw.front_face);
    vkCmdSetStencilTestEnable(cmd, draw.stencil_test);
    vkCmdPushConstants(cmd, draw.object_id);
    vkCmdDrawIndexed(cmd, draw.geometry);
}
```

### Summary

```
Material (artistic)      Pass (technical)       Backend (Vulkan)
┌──────────────────┐    ┌─────────────────┐    ┌──────────────────┐
│ blend_mode       │───→│ override?       │───→│ FIXED: pipeline  │
│ cull_mode        │    │ cull_override   │    │ DYNAMIC: vkCmd*  │
│ depth_write      │    │ depth_override  │    │                  │
│ textures, params │    │ shader_override │    │ Pipeline cache   │
└──────────────────┘    └─────────────────┘    └──────────────────┘
    Content creator         Render graph           Resolve + submit
```

### DrawCommand — Where Resolved State Lives

#### Why not on RenderInstance?

A RenderInstance represents a scene object (a mesh, a character, a particle system).
It has **one** material, but that material may participate in **multiple passes**
with **different render states**:

| Pass | cull_mode | depth_write | color_write |
|------|-----------|-------------|-------------|
| Shadow | Back | true | false |
| Forward | Back | true | true |
| Outline (toon) | **Front** | **false** | true |

A single `DynamicRenderState` on the RenderInstance cannot represent these
divergent states. The resolved state belongs on the **DrawCommand** — the unit
that maps 1:1 with an actual GPU draw call.

#### Resolution at draw list construction

For each visible instance × each pass of its material, the engine resolves
the final state and stores it in a DrawCommand:

```rust
for instance in &visible_instances {
    let mat = instance.material();
    for pass in mat.passes() {
        let state = resolve(mat, pass);
        draw_list.push(DrawCommand {
            mesh: instance.mesh,
            instance_data: instance.data,
            pipeline_key: pass.pipeline_key(),
            render_state: state,  // resolved DynamicRenderState
        });
    }
}
```

The backend receives a flat list of DrawCommands, each carrying its own
resolved `DynamicRenderState`. No resolution logic in the backend — it
only translates engine types to API calls.

#### This architecture scales to advanced features

The DrawCommand approach is not a simplification that limits future ambition.
Each advanced feature adds a **layer in the resolve function**, not a change
to the DrawCommand format or the backend:

| Feature | How it integrates |
|---------|-------------------|
| Material instances with inheritance | `resolve()` walks parent chain: `base → instance → dynamic` |
| Per-instance overrides (VFX, particles) | `RenderInstance` gains `Option<RenderStateOverride>`, fed into `resolve()` |
| Runtime material switching | DrawCommands are rebuilt each frame — changing material = different resolve input |
| Draw command caching | Cache DrawCommands, invalidate via dirty flags when material/pass changes (optimization, not architecture change) |
| Multi-backend support | DrawCommand is API-agnostic — each backend maps `DynamicRenderState` to its own API calls |

The resolve chain grows from:

```
material defaults → pass overrides → resolved
```

To (if needed later):

```
base material → material instance → per-instance override → pass overrides → resolved
```

The DrawCommand struct and the backend submission loop remain unchanged.

#### Why Unreal (and similar AAA engines) are more complex

Engines like Unreal, Frostbite, and id Tech did not arrive at their complexity
by design — it accumulated over decades of shipped titles, each adding edge
cases:

- **Material instance chains**: UE's `UMaterialInstance` can override any
  parameter from its parent, creating arbitrary-depth inheritance trees.
  The resolve must walk N levels at runtime.
- **Per-particle render state**: Niagara (UE's VFX system) allows each
  particle to override blend mode, depth write, etc. Two instances of the
  same material can have completely different render states.
- **Mesh Draw Command caching**: UE caches draw commands across frames to
  avoid per-frame rebuild. This requires a dirty-flag invalidation system
  (material changed? instance moved? LOD switched?) that adds significant
  complexity.
- **Feature level abstraction**: Supporting DX11, DX12, Vulkan, and Metal
  simultaneously means the render state model must abstract over different
  dynamic state capabilities per backend.
- **Backward compatibility**: Each UE version must support content and plugins
  from previous versions, preventing clean-slate redesigns.

Galaxy3D starts from scratch with Vulkan 1.3 as the sole target. The simple
resolve-into-DrawCommand architecture handles the same use cases without the
accumulated complexity. If advanced features are needed, they plug into the
resolve chain — the architecture doesn't change.

---

## Pipeline Source Decomposition — Who Provides What

A Vulkan pipeline requires many parameters, but no single entity owns them all.
Each parameter has a natural owner — the entity whose identity or purpose
determines the parameter's value.

### Source mapping

| Pipeline parameter | Source | Rationale |
|--------------------|--------|-----------|
| **Fragment shader** | Material | The material defines the surface appearance (PBR, Toon, Unlit, etc.) |
| **Vertex shader** | Instance creation (ShaderKey) | Same material can be static, skinned, instanced, billboard, etc. |
| **Vertex layout** | Geometry | The geometry defines the vertex format (position, normal, UV, tangent, joints, weights) |
| **Topology** | Geometry | TriangleList, LineList, etc. — intrinsic to the mesh data |
| **Color blend** | Material | Opaque, AlphaBlend, Additive — artistic choice of the content creator |
| **Rasterization** | Material (defaults) | Polygon mode, cull mode, front face — can be overridden per-pass |
| **Color/depth formats** | RenderFrame / render target | The target dictates the formats (RGBA8, RGBA16F, D32, etc.) |
| **Multisample count** | RenderFrame / render target | MSAA is a property of the framebuffer, not the material |

### Pipeline creation flow at instance creation

When a RenderInstance is created, the engine has access to all sources:

```
RenderInstance creation:
  ├── Mesh          → Geometry → vertex_layout, topology
  ├── Material      → fragment shader, blend, rasterization defaults
  ├── VertexShader  → ShaderKey (static, skinned, instanced, ...)
  └── RenderFrame   → color formats, depth format, multisample count

  → PipelineKey = hash(frag_shader, vert_shader, vertex_layout, topology,
                        blend_mode, color_formats, depth_format, sample_count)
  → PipelineCache.get_or_create(key)
```

### Vertex/Fragment shader compatibility validation

The fragment shader declares **inputs** (varyings) that the vertex shader must
provide as **outputs**. If there is a mismatch, the pipeline is invalid.

#### What must match (per the Vulkan spec)

For each `location` declared as input in the fragment shader:
1. The vertex shader must have an output at the same `location`
2. The **type** must match (vec2, vec3, vec4, float, etc.)
3. The **interpolation** qualifier must be compatible (flat, smooth, noperspective)

Locations not consumed by the fragment shader are silently ignored (vertex
shader can output more than the fragment shader reads).

#### Example — compatible pair

```glsl
// Vertex shader outputs
[[vk::location(0)]] float3 worldPos;
[[vk::location(1)]] float3 normal;
[[vk::location(2)]] float2 texCoord;
[[vk::location(3)]] float4 tangent;

// Fragment shader inputs
[[vk::location(0)]] float3 worldPos;    // ✅ match
[[vk::location(1)]] float3 normal;      // ✅ match
[[vk::location(2)]] float2 texCoord;    // ✅ match
[[vk::location(3)]] float4 tangent;     // ✅ match
```

#### Example — incompatible pair

```glsl
// Vertex shader outputs (simple static — no tangent)
[[vk::location(0)]] float3 worldPos;
[[vk::location(1)]] float3 normal;
[[vk::location(2)]] float2 texCoord;
// no location 3!

// Fragment shader inputs (PBR — expects tangent for normal mapping)
[[vk::location(0)]] float3 worldPos;    // ✅
[[vk::location(1)]] float3 normal;      // ✅
[[vk::location(2)]] float2 texCoord;    // ✅
[[vk::location(3)]] float4 tangent;     // ❌ missing from vertex shader!
```

#### Validation strategy

At pipeline creation time, the engine already has SPIR-V reflection data for
both shaders. The validation checks:

```rust
fn validate_shader_io(
    vert_outputs: &[ReflectedVarying],
    frag_inputs: &[ReflectedVarying],
) -> Result<()> {
    for frag_in in frag_inputs {
        match vert_outputs.iter().find(|v| v.location == frag_in.location) {
            None => error!("Fragment input location {} ({}) not provided by vertex shader",
                           frag_in.location, frag_in.name),
            Some(vert_out) if vert_out.type != frag_in.type =>
                error!("Type mismatch at location {}: vert={:?}, frag={:?}",
                       frag_in.location, vert_out.type, frag_in.type),
            _ => {} // OK
        }
    }
    Ok(())
}
```

This validation runs inside `PipelineCache.get_or_create()`, before calling
`vkCreateGraphicsPipelines`. Fail-fast with a clear error message rather
than a cryptic Vulkan validation error.

#### Practical implications for shader design

To maximize vert/frag compatibility, adopt a **varying interface convention**:

| Location | Name | Type | Used by |
|----------|------|------|---------|
| 0 | worldPos | vec3 | All shaders |
| 1 | normal | vec3 | All shaders |
| 2 | texCoord | vec2 | All shaders |
| 3 | tangent | vec4 | PBR, normal mapping |
| 4 | color | vec4 | Vertex color, particles |
| 5-7 | boneWeights, joints, ... | ... | Skinned meshes |

All vertex shaders should output at least locations 0-2 (the "base interface").
Fragment shaders that need tangent (location 3) can only be paired with vertex
shaders that provide it — the validation catches mismatches at creation time.

---

## Current State Audit — What Exists, What's Missing

> **Date**: 2026-03-29

### Pipeline data sources — exhaustive inventory

| Pipeline parameter | Target source | Exists today? | Current location |
|--------------------|---------------|---------------|------------------|
| Fragment shader | Material | ❌ | Material stores a `PipelineKey` (whole pipeline), not a shader |
| Vertex shader | RenderInstance (ShaderKey) | ❌ | RenderInstance has no shader field |
| Vertex layout | Geometry | ✅ | `Geometry.vertex_layout` |
| Topology | Geometry (SubMesh) | ✅ | `GeometrySubMesh.topology` |
| Color blend | Material | ❌ | Stored in `PipelineDesc.color_blend`, not on Material |
| Rasterization (polygon_mode) | Material | ❌ | Stored in `PipelineDesc.rasterization`, not on Material |
| Color formats | RenderFrame / render target | ❌ | Buried inside RenderTarget's texture, no simple accessor |
| Depth format | RenderFrame / render target | ❌ | Same — buried in texture info |
| Multisample count | RenderFrame / render target | ❌ | Hardcoded to `samples: 1` in render graph compile |

### What must change per entity

#### Material — replace PipelineKey with surface data

| Current field | Target field |
|---------------|-------------|
| `pipeline: PipelineKey` | `fragment_shader: ShaderKey` |
| — | `color_blend: ColorBlendState` |
| — | `polygon_mode: PolygonMode` (fill / wireframe) |
| `render_state: DynamicRenderState` | ✅ Already present (cull, depth, etc.) |
| `textures`, `params` | ✅ Already present |

#### RenderInstance — add vertex shader

| Current field | Missing field |
|---------------|--------------|
| vertex_buffer, index_buffer, lods, world_matrix, bounding_box | `vertex_shader: ShaderKey` |

#### RenderFrameInfo — new struct (does not exist)

Regroup render target properties needed for pipeline creation:

```rust
struct RenderFrameInfo {
    color_formats: SmallVec<TextureFormat>,
    depth_format: Option<TextureFormat>,
    sample_count: SampleCount,
}
```

Extracted from RenderTarget textures at render graph compile time,
passed to the pipeline cache when resolving a PipelineKey.

#### Geometry — already sufficient

`vertex_layout` on Geometry and `topology` on GeometrySubMesh are
already in the right place. Note: topology is per-submesh, which means
the PipelineKey can vary per submesh within the same mesh.

#### PipelineCache — new component (does not exist)

```rust
struct PipelineCache {
    cache: HashMap<PipelineKey, Arc<dyn Pipeline>>,
}

#[derive(Hash, Eq, PartialEq)]
struct PipelineKey {
    fragment_shader: ShaderKey,
    vertex_shader: ShaderKey,
    vertex_layout: VertexLayout,        // from Geometry
    topology: PrimitiveTopology,         // from GeometrySubMesh
    color_blend: ColorBlendState,        // from Material
    color_formats: SmallVec<TextureFormat>, // from RenderFrameInfo
    depth_format: Option<TextureFormat>, // from RenderFrameInfo
    sample_count: SampleCount,           // from RenderFrameInfo
}
```

### Visual summary

```
              TODAY                              TARGET

Material                        Material
├ pipeline: PipelineKey ──────► ├ fragment_shader: ShaderKey
├ textures                      ├ color_blend: ColorBlendState
├ params                        ├ polygon_mode: PolygonMode
└ render_state                  ├ textures
                                ├ params
                                └ render_state (unchanged)

RenderInstance                  RenderInstance
├ vertex_buffer                 ├ vertex_buffer
├ index_buffer                  ├ index_buffer
├ lods                          ├ lods
├ world_matrix                  ├ world_matrix
└ bounding_box                  ├ bounding_box
                                └ vertex_shader: ShaderKey    ← NEW

(does not exist)                RenderFrameInfo               ← NEW
                                ├ color_formats
                                ├ depth_format
                                └ sample_count

Geometry (unchanged)            Geometry
├ vertex_layout ✅              ├ vertex_layout
└ submesh.topology ✅           └ submesh.topology

(does not exist)                PipelineCache                 ← NEW
                                └ cache: HashMap<PipelineKey, Arc<Pipeline>>
```

---

## Lazy Pipeline Creation — Detailed Design

> **Date**: 2026-03-30
> **Status**: Design validated, not yet implemented

### Principle

Pipelines are never created explicitly by the user. They are derived
automatically from the combination of material, geometry, instance, and
render target properties, then cached for reuse.

### Two-phase key construction

The full PipelineKey is built in two phases because the information comes
from different sources at different times:

**Phase 1 — Instance creation time** (stored on the instance as
`InstancePipelineInfo`):

| Field | Source |
|-------|--------|
| `fragment_shader` | Material |
| `vertex_shader` | Passed at instance creation |
| `vertex_layout` | Geometry |
| `topology` | GeometrySubMesh |
| `color_blend` | Material |
| `polygon_mode` | Material |

**Phase 2 — Draw time** (provided by the drawer from the current render
pass):

| Field | Source |
|-------|--------|
| `color_formats` | Render targets of the current pass |
| `depth_format` | Depth target of the current pass |
| `sample_count` | Texture sample count of the current pass |

### PipelineCache — inside ResourceManager

The pipeline cache is a secondary index inside the ResourceManager.
Cached pipelines are regular `resource::Pipeline` objects stored in the
same SlotMap as manually created pipelines — no special case.

```rust
pub struct ResourceManager {
    // ... existing fields ...
    pipelines: SlotMap<PipelineKey, Arc<Pipeline>>,
    pipeline_names: FxHashMap<String, PipelineKey>,

    // Pipeline cache — secondary index for lazy pipeline creation
    pipeline_cache: HashMap<PipelineCacheKey, PipelineKey>,
}
```

The `PipelineCacheKey` is a hashable struct containing all pipeline
creation parameters:

```rust
#[derive(Hash, Eq, PartialEq, Clone)]
struct PipelineCacheKey {
    vertex_shader: ShaderKey,
    fragment_shader: ShaderKey,
    vertex_layout: VertexLayout,
    topology: PrimitiveTopology,
    color_blend: ColorBlendState,
    polygon_mode: PolygonMode,
    color_formats: Vec<TextureFormat>,
    depth_format: Option<TextureFormat>,
    sample_count: SampleCount,
}
```

The full struct is used as HashMap key (not a manually computed hash).
Rust's HashMap computes the hash internally and uses `Eq` for collision
resolution — no risk of false matches.

### resolve_pipeline — the core function

`resolve_pipeline` lives on `PipelineCache` (or as a method on
`ResourceManager`). It is **not** part of the `Drawer` trait — it is a
non-virtual, reusable function that any drawer implementation calls.

```rust
impl ResourceManager {
    pub fn resolve_pipeline(
        &mut self,
        instance: &mut RenderInstance,
        material: &Material,
        geometry: &Geometry,
        submesh: &GeometrySubMesh,
        pass_info: &PassInfo,
        graphics_device: &mut dyn GraphicsDevice,
    ) -> Result<PipelineKey> {
        // Check generation counters — still valid?
        if instance.is_pipeline_valid(
            pass_info.render_graph_generation,
            material.generation(),
        ) {
            return Ok(instance.cached_pipeline_key().unwrap());
        }

        // Build full cache key from all sources
        let cache_key = PipelineCacheKey {
            fragment_shader: material.fragment_shader(),
            vertex_shader: instance.vertex_shader(),
            vertex_layout: geometry.vertex_layout().clone(),
            topology: submesh.topology(),
            color_blend: material.color_blend().clone(),
            polygon_mode: material.polygon_mode(),
            color_formats: pass_info.color_formats.clone(),
            depth_format: pass_info.depth_format,
            sample_count: pass_info.sample_count,
        };

        // Look up in cache
        let pipeline_key = if let Some(&key) = self.pipeline_cache.get(&cache_key) {
            key  // cache hit — pipeline already exists in SlotMap
        } else {
            // Cache miss — create a new resource::Pipeline
            // Name derived from hash for uniqueness and debuggability
            let hash = calculate_hash(&cache_key);
            let name = format!("_cache_{:016X}", hash);
            let key = self.create_pipeline(name, PipelineDesc { ... }, graphics_device)?;
            self.pipeline_cache.insert(cache_key, key);
            key
        };

        // Cache the PipelineKey on the instance (direct SlotMap access)
        instance.set_cached_pipeline(
            pipeline_key,
            pass_info.render_graph_generation,
            material.generation(),
        );

        Ok(pipeline_key)
    }
}
```

### Cached PipelineKey on RenderInstance

Each instance caches its resolved `PipelineKey` (a lightweight SlotMap
index) to avoid HashMap lookups on subsequent frames:

```rust
struct RenderInstance {
    // Phase 1 info (set at creation)
    instance_pipeline_info: InstancePipelineInfo,

    // Cached resolved pipeline (set at first draw)
    cached_pipeline_key: Option<PipelineKey>,
    cached_render_graph_gen: u64,
    cached_material_gen: u64,
}
```

The `PipelineKey` is just a SlotMap key — as lightweight as a `usize`.
The instance does not own the pipeline, it points to it. To get the
actual pipeline, the drawer does `rm.pipeline(instance.cached_pipeline_key)`.

### Draw-time flow

```
For each instance:
  if instance.cached_render_graph_gen != render_graph.generation
  || instance.cached_material_gen != material.generation:
      // Stale or first draw → resolve pipeline
      cache_key = combine(instance.instance_pipeline_info, pass_info)
      pipeline_key = resource_manager.resolve_pipeline(...)
      instance.cached_pipeline_key = Some(pipeline_key)
      instance.cached_render_graph_gen = render_graph.generation
      instance.cached_material_gen = material.generation

  // Pipeline is ready → direct SlotMap access, no HashMap lookup
  pipeline = rm.pipeline(instance.cached_pipeline_key)
  cmd.bind_pipeline(pipeline)
  cmd.draw(...)
```

After the first frame, all instances have their cached PipelineKey and
both generation counters match → zero HashMap lookups, just two integer
comparisons per instance per frame.

### Usage in drawers

The `Drawer` trait remains fully virtual (overridable). Pipeline
resolution is a non-virtual function on `ResourceManager` that any
drawer calls:

```rust
impl Drawer for ForwardDrawer {
    fn draw(&self, scene: &Scene, view: &RenderView, cmd: &mut dyn CommandList) -> Result<()> {
        let rm = Engine::resource_manager()?;
        let mut rm = rm.lock().unwrap();

        for instance in view.visible_instances() {
            let pipeline_key = rm.resolve_pipeline(
                instance, material, geometry, submesh, &pass_info, gd,
            )?;
            let pipeline = rm.pipeline(pipeline_key).unwrap();
            cmd.bind_pipeline(pipeline.graphics_device_pipeline())?;
            cmd.draw(...)?;
        }
    }
}
```

A custom drawer (ParticleDrawer, DebugDrawer, etc.) calls the same
`rm.resolve_pipeline(...)` — the pipeline resolution logic is never
duplicated.

### Auto-generated pipeline naming

Pipelines created by the cache are named `"_cache_{hash:016X}"` where
the hash is derived from the `PipelineCacheKey`. This ensures:

- **Uniqueness**: each combination produces a distinct name
- **No collision**: the `_cache_` prefix avoids conflicts with manually
  named pipelines
- **Debuggability**: pipelines are visible in logs and debug tools with
  an identifiable name

### Invalidation via generation counters

Instead of dirty flags (which require iterating over all instances to mark
them dirty — O(n)), generation counters provide O(1) invalidation with
O(1) per-instance staleness detection.

| Source | Counter | Incremented when |
|--------|---------|------------------|
| RenderGraph | `render_graph.generation` | `compile()` — resize, MSAA change, etc. |
| Material | `material.generation` | Fragment shader, blend, or polygon mode changes (future) |

**How it works:**

1. The render graph increments its generation counter on recompile (O(1))
2. No instance is touched — no iteration needed
3. At draw time, each instance compares its cached generation against the
   current generation — if they differ, recalculate
4. The recalculation usually hits the PipelineCache (same formats, same
   shaders) → no GPU pipeline compilation, just a HashMap lookup

```
Frame 1 (1920×1080, MSAA 4x):
  render_graph.generation = 1

  Instance A: cached_render_graph_gen = 0 (initial)
    → 0 != 1 → stale → resolve pipeline → cache miss → compile
    → cached_render_graph_gen = 1

  Instance B: cached_render_graph_gen = 0
    → 0 != 1 → stale → resolve pipeline → cache hit (same key as A)
    → cached_render_graph_gen = 1

Frame 2:
  render_graph.generation = 1 (unchanged)

  Instance A: cached_render_graph_gen = 1
    → 1 == 1 → up to date → direct draw (zero cost)

--- User resizes window ---
--- render_graph.compile() → generation = 2 ---

Frame 3 (2560×1440, MSAA 4x):
  render_graph.generation = 2

  Instance A: cached_render_graph_gen = 1
    → 1 != 2 → stale → resolve pipeline → cache hit → no GPU compile
    → cached_render_graph_gen = 2
```

Same principle applies independently for `material.generation`.

### Sorting instances by pipeline

Once all instances have their cached pipeline, sorting by pipeline is
trivial — sort draw commands by pipeline pointer or pipeline ID before
submitting to the command list. This minimizes GPU pipeline bind changes.

### Deliberate technical debt: single pass per material

The current design assumes one pipeline per instance (one pass). When
multi-pass rendering is introduced (shadow, outline, etc.), each instance
will need a pipeline key **per pass**. The cached pipeline on the instance
will become a map `pass_id → (pipeline, generation)`.

This debt is accepted for now — the lazy pipeline creation must be
functional and stable before adding multi-pass complexity.

---

## Open Questions

- How to handle shader variants efficiently in Slang? (permutations, #ifdef, specialization constants?)
- Should the ShaderLibrary be compile-time (pre-built SPV) or runtime (Slang JIT)?
- How to integrate alpha-tested materials in technique-driven passes without full material binding?
- Should render_layers be on the Instance, the Mesh, or both?
- PipelineCache invalidation: when a shader is reloaded, how to rebuild affected pipelines?
