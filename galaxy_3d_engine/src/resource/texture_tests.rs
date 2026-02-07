/// Unit tests for texture.rs
///
/// Tests the Texture, TextureLayer, and AtlasRegion types without requiring GPU.
/// Uses MockRenderer for testing.

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use crate::renderer::{
    mock_renderer::MockRenderer,
    TextureDesc as RenderTextureDesc, TextureFormat, TextureUsage, MipmapMode,
};
#[cfg(test)]
use crate::resource::{
    Texture, TextureDesc, LayerDesc, AtlasRegionDesc, AtlasRegion,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a mock renderer for testing
fn create_mock_renderer() -> Arc<Mutex<dyn crate::renderer::Renderer>> {
    let renderer = MockRenderer::new();
    Arc::new(Mutex::new(renderer))
}

/// Create a simple texture descriptor (simple texture, 256x256)
fn create_simple_texture_desc(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> TextureDesc {
    TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            }
        ],
    }
}

/// Create an indexed texture descriptor (4 layers, 256x256)
fn create_indexed_texture_desc(renderer: Arc<Mutex<dyn crate::renderer::Renderer>>) -> TextureDesc {
    TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 4,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "layer0".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "layer1".to_string(),
                layer_index: 1,
                data: None,
                regions: vec![],
            },
        ],
    }
}

// ============================================================================
// SIMPLE TEXTURE TESTS
// ============================================================================

#[test]
fn test_create_simple_texture() {
    let renderer = create_mock_renderer();
    let desc = create_simple_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    assert!(texture.is_simple());
    assert!(!texture.is_indexed());
    assert_eq!(texture.layer_count(), 1);
}

#[test]
fn test_simple_texture_requires_one_layer() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1, // Simple texture
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![], // ERROR: must have exactly 1 layer
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_simple_texture_layer_must_be_index_0() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 1, // ERROR: must be 0
                data: None,
                regions: vec![],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_simple_texture_with_atlas_region() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "sprite1".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 64,
                            height: 64,
                        },
                    }
                ],
            }
        ],
    };

    let texture = Texture::from_desc(desc).unwrap();

    assert!(texture.is_simple());

    let layer = texture.layer(0).unwrap();
    assert!(layer.is_atlas());
    assert_eq!(layer.region_count(), 1);

    let region = layer.region_by_name("sprite1").unwrap();
    assert_eq!(region.x, 0);
    assert_eq!(region.y, 0);
    assert_eq!(region.width, 64);
    assert_eq!(region.height, 64);
}

// ============================================================================
// INDEXED TEXTURE TESTS
// ============================================================================

#[test]
fn test_create_indexed_texture() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    assert!(!texture.is_simple());
    assert!(texture.is_indexed());
    assert_eq!(texture.layer_count(), 2); // Only 2 layers initially populated
}

#[test]
fn test_indexed_texture_can_be_empty() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 4, // Indexed
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![], // OK: indexed can start empty
    };

    let texture = Texture::from_desc(desc).unwrap();

    assert!(texture.is_indexed());
    assert_eq!(texture.layer_count(), 0);
}

#[test]
fn test_indexed_texture_layer_bounds() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 2, // Only 2 layers
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "layer0".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "layer_invalid".to_string(),
                layer_index: 2, // ERROR: index >= array_layers
                data: None,
                regions: vec![],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// LAYER ACCESS TESTS
// ============================================================================

#[test]
fn test_layer_access_by_index() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    let layer0 = texture.layer(0);
    assert!(layer0.is_some());

    let layer1 = texture.layer(1);
    assert!(layer1.is_some());

    let layer_invalid = texture.layer(10);
    assert!(layer_invalid.is_none());
}

#[test]
fn test_layer_access_by_name() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    let layer = texture.layer_by_name("layer0");
    assert!(layer.is_some());
    assert_eq!(layer.unwrap().name(), "layer0");

    let layer = texture.layer_by_name("nonexistent");
    assert!(layer.is_none());
}

#[test]
fn test_layer_index_by_name() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    let index = texture.layer_index_by_name("layer1");
    assert_eq!(index, Some(1));

    let index = texture.layer_index_by_name("nonexistent");
    assert!(index.is_none());
}

// ============================================================================
// LAYER MODIFICATION TESTS
// ============================================================================

#[test]
fn test_add_layer_to_indexed_texture() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let mut texture = Texture::from_desc(desc).unwrap();

    assert_eq!(texture.layer_count(), 2);

    let new_layer = LayerDesc {
        name: "layer2".to_string(),
        layer_index: 2,
        data: None,
        regions: vec![],
    };

    let result = texture.add_layer(new_layer);
    assert!(result.is_ok());
    assert_eq!(texture.layer_count(), 3);
}

#[test]
fn test_add_layer_to_simple_texture_fails() {
    let renderer = create_mock_renderer();
    let desc = create_simple_texture_desc(renderer);

    let mut texture = Texture::from_desc(desc).unwrap();

    let new_layer = LayerDesc {
        name: "layer1".to_string(),
        layer_index: 1,
        data: None,
        regions: vec![],
    };

    let result = texture.add_layer(new_layer);
    assert!(result.is_err()); // Cannot add layer to simple texture
}

#[test]
fn test_add_duplicate_layer_name() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let mut texture = Texture::from_desc(desc).unwrap();

    let new_layer = LayerDesc {
        name: "layer0".to_string(), // Duplicate name
        layer_index: 2,
        data: None,
        regions: vec![],
    };

    let result = texture.add_layer(new_layer);
    assert!(result.is_err());
}

#[test]
fn test_add_duplicate_layer_index() {
    let renderer = create_mock_renderer();
    let desc = create_indexed_texture_desc(renderer);

    let mut texture = Texture::from_desc(desc).unwrap();

    let new_layer = LayerDesc {
        name: "new_layer".to_string(),
        layer_index: 0, // Duplicate index
        data: None,
        regions: vec![],
    };

    let result = texture.add_layer(new_layer);
    assert!(result.is_err());
}

// ============================================================================
// ATLAS REGION TESTS
// ============================================================================

#[test]
fn test_add_region_to_layer() {
    let renderer = create_mock_renderer();
    let desc = create_simple_texture_desc(renderer);

    let mut texture = Texture::from_desc(desc).unwrap();

    let region_desc = AtlasRegionDesc {
        name: "sprite1".to_string(),
        region: AtlasRegion {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        },
    };

    let result = texture.add_region("main", region_desc);
    assert!(result.is_ok());

    let layer = texture.layer(0).unwrap();
    assert_eq!(layer.region_count(), 1);
}

#[test]
fn test_region_bounds_validation() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "invalid".to_string(),
                        region: AtlasRegion {
                            x: 200,
                            y: 200,
                            width: 100, // Exceeds width (200 + 100 > 256)
                            height: 50,
                        },
                    }
                ],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_region_zero_dimension() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "invalid".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 0, // Zero width
                            height: 64,
                        },
                    }
                ],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_region_lookup() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "sprite1".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 64,
                            height: 64,
                        },
                    },
                    AtlasRegionDesc {
                        name: "sprite2".to_string(),
                        region: AtlasRegion {
                            x: 64,
                            y: 0,
                            width: 64,
                            height: 64,
                        },
                    }
                ],
            }
        ],
    };

    let texture = Texture::from_desc(desc).unwrap();

    // Access via texture.region() convenience method
    let region = texture.region("main", "sprite1");
    assert!(region.is_some());

    let region = texture.region("main", "sprite2");
    assert!(region.is_some());

    let region = texture.region("main", "nonexistent");
    assert!(region.is_none());
}

#[test]
fn test_duplicate_region_names() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "sprite".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 64,
                            height: 64,
                        },
                    },
                    AtlasRegionDesc {
                        name: "sprite".to_string(), // Duplicate name
                        region: AtlasRegion {
                            x: 64,
                            y: 0,
                            width: 64,
                            height: 64,
                        },
                    }
                ],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn test_duplicate_layer_names() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 4,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "layer".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "layer".to_string(), // Duplicate name
                layer_index: 1,
                data: None,
                regions: vec![],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

#[test]
fn test_duplicate_layer_indices() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 4,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "layer0".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![],
            },
            LayerDesc {
                name: "layer1".to_string(),
                layer_index: 0, // Duplicate index
                data: None,
                regions: vec![],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());
}

// ============================================================================
// COMPLEX TEXTURE TESTS
// ============================================================================

#[test]
fn test_complex_indexed_texture_with_atlas() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 512,
            height: 512,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 3,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "characters".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "hero".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 128,
                            height: 128,
                        },
                    },
                    AtlasRegionDesc {
                        name: "enemy".to_string(),
                        region: AtlasRegion {
                            x: 128,
                            y: 0,
                            width: 128,
                            height: 128,
                        },
                    }
                ],
            },
            LayerDesc {
                name: "environment".to_string(),
                layer_index: 1,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "tree".to_string(),
                        region: AtlasRegion {
                            x: 0,
                            y: 0,
                            width: 256,
                            height: 256,
                        },
                    }
                ],
            },
        ],
    };

    let texture = Texture::from_desc(desc).unwrap();

    assert!(texture.is_indexed());
    assert_eq!(texture.layer_count(), 2);

    // Verify characters layer
    let characters = texture.layer_by_name("characters").unwrap();
    assert!(characters.is_atlas());
    assert_eq!(characters.region_count(), 2);
    assert!(characters.region_by_name("hero").is_some());
    assert!(characters.region_by_name("enemy").is_some());

    // Verify environment layer
    let environment = texture.layer_by_name("environment").unwrap();
    assert!(environment.is_atlas());
    assert_eq!(environment.region_count(), 1);
    assert!(environment.region_by_name("tree").is_some());
}

// ============================================================================
// GETTER TESTS
// ============================================================================

#[test]
fn test_renderer_texture_getter() {
    let renderer = create_mock_renderer();
    let desc = create_simple_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    // renderer_texture() should return a valid Arc
    let renderer_tex = texture.renderer_texture();
    assert!(renderer_tex.info().width == 256);
    assert!(renderer_tex.info().height == 256);
}

#[test]
fn test_descriptor_set_getter() {
    let renderer = create_mock_renderer();
    let desc = create_simple_texture_desc(renderer);

    let texture = Texture::from_desc(desc).unwrap();

    // descriptor_set() should return a valid Arc
    let descriptor_set = texture.descriptor_set();
    assert!(Arc::strong_count(descriptor_set) >= 1);
}

#[test]
fn test_texture_layer_getters() {
    let renderer = create_mock_renderer();
    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "test_layer".to_string(),
                layer_index: 0,
                data: None,
                regions: vec![
                    AtlasRegionDesc {
                        name: "region1".to_string(),
                        region: AtlasRegion { x: 0, y: 0, width: 64, height: 64 },
                    },
                    AtlasRegionDesc {
                        name: "region2".to_string(),
                        region: AtlasRegion { x: 64, y: 0, width: 64, height: 64 },
                    }
                ],
            }
        ],
    };

    let texture = Texture::from_desc(desc).unwrap();
    let layer = texture.layer(0).unwrap();

    // Test layer_index()
    assert_eq!(layer.layer_index(), 0);

    // Test name()
    assert_eq!(layer.name(), "test_layer");

    // Test region_index_by_name()
    assert_eq!(layer.region_index_by_name("region1"), Some(0));
    assert_eq!(layer.region_index_by_name("region2"), Some(1));
    assert_eq!(layer.region_index_by_name("nonexistent"), None);

    // Test region() by index
    assert!(layer.region(0).is_some());
    assert!(layer.region(1).is_some());
    assert!(layer.region(99).is_none());
}

// ============================================================================
// LAYER DATA VALIDATION TESTS
// ============================================================================

#[test]
fn test_layer_data_size_validation_rgba8() {
    let renderer = create_mock_renderer();

    // For 256x256 RGBA8 texture: expected size = 256 * 256 * 4 = 262144 bytes
    let correct_size = 256 * 256 * 4;
    let wrong_size = 1000;

    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: Some(vec![0u8; wrong_size]), // WRONG SIZE!
                regions: vec![],
            }
        ],
    };

    let result = Texture::from_desc(desc);
    assert!(result.is_err());

    // Now test with correct size
    let renderer2 = create_mock_renderer();
    let desc_correct = TextureDesc {
        renderer: renderer2,
        texture: RenderTextureDesc {
            width: 256,
            height: 256,
            format: TextureFormat::R8G8B8A8_UNORM,
            usage: TextureUsage::Sampled,
            array_layers: 1,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "main".to_string(),
                layer_index: 0,
                data: Some(vec![0u8; correct_size]),
                regions: vec![],
            }
        ],
    };

    let result_correct = Texture::from_desc(desc_correct);
    assert!(result_correct.is_ok());
}

#[test]
fn test_layer_data_upload_multiple_layers() {
    let renderer = create_mock_renderer();
    let layer_size = 128 * 128 * 4; // 128x128 RGBA8

    let desc = TextureDesc {
        renderer,
        texture: RenderTextureDesc {
            width: 128,
            height: 128,
            format: TextureFormat::B8G8R8A8_SRGB,
            usage: TextureUsage::Sampled,
            array_layers: 3,
            mipmap: MipmapMode::None,
            data: None,
        },
        layers: vec![
            LayerDesc {
                name: "layer0".to_string(),
                layer_index: 0,
                data: Some(vec![255u8; layer_size]),
                regions: vec![],
            },
            LayerDesc {
                name: "layer1".to_string(),
                layer_index: 1,
                data: Some(vec![128u8; layer_size]),
                regions: vec![],
            },
            LayerDesc {
                name: "layer2".to_string(),
                layer_index: 2,
                data: None, // This layer has no initial data
                regions: vec![],
            }
        ],
    };

    let texture = Texture::from_desc(desc).unwrap();
    assert_eq!(texture.layer_count(), 3);
}
