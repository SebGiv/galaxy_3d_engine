/// Render target — where a scene gets rendered to.
///
/// A render target represents a rendering destination (screen, texture, etc.).
/// Render targets can only be created via `TargetManager::create_render_target()`.
pub struct RenderTarget {
}

impl RenderTarget {
    /// Internal only — created via TargetManager::create_render_target()
    pub(crate) fn new() -> Self {
        Self {}
    }
}
