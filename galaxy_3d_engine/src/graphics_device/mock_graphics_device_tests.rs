/// Unit tests for MockGraphicsDevice and associated mock types.
///
/// Tests all methods of the mock graphics_device and mock types to ensure
/// complete test coverage.

use crate::graphics_device::mock_graphics_device::*;
use crate::graphics_device::{
    GraphicsDevice, Buffer, Texture, Pipeline, CommandList,
    RenderPass, RenderTarget, Swapchain, BindingGroup, Framebuffer,
    BufferDesc, BufferUsage, TextureDesc, TextureFormat,
    TextureUsage, MipmapMode, ShaderDesc, ShaderStage, PipelineDesc,
    RenderPassDesc, FramebufferDesc,
    Viewport, Rect2D, ClearValue,
    IndexType, VertexLayout, VertexBinding, VertexAttribute,
    BufferFormat, VertexInputRate, PrimitiveTopology,
    TextureType,
};
use std::sync::{Arc, Mutex};

// ============================================================================
// MockBuffer Tests
// ============================================================================

#[test]
fn test_mock_buffer_creation() {
    let buffer = MockBuffer::new(1024, "test_buffer".to_string());
    assert_eq!(buffer.size, 1024);
    assert_eq!(buffer.name, "test_buffer");
}

#[test]
fn test_mock_buffer_update() {
    let buffer = MockBuffer::new(1024, "test_buffer".to_string());
    let data = vec![1u8, 2, 3, 4];

    let result = buffer.update(0, &data);
    assert!(result.is_ok());
}

// ============================================================================
// MockTexture Tests
// ============================================================================

#[test]
fn test_mock_texture_creation() {
    let texture = MockTexture::new(256, 256, 1, TextureType::Tex2D, "test_texture".to_string());
    assert_eq!(texture.name, "test_texture");

    let info = texture.info();
    assert_eq!(info.width, 256);
    assert_eq!(info.height, 256);
    assert_eq!(info.array_layers, 1);
    assert_eq!(info.mip_levels, 1);
}

#[test]
fn test_mock_texture_info() {
    let texture = MockTexture::new(512, 1024, 4, TextureType::Array2D, "indexed_texture".to_string());

    let info = texture.info();
    assert_eq!(info.width, 512);
    assert_eq!(info.height, 1024);
    assert_eq!(info.array_layers, 4);
    assert_eq!(info.format, TextureFormat::R8G8B8A8_UNORM);
    assert_eq!(info.usage, TextureUsage::Sampled);
}

// ============================================================================
// MockShader Tests
// ============================================================================

#[test]
fn test_mock_shader_creation() {
    let shader = MockShader::new("vertex_shader".to_string());
    assert_eq!(shader.name, "vertex_shader");
}

// ============================================================================
// MockPipeline Tests
// ============================================================================

#[test]
fn test_mock_pipeline_creation() {
    let pipeline = MockPipeline::new("test_pipeline".to_string());
    assert_eq!(pipeline.name, "test_pipeline");
}

// ============================================================================
// MockCommandList Tests
// ============================================================================

#[test]
fn test_mock_command_list_creation() {
    let cmd_list = MockCommandList::new();
    assert_eq!(cmd_list.commands.len(), 0);
}

#[test]
fn test_mock_command_list_begin_end() {
    let mut cmd_list = MockCommandList::new();

    cmd_list.begin().unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "begin");

    cmd_list.end().unwrap();
    assert_eq!(cmd_list.commands.len(), 2);
    assert_eq!(cmd_list.commands[1], "end");
}

#[test]
fn test_mock_command_list_render_pass() {
    let mut cmd_list = MockCommandList::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let framebuffer: Arc<dyn Framebuffer> = Arc::new(MockFramebuffer::new(800, 600));
    let clear_values = vec![ClearValue::Color([0.0, 0.0, 0.0, 1.0])];

    cmd_list.begin_render_pass(&render_pass, &framebuffer, &clear_values).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "begin_render_pass");

    cmd_list.end_render_pass().unwrap();
    assert_eq!(cmd_list.commands.len(), 2);
    assert_eq!(cmd_list.commands[1], "end_render_pass");
}

#[test]
fn test_mock_command_list_bind_pipeline() {
    let mut cmd_list = MockCommandList::new();
    let pipeline: Arc<dyn Pipeline> = Arc::new(MockPipeline::new("test".to_string()));

    cmd_list.bind_pipeline(&pipeline).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "bind_pipeline");
}

#[test]
fn test_mock_command_list_bind_buffers() {
    let mut cmd_list = MockCommandList::new();
    let buffer: Arc<dyn Buffer> = Arc::new(MockBuffer::new(1024, "buffer".to_string()));

    cmd_list.bind_vertex_buffer(&buffer, 0).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "bind_vertex_buffer");

    cmd_list.bind_index_buffer(&buffer, 0, IndexType::U16).unwrap();
    assert_eq!(cmd_list.commands.len(), 2);
    assert_eq!(cmd_list.commands[1], "bind_index_buffer");
}

#[test]
fn test_mock_command_list_bind_binding_group() {
    let mut cmd_list = MockCommandList::new();
    let pipeline: Arc<dyn Pipeline> = Arc::new(MockPipeline::new("test".to_string()));
    let binding_group: Arc<dyn BindingGroup> = Arc::new(MockBindingGroup::new("bg".to_string(), 0));

    cmd_list.bind_binding_group(&pipeline, 0, &binding_group).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "bind_binding_group");
}

#[test]
fn test_mock_command_list_draw() {
    let mut cmd_list = MockCommandList::new();

    cmd_list.draw(6, 0).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "draw");
}

#[test]
fn test_mock_command_list_draw_indexed() {
    let mut cmd_list = MockCommandList::new();

    cmd_list.draw_indexed(6, 0, 0).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "draw_indexed");
}

#[test]
fn test_mock_command_list_set_viewport() {
    let mut cmd_list = MockCommandList::new();
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    cmd_list.set_viewport(viewport).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "set_viewport");
}

#[test]
fn test_mock_command_list_set_scissor() {
    let mut cmd_list = MockCommandList::new();
    let scissor = Rect2D {
        x: 0,
        y: 0,
        width: 800,
        height: 600,
    };

    cmd_list.set_scissor(scissor).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "set_scissor");
}

#[test]
fn test_mock_command_list_push_constants() {
    let mut cmd_list = MockCommandList::new();
    let data = vec![1u8, 2, 3, 4];

    cmd_list.push_constants(&[ShaderStage::Vertex], 0, &data).unwrap();
    assert_eq!(cmd_list.commands.len(), 1);
    assert_eq!(cmd_list.commands[0], "push_constants");
}

#[test]
fn test_mock_command_list_complete_workflow() {
    let mut cmd_list = MockCommandList::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let framebuffer: Arc<dyn Framebuffer> = Arc::new(MockFramebuffer::new(800, 600));
    let pipeline: Arc<dyn Pipeline> = Arc::new(MockPipeline::new("test".to_string()));
    let buffer: Arc<dyn Buffer> = Arc::new(MockBuffer::new(1024, "buffer".to_string()));

    // Complete render workflow
    cmd_list.begin().unwrap();
    cmd_list.begin_render_pass(&render_pass, &framebuffer, &vec![]).unwrap();
    cmd_list.bind_pipeline(&pipeline).unwrap();
    cmd_list.bind_vertex_buffer(&buffer, 0).unwrap();
    cmd_list.draw(6, 0).unwrap();
    cmd_list.end_render_pass().unwrap();
    cmd_list.end().unwrap();

    assert_eq!(cmd_list.commands.len(), 7);
    assert_eq!(cmd_list.commands[0], "begin");
    assert_eq!(cmd_list.commands[6], "end");
}

// ============================================================================
// MockRenderPass Tests
// ============================================================================

#[test]
fn test_mock_render_pass_creation() {
    let _render_pass = MockRenderPass::new();
    // No methods to test, just verify it exists
}

// ============================================================================
// MockRenderTarget Tests
// ============================================================================

#[test]
fn test_mock_render_target_creation() {
    let render_target = MockRenderTarget::new(1920, 1080);
    assert_eq!(render_target.width(), 1920);
    assert_eq!(render_target.height(), 1080);
    assert_eq!(render_target.format(), TextureFormat::R8G8B8A8_UNORM);
}

#[test]
fn test_mock_render_target_getters() {
    let render_target = MockRenderTarget::new(640, 480);
    assert_eq!(render_target.width, 640);
    assert_eq!(render_target.height, 480);
}

// ============================================================================
// MockSwapchain Tests
// ============================================================================

#[test]
fn test_mock_swapchain_creation() {
    let swapchain = MockSwapchain::new(3);
    assert_eq!(swapchain.image_count, 3);
}

#[test]
fn test_mock_swapchain_acquire_next_image() {
    let mut swapchain = MockSwapchain::new(3);

    let index = swapchain.acquire_next_image().unwrap();
    assert_eq!(index, 0);
}

#[test]
fn test_mock_swapchain_present() {
    let mut swapchain = MockSwapchain::new(3);

    let result = swapchain.present(0);
    assert!(result.is_ok());
}

#[test]
fn test_mock_swapchain_getters() {
    let swapchain = MockSwapchain::new(3);

    assert_eq!(swapchain.image_count(), 3);
    assert_eq!(swapchain.width(), 800);
    assert_eq!(swapchain.height(), 600);
    assert_eq!(swapchain.format(), TextureFormat::B8G8R8A8_UNORM);
}

#[test]
fn test_mock_swapchain_recreate() {
    let mut swapchain = MockSwapchain::new(3);

    let result = swapchain.recreate(1024, 768);
    assert!(result.is_ok());
}

// ============================================================================
// MockBindingGroup Tests
// ============================================================================

#[test]
fn test_mock_binding_group_creation() {
    let binding_group = MockBindingGroup::new("test_bg".to_string(), 1);
    assert_eq!(binding_group.name, "test_bg");
    assert_eq!(binding_group.set_index, 1);
}

#[test]
fn test_mock_binding_group_trait() {
    let binding_group = MockBindingGroup::new("bg".to_string(), 2);
    let bg: &dyn BindingGroup = &binding_group;
    assert_eq!(bg.set_index(), 2);
}

// ============================================================================
// MockGraphicsDevice Tests
// ============================================================================

#[test]
fn test_mock_graphics_device_creation() {
    let graphics_device = MockGraphicsDevice::new();

    assert_eq!(graphics_device.get_created_buffers().len(), 0);
    assert_eq!(graphics_device.get_created_textures().len(), 0);
    assert_eq!(graphics_device.get_created_shaders().len(), 0);
    assert_eq!(graphics_device.get_created_pipelines().len(), 0);
}

#[test]
fn test_mock_graphics_device_create_texture() {
    let mut graphics_device = MockGraphicsDevice::new();

    let desc = TextureDesc {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mipmap: MipmapMode::None,
        data: None,
        texture_type: TextureType::Tex2D,
    };

    let _texture = graphics_device.create_texture(desc).unwrap();

    let created_textures = graphics_device.get_created_textures();
    assert_eq!(created_textures.len(), 1);
    assert_eq!(created_textures[0], "texture_256x256");
}

#[test]
fn test_mock_graphics_device_create_buffer() {
    let mut graphics_device = MockGraphicsDevice::new();

    let desc = BufferDesc {
        size: 1024,
        usage: BufferUsage::Vertex,
    };

    let _buffer = graphics_device.create_buffer(desc).unwrap();

    let created_buffers = graphics_device.get_created_buffers();
    assert_eq!(created_buffers.len(), 1);
    assert_eq!(created_buffers[0], "buffer_1024");
}

#[test]
fn test_mock_graphics_device_create_shader() {
    let mut graphics_device = MockGraphicsDevice::new();

    let desc = ShaderDesc {
        stage: ShaderStage::Vertex,
        code: &[1, 2, 3, 4],
        entry_point: "main".to_string(),
    };

    let _shader = graphics_device.create_shader(desc).unwrap();

    let created_shaders = graphics_device.get_created_shaders();
    assert_eq!(created_shaders.len(), 1);
    assert!(created_shaders[0].contains("Vertex"));
}

#[test]
fn test_mock_graphics_device_create_shader_fragment() {
    let mut graphics_device = MockGraphicsDevice::new();

    let desc = ShaderDesc {
        stage: ShaderStage::Fragment,
        code: &[1, 2, 3, 4],
        entry_point: "main".to_string(),
    };

    let _shader = graphics_device.create_shader(desc).unwrap();

    let created_shaders = graphics_device.get_created_shaders();
    assert_eq!(created_shaders.len(), 1);
    assert!(created_shaders[0].contains("Fragment"));
}

#[test]
fn test_mock_graphics_device_create_pipeline() {
    let mut graphics_device = MockGraphicsDevice::new();

    let vertex_shader = Arc::new(MockShader::new("vert".to_string()));
    let fragment_shader = Arc::new(MockShader::new("frag".to_string()));

    let vertex_layout = VertexLayout {
        bindings: vec![
            VertexBinding {
                binding: 0,
                stride: 8,
                input_rate: VertexInputRate::Vertex,
            }
        ],
        attributes: vec![
            VertexAttribute {
                location: 0,
                binding: 0,
                format: BufferFormat::R32G32_SFLOAT,
                offset: 0,
            }
        ],
    };

    let desc = PipelineDesc {
        vertex_shader,
        fragment_shader,
        vertex_layout,
        topology: PrimitiveTopology::TriangleList,
        push_constant_ranges: vec![],
        binding_group_layouts: vec![],
        rasterization: Default::default(),
        depth_stencil: Default::default(),
        color_blend: Default::default(),
        multisample: Default::default(),
    };

    let _pipeline = graphics_device.create_pipeline(desc).unwrap();

    let created_pipelines = graphics_device.get_created_pipelines();
    assert_eq!(created_pipelines.len(), 1);
    assert_eq!(created_pipelines[0], "pipeline");
}

#[test]
fn test_mock_graphics_device_create_command_list() {
    let graphics_device = MockGraphicsDevice::new();

    let _cmd_list = graphics_device.create_command_list().unwrap();
    // CommandList is a boxed trait, can't easily inspect its contents
}

#[test]
fn test_mock_graphics_device_create_render_pass() {
    let graphics_device = MockGraphicsDevice::new();

    let desc = RenderPassDesc {
        color_attachments: vec![],
        depth_stencil_attachment: None,
    };

    let _render_pass = graphics_device.create_render_pass(&desc).unwrap();
    // No methods to verify on RenderPass
}

// ============================================================================
// MockFramebuffer Tests
// ============================================================================

#[test]
fn test_mock_framebuffer_color_only() {
    let graphics_device = MockGraphicsDevice::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let color_rt: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(800, 600));

    let framebuffer = graphics_device.create_framebuffer(&FramebufferDesc {
        render_pass: &render_pass,
        color_attachments: vec![color_rt],
        depth_stencil_attachment: None,
        width: 800,
        height: 600,
    }).unwrap();

    assert_eq!(framebuffer.width(), 800);
    assert_eq!(framebuffer.height(), 600);
}

#[test]
fn test_mock_framebuffer_color_and_depth_stencil() {
    let graphics_device = MockGraphicsDevice::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let color_rt: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1920, 1080));
    let depth_rt: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1920, 1080));

    let framebuffer = graphics_device.create_framebuffer(&FramebufferDesc {
        render_pass: &render_pass,
        color_attachments: vec![color_rt],
        depth_stencil_attachment: Some(depth_rt),
        width: 1920,
        height: 1080,
    }).unwrap();

    assert_eq!(framebuffer.width(), 1920);
    assert_eq!(framebuffer.height(), 1080);
}

#[test]
fn test_mock_framebuffer_multiple_color_attachments() {
    let graphics_device = MockGraphicsDevice::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let color_rt0: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1024, 1024));
    let color_rt1: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1024, 1024));
    let color_rt2: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1024, 1024));
    let depth_rt: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(1024, 1024));

    let framebuffer = graphics_device.create_framebuffer(&FramebufferDesc {
        render_pass: &render_pass,
        color_attachments: vec![color_rt0, color_rt1, color_rt2],
        depth_stencil_attachment: Some(depth_rt),
        width: 1024,
        height: 1024,
    }).unwrap();

    assert_eq!(framebuffer.width(), 1024);
    assert_eq!(framebuffer.height(), 1024);
}

#[test]
fn test_mock_begin_render_pass_with_framebuffer() {
    let graphics_device = MockGraphicsDevice::new();
    let render_pass: Arc<dyn RenderPass> = Arc::new(MockRenderPass::new());
    let color_rt: Arc<dyn RenderTarget> = Arc::new(MockRenderTarget::new(800, 600));

    let framebuffer = graphics_device.create_framebuffer(&FramebufferDesc {
        render_pass: &render_pass,
        color_attachments: vec![color_rt],
        depth_stencil_attachment: None,
        width: 800,
        height: 600,
    }).unwrap();

    let mut cmd_list = MockCommandList::new();
    cmd_list.begin().unwrap();
    cmd_list.begin_render_pass(
        &render_pass,
        &framebuffer,
        &[ClearValue::Color([0.0, 0.0, 0.0, 1.0])],
    ).unwrap();
    cmd_list.end_render_pass().unwrap();
    cmd_list.end().unwrap();

    assert_eq!(cmd_list.commands.len(), 4);
    assert_eq!(cmd_list.commands[0], "begin");
    assert_eq!(cmd_list.commands[1], "begin_render_pass");
    assert_eq!(cmd_list.commands[2], "end_render_pass");
    assert_eq!(cmd_list.commands[3], "end");
}

// Note: test_mock_graphics_device_create_swapchain removed
// EventLoop must be created on main thread, incompatible with test framework

#[test]
fn test_mock_graphics_device_submit() {
    let graphics_device = MockGraphicsDevice::new();
    let cmd_list = MockCommandList::new();

    let commands: Vec<&dyn CommandList> = vec![&cmd_list];
    let result = graphics_device.submit(&commands);
    assert!(result.is_ok());
}

#[test]
fn test_mock_graphics_device_submit_with_swapchain() {
    let graphics_device = MockGraphicsDevice::new();
    let cmd_list = MockCommandList::new();
    let swapchain = MockSwapchain::new(3);

    let commands: Vec<&dyn CommandList> = vec![&cmd_list];
    let result = graphics_device.submit_with_swapchain(&commands, &swapchain, 0);
    assert!(result.is_ok());
}

#[test]
fn test_mock_graphics_device_wait_idle() {
    let graphics_device = MockGraphicsDevice::new();

    let result = graphics_device.wait_idle();
    assert!(result.is_ok());
}

#[test]
fn test_mock_graphics_device_stats() {
    let graphics_device = MockGraphicsDevice::new();

    let _stats = graphics_device.stats();
    // GraphicsDeviceStats::default() should return all zeros
}

#[test]
fn test_mock_graphics_device_resize() {
    let mut graphics_device = MockGraphicsDevice::new();

    graphics_device.resize(1920, 1080);
    // No state to verify, just ensure it doesn't panic
}

#[test]
fn test_mock_graphics_device_multiple_resources() {
    let mut graphics_device = MockGraphicsDevice::new();

    // Create multiple resources
    for i in 0..5 {
        let buffer_desc = BufferDesc {
            size: 1024 * (i + 1) as u64,
            usage: BufferUsage::Vertex,
        };
        graphics_device.create_buffer(buffer_desc).unwrap();

        let texture_desc = TextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
            texture_type: TextureType::Tex2D,
        };
        graphics_device.create_texture(texture_desc).unwrap();
    }

    assert_eq!(graphics_device.get_created_buffers().len(), 5);
    assert_eq!(graphics_device.get_created_textures().len(), 5);
}

#[test]
fn test_mock_graphics_device_tracking_persistence() {
    let mock = Arc::new(Mutex::new(MockGraphicsDevice::new()));
    let graphics_device: Arc<Mutex<dyn GraphicsDevice>> = mock.clone();

    // Create some resources through the trait interface
    {
        let mut r = graphics_device.lock().unwrap();
        let desc = BufferDesc {
            size: 2048,
            usage: BufferUsage::Index,
        };
        r.create_buffer(desc).unwrap();
    }

    // Verify tracking persists
    let created_buffers = mock.lock().unwrap().get_created_buffers();
    assert_eq!(created_buffers.len(), 1);
    assert_eq!(created_buffers[0], "buffer_2048");
}
