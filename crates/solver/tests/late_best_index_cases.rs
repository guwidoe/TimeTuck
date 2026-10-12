// Adapted pinned DefaultLevelScoreStateTest and TEST-only buildStepScope helper.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn update_then_check<S: Score>(initial: S, end: S) -> bool {
    let mut state: LateBestLevelState<_> = LateBestLevelState::at_best_step(initial, 0);
    state.refresh_best_step(0, || end.clone());
    state.higher_reset_levels(&1, || &end)
}
#[test]
fn hard_improvement_triggers_reset() {
    assert!(update_then_check(
        HardSoftScore::new(-1, -1000),
        HardSoftScore::new(0, -200)
    ));
}
#[test]
fn soft_only_improvement_does_not_trigger_reset() {
    assert!(!update_then_check(
        HardSoftScore::new(-1, -1000),
        HardSoftScore::new(-1, -200)
    ));
}
#[test]
fn medium_improvement_triggers_reset_for_hard_medium_soft_score() {
    assert!(update_then_check(
        HardMediumSoftScore::new(-1, -100, -1000),
        HardMediumSoftScore::new(-1, 0, -1000)
    ));
}
#[test]
fn soft_only_improvement_does_not_trigger_reset_for_hard_medium_soft_score() {
    assert!(!update_then_check(
        HardMediumSoftScore::new(-1, -100, -1000),
        HardMediumSoftScore::new(-1, -100, -500)
    ));
}
#[test]
fn hard_improvement_triggers_reset_for_bendable_score() {
    assert!(update_then_check(
        BendableScore::new([-1], [0, 0]),
        BendableScore::new([0], [-10, -10])
    ));
}
#[test]
fn soft_improvement_does_not_trigger_reset_for_bendable_score() {
    assert!(!update_then_check(
        BendableScore::new([-1], [0, 0]),
        BendableScore::new([-1], [1, 1])
    ));
}
#[test]
fn first_step_always_checks_level() {
    let initial = HardSoftScore::new(-1, -1000);
    let improved = HardSoftScore::new(0, -200);
    let mut state: LateBestLevelState<_> = LateBestLevelState::at_best_step(initial, 0);
    state.refresh_best_step(-1, || initial);
    assert!(state.higher_reset_levels(&0, || &improved));
}
#[test]
fn skips_check_when_step_index_unchanged() {
    let initial = HardSoftScore::new(-1, -1000);
    let hard = HardSoftScore::new(0, -200);
    let soft = HardSoftScore::new(0, -100);
    let mut state: LateBestLevelState<_> = LateBestLevelState::at_best_step(initial, 0);
    state.refresh_best_step(1, || hard);
    state.refresh_best_step(1, || soft);
    assert!(!state.higher_reset_levels(&1, || &soft));
}
#[test]
fn update_refreshes_baseline_for_next_step() {
    let initial = HardSoftScore::new(-1, -1000);
    let hard = HardSoftScore::new(0, -200);
    let soft = HardSoftScore::new(0, -100);
    let mut state: LateBestLevelState<_> = LateBestLevelState::at_best_step(initial, 0);
    state.refresh_best_step(1, || hard);
    assert!(!state.higher_reset_levels(&1, || &hard));
    state.refresh_best_step(1, || hard);
    assert!(!state.higher_reset_levels(&2, || &soft));
}
