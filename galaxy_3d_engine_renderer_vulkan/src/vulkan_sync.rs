/// Synchronization helpers built on `VK_KHR_synchronization2`
/// (core in Vulkan 1.3).
///
/// Centralises the construction of `VkImageMemoryBarrier2`, the emission
/// of `vkCmdPipelineBarrier2`, and the submission of command buffers via
/// `vkQueueSubmit2` so the rest of the backend can deal in small tuples
/// instead of repeating the Vulkan boilerplate.

use galaxy_3d_engine::galaxy3d::Result;
use galaxy_3d_engine::galaxy3d::render::AccessType;
use galaxy_3d_engine::engine_err;
use ash::vk;

/// Map an engine `AccessType` to the matching `VkPipelineStageFlags2` +
/// `VkAccessFlags2` pair.
///
/// The v2 stage/access enums are 64-bit and allow finer-grained
/// synchronisation than the v1 equivalents used elsewhere in the backend.
pub(crate) fn access_type_to_stage_access_2(
    access: AccessType,
) -> (vk::PipelineStageFlags2, vk::AccessFlags2) {
    match access {
        AccessType::ColorAttachmentWrite => (
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        ),
        AccessType::ColorAttachmentRead => (
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_READ,
        ),
        AccessType::DepthStencilWrite => (
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ),
        AccessType::DepthStencilReadOnly => (
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ,
        ),
        AccessType::FragmentShaderRead => (
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::SHADER_READ,
        ),
        AccessType::VertexShaderRead => (
            vk::PipelineStageFlags2::VERTEX_SHADER,
            vk::AccessFlags2::SHADER_READ,
        ),
        AccessType::ComputeRead => (
            vk::PipelineStageFlags2::COMPUTE_SHADER,
            vk::AccessFlags2::SHADER_READ,
        ),
        AccessType::ComputeWrite => (
            vk::PipelineStageFlags2::COMPUTE_SHADER,
            vk::AccessFlags2::SHADER_WRITE,
        ),
        AccessType::TransferRead => (
            vk::PipelineStageFlags2::ALL_TRANSFER,
            vk::AccessFlags2::TRANSFER_READ,
        ),
        AccessType::TransferWrite => (
            vk::PipelineStageFlags2::ALL_TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
        ),
        AccessType::RayTracingRead => (
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::SHADER_READ,
        ),
    }
}

/// Build a single `VkImageMemoryBarrier2` for the given image and transition.
///
/// `src_queue_family` / `dst_queue_family` are set to `QUEUE_FAMILY_IGNORED`
/// (no ownership transfer); all mip levels and array layers are covered.
pub(crate) fn image_barrier2(
    image: vk::Image,
    aspect: vk::ImageAspectFlags,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_stage: vk::PipelineStageFlags2,
    src_access: vk::AccessFlags2,
    dst_stage: vk::PipelineStageFlags2,
    dst_access: vk::AccessFlags2,
) -> vk::ImageMemoryBarrier2<'static> {
    vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src_stage)
        .src_access_mask(src_access)
        .dst_stage_mask(dst_stage)
        .dst_access_mask(dst_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: aspect,
            base_mip_level: 0,
            level_count: vk::REMAINING_MIP_LEVELS,
            base_array_layer: 0,
            layer_count: vk::REMAINING_ARRAY_LAYERS,
        })
}

/// Emit one or more `VkImageMemoryBarrier2` as a single
/// `vkCmdPipelineBarrier2` call.
///
/// Prefer batching related barriers through this entry-point rather than
/// issuing one pipeline barrier per image — it lets the driver combine
/// stages/accesses more efficiently.
///
/// # Safety
///
/// The caller must ensure `cmd` is a currently-recording command buffer
/// and `device` owns `cmd`.
pub(crate) unsafe fn emit_image_barriers2(
    device: &ash::Device,
    cmd: vk::CommandBuffer,
    barriers: &[vk::ImageMemoryBarrier2],
) {
    if barriers.is_empty() {
        return;
    }
    let dependency_info = vk::DependencyInfo::default().image_memory_barriers(barriers);
    device.cmd_pipeline_barrier2(cmd, &dependency_info);
}

/// Maximum number of semaphores in a single `submit_command_buffers` call.
/// Picked high enough to cover every current and foreseeable use:
/// frame submits wait on the acquire and signal the render-finished
/// semaphore (1 each); upload submits pass `&[]`. Going beyond this in
/// the future is cheap — just bump the constant.
const MAX_SEMAPHORES_PER_SUBMIT: usize = 4;
/// Maximum number of command buffers in a single `submit_command_buffers`.
/// Covers any per-frame multi-CB submit pattern.
const MAX_COMMAND_BUFFERS_PER_SUBMIT: usize = 8;

/// Submit one or more command buffers via `vkQueueSubmit2`.
///
/// `wait` / `signal` are `(semaphore, stage_mask)` pairs. Pass empty slices
/// for submits that don't need any binary-semaphore synchronisation
/// (e.g. one-shot upload submits followed by `queue_wait_idle`).
///
/// Implementation detail: builds the `VkSemaphoreSubmitInfo` /
/// `VkCommandBufferSubmitInfo` arrays on the stack (fixed-capacity
/// buffers) — no heap allocation per call.
///
/// # Safety
///
/// `queue` and all semaphores must belong to `device`.
pub(crate) unsafe fn submit_command_buffers(
    device: &ash::Device,
    queue: vk::Queue,
    cmd_buffers: &[vk::CommandBuffer],
    wait: &[(vk::Semaphore, vk::PipelineStageFlags2)],
    signal: &[(vk::Semaphore, vk::PipelineStageFlags2)],
    fence: vk::Fence,
) -> Result<()> {
    if wait.len() > MAX_SEMAPHORES_PER_SUBMIT
        || signal.len() > MAX_SEMAPHORES_PER_SUBMIT
    {
        engine_err!("galaxy3d::vulkan",
            "submit_command_buffers: too many semaphores (wait={}, signal={}, max={})",
            wait.len(), signal.len(), MAX_SEMAPHORES_PER_SUBMIT);
    }
    if cmd_buffers.len() > MAX_COMMAND_BUFFERS_PER_SUBMIT {
        engine_err!("galaxy3d::vulkan",
            "submit_command_buffers: too many command buffers ({} > max={})",
            cmd_buffers.len(), MAX_COMMAND_BUFFERS_PER_SUBMIT);
    }

    // Stack-allocated fixed-capacity arrays — zero heap allocation.
    let mut wait_infos: [vk::SemaphoreSubmitInfo; MAX_SEMAPHORES_PER_SUBMIT] =
        [vk::SemaphoreSubmitInfo::default(); MAX_SEMAPHORES_PER_SUBMIT];
    for (i, &(sem, stage)) in wait.iter().enumerate() {
        wait_infos[i] = vk::SemaphoreSubmitInfo::default()
            .semaphore(sem)
            .stage_mask(stage);
    }

    let mut signal_infos: [vk::SemaphoreSubmitInfo; MAX_SEMAPHORES_PER_SUBMIT] =
        [vk::SemaphoreSubmitInfo::default(); MAX_SEMAPHORES_PER_SUBMIT];
    for (i, &(sem, stage)) in signal.iter().enumerate() {
        signal_infos[i] = vk::SemaphoreSubmitInfo::default()
            .semaphore(sem)
            .stage_mask(stage);
    }

    let mut cmd_infos: [vk::CommandBufferSubmitInfo; MAX_COMMAND_BUFFERS_PER_SUBMIT] =
        [vk::CommandBufferSubmitInfo::default(); MAX_COMMAND_BUFFERS_PER_SUBMIT];
    for (i, &cb) in cmd_buffers.iter().enumerate() {
        cmd_infos[i] = vk::CommandBufferSubmitInfo::default().command_buffer(cb);
    }

    let submit_info = vk::SubmitInfo2::default()
        .wait_semaphore_infos(&wait_infos[..wait.len()])
        .command_buffer_infos(&cmd_infos[..cmd_buffers.len()])
        .signal_semaphore_infos(&signal_infos[..signal.len()]);

    device
        .queue_submit2(queue, &[submit_info], fence)
        .map_err(|e| engine_err!("galaxy3d::vulkan", "queue_submit2 failed: {:?}", e))
}
