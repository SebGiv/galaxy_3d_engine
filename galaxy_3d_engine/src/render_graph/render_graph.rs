/// Render graph — a DAG describing how a frame is rendered.
///
/// A render graph defines the sequence of render passes and the
/// render targets that connect them. It is the high-level description
/// of a complete rendering pipeline.
///
/// Render graphs can only be created via `RenderGraphManager::create_render_graph()`.
pub struct RenderGraph {
}

impl RenderGraph {
    /// Internal only — created via RenderGraphManager::create_render_graph()
    pub(crate) fn new() -> Self {
        Self {}
    }
}
