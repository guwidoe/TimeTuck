//! Original mass-list successor reference, metadata and control regressions.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn permutations(values: &[usize], size: usize) -> Vec<Vec<usize>> {
    fn visit(values: &[usize], size: usize, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if prefix.len() == size {
            out.push(prefix.clone());
            return;
        }
        for v in values {
            if !prefix.contains(v) {
                prefix.push(*v);
                visit(values, size, prefix, out);
                prefix.pop();
            }
        }
    }
    let mut out = vec![];
    visit(values, size, &mut vec![], &mut out);
    out
}
fn reference(before: &[Vec<usize>], group: &[usize], to: Option<ListPosition>) -> Vec<Vec<usize>> {
    before
        .iter()
        .enumerate()
        .map(|(e, row)| {
            let mut out = vec![];
            for boundary in 0..=row.len() {
                if to
                    == Some(ListPosition {
                        entity: e,
                        index: boundary,
                    })
                {
                    out.extend_from_slice(group);
                }
                if let Some(v) = row.get(boundary).filter(|v| !group.contains(v)) {
                    out.push(*v);
                }
            }
            out
        })
        .collect()
}
#[test]
fn assigned_group_successors_match_independent_reference_across_layouts_pins_ranges_and_sizes() {
    let mut comparisons = 0;
    for mut layout in 0..27 {
        let mut rows = [vec![], vec![]];
        for v in 0..3 {
            let owner = layout % 3;
            layout /= 3;
            if owner < 2 {
                rows[owner].push(v);
            }
        }
        rows[0].reverse();
        for optional in [false, true] {
            for pin in 0..3 {
                let range: BTreeSet<_> = rows[1].iter().copied().chain([2]).collect();
                let p = ListProblem::new(
                    vec![2, 0, 1],
                    vec![
                        ListEntity::new(rows[0].clone())
                            .with_pinned(pin == 2)
                            .with_pinned_prefix(if pin == 1 { rows[0].len().min(1) } else { 0 }),
                        ListEntity::new(rows[1].clone())
                            .with_value_range(range.iter().copied().collect()),
                    ],
                )
                .unwrap();
                let p = if optional { p.allowing_unassigned() } else { p };
                let a = p.initial_assignment();
                let eligible: Vec<_> = p
                    .values()
                    .iter()
                    .copied()
                    .filter(|v| {
                        a.location(v).is_some_and(|at| {
                            !p.entities()[at.entity].is_pinned()
                                && at.index >= p.entities()[at.entity].pinned_prefix()
                        })
                    })
                    .collect();
                let mut sizes: Vec<_> = (0..=4)
                    .map(|n| (ListSampleSize::exactly(n), n, n))
                    .collect();
                sizes.extend([
                    (ListSampleSize::all(), eligible.len(), eligible.len()),
                    (ListSampleSize::between(0, 2).unwrap(), 0, 2),
                    (ListSampleSize::between(2, 3).unwrap(), 2, 3),
                    (
                        ListSampleSize::between(3, usize::MAX).unwrap(),
                        3,
                        usize::MAX,
                    ),
                    (ListMassChangeConfig::default().sample_size(), 2, usize::MAX),
                    (ListSampleSize::between(7, 8).unwrap(), 7, 8),
                ]);
                for (size, min, max) in sizes {
                    for unassign in [None, Some(false), Some(true)] {
                        let mut config = ListMassChangeConfig::new(size);
                        if let Some(enabled) = unassign {
                            config = config.with_unassignment(enabled);
                        }
                        if !optional && unassign == Some(true) {
                            assert!(matches!(
                                list_mass_change_moves(&p, &a, config),
                                Err(ListError::UnassignmentNotAllowed)
                            ));
                            continue;
                        }
                        let can_remove = unassign.unwrap_or(optional);
                        let mut expected = BTreeSet::new();
                        for k in min.max(2)..=max.min(eligible.len()) {
                            for group in permutations(&eligible, k) {
                                let destinations = std::iter::once(None)
                                    .filter(|_| can_remove)
                                    .chain(a.lists().iter().enumerate().flat_map(|(e, row)| {
                                        (0..=row.len()).map(move |index| {
                                            Some(ListPosition { entity: e, index })
                                        })
                                    }));
                                for to in destinations {
                                    let next = reference(a.lists(), &group, to);
                                    if next == a.lists() {
                                        continue;
                                    }
                                    if p.entities().iter().enumerate().all(|(e, entity)| {
                                        (!entity.is_pinned() || next[e] == a.lists()[e])
                                            && next[e].starts_with(
                                                &a.lists()[e][..entity.pinned_prefix()],
                                            )
                                            && next[e].iter().all(|v| {
                                                entity.value_range().is_none_or(|r| r.contains(v))
                                            })
                                    }) {
                                        expected.insert(next);
                                    }
                                }
                            }
                        }
                        let candidates: Vec<_> =
                            list_mass_change_moves(&p, &a, config).unwrap().collect();
                        let actual: BTreeSet<_> = candidates
                            .iter()
                            .map(|m| {
                                assert!(
                                    m.members().len() >= min.max(2) && m.members().len() <= max
                                );
                                assert!(m.members().iter().all(|v| eligible.contains(v)));
                                assert!(can_remove || m.destination().is_some());
                                let next = reference(a.lists(), m.members(), m.destination());
                                let mut state = a.clone();
                                {
                                    let trial = p.apply(&mut state, m.movement()).unwrap();
                                    assert_eq!(trial.lists(), next);
                                }
                                assert_eq!(state, a);
                                p.apply(&mut state, m.movement()).unwrap().commit();
                                p.validate(&state).unwrap();
                                assert_ne!(state, a);
                                assert!(
                                    p.values()
                                        .iter()
                                        .filter(|v| a.location(v).is_none())
                                        .all(|v| state.location(v).is_none())
                                );
                                if p.is_complete(&a) {
                                    assert!(p.is_complete(&state));
                                }
                                state.lists().to_vec()
                            })
                            .collect();
                        assert_eq!(actual, expected, "config={config:?},state={a:?}");
                        assert_eq!(
                            candidates
                                .into_iter()
                                .map(ListMassChange::into_move)
                                .collect::<Vec<_>>(),
                            list_moves(&p, &a, Neighborhood::MassListChange(config))
                                .unwrap()
                                .collect::<Vec<_>>()
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 4455);
}
#[test]
fn all_means_global_assigned_population_and_small_empty_bounds_are_fused() {
    assert!(matches!(
        ListSampleSize::between(3, 2),
        Err(ListError::InvalidSampleSize {
            minimum: 3,
            maximum: 2
        })
    ));
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1]).with_value_range(vec![0, 1]),
            ListEntity::new(vec![]).with_value_range(vec![0]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let config = ListMassChangeConfig::new(ListSampleSize::all()).with_unassignment(false);
    let moves: Vec<_> = list_mass_change_moves(&p, &a, config).unwrap().collect();
    assert!(!moves.is_empty());
    assert!(
        moves
            .iter()
            .all(|m| m.members().len() == 2 && m.destination().unwrap().entity == 0)
    );
    for size in [
        ListSampleSize::exactly(0),
        ListSampleSize::exactly(1),
        ListSampleSize::exactly(usize::MAX),
    ] {
        let mut iter = list_mass_change_moves(&p, &a, ListMassChangeConfig::new(size)).unwrap();
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
    let empty = ListProblem::<usize>::new(vec![], vec![]).unwrap();
    assert_eq!(
        list_mass_change_moves(
            &empty,
            &empty.initial_assignment(),
            ListMassChangeConfig::default()
        )
        .unwrap()
        .count(),
        0
    );
}
#[derive(Debug)]
struct Tracked(usize, Rc<Cell<usize>>);
impl Clone for Tracked {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Tracked {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Tracked {}
#[test]
fn full_group_permutations_are_lazy_and_clone_only_polled_candidates() {
    let calls = Rc::new(Cell::new(0));
    let values = || (0..80).map(|v| Tracked(v, calls.clone())).collect();
    let p = ListProblem::new(values(), vec![ListEntity::new(values())]).unwrap();
    let a = p.initial_assignment();
    calls.set(0);
    let mut moves =
        list_mass_change_moves(&p, &a, ListMassChangeConfig::new(ListSampleSize::all())).unwrap();
    assert_eq!(calls.get(), 0);
    assert!(moves.next().is_some());
    assert!(moves.next().is_some());
    assert!(calls.get() < 1600);
}

#[test]
fn optional_group_unassignment_changes_search_without_inserting_missing_values() {
    let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .allowing_unassigned();
    let mut network = ListConstraintNetwork::new(1, 3);
    network
        .add_constraint("length", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            SimpleScore::new(-(row.len() as i64))
        })
        .unwrap();
    for enabled in [false, true] {
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::MassListChange(
                ListMassChangeConfig::new(ListSampleSize::exactly(2)).with_unassignment(enabled),
            ),
            acceptance: Acceptance::StrictImprovement,
            max_steps: 3,
            max_evaluations: 50,
            assert_scores: true,
            ..Default::default()
        };
        let a = list_local_search_from(&p, &network, p.initial_assignment(), config).unwrap();
        let mut builder = StreamGraphBuilder::new();
        let sources = ListStreamSources::new(&mut builder);
        builder
            .score("length", sources.entities.port(), |(_, row)| {
                SimpleScore::new(-(row.len() as i64))
            })
            .unwrap();
        let b = list_stream_local_search_from(
            &p,
            builder.build(),
            sources,
            p.initial_assignment(),
            config,
        )
        .unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(a.evaluations, b.evaluations);
        let best = a.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(if enabled { 0 } else { -2 }));
        assert!(best.assignment.location(&2).is_none());
        let mixed = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p.clone(),
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 3);
        n.add_constraint(
            "score",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |f| {
                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&f[0], &f[1]) else {
                    unreachable!()
                };
                SimpleScore::new(v.copied().unwrap_or(0) as i64 - row.len() as i64)
            },
        )
        .unwrap();
        for joint_moves in [false, true] {
            let c = MixedLocalSearchConfig {
                search: config,
                joint_moves,
            };
            let a = mixed_local_search_from(&mixed, &n, mixed.initial_assignment(), c).unwrap();
            let mut builder = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut builder);
            builder
                .score("length", sources.lists.entities.port(), |(_, row)| {
                    SimpleScore::new(-(row.len() as i64))
                })
                .unwrap();
            builder
                .score("basic", sources.basic.port(), |(_, v)| {
                    SimpleScore::new(v.unwrap_or(0) as i64)
                })
                .unwrap();
            let b = mixed_stream_local_search_from(
                &mixed,
                builder.build(),
                sources,
                mixed.initial_assignment(),
                c,
            )
            .unwrap();
            assert_eq!(a.best, b.best);
            assert_eq!(a.evaluations, b.evaluations);
            let best = a.best.unwrap();
            assert_eq!(best.score, SimpleScore::new(if enabled { 1 } else { -1 }));
            assert!(best.assignment.lists().location(&2).is_none());
        }
    }
}

#[test]
fn invalid_unassignment_fails_before_any_phase_scoring_or_factory() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let config = LocalSearchConfig {
        neighborhood: Neighborhood::MassListChange(
            ListMassChangeConfig::new(ListSampleSize::exactly(2)).with_unassignment(true),
        ),
        ..Default::default()
    };
    let sequence = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(config),
        ],
        ..Default::default()
    };
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("score", vec![ListScope::Entity(0)], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    for budget in [0, 100] {
        let mut c = sequence.clone();
        c.limits.max_evaluations = budget;
        assert!(matches!(
            solve_list(&p, &n, c.clone()),
            Err(SolverError::Phase { phase: 1, .. })
        ));
        let r = solve_list_stream::<usize, SimpleScore>(
            &p,
            || {
                calls.set(calls.get() + 1);
                unreachable!("invalid plan factory")
            },
            c,
        );
        assert!(matches!(r, Err(SolverError::Phase { phase: 1, .. })));
    }
    assert!(matches!(
        list_local_search_from(&p, &n, p.initial_assignment(), config),
        Err(ListLocalSearchError::Scoring(_))
    ));
    let mixed = MixedProblem::new(FiniteProblem::<usize>::new(vec![]).unwrap(), p);
    let mut n = MixedConstraintNetwork::new(0, 1, 2);
    n.add_constraint("score", vec![MixedScope::ListEntity(0)], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let c = MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(MixedConstructionConfig::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: config,
                joint_moves: true,
            }),
        ],
        ..Default::default()
    };
    assert!(matches!(
        solve_mixed(&mixed, &n, c.clone()),
        Err(SolverError::Phase { phase: 1, .. })
    ));
    assert!(matches!(
        solve_mixed_stream::<usize, usize, SimpleScore>(
            &mixed,
            || {
                calls.set(calls.get() + 1);
                unreachable!("invalid mixed factory")
            },
            c
        ),
        Err(SolverError::Phase { phase: 1, .. })
    ));
    assert!(
        mixed_moves(
            &mixed,
            &mixed.initial_assignment(),
            config.neighborhood,
            true
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}
