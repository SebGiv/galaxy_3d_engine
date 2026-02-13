/// Render pass node in a render graph.
///
/// High-level description of a rendering step (e.g. shadow pass,
/// geometry pass, post-process pass). This is a DAG node — not to
/// be confused with `render::RenderPass` which is the low-level
/// GPU render pass configuration.
///
/// Each pass declares which targets it reads from (inputs) and
/// writes to (outputs), using target indices within the parent
/// `RenderGraph`.
pub struct RenderPass {
    /// Target indices this pass reads from
    inputs: Vec<usize>,
    /// Target indices this pass writes to
    outputs: Vec<usize>,
}

impl RenderPass {
    pub(crate) fn new() -> Self {
        Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Get the target indices this pass reads from
    pub fn inputs(&self) -> &[usize] {
        &self.inputs
    }

    /// Get the target indices this pass writes to
    pub fn outputs(&self) -> &[usize] {
        &self.outputs
    }

    /// Add an input target index
    pub(crate) fn add_input(&mut self, target_id: usize) {
        self.inputs.push(target_id);
    }

    /// Add an output target index
    pub(crate) fn add_output(&mut self, target_id: usize) {
        self.outputs.push(target_id);
    }
}
