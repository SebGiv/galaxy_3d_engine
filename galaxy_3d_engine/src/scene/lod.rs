//! LOD hysteresis selection.
//!
//! Industry-standard two-threshold hysteresis (Unity LOD Group, Unreal
//! `Dithered LOD Transitions`): each frontier between two consecutive LODs
//! has a `(drop, raise)` pair with `raise > drop`. Between the two values,
//! whichever LOD is currently bound stays bound — that's the "dead zone"
//! that absorbs small frame-to-frame jitter of the screen-space metric.
//!
//! Direct jump of several LODs in a single frame is allowed when the
//! screen-size metric changes abruptly (camera cut, teleport, spawn).
//! Residual popping is meant to be masked by dither + TAA at the shading
//! stage, not by throttling the LOD selection temporally.

/// Select the new LOD index from the previous one, the current screen-space
/// size of the object, and the per-frontier thresholds.
///
/// # Arguments
///
/// * `current`     — LOD index bound last frame.
/// * `screen_size` — projected diameter in pixels.
/// * `thresholds`  — `(drop, raise)` pairs, one per frontier.
///                   `thresholds.len()` must equal `lod_count - 1`.
///                   Must be decreasing: `thresholds[i].0 > thresholds[i+1].1`.
///
/// # Semantics
///
/// * **Drop to a coarser LOD** (larger index): happens as soon as the target
///   LOD, computed directly from `screen_size` against the `drop` thresholds,
///   is `>=` current. The jump can span several LODs.
/// * **Rise to a finer LOD** (smaller index): one frontier at a time, and
///   only if `screen_size` exceeds that frontier's `raise` threshold.
/// * The result is clamped to `[0, thresholds.len()]` (i.e. a valid LOD
///   index even when `current` is out of range of the current chain).
pub fn apply_hysteresis(
    current: u8,
    screen_size: f32,
    thresholds: &[(f32, f32)],
) -> u8 {
    let lod_count = thresholds.len() + 1;
    if lod_count == 0 {
        return 0;
    }
    let max_lod = (lod_count - 1) as u8;
    let current = current.min(max_lod);

    // The "ideal" LOD assuming no hysteresis: advance while screen_size < drop
    // of the current frontier.
    let mut ideal: u8 = 0;
    while (ideal as usize) < thresholds.len()
        && screen_size < thresholds[ideal as usize].0
    {
        ideal += 1;
    }

    if ideal >= current {
        // Going to a coarser (or equal) LOD — immediate jump, any distance.
        ideal
    } else {
        // We want to ascend (current > ideal). Only cross each frontier if
        // screen_size is strictly above its raise threshold. Stop otherwise.
        let mut lod = current;
        while lod > ideal
            && screen_size > thresholds[(lod - 1) as usize].1
        {
            lod -= 1;
        }
        lod
    }
}

#[cfg(test)]
#[path = "lod_tests.rs"]
mod tests;
