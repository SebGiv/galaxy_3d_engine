//! Unit tests for VulkanGraphicsDevice backend
//!
//! These tests verify that VulkanGraphicsDevice correctly implements the GraphicsDevice trait.
//! All tests require a GPU and are marked with #[ignore].
//!
//! Run with: cargo test --test vulkan_renderer_tests -- --ignored

use galaxy_3d_engine::galaxy3d::GraphicsDevice;
use galaxy_3d_engine::galaxy3d::render::{
    TextureDesc, TextureFormat, TextureUsage, TextureType, MipmapMode, TextureData,
    BufferDesc, BufferUsage, ShaderDesc, ShaderStage,
    Config,
};
use galaxy_3d_engine_renderer_vulkan::galaxy3d::VulkanGraphicsDevice;
use winit::event_loop::EventLoop;
use winit::window::Window;

/// Helper to create a test window for Vulkan
#[allow(deprecated)]
fn create_test_window() -> (Window, EventLoop<()>) {
    let event_loop = EventLoop::new().unwrap();
    let window_attrs = Window::default_attributes()
        .with_title("Vulkan GraphicsDevice Test")
        .with_inner_size(winit::dpi::LogicalSize::new(800, 600))
        .with_visible(false); // Hidden window for tests
    let window = event_loop.create_window(window_attrs).unwrap();
    (window, event_loop)
}

// ============================================================================
// TEXTURE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_simple_texture() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = TextureDesc {
        width: 256,
        height: 256,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mipmap: MipmapMode::None,
        texture_type: TextureType::Tex2D,
        data: None,
    };

    let texture = graphics_device.create_texture(desc).unwrap();
    let info = texture.info();

    assert_eq!(info.width, 256);
    assert_eq!(info.height, 256);
    assert_eq!(info.format, TextureFormat::R8G8B8A8_UNORM);
    assert_eq!(info.array_layers, 1);
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_texture_with_data() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    // Create 4x4 RGBA texture (64 bytes total)
    let data: Vec<u8> = (0..64).collect();

    let desc = TextureDesc {
        width: 4,
        height: 4,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 1,
        mipmap: MipmapMode::None,
        texture_type: TextureType::Tex2D,
        data: Some(TextureData::Single(data)),
    };

    let texture = graphics_device.create_texture(desc).unwrap();
    let info = texture.info();

    assert_eq!(info.width, 4);
    assert_eq!(info.height, 4);
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_texture_array() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = TextureDesc {
        width: 128,
        height: 128,
        format: TextureFormat::R8G8B8A8_UNORM,
        usage: TextureUsage::Sampled,
        array_layers: 4, // 4 layers
        mipmap: MipmapMode::None,
        texture_type: TextureType::Array2D,
        data: None,
    };

    let texture = graphics_device.create_texture(desc).unwrap();
    let info = texture.info();

    assert_eq!(info.array_layers, 4);
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_depth_texture() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = TextureDesc {
        width: 512,
        height: 512,
        format: TextureFormat::D32_FLOAT,
        usage: TextureUsage::DepthStencil,
        array_layers: 1,
        mipmap: MipmapMode::None,
        texture_type: TextureType::Tex2D,
        data: None,
    };

    let texture = graphics_device.create_texture(desc).unwrap();
    let info = texture.info();

    assert_eq!(info.format, TextureFormat::D32_FLOAT);
}

// ============================================================================
// BUFFER TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_vertex_buffer() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = BufferDesc {
        size: 1024,
        usage: BufferUsage::Vertex,
    };

    let buffer = graphics_device.create_buffer(desc).unwrap();

    // Try to update buffer
    let data: Vec<u8> = vec![0u8; 256];
    buffer.update(0, &data).unwrap();
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_index_buffer() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = BufferDesc {
        size: 512,
        usage: BufferUsage::Index,
    };

    let buffer = graphics_device.create_buffer(desc).unwrap();

    // Create index data
    let indices: Vec<u16> = vec![0, 1, 2, 2, 3, 0];
    let data: Vec<u8> = indices.iter()
        .flat_map(|&i| i.to_le_bytes())
        .collect();

    buffer.update(0, &data).unwrap();
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_uniform_buffer() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let desc = BufferDesc {
        size: 256,
        usage: BufferUsage::Uniform,
    };

    let buffer = graphics_device.create_buffer(desc).unwrap();

    // Update with uniform data (e.g., MVP matrix)
    let data: Vec<u8> = vec![0u8; 64];
    buffer.update(0, &data).unwrap();
}

// ============================================================================
// SHADER TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_vertex_shader() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    // Minimal valid SPIR-V shader (just the header)
    // This is a dummy shader for testing - in real usage, you'd load compiled shaders
    let spirv_code = create_dummy_spirv_vertex_shader();

    let desc = ShaderDesc {
        stage: ShaderStage::Vertex,
        entry_point: "main".to_string(),
        code: &spirv_code,
    };

    // Note: This will likely fail with invalid SPIR-V, but tests the API
    let _result = graphics_device.create_shader(desc);

    // We don't assert success because we're using dummy SPIR-V
    // In a real test, you'd use valid compiled shaders
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_fragment_shader() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let spirv_code = create_dummy_spirv_fragment_shader();

    let desc = ShaderDesc {
        stage: ShaderStage::Fragment,
        entry_point: "main".to_string(),
        code: &spirv_code,
    };

    let _result = graphics_device.create_shader(desc);
}

// ============================================================================
// COMMAND LIST TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
fn test_vulkan_create_command_list() {
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let mut cmd_list = graphics_device.create_command_list().unwrap();

    // Test basic command list operations
    cmd_list.begin().unwrap();
    cmd_list.end().unwrap();
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_multiple_command_lists() {
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    // Create multiple command lists
    let mut cmd1 = graphics_device.create_command_list().unwrap();
    let mut cmd2 = graphics_device.create_command_list().unwrap();
    let mut cmd3 = graphics_device.create_command_list().unwrap();

    cmd1.begin().unwrap();
    cmd1.end().unwrap();

    cmd2.begin().unwrap();
    cmd2.end().unwrap();

    cmd3.begin().unwrap();
    cmd3.end().unwrap();
}

// ============================================================================
// RENDERER LIFECYCLE TESTS
// ============================================================================

#[test]
#[ignore] // Requires GPU
fn test_vulkan_wait_idle() {
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    graphics_device.wait_idle().unwrap();
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_get_stats() {
    let (window, _event_loop) = create_test_window();
    let graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    let stats = graphics_device.stats();

    // Stats should be initialized
    assert_eq!(stats.draw_calls, 0);
    assert_eq!(stats.triangles, 0);
}

#[test]
#[ignore] // Requires GPU
fn test_vulkan_resize() {
    let (window, _event_loop) = create_test_window();
    let mut graphics_device = VulkanGraphicsDevice::new(&window, Config::default()).unwrap();

    // Test resize operation
    graphics_device.resize(1024, 768);
    graphics_device.resize(1920, 1080);
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a dummy SPIR-V vertex shader for testing
/// Note: This is NOT valid SPIR-V, just for API testing
fn create_dummy_spirv_vertex_shader() -> Vec<u8> {
    // SPIR-V magic number (0x07230203) + minimal header
    vec![
        0x03, 0x02, 0x23, 0x07, // Magic number
        0x00, 0x00, 0x01, 0x00, // Version 1.0
        0x00, 0x00, 0x00, 0x00, // Generator
        0x01, 0x00, 0x00, 0x00, // Bound
        0x00, 0x00, 0x00, 0x00, // Schema
    ]
}

/// Create a dummy SPIR-V fragment shader for testing
fn create_dummy_spirv_fragment_shader() -> Vec<u8> {
    vec![
        0x03, 0x02, 0x23, 0x07,
        0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ]
}
