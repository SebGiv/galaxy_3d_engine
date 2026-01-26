# Galaxy3DEngine

> Modern 3D rendering engine in Rust with Vulkan backend

## Features

- ✅ **Backend-Agnostic API**: 100% portable, ready for Direct3D 12
- ✅ **Texture System**: PNG/BMP/JPEG support with alpha blending
- ✅ **Descriptor Sets**: Abstract API, no Vulkan types exposed
- ✅ **Modern Architecture**: Clean separation render/presentation
- ✅ **Push Constants**: Animation support (rotating triangles)
- ✅ **Command Lists**: Flexible command recording
- ✅ **Render Targets**: Ready for render-to-texture
- ✅ **Multi-Screen**: Swapchain recreation automatique
- ✅ **Zero Validation Errors**: Clean Vulkan implementation
- ✅ **Memory Safe**: Proper framebuffer lifecycle management

## Architecture

**Core Traits** (100% Backend-Agnostic):
- `Renderer` - Main device interface (factory + submit)
- `RendererCommandList` - Command recording (replaces old RendererFrame)
- `RendererSwapchain` - Swapchain management (acquire/present)
- `RendererRenderTarget` - Render target (texture or swapchain)
- `RendererRenderPass` - Render pass configuration
- `RendererDescriptorSet` - Descriptor set abstraction (new!)
- `RendererTexture`, `RendererBuffer`, `RendererShader`, `RendererPipeline`

**Vulkan Backend**: Complete implementation in `galaxy_3d_engine_renderer_vulkan`

## Quick Start

```bash
cd Games/galaxy3d_demo
cargo run
```

Affiche 3 quads texturés (PNG, BMP, JPEG) avec transparence alpha.

## Documentation

- [CLAUDE.md](./CLAUDE.md) - Complete design document
- [USAGE.md](./USAGE.md) - API usage guide (English)
- [USAGE.fr.md](./USAGE.fr.md) - Guide d'utilisation (Français)

## Requirements

- Rust 1.92+ (2024 edition)
- Vulkan SDK 1.4+
- GPU with Vulkan 1.3+ support

## Status

**Phase 9** (2026-01-27): Backend-Agnostic API ✅
- 100% portable, ready for Direct3D 12 backend
- Zero Vulkan references in demo (0 violations, 0 leaks)
- Texture system with PNG/BMP/JPEG support
- Alpha blending support
- Descriptor sets abstraction
- Score: 10/10 portability

## License

MIT
