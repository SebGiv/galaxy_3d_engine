/// Render pass — a node in a render graph.
///
/// Owns its `PassAction` plus the list of `ResourceAccess` declarations
/// that drive synchronisation and attachment setup.
///
/// The pass keeps an always-valid cache (`framebuffer_key`, `pass_info`,
/// `gd_render_pass`, `clear_values`) computed eagerly by the
/// `RenderGraphManager` at construction time and rebuilt by the manager
/// whenever an access is mutated. Errors surface at the mutation call,
/// not at the next frame.

use std::sync::Arc;
use crate::graphics_device;
use crate::resource::resource_manager::PassInfo;
use super::access_type::ResourceAccess;
use super::frame_buffer::FramebufferKey;
use super::pass_action::PassAction;

slotmap::new_key_type! {
    /// Stable key for a `RenderPass` in the `RenderGraphManager`.
    pub struct RenderPassKey;
}

/// A single node of a render graph.
pub struct RenderPass {
    name: String,
    accesses: Vec<ResourceAccess>,
    action: Box<dyn PassAction>,

    // ===== Always-valid cache (rebuilt eagerly by the manager) =====
    /// Set when the pass has at least one attachment access.
    /// `None` for compute-only / read-only passes.
    framebuffer_key: Option<FramebufferKey>,
    pass_info: Option<PassInfo>,
    gd_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
    clear_values: Vec<graphics_device::ClearValue>,
}

impl RenderPass {
    /// Internal — created via `RenderGraphManager::create_render_pass()`.
    /// All cache fields are passed in pre-built; the pass is ready to
    /// execute immediately.
    pub(crate) fn new(
        name: String,
        accesses: Vec<ResourceAccess>,
        action: Box<dyn PassAction>,
        framebuffer_key: Option<FramebufferKey>,
        pass_info: Option<PassInfo>,
        gd_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
        clear_values: Vec<graphics_device::ClearValue>,
    ) -> Self {
        Self {
            name,
            accesses,
            action,
            framebuffer_key,
            pass_info,
            gd_render_pass,
            clear_values,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn accesses(&self) -> &[ResourceAccess] {
        &self.accesses
    }

    pub fn pass_info(&self) -> Option<&PassInfo> {
        self.pass_info.as_ref()
    }

    pub fn framebuffer_key(&self) -> Option<FramebufferKey> {
        self.framebuffer_key
    }

    pub(crate) fn gd_render_pass(&self) -> Option<&Arc<dyn graphics_device::RenderPass>> {
        self.gd_render_pass.as_ref()
    }

    pub(crate) fn clear_values(&self) -> &[graphics_device::ClearValue] {
        &self.clear_values
    }

    pub(crate) fn action_mut(&mut self) -> &mut dyn PassAction {
        self.action.as_mut()
    }

    /// Crate-internal mutable access to the access list — only the
    /// `RenderGraphManager`'s setters mutate this, then immediately call
    /// `set_cache` to keep the cache in sync.
    pub(crate) fn accesses_mut(&mut self) -> &mut Vec<ResourceAccess> {
        &mut self.accesses
    }

    /// Crate-internal: replace the entire access list in one shot.
    pub(crate) fn replace_accesses(&mut self, new_accesses: Vec<ResourceAccess>) {
        self.accesses = new_accesses;
    }

    /// Crate-internal: install a freshly built cache. Called by the
    /// manager right after every mutation that could invalidate the
    /// previous cache.
    pub(crate) fn set_cache(
        &mut self,
        framebuffer_key: Option<FramebufferKey>,
        pass_info: Option<PassInfo>,
        gd_render_pass: Option<Arc<dyn graphics_device::RenderPass>>,
        clear_values: Vec<graphics_device::ClearValue>,
    ) {
        self.framebuffer_key = framebuffer_key;
        self.pass_info = pass_info;
        self.gd_render_pass = gd_render_pass;
        self.clear_values = clear_values;
    }
}
