// Adapted from pinned Timefold Community subpillar provider tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::collections::BTreeSet;
use timetuck::*;

fn problem(
    values: usize,
    initial: &[usize],
    optional: bool,
    pinned_first: bool,
) -> FiniteProblem<usize> {
    FiniteProblem::new(
        initial
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let var = Variable::new((0..values).collect())
                    .with_initial(v)
                    .with_pinned(pinned_first && i == 0);
                if optional {
                    var.allowing_unassigned()
                } else {
                    var
                }
            })
            .collect(),
    )
    .unwrap()
}
fn config(mode: PillarChangeMode) -> SubPillarConfig {
    SubPillarConfig {
        selection: SubPillarSelection::combinations(2, 2).unwrap(),
        changes: mode,
        swap_selection: None,
    }
}
fn moves(p: &FiniteProblem<usize>, c: SubPillarConfig, limit: usize) -> Vec<Move<usize>> {
    subpillar_moves(p, &p.initial_assignment(), c)
        .unwrap()
        .take(limit)
        .collect()
}
fn unassigns(m: &Move<usize>) -> bool {
    let Move::Composite(parts) = m else { panic!() };
    !parts.is_empty()
        && parts
            .iter()
            .all(|m| matches!(m, Move::Change { value: None, .. }))
}

#[test]
fn change_subpillar_members_are_always_a_subset_of_the_full_pillar() {
    let p = problem(2, &[0; 5], false, false);
    let moves = moves(&p, config(PillarChangeMode::All), 200);
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| m.touched_variables().len() <= 2
                && m.touched_variables().iter().all(|&v| v < 5))
    );
}
#[test]
fn change_different_draws_produce_different_subpillars() {
    let p = problem(2, &[0; 5], false, false);
    let distinct: BTreeSet<_> = moves(&p, config(PillarChangeMode::All), 200)
        .into_iter()
        .map(|m| m.touched_variables())
        .collect();
    assert!(distinct.len() > 1);
}
#[test]
fn change_pinned_entity_excluded_from_subpillar() {
    let p = problem(2, &[0; 3], false, true);
    let moves = moves(&p, config(PillarChangeMode::All), 50);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !m.touched_variables().contains(&0)));
}
#[test]
fn change_crossing_null_default_true_also_unassigns_subpillar() {
    let p = problem(3, &[0; 6], true, false);
    let moves = moves(&p, config(PillarChangeMode::All), 500);
    assert!(moves.iter().any(unassigns));
}
#[test]
fn change_crossing_null_false_never_unassigns_subpillar() {
    let p = problem(3, &[0; 6], true, false);
    let moves = moves(&p, config(PillarChangeMode::AssignedOnly), 200);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !unassigns(m)));
}
#[test]
fn unassign_subpillar_members_are_always_a_subset_of_the_full_pillar() {
    let p = problem(2, &[0; 6], true, false);
    let moves = moves(&p, config(PillarChangeMode::UnassignOnly), 200);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| m.touched_variables().len() <= 2
        && m.touched_variables().iter().all(|&v| v < 6)
        && unassigns(m)));
}
#[test]
fn unassign_different_draws_produce_different_subpillars() {
    let p = problem(2, &[0; 6], true, false);
    let distinct: BTreeSet<_> = moves(&p, config(PillarChangeMode::UnassignOnly), 200)
        .into_iter()
        .map(|m| m.touched_variables())
        .collect();
    assert!(distinct.len() > 1);
}
#[test]
fn unassign_survives_slices_smaller_than_the_samplers_minimum_size() {
    let initial: Vec<_> = [0, 0].into_iter().chain(1..=20).collect();
    let p = problem(21, &initial, true, false);
    let moves = moves(&p, config(PillarChangeMode::UnassignOnly), 50);
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| m.touched_variables() == [0, 1] && unassigns(m))
    );
}
#[test]
fn unassign_pinned_entity_never_unassigned() {
    let p = problem(1, &[0; 3], true, true);
    let moves = moves(&p, config(PillarChangeMode::UnassignOnly), 50);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !m.touched_variables().contains(&0)));
}
fn swap_config(left: usize, right: usize) -> SubPillarConfig {
    SubPillarConfig {
        selection: SubPillarSelection::combinations(left, left).unwrap(),
        changes: PillarChangeMode::Disabled,
        swap_selection: Some(SubPillarSelection::combinations(right, right).unwrap()),
    }
}
#[test]
fn swap_subpillar_members_are_always_a_subset_of_the_full_pillar() {
    let p = problem(2, &[0, 0, 0, 0, 0, 1, 1, 1, 1, 1], false, false);
    let moves = moves(&p, swap_config(2, 2), 200);
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| m.touched_variables().len() <= 4
                && m.touched_variables().iter().all(|&v| v < 10))
    );
}
#[test]
fn swap_different_draws_produce_different_subpillars() {
    let p = problem(2, &[0, 0, 0, 0, 0, 1, 1, 1, 1, 1], false, false);
    let distinct: BTreeSet<_> = moves(&p, swap_config(2, 2), 200)
        .into_iter()
        .map(|m| m.touched_variables())
        .collect();
    assert!(distinct.len() > 1);
}
#[test]
fn swap_both_sides_respect_their_own_rule() {
    let p = problem(2, &[0, 0, 0, 0, 0, 1, 1, 1, 1, 1], false, false);
    let moves = moves(&p, swap_config(1, 3), 200);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| m.touched_variables().len() <= 4));
}
#[test]
fn swap_pinned_entity_never_participates_in_subpillar_swap() {
    let p = problem(2, &[0, 0, 0, 1, 1], false, true);
    let moves = moves(&p, swap_config(2, 2), 100);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !m.touched_variables().contains(&0)));
}
