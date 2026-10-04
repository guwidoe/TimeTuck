// Adapted from pinned Community MassListChangeMoveProviderTest, Apache-2.0.
// Method fixtures, finite enumeration substitutions and notices: compat/provenance.json.
use timetuck::*;
fn one(n: usize, optional: bool) -> ListProblem<usize> {
    let p = ListProblem::new((0..n).collect(), vec![ListEntity::new((0..n).collect())]).unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
fn samples(
    p: &ListProblem<usize>,
    config: ListMassChangeConfig,
    limit: usize,
) -> Vec<ListMassChange<usize>> {
    list_mass_change_moves(p, &p.initial_assignment(), config)
        .unwrap()
        .take(limit)
        .collect()
}
fn exact(n: usize) -> ListMassChangeConfig {
    ListMassChangeConfig::new(ListSampleSize::exactly(n))
}
#[test]
fn size_one_sample_produces_no_move() {
    assert!(samples(&one(5, false), exact(1), 10).is_empty());
}
#[test]
fn sample_never_includes_an_unassigned_value() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .allowing_unassigned();
    let moves = samples(&p, exact(2), 50);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| !m.members().contains(&2)));
}
#[test]
fn destination_respects_every_members_value_range() {
    let p = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0, 1]).with_value_range(vec![0, 1]),
            ListEntity::new(vec![]).with_value_range(vec![0]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let moves = samples(&p, ListMassChangeConfig::new(ListSampleSize::all()), 50);
    assert!(!moves.is_empty());
    for m in moves {
        if let Some(to) = m.destination() {
            assert!(
                m.members()
                    .iter()
                    .all(|v| p.entities()[to.entity].value_range().unwrap().contains(v))
            );
        }
    }
}
#[test]
fn crossing_null_default_true_also_unassigns_whole_sample() {
    assert!(
        samples(&one(8, true), exact(2), 1000)
            .iter()
            .any(|m| m.destination().is_none())
    );
}
#[test]
fn crossing_null_false_never_unassigns_sample() {
    let moves = samples(&one(8, true), exact(2).with_unassignment(false), 500);
    assert!(!moves.is_empty());
    assert!(moves.iter().all(|m| m.destination().is_some()));
}
#[test]
fn constructor_rejects_explicit_crossing_null_on_variable_without_unassigned_values() {
    let p = ListProblem::<usize>::new(vec![], vec![]).unwrap();
    assert!(matches!(
        list_mass_change_moves(
            &p,
            &p.initial_assignment(),
            exact(2).with_unassignment(true)
        ),
        Err(ListError::UnassignmentNotAllowed)
    ));
}
#[test]
fn different_draws_produce_different_samples() {
    let groups: std::collections::HashSet<_> = samples(&one(10, false), exact(3), 100)
        .into_iter()
        .map(|m| m.members().to_vec())
        .collect();
    assert!(groups.len() > 1);
}
fn prefix() -> ListProblem<usize> {
    ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(2)],
    )
    .unwrap()
    .allowing_unassigned()
}
#[test]
fn pinned_value_never_joins_a_sample() {
    let moves = samples(&prefix(), exact(2), 100);
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| !m.members().contains(&0) && !m.members().contains(&1))
    );
}
#[test]
fn destination_never_falls_in_the_pinned_prefix() {
    let moves = samples(&prefix(), exact(2), 100);
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| m.destination().is_none_or(|p| p.index >= 2))
    );
}
#[test]
fn fully_pinned_entity_is_never_drawn_from_or_into() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned(true),
            ListEntity::new((3..8).collect()),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let moves = samples(&p, exact(2), 100);
    assert!(!moves.is_empty());
    for m in moves {
        assert!(m.members().iter().all(|v| ![0, 1, 2].contains(v)));
        assert!(m.destination().is_none_or(|p| p.entity != 0));
    }
}
