/// Update strategies.
///
/// An Updater synchronizes scene data to GPU resources (e.g. SSBO).
/// Currently a placeholder for the upcoming SSBO work.

use crate::error::Result;
use super::scene::Scene;

/// Strategy for synchronizing scene data to GPU buffers.
///
/// Called once per frame before culling. `&mut self` allows
/// stateful implementations to track dirty state and manage
/// GPU buffer allocations.
pub trait Updater: Send + Sync {
    /// Update GPU resources from the scene's current state.
    fn update(&mut self, scene: &Scene) -> Result<()>;
}

/// No-op updater — does nothing.
///
/// Placeholder for scenes that don't need GPU buffer synchronization.
pub struct NoOpUpdater;

impl NoOpUpdater {
    pub fn new() -> Self {
        Self
    }
}

impl Updater for NoOpUpdater {
    fn update(&mut self, _scene: &Scene) -> Result<()> {
        Ok(())
    }
}
