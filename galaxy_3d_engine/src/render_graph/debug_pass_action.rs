/// Debug pass action — draws selected RenderInstances with debug
/// visualizations on top of the scene.
///
/// Two independent lists are processed in sequence each frame:
///   - `wireframe_entries` : draw the instance's mesh in line polygon
///     mode, reusing each `RenderInstance`'s `default_vertex_shader`
///     paired with the shared debug fragment shader.
///   - `bb_entries` : draw the instance's world-space bounding box as a
///     unit-cube outline, using a dedicated bounding-box vertex shader
///     paired with the same shared debug fragment shader.
///
/// The fragment shader is shared between both modes — it only reads the
/// per-entry color from a push-constant and ignores every varying.
///
/// No depth read/write, no material lookup, alpha blending always on so
/// the alpha channel of the entry color controls transparency.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use glam::{Mat4, Vec3};
use rustc_hash::FxHashMap;

use crate::engine::Engine;
use crate::engine_err;
use crate::error::Result;
use crate::graphics_device::{
    self, BindingGroup, BindingGroupLayoutDesc, BindingResource, BindingSlotDesc, BindingType,
    BlendFactor, BlendOp, Buffer, BufferDesc, BufferFormat, BufferUpdateMode, BufferUsage,
    ColorBlendState, ColorWriteMask, CommandList, CullMode, DynamicRenderState, IndexType,
    MultisampleState, PolygonMode, PrimitiveTopology, RasterizationState, ShaderStageFlags,
    VertexAttribute, VertexBinding, VertexInputRate, VertexLayout,
};
use crate::resource::pipeline::PipelineDesc;
use crate::resource::resource_manager::{PassInfo, PipelineKey, ShaderKey};
use crate::scene::{RenderInstanceKey, Scene};

use super::pass_action::{PassAction, SceneBinding};

// ============================================================
// Debug display modes
// ============================================================

/// Debug visualization style. Used internally as a tag in the pipeline
/// cache key so that wireframe and bounding-box pipelines never collide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugDisplayMode {
    /// Render instance triangles as line segments (`PolygonMode::Line`).
    Wireframe,
    /// Render the instance's world-space AABB as a unit-cube outline
    /// (`PrimitiveTopology::LineList`).
    BoundingBox,
}

// ============================================================
// Debug draw entry
// ============================================================

/// One instance to be drawn by the debug pass.
///
/// Used identically by the wireframe and bounding-box lists.
/// `line_width` is set as dynamic state per draw — values other than 1.0
/// require the Vulkan `wideLines` feature (already enabled by the engine).
/// `color.a` controls transparency through the alpha-blend pipeline.
#[derive(Debug, Clone, Copy)]
pub struct DebugDrawEntry {
    /// Which RenderInstance to draw.
    pub instance_key: RenderInstanceKey,
    /// RGBA color in linear space; alpha is the per-line transparency.
    pub color: [f32; 4],
    /// Line width in pixels (clamped by hardware to its supported range).
    pub line_width: f32,
}

// ============================================================
// Pipeline cache key
// ============================================================

/// Cache key for the per-pass pipeline lookup table.
///
/// Line width is dynamic, so it is *not* part of the key. Color is a
/// fragment-shader push constant, so it is also not part of the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DebugPipelineCacheKey {
    vs: ShaderKey,
    fs: ShaderKey,
    mode: DebugDisplayMode,
}

// ============================================================
// Push-constant layout
// ============================================================

// Wireframe mode — single 32-byte block shared by both stages:
//   offset  0..4   draw_slot   (u32)   — read by VERTEX
//   offset  4..16  pad         (12 B)  — vec4 alignment
//   offset 16..32  color       (vec4)  — read by FRAGMENT
// Pushed in one call (VERTEX | FRAGMENT) — Slang fuses both shader
// declarations into a single VkPushConstantRange.
const DEBUG_WF_PC_SIZE: usize = 32;

// Bounding-box mode — two non-overlapping byte regions inside the
// push-constant block:
//   color     (vec4) at offset 16, size 16 — read by FRAGMENT
//   bb_matrix (mat4) at offset 32, size 64 — read by VERTEX
// The engine merges shader-side push-constant blocks by Slang variable
// name (`pc`), so the pipeline layout exposes a single combined range
// `(VERTEX|FRAGMENT, 0, 96)`. To satisfy VUID-vkCmdPushConstants-01796
// (each push must cover all stages of every overlapping range), both
// regions are pushed with `VERTEX_FRAGMENT` even though only one stage
// actually reads each region.
const DEBUG_BB_FS_PC_OFFSET: u32 = 16;
const DEBUG_BB_VS_PC_OFFSET: u32 = 32;

// ============================================================
// Cube geometry (bounding-box mesh)
// ============================================================

// Unit cube spanning [-0.5, +0.5]^3 — eight corners indexed by
// (x_sign, y_sign, z_sign) bits. Vertex layout is position-only, so the
// VS that consumes this buffer must declare a single
// `[[vk::location(0)]] float3 position` input.
//
// Indices form a `LineList` of the 12 cube edges (24 indices).
const CUBE_VERTEX_COUNT: usize = 8;
const CUBE_INDEX_COUNT: u32 = 24;
const CUBE_VERTEX_STRIDE: u32 = 12; // float3
const CUBE_VERTEX_BUFFER_SIZE: u64 = (CUBE_VERTEX_COUNT * 12) as u64;
const CUBE_INDEX_BUFFER_SIZE: u64 = (CUBE_INDEX_COUNT as usize * 2) as u64;

#[rustfmt::skip]
const CUBE_VERTICES: [[f32; 3]; CUBE_VERTEX_COUNT] = [
    [-0.5, -0.5, -0.5], // 0
    [ 0.5, -0.5, -0.5], // 1
    [-0.5, -0.5,  0.5], // 2
    [ 0.5, -0.5,  0.5], // 3
    [-0.5,  0.5, -0.5], // 4
    [ 0.5,  0.5, -0.5], // 5
    [-0.5,  0.5,  0.5], // 6
    [ 0.5,  0.5,  0.5], // 7
];

#[rustfmt::skip]
const CUBE_INDICES: [u16; CUBE_INDEX_COUNT as usize] = [
    // bottom face (y = -0.5)
    0, 1,  1, 3,  3, 2,  2, 0,
    // top face (y = +0.5)
    4, 5,  5, 7,  7, 6,  6, 4,
    // vertical edges
    0, 4,  1, 5,  2, 6,  3, 7,
];

/// Vertex layout for the bounding-box cube — single `position: vec3`
/// attribute at location 0.
fn cube_vertex_layout() -> VertexLayout {
    VertexLayout {
        bindings: vec![VertexBinding {
            binding: 0,
            stride: CUBE_VERTEX_STRIDE,
            input_rate: VertexInputRate::Vertex,
        }],
        attributes: vec![VertexAttribute {
            location: 0,
            binding: 0,
            format: BufferFormat::R32G32B32_SFLOAT,
            offset: 0,
        }],
    }
}

// ============================================================
// Debug pass action
// ============================================================

/// Debug pass action — draws selected scene instances with a chosen
/// debug visualization. See module-level docs.
pub struct DebugPassAction {
    scene: Arc<Mutex<Scene>>,
    // ---- Wireframe pass ----
    wireframe_entries: Arc<Mutex<Vec<DebugDrawEntry>>>,
    // ---- Bounding-box pass ----
    bb_entries: Arc<Mutex<Vec<DebugDrawEntry>>>,
    bb_vertex_shader: ShaderKey,
    cube_vertex_buffer: Arc<dyn Buffer>,
    cube_index_buffer: Arc<dyn Buffer>,
    // ---- Shared ----
    fragment_shader: ShaderKey,
    binding_group: Arc<dyn BindingGroup>,
    bind_textures: bool,
    /// Pre-computed dynamic-bindings mask for set 1, derived from the
    /// `SceneBinding` list at `new()` (any binding referencing a
    /// `BufferUpdateMode::Dynamic` buffer becomes dynamic). Reused for every
    /// pipeline this action lazily creates so the pipeline layout matches
    /// the dynamic-offset descriptor sets built by `ScenePassAction`-style
    /// binding groups.
    dynamic_bindings: graphics_device::DynamicBindings,
    /// Lazily-populated cache of pipelines keyed by (vs, fs, mode).
    pipeline_cache: FxHashMap<DebugPipelineCacheKey, PipelineKey>,
}

impl DebugPassAction {
    /// Create a `DebugPassAction`.
    ///
    /// `bindings` mirrors the per-pass descriptor set 1 used by
    /// `ScenePassAction` (typically: FrameData UBO + InstanceData SSBO at
    /// minimum, since the per-instance wireframe vertex shader reads
    /// both). The bounding-box vertex shader does not depend on these
    /// bindings but the binding group is shared.
    ///
    /// The cube vertex/index buffers used by the bounding-box pass are
    /// created here directly on the graphics device — they are not
    /// registered in the resource manager because they are an internal
    /// implementation detail of this action.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scene: Arc<Mutex<Scene>>,
        wireframe_entries: Arc<Mutex<Vec<DebugDrawEntry>>>,
        bb_entries: Arc<Mutex<Vec<DebugDrawEntry>>>,
        fragment_shader: ShaderKey,
        bb_vertex_shader: ShaderKey,
        bindings: Vec<SceneBinding>,
        bind_textures: bool,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<Self> {
        // Layout: for UBO/SSBO we auto-detect the dynamic variant from the
        // buffer's `update_mode()` so a Dynamic buffer wires correctly without
        // any extra plumbing on the caller side.
        let layout = BindingGroupLayoutDesc {
            entries: bindings
                .iter()
                .enumerate()
                .map(|(i, b)| BindingSlotDesc {
                    binding: i as u32,
                    binding_type: match b {
                        SceneBinding::UniformBuffer(buf) => match buf.graphics_device_buffer().update_mode() {
                            graphics_device::BufferUpdateMode::Static => BindingType::UniformBuffer,
                            graphics_device::BufferUpdateMode::Dynamic => BindingType::UniformBufferDynamic,
                        },
                        SceneBinding::StorageBuffer(buf) => match buf.graphics_device_buffer().update_mode() {
                            graphics_device::BufferUpdateMode::Static => BindingType::StorageBuffer,
                            graphics_device::BufferUpdateMode::Dynamic => BindingType::StorageBufferDynamic,
                        },
                        SceneBinding::SampledTexture(_, _) => BindingType::CombinedImageSampler,
                    },
                    count: 1,
                    stage_flags: ShaderStageFlags::VERTEX_FRAGMENT,
                })
                .collect(),
        };

        let resources: Vec<BindingResource> = bindings
            .iter()
            .map(|b| match b {
                SceneBinding::UniformBuffer(buf) => {
                    BindingResource::UniformBuffer(buf.graphics_device_buffer())
                }
                SceneBinding::StorageBuffer(buf) => {
                    BindingResource::StorageBuffer(buf.graphics_device_buffer())
                }
                SceneBinding::SampledTexture(tex, sampler_type) => {
                    BindingResource::SampledTexture(
                        tex.graphics_device_texture().as_ref(),
                        *sampler_type,
                    )
                }
            })
            .collect();

        let binding_group = graphics_device.create_binding_group_from_layout(
            &layout,
            1, // Set 1: per-pass bindings (set 0 is reserved for bindless textures).
            &resources,
        )?;

        // ---- Cube vertex buffer ----
        let cube_vertex_buffer = graphics_device.create_buffer(BufferDesc {
            size: CUBE_VERTEX_BUFFER_SIZE,
            usage: BufferUsage::Vertex,
            update_mode: BufferUpdateMode::Static,
        })?;
        cube_vertex_buffer.update(0, bytemuck::cast_slice(&CUBE_VERTICES))?;

        // ---- Cube index buffer ----
        let cube_index_buffer = graphics_device.create_buffer(BufferDesc {
            size: CUBE_INDEX_BUFFER_SIZE,
            usage: BufferUsage::Index,
            update_mode: BufferUpdateMode::Static,
        })?;
        cube_index_buffer.update(0, bytemuck::cast_slice(&CUBE_INDICES))?;

        // Pre-compute the dynamic-bindings mask for set 1 from the scene
        // bindings: any (set=1, binding=i) referencing a Dynamic buffer is
        // marked dynamic so pipelines created on the fly later get a layout
        // with `*_BUFFER_DYNAMIC` for those slots.
        let mut dynamic_bindings = graphics_device::DynamicBindings::new();
        for (i, b) in bindings.iter().enumerate() {
            let is_dynamic = match b {
                SceneBinding::UniformBuffer(buf) | SceneBinding::StorageBuffer(buf) => matches!(
                    buf.graphics_device_buffer().update_mode(),
                    graphics_device::BufferUpdateMode::Dynamic
                ),
                SceneBinding::SampledTexture(_, _) => false,
            };
            if is_dynamic {
                dynamic_bindings.add(1, i as u32);
            }
        }

        Ok(Self {
            scene,
            wireframe_entries,
            bb_entries,
            bb_vertex_shader,
            cube_vertex_buffer,
            cube_index_buffer,
            fragment_shader,
            binding_group,
            bind_textures,
            dynamic_bindings,
            pipeline_cache: FxHashMap::default(),
        })
    }
}

// ============================================================
// Helpers
// ============================================================

/// Color blend state for transparent debug overlays.
///
/// Standard `SrcAlpha / OneMinusSrcAlpha` over-blending: a single pipeline
/// can render any alpha value supplied via the fragment push constant.
fn debug_alpha_blend_state() -> ColorBlendState {
    ColorBlendState {
        blend_enable: true,
        src_color_factor: BlendFactor::SrcAlpha,
        dst_color_factor: BlendFactor::OneMinusSrcAlpha,
        color_blend_op: BlendOp::Add,
        src_alpha_factor: BlendFactor::One,
        dst_alpha_factor: BlendFactor::OneMinusSrcAlpha,
        alpha_blend_op: BlendOp::Add,
        color_write_mask: ColorWriteMask::ALL,
        color_write_enable: true,
    }
}

/// Rasterization state for a given debug mode.
///
/// Wireframe relies on `PolygonMode::Line` to convert triangle lists to
/// edges. BoundingBox already provides line primitives via
/// `PrimitiveTopology::LineList`, so it stays in `Fill` mode.
fn debug_rasterization_state(mode: DebugDisplayMode) -> RasterizationState {
    let polygon_mode = match mode {
        DebugDisplayMode::Wireframe => PolygonMode::Line,
        DebugDisplayMode::BoundingBox => PolygonMode::Fill,
    };
    RasterizationState {
        polygon_mode,
        ..Default::default()
    }
}

/// Topology for a given debug mode.
fn debug_primitive_topology(mode: DebugDisplayMode) -> PrimitiveTopology {
    match mode {
        DebugDisplayMode::Wireframe => PrimitiveTopology::TriangleList,
        DebugDisplayMode::BoundingBox => PrimitiveTopology::LineList,
    }
}

/// Dynamic state for a debug draw — depth/cull off, line width applied.
fn debug_dynamic_state(line_width: f32) -> DynamicRenderState {
    let mut state = DynamicRenderState::default();
    state.cull_mode = CullMode::None;
    state.depth_test_enable = false;
    state.depth_write_enable = false;
    state.line_width = line_width.max(1.0);
    state
}

/// Generate a pipeline name unique per `(vs, fs, mode)` triple.
fn debug_pipeline_name(key: &DebugPipelineCacheKey) -> String {
    let mut h = DefaultHasher::new();
    key.hash(&mut h);
    let mode_tag = match key.mode {
        DebugDisplayMode::Wireframe => "wireframe",
        DebugDisplayMode::BoundingBox => "boundingbox",
    };
    format!("_debug_{}_{:016X}", mode_tag, h.finish())
}

/// Compute the GPU bounding-box transform for a render instance.
///
/// The local AABB is first projected into world space using the Arvo
/// method (`AABB::transformed`), then a translation × scale matrix is
/// built so that the unit cube `[-0.5, +0.5]^3` maps onto the
/// world-space AABB. Rotation is intentionally dropped — a world-space
/// AABB is by definition axis-aligned.
fn bb_world_matrix(local_aabb: &crate::scene::AABB, world: &Mat4) -> Mat4 {
    let world_aabb = local_aabb.transformed(world);
    let center: Vec3 = world_aabb.center();
    let size: Vec3 = world_aabb.max - world_aabb.min;
    Mat4::from_translation(center) * Mat4::from_scale(size)
}

// ============================================================
// PassAction implementation
// ============================================================

impl PassAction for DebugPassAction {
    fn execute(
        &mut self,
        cmd: &mut dyn CommandList,
        pass_info: &PassInfo,
        graphics_device: &mut dyn graphics_device::GraphicsDevice,
    ) -> Result<()> {
        let scene = self.scene.lock().unwrap();
        let wireframe_entries = self.wireframe_entries.lock().unwrap();
        let bb_entries = self.bb_entries.lock().unwrap();
        if wireframe_entries.is_empty() && bb_entries.is_empty() {
            return Ok(());
        }

        let rm_arc = Engine::resource_manager()?;
        let mut rm = rm_arc.lock().unwrap();

        let bg_set_index = self.binding_group.set_index();

        // ============================================================
        // Wireframe pass
        // ============================================================
        for entry in wireframe_entries.iter() {
            // Look up the instance — silently skip stale keys, this is a
            // diagnostic overlay and should never crash the frame.
            let instance = match scene.render_instance(entry.instance_key) {
                Some(i) => i,
                None => continue,
            };

            let vs = instance.default_vertex_shader();
            let fs = self.fragment_shader;
            let geometry_key = instance.geometry();
            let geometry_mesh_id = instance.geometry_mesh_id();

            // ---- Resolve / create the pipeline ----
            let cache_key = DebugPipelineCacheKey {
                vs,
                fs,
                mode: DebugDisplayMode::Wireframe,
            };
            let pipeline_key = if let Some(&k) = self.pipeline_cache.get(&cache_key) {
                k
            } else {
                let geo = rm.geometry(geometry_key).ok_or_else(|| {
                    engine_err!(
                        "galaxy3d::DebugPassAction",
                        "Geometry not found for RenderInstance"
                    )
                })?;
                let vertex_layout = (**geo.vertex_layout()).clone();
                let pipeline_name = debug_pipeline_name(&cache_key);

                let desc = PipelineDesc {
                    vertex_shader: vs,
                    fragment_shader: fs,
                    vertex_layout,
                    topology: debug_primitive_topology(DebugDisplayMode::Wireframe),
                    rasterization: debug_rasterization_state(DebugDisplayMode::Wireframe),
                    color_blend: debug_alpha_blend_state(),
                    multisample: MultisampleState {
                        sample_count: pass_info.sample_count,
                        alpha_to_coverage_enable: false,
                    },
                    color_formats: pass_info.color_formats.clone(),
                    depth_format: pass_info.depth_format,
                    dynamic_bindings: self.dynamic_bindings,
                };

                let new_key = rm.create_pipeline(pipeline_name, desc, graphics_device)?;
                self.pipeline_cache.insert(cache_key, new_key);
                new_key
            };

            // ---- Bind pipeline + per-pass binding group ----
            let pipeline = rm.pipeline(pipeline_key).ok_or_else(|| {
                engine_err!(
                    "galaxy3d::DebugPassAction",
                    "Pipeline disappeared between cache insertion and bind"
                )
            })?;
            let gd_pipeline = pipeline.graphics_device_pipeline();
            cmd.bind_pipeline(gd_pipeline)?;
            if self.bind_textures {
                cmd.bind_textures()?;
            }
            cmd.bind_binding_group(gd_pipeline, bg_set_index, &self.binding_group)?;

            // ---- Per-entry dynamic state ----
            cmd.set_dynamic_state(&debug_dynamic_state(entry.line_width))?;

            // ---- Per-entry push-constant buffer (color goes at offset 16,
            //      draw_slot will be patched in at offset 0 per submesh) ----
            let mut pc_buffer = [0u8; DEBUG_WF_PC_SIZE];
            pc_buffer[16..32].copy_from_slice(bytemuck::bytes_of(&entry.color));

            // ---- Draw every submesh at LOD 0 ----
            let geo = rm.geometry(geometry_key).ok_or_else(|| {
                engine_err!(
                    "galaxy3d::DebugPassAction",
                    "Geometry vanished after pipeline resolve"
                )
            })?;
            let geom_mesh = geo.mesh(geometry_mesh_id).ok_or_else(|| {
                engine_err!(
                    "galaxy3d::DebugPassAction",
                    "GeometryMesh id {} missing from Geometry",
                    geometry_mesh_id
                )
            })?;

            cmd.bind_vertex_buffer(geo.vertex_buffer(), 0)?;
            if let Some(ib) = geo.index_buffer() {
                cmd.bind_index_buffer(ib, 0, geo.index_type())?;
            }

            for sm_idx in 0..instance.sub_mesh_count() {
                let render_sm = match instance.sub_mesh(sm_idx) {
                    Some(s) => s,
                    None => continue,
                };
                let geom_sm = match geom_mesh.submesh(render_sm.geometry_submesh_id()) {
                    Some(s) => s,
                    None => continue,
                };
                // Always pick LOD 0 for debug — wireframes need the full
                // mesh detail to be readable.
                let lod = match geom_sm.lod(0) {
                    Some(l) => l,
                    None => continue,
                };

                // Patch the draw_slot into the shared buffer and push the
                // full 32-byte block in one call (Slang reflects the block
                // as a single VERTEX|FRAGMENT range).
                let draw_slot = render_sm.draw_slot();
                pc_buffer[0..4].copy_from_slice(bytemuck::bytes_of(&draw_slot));
                cmd.push_constants(ShaderStageFlags::VERTEX_FRAGMENT, 0, &pc_buffer)?;

                if lod.index_count() > 0 {
                    cmd.draw_indexed(
                        lod.index_count(),
                        lod.index_offset(),
                        lod.vertex_offset() as i32,
                    )?;
                } else {
                    cmd.draw(lod.vertex_count(), lod.vertex_offset())?;
                }
            }
        }

        // ============================================================
        // Bounding-box pass
        // ============================================================
        if !bb_entries.is_empty() {
            let vs = self.bb_vertex_shader;
            let fs = self.fragment_shader;
            let cache_key = DebugPipelineCacheKey {
                vs,
                fs,
                mode: DebugDisplayMode::BoundingBox,
            };

            // ---- Resolve / create the pipeline (one per pass, fixed cube layout) ----
            let pipeline_key = if let Some(&k) = self.pipeline_cache.get(&cache_key) {
                k
            } else {
                let pipeline_name = debug_pipeline_name(&cache_key);
                let desc = PipelineDesc {
                    vertex_shader: vs,
                    fragment_shader: fs,
                    vertex_layout: cube_vertex_layout(),
                    topology: debug_primitive_topology(DebugDisplayMode::BoundingBox),
                    rasterization: debug_rasterization_state(DebugDisplayMode::BoundingBox),
                    color_blend: debug_alpha_blend_state(),
                    multisample: MultisampleState {
                        sample_count: pass_info.sample_count,
                        alpha_to_coverage_enable: false,
                    },
                    color_formats: pass_info.color_formats.clone(),
                    depth_format: pass_info.depth_format,
                    dynamic_bindings: self.dynamic_bindings,
                };

                let new_key = rm.create_pipeline(pipeline_name, desc, graphics_device)?;
                self.pipeline_cache.insert(cache_key, new_key);
                new_key
            };

            let pipeline = rm.pipeline(pipeline_key).ok_or_else(|| {
                engine_err!(
                    "galaxy3d::DebugPassAction",
                    "BB pipeline disappeared between cache insertion and bind"
                )
            })?;
            let gd_pipeline = pipeline.graphics_device_pipeline();
            cmd.bind_pipeline(gd_pipeline)?;
            if self.bind_textures {
                cmd.bind_textures()?;
            }
            cmd.bind_binding_group(gd_pipeline, bg_set_index, &self.binding_group)?;

            // Cube buffers are bound once for the whole BB list.
            cmd.bind_vertex_buffer(&self.cube_vertex_buffer, 0)?;
            cmd.bind_index_buffer(&self.cube_index_buffer, 0, IndexType::U16)?;

            for entry in bb_entries.iter() {
                let instance = match scene.render_instance(entry.instance_key) {
                    Some(i) => i,
                    None => continue,
                };

                // ---- Per-entry dynamic state (line width) ----
                cmd.set_dynamic_state(&debug_dynamic_state(entry.line_width))?;

                // ---- Push color (offset 16, 16 bytes) ----
                //      Stage flags must include both VERTEX and FRAGMENT —
                //      the pipeline layout exposes a single merged range
                //      covering both stages (see DEBUG_BB_*_PC_OFFSET comment).
                cmd.push_constants(
                    ShaderStageFlags::VERTEX_FRAGMENT,
                    DEBUG_BB_FS_PC_OFFSET,
                    bytemuck::bytes_of(&entry.color),
                )?;

                // ---- Push BB matrix (offset 32, 64 bytes) ----
                let bb_mat = bb_world_matrix(instance.bounding_box(), instance.world_matrix());
                let bb_cols: [[f32; 4]; 4] = bb_mat.to_cols_array_2d();
                cmd.push_constants(
                    ShaderStageFlags::VERTEX_FRAGMENT,
                    DEBUG_BB_VS_PC_OFFSET,
                    bytemuck::cast_slice(&bb_cols),
                )?;

                // ---- Draw the cube outline (12 edges, 24 indices) ----
                cmd.draw_indexed(CUBE_INDEX_COUNT, 0, 0)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "debug_pass_action_tests.rs"]
mod tests;
