use super::*;

// thresholds per frontier: (drop, raise). raise > drop, decreasing across frontiers.
// Example chain LOD0→LOD1 at 50/60, LOD1→LOD2 at 20/30, LOD2→LOD3 at 5/10.
fn standard_thresholds() -> Vec<(f32, f32)> {
    vec![(50.0, 60.0), (20.0, 30.0), (5.0, 10.0)]
}

#[test]
fn test_no_thresholds_returns_zero() {
    assert_eq!(apply_hysteresis(0, 100.0, &[]), 0);
    assert_eq!(apply_hysteresis(5, 1.0, &[]), 0);
}

#[test]
fn test_high_screen_size_stays_on_lod0() {
    let t = standard_thresholds();
    assert_eq!(apply_hysteresis(0, 1000.0, &t), 0);
    // Large enough to rise all the way from the worst LOD
    assert_eq!(apply_hysteresis(3, 1000.0, &t), 0);
}

#[test]
fn test_direct_jump_to_last_lod_on_camera_cut() {
    let t = standard_thresholds();
    // Was up close (LOD0), camera cut far away
    assert_eq!(apply_hysteresis(0, 2.0, &t), 3);
}

#[test]
fn test_dead_zone_keeps_current_lod() {
    let t = standard_thresholds();
    // Between drop=50 and raise=60 for the LOD0↔LOD1 frontier.
    // If we were on LOD0, we stay (screen_size not low enough to drop).
    assert_eq!(apply_hysteresis(0, 55.0, &t), 0);
    // If we were on LOD1, we stay (screen_size not high enough to raise).
    assert_eq!(apply_hysteresis(1, 55.0, &t), 1);
}

#[test]
fn test_rise_requires_raise_not_drop() {
    let t = standard_thresholds();
    // Just above drop (51) is NOT enough to rise from LOD1 to LOD0.
    assert_eq!(apply_hysteresis(1, 51.0, &t), 1);
    // Crossing raise (61) allows the rise.
    assert_eq!(apply_hysteresis(1, 61.0, &t), 0);
}

#[test]
fn test_drop_as_soon_as_below_drop() {
    let t = standard_thresholds();
    // Just below drop (49) is enough to descend from LOD0 to LOD1.
    assert_eq!(apply_hysteresis(0, 49.0, &t), 1);
}

#[test]
fn test_rise_stops_at_first_unsatisfied_frontier() {
    let t = standard_thresholds();
    // From LOD3, screen_size = 15 should rise to LOD2 (raise=10 crossed)
    // but not further (LOD1 requires raise=30, not met).
    assert_eq!(apply_hysteresis(3, 15.0, &t), 2);
}

#[test]
fn test_rise_across_multiple_frontiers() {
    let t = standard_thresholds();
    // From LOD3, a big screen size crosses all `raise` thresholds.
    assert_eq!(apply_hysteresis(3, 500.0, &t), 0);
}

#[test]
fn test_current_lod_out_of_range_clamped() {
    let t = standard_thresholds();
    // chain has 4 LODs (0..=3); a stale current=10 is clamped to 3.
    assert_eq!(apply_hysteresis(10, 1000.0, &t), 0);
    assert_eq!(apply_hysteresis(10, 2.0, &t), 3);
}

#[test]
fn test_single_frontier_two_lods() {
    let t = vec![(30.0, 40.0)];
    assert_eq!(apply_hysteresis(0, 100.0, &t), 0);
    assert_eq!(apply_hysteresis(0, 29.0, &t), 1);
    assert_eq!(apply_hysteresis(1, 35.0, &t), 1);   // dead zone
    assert_eq!(apply_hysteresis(1, 41.0, &t), 0);
}
