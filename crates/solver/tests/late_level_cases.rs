// Adapted DefaultLevelScoreStateTest, Apache-2.0; see compat/provenance.json.
// Explicit initial/new-best comparison replaces Java step-scope caching.
use std::cmp::Ordering;
use timetuck::*;
#[test]
fn hard_improvement_triggers_reset() {
    let initial = HardSoftScore::new(-1, -1000);
    let improved = HardSoftScore::new(0, -200);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Greater);
}
#[test]
fn soft_only_improvement_does_not_trigger_reset() {
    let initial = HardSoftScore::new(-1, -1000);
    let improved = HardSoftScore::new(-1, -200);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Equal);
}
#[test]
fn medium_improvement_triggers_reset_for_hard_medium_soft_score() {
    let initial = HardMediumSoftScore::new(-1, -100, -1000);
    let improved = HardMediumSoftScore::new(-1, 0, -1000);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Greater);
}
#[test]
fn soft_only_improvement_does_not_trigger_reset_for_hard_medium_soft_score() {
    let initial = HardMediumSoftScore::new(-1, -100, -1000);
    let improved = HardMediumSoftScore::new(-1, -100, -500);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Equal);
}
#[test]
fn hard_improvement_triggers_reset_for_bendable_score() {
    let initial = BendableScore::new([-1], [0, 0]);
    let improved = BendableScore::new([0], [-10, -10]);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Greater);
}
#[test]
fn soft_improvement_does_not_trigger_reset_for_bendable_score() {
    let initial = BendableScore::new([-1], [0, 0]);
    let improved = BendableScore::new([-1], [1, 1]);
    assert_eq!(improved.late_acceptance_cmp(&initial), Ordering::Equal);
}
