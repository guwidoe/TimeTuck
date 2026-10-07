use std::{collections::BTreeSet, num::NonZeroUsize};
use timetuck::*;
fn states(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    moves: impl Iterator<Item = ListMove<i32>>,
) -> BTreeSet<Vec<Vec<i32>>> {
    moves
        .map(|m| {
            let mut next = a.clone();
            p.apply(&mut next, &m).unwrap().commit();
            next.lists().to_vec()
        })
        .collect()
}
#[test]
fn both_enumerators_reach_all_small_required_and_optional_successors() {
    for optional in [false, true] {
        for code in 0..27 {
            let mut rows = vec![vec![], vec![]];
            let mut code = code;
            for v in [2, 4, 6] {
                let owner = code % 3;
                code /= 3;
                if owner < 2 {
                    rows[owner].push(v);
                }
            }
            let p = ListProblem::new(
                vec![2, 4, 6],
                rows.into_iter().map(ListEntity::new).collect(),
            )
            .unwrap();
            let p = if optional { p.allowing_unassigned() } else { p };
            let a = p.initial_assignment();
            let expected = states(
                &p,
                &a,
                list_assign_moves(&p, &a)
                    .unwrap()
                    .chain(list_change_moves(&p, &a).unwrap())
                    .chain(list_unassign_moves(&p, &a).unwrap()),
            );
            let anchored =
                list_anchored_value_changes(&p, &a, p.values(), p.values(), optional, false)
                    .unwrap()
                    .map(ListValueChange::into_move);
            assert_eq!(states(&p, &a, anchored), expected);
            let destinations: Vec<_> = list_destinations(&p, &a, optional).unwrap().collect();
            let explicit = list_value_changes(
                &p,
                &a,
                p.values(),
                &destinations,
                ListChangePairing::Cartesian,
                false,
            )
            .unwrap()
            .map(ListValueChange::into_move);
            assert_eq!(states(&p, &a, explicit), expected);
        }
    }
}
#[test]
fn pre_removal_positions_adjust_execution_and_undo_without_losing_metadata() {
    let p = ListProblem::new(vec![2, 4, 6], vec![ListEntity::new(vec![2, 4, 6])]).unwrap();
    let a = p.initial_assignment();
    let destinations = [
        Some(ListPosition {
            entity: 0,
            index: 2,
        }),
        Some(ListPosition {
            entity: 0,
            index: 3,
        }),
    ];
    let candidates: Vec<_> = list_value_changes(
        &p,
        &a,
        &[2],
        &destinations,
        ListChangePairing::Cartesian,
        false,
    )
    .unwrap()
    .collect();
    for (candidate, expected) in candidates.iter().zip([vec![4, 2, 6], vec![4, 6, 2]]) {
        assert_eq!(
            candidate.source(),
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        );
        let mut next = a.clone();
        {
            let applied = p.apply(&mut next, candidate.movement()).unwrap();
            assert_eq!(applied.lists(), std::slice::from_ref(&expected));
        }
        assert_eq!(next, a);
        p.apply(&mut next, candidate.movement()).unwrap().commit();
        assert_eq!(next.lists(), &[expected]);
    }
    assert_eq!(candidates[0].destination(), destinations[0]);
    assert_eq!(candidates[1].destination(), destinations[1]);
    let adjacent = [
        Some(ListPosition {
            entity: 0,
            index: 0,
        }),
        Some(ListPosition {
            entity: 0,
            index: 1,
        }),
    ];
    let noops: Vec<_> =
        list_value_changes(&p, &a, &[2], &adjacent, ListChangePairing::Cartesian, true)
            .unwrap()
            .collect();
    assert_eq!(noops.len(), 2);
    assert_ne!(noops[0].destination(), noops[1].destination());
    assert_eq!(noops[0].movement(), noops[1].movement());
}
#[test]
fn validation_pins_paired_inputs_and_dependencies_filter_before_execution() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]).with_pinned_prefix(1)],
    )
    .unwrap();
    let a = p.initial_assignment();
    let positions = [
        Some(ListPosition {
            entity: 0,
            index: 0,
        }),
        Some(ListPosition {
            entity: 0,
            index: 1,
        }),
        Some(ListPosition {
            entity: 0,
            index: 2,
        }),
    ];
    let actual: Vec<_> = list_value_changes(
        &p,
        &a,
        &[1, 2, 3],
        &positions,
        ListChangePairing::Paired,
        false,
    )
    .unwrap()
    .collect();
    assert_eq!(actual.len(), 1);
    assert_eq!(actual[0].value(), &3);
    assert_eq!(actual[0].source(), None);
    assert_eq!(actual[0].destination(), positions[2]);
    assert!(matches!(
        list_value_changes(&p, &a, &[99], &[], ListChangePairing::Paired, false),
        Err(ListError::ValueOutsideDomain)
    ));
    assert!(matches!(
        list_value_changes(&p, &a, &[], &[None], ListChangePairing::Paired, false),
        Err(ListError::UnassignmentNotAllowed)
    ));
    assert!(matches!(
        list_value_changes(
            &p,
            &a,
            &[],
            &[Some(ListPosition {
                entity: 0,
                index: 4
            })],
            ListChangePairing::Paired,
            false
        ),
        Err(ListError::InsertionIndex(_))
    ));
    assert!(matches!(
        list_anchored_value_changes(&p, &a, &[], &[99], false, false),
        Err(ListError::ValueOutsideDomain)
    ));
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![1]])
        .unwrap();
    let a = p.initial_assignment();
    assert_eq!(
        list_anchored_value_changes(&p, &a, p.values(), p.values(), false, false)
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn sampled_pairs_are_reproducible_bounded_and_lazy() {
    let p = ListProblem::new(vec![2, 4, 6], vec![ListEntity::new(vec![2, 4])])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    let positions: Vec<_> = list_destinations(&p, &a, true).unwrap().collect();
    let all: Vec<_> = list_value_changes(
        &p,
        &a,
        p.values(),
        &positions,
        ListChangePairing::Cartesian,
        true,
    )
    .unwrap()
    .collect();
    let mut rng = SampleRandom::new(51);
    let mut same = rng.clone();
    let bound = NonZeroUsize::new(1000).unwrap();
    drop(sample_list_value_changes(&p, &a, &mut rng, 100, true, true).unwrap());
    assert_eq!(rng.index(bound), same.index(bound));
    let draws: Vec<_> = sample_list_value_changes(&p, &a, &mut rng, 300, true, true)
        .unwrap()
        .collect();
    assert_eq!(
        draws,
        sample_list_value_changes(&p, &a, &mut same, 300, true, true)
            .unwrap()
            .collect::<Vec<_>>()
    );
    assert_eq!(draws.len(), 300);
    assert!(draws.iter().all(|c| all.contains(c)));
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)]).unwrap();
    let a = p.initial_assignment();
    let mut same = rng.clone();
    assert_eq!(
        sample_list_value_changes(&p, &a, &mut rng, usize::MAX, false, false)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(rng.index(bound), same.index(bound));
    let p = ListProblem::new(
        vec![1],
        vec![ListEntity::new(vec![]).with_value_range(vec![])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut same = rng.clone();
    assert_eq!(
        sample_list_value_changes(&p, &a, &mut rng, 1000, false, false)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(rng.index(bound), same.index(bound));
}
fn provider() -> ListValueChangeProvider<i32> {
    ListValueChangeProvider {
        values: vec![6, 2, 4],
        anchors: vec![2, 4, 6],
        include_unassigned: false,
        include_noops: false,
    }
}
#[test]
fn provider_constructs_lists_and_relocates_for_both_model_types() {
    let p = ListProblem::new(
        vec![2, 4, 6],
        vec![ListEntity::new(vec![2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 3);
    n.add_constraint("receiver", vec![ListScope::Entity(1)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let plan = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                assert_scores: true,
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let mut construct = provider();
    let mut improve = provider();
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(0, &mut construct);
    extensions.move_providers.bind(1, &mut improve);
    let result = solve_list_with_extensions(&p, &n, plan, extensions).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    assert!(result.assignment.lists()[0].is_empty());
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![8, 9]).with_initial(8)]).unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 3);
    n.add_constraint("receiver", vec![MixedScope::ListEntity(1)], |f| {
        let MixedFact::ListEntity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let plan = MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(MixedConstructionConfig::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 3,
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(0, &mut construct);
    extensions.move_providers.bind(1, &mut improve);
    let result = solve_mixed_with_extensions(&p, &n, plan, extensions).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(3)));
    assert_eq!(result.assignment.basic().values(), &[Some(8)]);
    assert!(result.assignment.lists().lists()[0].is_empty());
}
