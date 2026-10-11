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

#[test]
fn forager_does_not_pick_a_rejected_structurally_flawed_move() {
    use timetuck::{ForagerConfig, MoveForager, MoveSelection, RejectedMovePolicy};
    let previous = SimpleScore::new(-100).with_structural(-8);
    let mut forager = MoveForager::new(ForagerConfig {
        selection: MoveSelection::BestAccepted, // NEVER quit early.
        current_score: previous,
        best_score: previous,
        // Exercise the structural rejection even when ordinary rejected moves
        // may enter the fallback pool; construction itself needs no fallback.
        rejected_moves: RejectedMovePolicy::BestScore,
        ..Default::default()
    });
    // Source InnerScore has7 unassigned plus structural-1/ordinary0. Native
    // structural state combines incompleteness with the rejected flaw. The
    // source's initialized structurally-flawed sentinel maps to accepted=false.
    let unassigned = 7;
    let flawed = SimpleScore::new(0).with_structural(-1);
    let candidate = flawed.with_structural(flawed.structural() - unassigned);
    forager.add(candidate, false, 'a');
    assert_eq!(forager.finalist_count(), 0);
    assert_eq!(forager.accepted_candidates(), 0);
    assert!(!forager.is_quit_early());
    assert_eq!(
        forager.pick(),
        None,
        "picked a rejected structurally flawed move"
    );
}
