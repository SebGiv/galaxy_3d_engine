# Testing Guide - Galaxy 3D Engine

## 📊 Test Overview

| Category | Count | GPU Required | Location |
|----------|-------|--------------|----------|
| **Unit Tests CORE** | 70 | ❌ No | `galaxy_3d_engine/src/**/*_tests.rs` |
| **Unit Tests VULKAN** | 14 | ✅ Yes | `galaxy_3d_engine_renderer_vulkan/tests/` |
| **Integration Tests** | 3 | ✅ Yes | `galaxy_3d_engine/tests/` |
| **TOTAL** | **87** | - | - |

### Code Coverage: **49.87%** (regions), **47.29%** (lines)

## 🚀 Running Tests

### Unit Tests (No GPU)

```bash
# Run all unit tests (fast, no GPU)
cargo test --lib

# Run tests for a specific module
cargo test --lib resource::mesh
cargo test --lib resource::texture
cargo test --lib resource::resource_manager

# Run with code coverage
cargo llvm-cov --lib --html
# Open: target/llvm-cov/html/index.html
```

### Unit Tests (GPU Required - Vulkan Backend)

```bash
cd galaxy_3d_engine_renderer_vulkan

# Run Vulkan renderer tests (requires GPU)
cargo test --test vulkan_renderer_tests -- --ignored

# Run specific test
cargo test --test vulkan_renderer_tests test_vulkan_create_simple_texture -- --ignored
```

### Integration Tests (GPU Required)

```bash
cd galaxy_3d_engine

# Run all integration tests (requires GPU)
cargo test --test resource_integration_tests -- --ignored

# Run specific test
cargo test --test resource_integration_tests test_integration_create_texture_with_vulkan -- --ignored
```

### All Tests (Including GPU Tests)

```bash
# From workspace root
cargo test --workspace --all-targets -- --ignored
```

## 📁 Test Organization

### 1. Unit Tests CORE (`src/**/*_tests.rs`)

**Pattern:** Separate test files alongside source files using `#[cfg(test)]` and `#[path = "..."]`

```
src/
├── resource/
│   ├── mesh.rs
│   ├── mesh_tests.rs           # 25 tests - 84.46% coverage ✅
│   ├── texture.rs
│   ├── texture_tests.rs        # 30 tests - 76.09% coverage ✅
│   ├── resource_manager.rs
│   └── resource_manager_tests.rs  # 30 tests - 54.23% coverage ⚠️
└── renderer/
    └── mock_renderer.rs        # Mock for GPU-less testing
```

**Features:**
- ✅ Uses `MockRenderer` (no GPU required)
- ✅ Tests all core resource types (Texture, Mesh, Pipeline)
- ✅ Fast execution (< 1 second)
- ✅ Runs in CI/CD without GPU

**Example:**
```rust
#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
```

### 2. Unit Tests VULKAN (`galaxy_3d_engine_renderer_vulkan/tests/`)

**Pattern:** Integration tests directory with `#[ignore]` attribute

```
galaxy_3d_engine_renderer_vulkan/
└── tests/
    └── vulkan_renderer_tests.rs  # 14 tests
```

**Tests:**
- ✅ Texture creation (simple, with data, array, depth)
- ✅ Buffer creation (vertex, index, uniform)
- ✅ Shader creation (vertex, fragment)
- ✅ Command lists
- ✅ Renderer lifecycle (wait_idle, stats, resize)

**All tests marked with:**
```rust
#[test]
#[ignore] // Requires GPU
```

### 3. Integration Tests (`galaxy_3d_engine/tests/`)

**Pattern:** Integration tests using real VulkanRenderer

```
galaxy_3d_engine/
└── tests/
    └── resource_integration_tests.rs  # 3 tests
```

**Tests:**
- ✅ Engine + ResourceManager + VulkanRenderer
- ✅ Real GPU resource creation
- ✅ Marked with `#[ignore]` and `#[serial]` (singleton Engine)

## 📈 Coverage Analysis

### Excellent Coverage (>75%)
- ✅ **resource/mesh.rs**: 84.46% - Comprehensive mesh hierarchy testing
- ✅ **resource/texture.rs**: 76.09% - Thorough texture and atlas testing

### Good Coverage (50-75%)
- ⚠️ **resource/resource_manager.rs**: 54.23% - Core functionality covered, advanced features need work
- ⚠️ **log.rs**: 61.29% - Logging system reasonably covered

### Needs Improvement (<50%)
- ⚠️ **resource/pipeline.rs**: 41.11% - Pipeline variants and validation need more tests
- ⚠️ **renderer/mock_renderer.rs**: 32.44% - Not all mock methods exercised
- ❌ **engine.rs**: 5.92% - Singleton pattern, difficult to test extensively

### Not Applicable (0%)
- ℹ️ **error.rs, renderer/\*.rs**: Definition files (traits, enums, structs)

## 🎯 Testing Best Practices

### 1. Test Naming Convention

```rust
// ✅ Good
#[test]
fn test_create_simple_texture() { ... }

#[test]
fn test_mesh_validation_vertex_overflow() { ... }

// ❌ Bad
#[test]
fn test1() { ... }

#[test]
fn it_works() { ... }
```

### 2. Test Structure (AAA Pattern)

```rust
#[test]
fn test_example() {
    // Arrange - Setup
    let renderer = MockRenderer::new();
    let desc = create_test_descriptor();

    // Act - Execute
    let result = create_resource(desc);

    // Assert - Verify
    assert!(result.is_ok());
    assert_eq!(result.unwrap().id(), 42);
}
```

### 3. Helper Functions

Prefer helper functions for common setup:

```rust
fn create_simple_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![...],
        attributes: vec![...],
    }
}

fn create_quad_vertex_data() -> Vec<u8> {
    let vertices: Vec<f32> = vec![...];
    vertices.iter().flat_map(|&f| f.to_le_bytes()).collect()
}
```

### 4. GPU Test Attributes

Always use both `#[ignore]` and document GPU requirement:

```rust
#[test]
#[ignore] // Requires GPU
fn test_vulkan_feature() {
    // Test implementation
}
```

For Engine singleton tests, also use `#[serial]`:

```rust
use serial_test::serial;

#[test]
#[ignore] // Requires GPU
#[serial]  // Engine is a singleton
fn test_engine_feature() {
    Engine::initialize().unwrap();
    // Test implementation
    Engine::shutdown();
}
```

## 🔧 Improving Coverage

### Priority Areas for Additional Tests

1. **resource/resource_manager.rs** (54.23% → target 70%)
   - [ ] Test resource removal edge cases
   - [ ] Test concurrent access patterns
   - [ ] Test error recovery scenarios

2. **resource/pipeline.rs** (41.11% → target 60%)
   - [ ] Test all pipeline variant combinations
   - [ ] Test pipeline validation errors
   - [ ] Test pipeline selection by name/index

3. **engine.rs** (5.92% → target 30%)
   - [ ] Test Engine initialization failures
   - [ ] Test multiple renderer management
   - [ ] Test cleanup on shutdown

### How to Add Tests

1. **Identify untested code**: Open `target/llvm-cov/html/index.html`
2. **Find red lines**: Lines highlighted in red are not covered
3. **Write targeted tests**: Create tests that exercise those specific paths
4. **Re-run coverage**: Verify improvement with `cargo llvm-cov --lib --html`

## 📚 Dependencies

### Test-Only Dependencies (`[dev-dependencies]`)

```toml
[dev-dependencies]
galaxy_3d_engine_renderer_vulkan = { path = "../galaxy_3d_engine_renderer_vulkan" }
serial_test = "3"  # For sequential tests (Engine singleton)
```

**Important:** These dependencies are **only** used for testing, not included in production builds.

## 🔍 Troubleshooting

### Tests Hanging

If GPU tests hang, check:
- Vulkan drivers are installed
- GPU is not in use by another application
- Try running tests sequentially: `cargo test -- --test-threads=1`

### MockRenderer Issues

If unit tests fail with MockRenderer errors:
- Ensure you're not accidentally using real GPU calls
- Check that all mock methods return appropriate test values
- Verify MockRenderer tracks resources correctly

### Coverage Report Empty

If coverage report shows 0%:
- Ensure you're running `cargo llvm-cov --lib` (not just `cargo test`)
- Check that `cargo-llvm-cov` is installed: `cargo install cargo-llvm-cov`
- Try cleaning first: `cargo clean && cargo llvm-cov --lib`

## 📝 Adding New Tests

### Step 1: Create Test File

For module `src/foo/bar.rs`, create `src/foo/bar_tests.rs`:

```rust
//! Unit tests for Bar module

#[cfg(test)]
use crate::foo::bar::*;

#[test]
fn test_bar_basic() {
    // Test implementation
}
```

### Step 2: Link Test File

In `src/foo/bar.rs`, add at the end:

```rust
#[cfg(test)]
#[path = "bar_tests.rs"]
mod tests;
```

### Step 3: Run and Verify

```bash
cargo test --lib foo::bar
cargo llvm-cov --lib --html  # Check coverage improved
```

## 🎯 Test Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| **Total Tests** | 87 | 100+ | 🟢 Good |
| **Coverage** | 49.87% | 60%+ | 🟡 Needs improvement |
| **GPU Tests** | 17 | 20+ | 🟢 Good |
| **Core Coverage** | 84.46% (mesh) | 80%+ | 🟢 Excellent |

## 🚦 CI/CD Integration

### GitHub Actions Example

```yaml
name: Tests

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
      - name: Run unit tests (no GPU)
        run: cargo test --lib
      - name: Generate coverage
        run: |
          cargo install cargo-llvm-cov
          cargo llvm-cov --lib --lcov --output-path lcov.info
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info

  # GPU tests would run on self-hosted runners with GPU
```

## 📖 Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [cargo-llvm-cov Documentation](https://github.com/taiki-e/cargo-llvm-cov)
- [Testing Best Practices](https://matklad.github.io/2021/05/31/how-to-test.html)

---

**Last Updated:** 2026-02-07
**Test Count:** 87 tests (70 unit + 14 Vulkan + 3 integration)
**Coverage:** 49.87% regions, 47.29% lines
