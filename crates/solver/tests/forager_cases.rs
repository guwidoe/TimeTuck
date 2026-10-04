// Adapted Apache-2.0 pinned Community AcceptedLocalSearchForagerTest.
// Preserve all fixture scores, identities and early-stop assertions.
// Native saved payloads and supplied finalist indices replace Java scopes/random.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroU64;
use timetuck::{
    ForagerConfig, MoveForager, MoveSelection, RejectedMovePolicy, SimpleScore, TieBreaking,
};

fn forager(selection: MoveSelection, limit: u64, random: bool) -> MoveForager<char, SimpleScore> {
    MoveForager::new(ForagerConfig {
        selection,
        current_score: SimpleScore::new(-100),
        best_score: SimpleScore::new(-10),
        accepted_candidate_limit: NonZeroU64::new(limit),
        tie_breaking: if random {
            TieBreaking::Random { seed: 0 }
        } else {
            TieBreaking::First
        },
        rejected_moves: RejectedMovePolicy::BestScore,
    })
}
fn observe(
    f: &mut MoveForager<char, SimpleScore>,
    id: char,
    score: i64,
    accepted: bool,
    quit: bool,
) {
    f.add(SimpleScore::new(score), accepted, id);
    assert_eq!(f.is_quit_early(), quit);
}
#[test]
fn pick_move_max_score_accepted() {
    let mut f = forager(MoveSelection::BestAccepted, i32::MAX as u64, true);
    for (id, score, accepted) in [
        ('a', -20, true),
        ('b', -1, false),
        ('c', -20, false),
        ('d', -2, true),
        ('e', -300, true),
    ] {
        observe(&mut f, id, score, accepted, false);
    }
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('d'));
}
#[test]
fn pick_move_max_score_unaccepted() {
    let mut f = forager(MoveSelection::BestAccepted, i32::MAX as u64, true);
    for (id, score) in [('a', -20), ('b', -1), ('c', -20), ('d', -2), ('e', -300)] {
        observe(&mut f, id, score, false, false);
    }
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('b'));
}
#[test]
fn pick_move_first_best_score_improving() {
    let mut f = forager(
        MoveSelection::FirstBestScoreImproving,
        i32::MAX as u64,
        true,
    );
    observe(&mut f, 'a', -1, false, false);
    observe(&mut f, 'b', -20, true, false);
    observe(&mut f, 'c', -300, true, false);
    observe(&mut f, 'd', -1, true, true);
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('d'));
}
#[test]
fn pick_move_first_last_step_score_improving() {
    let mut f = forager(
        MoveSelection::FirstLastStepScoreImproving,
        i32::MAX as u64,
        true,
    );
    observe(&mut f, 'a', -1, false, false);
    observe(&mut f, 'b', -300, true, false);
    observe(&mut f, 'c', -4000, true, false);
    observe(&mut f, 'd', -20, true, true);
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('d'));
}
#[test]
fn pick_move_accepted_break_tie_randomly() {
    let mut f = forager(MoveSelection::BestAccepted, 4, true);
    observe(&mut f, 'a', -20, false, false);
    observe(&mut f, 'b', -1, true, false);
    observe(&mut f, 'c', -1, true, false);
    observe(&mut f, 'd', -20, true, false);
    observe(&mut f, 'e', -1, true, true);
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('c'));
}
#[test]
fn pick_move_accepted_break_tie_first() {
    let mut f = forager(MoveSelection::BestAccepted, 4, false);
    observe(&mut f, 'a', -20, false, false);
    observe(&mut f, 'b', -1, true, false);
    observe(&mut f, 'c', -1, true, false);
    observe(&mut f, 'd', -20, true, false);
    observe(&mut f, 'e', -1, true, true);
    assert_eq!(f.pick_with(|_| 1).unwrap(), Some('b'));
}
#[test]
fn forager_does_not_pick_a_rejected_structurally_flawed_move() {
    let mut f = forager(MoveSelection::BestAccepted, i32::MAX as u64, false);
    f.add(SimpleScore::ZERO.with_structural(-1), false, 'a');
    assert_eq!(f.finalist_count(), 0);
    assert_eq!(f.pick(), None);
}
