/// SamplerCache — internal VkSampler management for the Vulkan backend
///
/// Creates and caches VkSampler objects on first use. Typical engines only
/// need 5-6 samplers total, so this is extremely lightweight.

use galaxy_3d_engine::galaxy3d::render::SamplerType;
use crate::vulkan_context::GpuContext;
use ash::vk;
use std::collections::HashMap;
use std::sync::Arc;

/// Internal sampler cache — creates VkSampler on first use, destroys on shutdown/drop
pub(crate) struct SamplerCache {
    ctx: Option<Arc<GpuContext>>,
    cache: HashMap<SamplerType, vk::Sampler>,
}

impl SamplerCache {
    pub(crate) fn new(ctx: Arc<GpuContext>) -> Self {
        Self {
            ctx: Some(ctx),
            cache: HashMap::new(),
        }
    }

    /// Get or create a VkSampler for the given type
    pub(crate) fn get(&mut self, sampler_type: SamplerType) -> vk::Sampler {
        if let Some(&sampler) = self.cache.get(&sampler_type) {
            return sampler;
        }

        let ctx = self.ctx.as_ref().expect("SamplerCache used after shutdown");
        let sampler = Self::create_vk_sampler(ctx, sampler_type);
        self.cache.insert(sampler_type, sampler);
        sampler
    }

    /// Destroy all cached VkSamplers and release the GpuContext reference.
    /// Must be called during VulkanGraphicsDevice::drop() while the device is still alive.
    pub(crate) fn shutdown(&mut self) {
        if let Some(ctx) = &self.ctx {
            for (_, sampler) in self.cache.drain() {
                unsafe { ctx.device.destroy_sampler(sampler, None); }
            }
        }
        self.ctx = None;
    }

    fn create_vk_sampler(ctx: &GpuContext, sampler_type: SamplerType) -> vk::Sampler {
        let (mag, min, mipmap, address, anisotropy, border, compare) = match sampler_type {
            SamplerType::LinearRepeat => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                Some(16.0),
                vk::BorderColor::FLOAT_OPAQUE_BLACK,
                false,
            ),
            SamplerType::LinearClamp => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                Some(16.0),
                vk::BorderColor::FLOAT_OPAQUE_BLACK,
                false,
            ),
            SamplerType::NearestRepeat => (
                vk::Filter::NEAREST,
                vk::Filter::NEAREST,
                vk::SamplerMipmapMode::NEAREST,
                vk::SamplerAddressMode::REPEAT,
                None,
                vk::BorderColor::FLOAT_OPAQUE_BLACK,
                false,
            ),
            SamplerType::NearestClamp => (
                vk::Filter::NEAREST,
                vk::Filter::NEAREST,
                vk::SamplerMipmapMode::NEAREST,
                vk::SamplerAddressMode::CLAMP_TO_EDGE,
                None,
                vk::BorderColor::FLOAT_OPAQUE_BLACK,
                false,
            ),
            SamplerType::Shadow => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::NEAREST,
                vk::SamplerAddressMode::CLAMP_TO_BORDER,
                None,
                vk::BorderColor::FLOAT_OPAQUE_WHITE,
                true,
            ),
            SamplerType::Anisotropic => (
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                Some(16.0),
                vk::BorderColor::FLOAT_OPAQUE_BLACK,
                false,
            ),
        };

        let mut create_info = vk::SamplerCreateInfo::default()
            .mag_filter(mag)
            .min_filter(min)
            .mipmap_mode(mipmap)
            .address_mode_u(address)
            .address_mode_v(address)
            .address_mode_w(address)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE)
            .border_color(border)
            .unnormalized_coordinates(false);

        if compare {
            create_info = create_info
                .compare_enable(true)
                .compare_op(vk::CompareOp::LESS_OR_EQUAL);
        } else {
            create_info = create_info
                .compare_enable(false)
                .compare_op(vk::CompareOp::ALWAYS);
        }

        if let Some(max_aniso) = anisotropy {
            create_info = create_info
                .anisotropy_enable(true)
                .max_anisotropy(max_aniso);
        } else {
            create_info = create_info
                .anisotropy_enable(false)
                .max_anisotropy(1.0);
        }

        unsafe {
            ctx.device.create_sampler(&create_info, None)
                .expect("Failed to create VkSampler")
        }
    }
}

impl Drop for SamplerCache {
    fn drop(&mut self) {
        // If shutdown() was called, ctx is None and cache is empty — nothing to do.
        // Otherwise, destroy remaining samplers (fallback safety).
        if let Some(ctx) = &self.ctx {
            for (_, sampler) in self.cache.drain() {
                unsafe { ctx.device.destroy_sampler(sampler, None); }
            }
        }
    }
}
