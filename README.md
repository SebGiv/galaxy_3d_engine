# Galaxy3DEngine

> Modern 3D rendering engine in Rust with Vulkan backend

## Features

- ✅ **Modern Architecture** (Proposition 2): Séparation render/présentation
- ✅ **Push Constants**: Animation support (rotating triangles)
- ✅ **Command Lists**: Flexible command recording
- ✅ **Render Targets**: Ready for render-to-texture
- ✅ **Multi-Screen**: Swapchain recreation automatique
- ✅ **Zero Validation Errors**: Clean Vulkan implementation
- ✅ **Memory Safe**: Proper framebuffer lifecycle management

## Architecture

**Core Traits**:
- `RendererDevice` - Main device interface (factory + submit)
- `RenderCommandList` - Command recording (replaces old RendererFrame)
- `RendererSwapchain` - Swapchain management (acquire/present)
- `RendererRenderTarget` - Render target (texture or swapchain)
- `RendererRenderPass` - Render pass configuration
- `RendererTexture`, `RendererBuffer`, `RendererShader`, `RendererPipeline`

**Vulkan Backend**: Complete implementation in `galaxy_3d_engine_renderer_vulkan`

## Quick Start

```bash
cd Games/galaxy3d_demo
cargo run
```

Affiche 3 triangles colorés animés (rotation via push constants).

## Documentation

- [CLAUDE.md](./CLAUDE.md) - Complete design document
- [USAGE.md](./USAGE.md) - API usage guide (English)
- [USAGE.fr.md](./USAGE.fr.md) - Guide d'utilisation (Français)

## Requirements

- Rust 1.92+ (2024 edition)
- Vulkan SDK 1.4+
- GPU with Vulkan 1.3+ support

## Status

**Phase 7** (2026-01-25): Architecture Moderne ✅
- Push constants support
- Framebuffer memory leaks fixed
- Command list double buffering
- Ready for render-to-texture

## License

MIT
