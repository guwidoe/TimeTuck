// Adapted from Timefold Community 2.7.0 DefaultConstructionHeuristicForagerTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Upstream separate unassigned counts map to native network structural counts.
use timetuck::{ConstructionPickEarly as Pick, HardSoftScore, SimpleScore};

#[test]
fn check_pick_early_never() {
    let previous = SimpleScore::new(-100).with_structural(-8);
    for value in [-110, -100, -90] {
        assert!(!Pick::Never.should_pick(previous, SimpleScore::new(value).with_structural(-7)));
    }
}
#[test]
fn check_pick_early_first_non_deteriorating_score() {
    let previous = SimpleScore::new(-100).with_structural(-8);
    let pick = Pick::FirstNonDeterioratingScore;
    assert!(!pick.should_pick(previous, SimpleScore::new(-110).with_structural(-7)));
    assert!(pick.should_pick(previous, SimpleScore::new(-100).with_structural(-7)));
}
#[test]
fn check_pick_early_first_feasible_score() {
    let previous = HardSoftScore::new(0, -100).with_structural(-8);
    let pick = Pick::FirstFeasibleScore;
    assert!(!pick.should_pick(previous, HardSoftScore::new(-1, -110).with_structural(-7)));
    assert!(!pick.should_pick(previous, HardSoftScore::new(-1, -90).with_structural(-7)));
    assert!(pick.should_pick(previous, HardSoftScore::new(0, -110).with_structural(-7)));
}
#[test]
fn check_pick_early_first_feasible_score_or_non_deteriorating_hard() {
    let previous = HardSoftScore::new(-10, -100).with_structural(-8);
    let pick = Pick::FirstFeasibleScoreOrNonDeterioratingHard;
    assert!(!pick.should_pick(previous, HardSoftScore::new(-11, -110).with_structural(-7)));
    assert!(!pick.should_pick(previous, HardSoftScore::new(-11, -90).with_structural(-7)));
    assert!(pick.should_pick(previous, HardSoftScore::new(-10, -110).with_structural(-7)));
}
