//! Unit tests for the synchronisation helpers.
//!
//! These exercise the pure builder functions (`image_barrier2`,
//! `buffer_barrier2`, `whole_image_subresource_range`) by inspecting the
//! returned struct fields. No Vulkan device is required.

use ash::vk;
use super::{
    buffer_barrier2, image_barrier2, whole_image_subresource_range,
};

#[test]
fn whole_image_subresource_range_uses_remaining_sentinels() {
    let r = whole_image_subresource_range(vk::ImageAspectFlags::DEPTH);
    assert_eq!(r.aspect_mask, vk::ImageAspectFlags::DEPTH);
    assert_eq!(r.base_mip_level, 0);
    assert_eq!(r.level_count, vk::REMAINING_MIP_LEVELS);
    assert_eq!(r.base_array_layer, 0);
    assert_eq!(r.layer_count, vk::REMAINING_ARRAY_LAYERS);
}

#[test]
fn image_barrier2_preserves_subresource_range_exactly() {
    let range = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 1,
        level_count: 1,
        base_array_layer: 2,
        layer_count: 1,
    };
    let b = image_barrier2(
        vk::Image::null(),
        range,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
    );
    assert_eq!(b.subresource_range.aspect_mask, vk::ImageAspectFlags::COLOR);
    assert_eq!(b.subresource_range.base_mip_level, 1);
    assert_eq!(b.subresource_range.level_count, 1);
    assert_eq!(b.subresource_range.base_array_layer, 2);
    assert_eq!(b.subresource_range.layer_count, 1);
    assert_eq!(b.old_layout, vk::ImageLayout::UNDEFINED);
    assert_eq!(b.new_layout, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
}

#[test]
fn image_barrier2_remaining_sentinels_pass_through() {
    let range = whole_image_subresource_range(vk::ImageAspectFlags::COLOR);
    let b = image_barrier2(
        vk::Image::null(),
        range,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
    );
    assert_eq!(b.subresource_range.level_count, vk::REMAINING_MIP_LEVELS);
    assert_eq!(b.subresource_range.layer_count, vk::REMAINING_ARRAY_LAYERS);
}

#[test]
fn buffer_barrier2_preserves_offset_and_size_exactly() {
    let b = buffer_barrier2(
        vk::Buffer::null(),
        256,
        1024,
        vk::PipelineStageFlags2::COMPUTE_SHADER,
        vk::AccessFlags2::SHADER_WRITE,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_READ,
    );
    assert_eq!(b.offset, 256);
    assert_eq!(b.size, 1024);
    assert_eq!(b.src_stage_mask, vk::PipelineStageFlags2::COMPUTE_SHADER);
    assert_eq!(b.dst_access_mask, vk::AccessFlags2::SHADER_READ);
}

#[test]
fn buffer_barrier2_whole_size_sentinel_passes_through() {
    let b = buffer_barrier2(
        vk::Buffer::null(),
        0,
        vk::WHOLE_SIZE,
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        vk::PipelineStageFlags2::VERTEX_SHADER,
        vk::AccessFlags2::SHADER_READ,
    );
    assert_eq!(b.offset, 0);
    assert_eq!(b.size, vk::WHOLE_SIZE);
}

#[test]
fn engine_sentinels_are_bit_equal_to_vulkan_sentinels() {
    // The engine exposes its own constants; this regression test
    // protects against future mismatches.
    assert_eq!(
        galaxy_3d_engine::galaxy3d::render_graph::REMAINING_MIP_LEVELS,
        vk::REMAINING_MIP_LEVELS
    );
    assert_eq!(
        galaxy_3d_engine::galaxy3d::render_graph::REMAINING_ARRAY_LAYERS,
        vk::REMAINING_ARRAY_LAYERS
    );
    assert_eq!(
        galaxy_3d_engine::galaxy3d::render_graph::WHOLE_SIZE,
        vk::WHOLE_SIZE
    );
}
