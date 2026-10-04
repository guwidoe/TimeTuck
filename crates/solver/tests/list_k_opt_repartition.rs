// Original combined-cycle/partition references, independent of the generator.
use std::collections::BTreeSet;
use timetuck::*;
fn edges(row: &[usize]) -> BTreeSet<(usize, usize)> {
    (0..row.len())
        .map(|i| {
            let a = row[i];
            let b = row[(i + 1) % row.len()];
            (a.min(b), a.max(b))
        })
        .collect()
}
fn tours(n: usize) -> Vec<Vec<usize>> {
    fn visit(n: usize, row: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if row.len() == n {
            out.push(row.clone());
            return;
        }
        for i in 0..n {
            if !row.contains(&i) {
                row.push(i);
                visit(n, row, out);
                row.pop();
            }
        }
    }
    let mut out = vec![];
    visit(n, &mut vec![], &mut out);
    out
}
fn partitions(n: usize, m: usize) -> Vec<Vec<usize>> {
    if m == 1 {
        return vec![vec![n]];
    }
    (0..=n)
        .flat_map(|i| {
            partitions(n - i, m - 1)
                .into_iter()
                .map(move |tail| std::iter::once(i).chain(tail).collect())
        })
        .collect()
}
fn split(row: &[usize], lengths: &[usize]) -> Vec<Vec<usize>> {
    let mut start = 0;
    lengths
        .iter()
        .map(|&len| {
            let r = row[start..start + len].to_vec();
            start += len;
            r
        })
        .collect()
}
fn cfg(min: usize, max: usize, seed: u64, attempts: usize) -> ListKOptSampling {
    ListKOptSampling::new(ListKOptConfig::new(min, max).unwrap(), seed, attempts)
}
#[test]
fn every_small_tour_and_partition_matches_independent_rows_and_undo() {
    let mut checked = 0;
    for n in 4..=6 {
        let original: Vec<_> = (0..n).collect();
        let before_edges = edges(&original);
        for m in [2, 3] {
            let mut initial = vec![vec![]; m];
            initial[0] = original.clone();
            let p = ListProblem::new(
                original.clone(),
                initial.into_iter().map(ListEntity::new).collect(),
            )
            .unwrap();
            let a = p.initial_assignment();
            let entities: Vec<_> = (0..m).collect();
            for tour in tours(n) {
                let after_edges = edges(&tour);
                let removed: Vec<_> = before_edges.difference(&after_edges).copied().collect();
                if removed.len() < 2 {
                    continue;
                }
                let added: Vec<_> = after_edges.difference(&before_edges).copied().collect();
                for lengths in partitions(n, m) {
                    let movement = p
                        .k_opt_repartition_move(
                            &a,
                            &entities,
                            &removed,
                            &added,
                            (tour[0], tour[1]),
                            &lengths,
                        )
                        .unwrap();
                    let mut trial = a.clone();
                    {
                        let changed = p.apply(&mut trial, &movement).unwrap();
                        assert_eq!(changed.lists(), split(&tour, &lengths));
                        assert!(p.is_complete(&changed));
                    }
                    assert_eq!(trial, a);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 28070);
}
#[test]
fn sampled_outputs_cover_small_partitions_and_have_exact_edge_differences() {
    for n in 4..=5 {
        let p = ListProblem::new(
            (0..n).collect(),
            vec![
                ListEntity::new((0..n / 2).collect()),
                ListEntity::new((n / 2..n).collect()),
            ],
        )
        .unwrap();
        let a = p.initial_assignment();
        let original = edges(&(0..n).collect::<Vec<_>>());
        for k in 2..=n {
            let expected: BTreeSet<_> = tours(n)
                .into_iter()
                .filter(|t| original.difference(&edges(t)).count() == k)
                .flat_map(|t| {
                    partitions(n, 2)
                        .into_iter()
                        .map(move |lengths| split(&t, &lengths))
                })
                .collect();
            let mut actual = BTreeSet::new();
            for seed in 0..16 {
                for movement in
                    sample_list_k_opt_repartitions(&p, &a, cfg(k, k, seed, 2000)).unwrap()
                {
                    let mut trial = a.clone();
                    let changed = p.apply(&mut trial, &movement).unwrap();
                    assert!(expected.contains(changed.lists()));
                    actual.insert(changed.lists().to_vec());
                }
            }
            assert_eq!(actual, expected, "n={n},k={k}");
        }
    }
}
#[test]
fn selected_order_prefixes_ranges_and_unselected_lists_are_preserved() {
    let p = ListProblem::new(
        (0..11).collect(),
        vec![
            ListEntity::new(vec![6, 3, 4, 5])
                .with_pinned_prefix(1)
                .with_value_range(vec![6, 2, 3, 4, 5]),
            ListEntity::new(vec![7, 0, 1, 2])
                .with_pinned_prefix(1)
                .with_value_range(vec![7, 0, 1, 2, 4]),
            ListEntity::new(vec![8, 9]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let movement = p
        .k_opt_repartition_move(
            &a,
            &[1, 0],
            &[(1, 2), (4, 5)],
            &[(1, 4), (2, 5)],
            (0, 1),
            &[3, 3],
        )
        .unwrap();
    let mut trial = a.clone();
    {
        let changed = p.apply(&mut trial, &movement).unwrap();
        assert_eq!(
            changed.lists(),
            &[vec![6, 3, 2, 5], vec![7, 0, 1, 4], vec![8, 9]]
        );
        assert_eq!(changed.location(&10), None);
    }
    assert_eq!(trial, a);
    assert!(matches!(
        p.k_opt_repartition_move(
            &a,
            &[1, 0],
            &[(1, 2), (4, 5)],
            &[(1, 4), (2, 5)],
            (0, 1),
            &[6, 0]
        ),
        Err(ListKOptError::List(ListError::ValueOutsideEntityRange {
            entity: 1
        }))
    ));
    let mut count = 0;
    for m in sample_list_k_opt_repartitions(&p, &a, cfg(2, 6, 8, 4000)).unwrap() {
        let mut trial = a.clone();
        let changed = p.apply(&mut trial, &m).unwrap();
        assert_eq!(changed.lists()[0][0], 6);
        assert_eq!(changed.lists()[1][0], 7);
        assert_eq!(changed.lists()[2], vec![8, 9]);
        assert_eq!(changed.location(&10), None);
        assert!(p.validate(&changed).is_ok());
        count += 1;
    }
    assert!(count > 20);
}
#[test]
fn invalid_partitions_entities_and_sources_fail_without_mutation() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new((0..6).collect()),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let run = |entities: &[usize], lengths: &[usize]| {
        p.k_opt_repartition_move(
            &a,
            entities,
            &[(1, 2), (4, 5)],
            &[(1, 4), (2, 5)],
            (0, 1),
            lengths,
        )
    };
    for lengths in [vec![], vec![6], vec![2, 3], vec![usize::MAX, 1]] {
        assert_eq!(run(&[0, 1], &lengths), Err(ListKOptError::Partition));
    }
    assert_eq!(run(&[0, 0], &[3, 3]), Err(ListKOptError::Entities));
    assert_eq!(run(&[], &[]), Err(ListKOptError::Entities));
    assert!(matches!(
        run(&[0, 3], &[3, 3]),
        Err(ListKOptError::List(ListError::EntityIndex { .. }))
    ));
    assert_eq!(
        run(&[0, 2], &[3, 3]),
        Err(ListKOptError::List(ListError::PinChanged { entity: 2 }))
    );
    assert!(
        sample_list_k_opt_repartitions(&p, &ListAssignment::new(vec![]), cfg(2, 4, 0, 0)).is_err()
    );
    for orientation in [(usize::MAX, 1), (6, 1), (0, usize::MAX), (0, 0)] {
        assert_eq!(
            p.k_opt_repartition_move(
                &a,
                &[0, 1],
                &[(1, 2), (4, 5)],
                &[(1, 4), (2, 5)],
                orientation,
                &[3, 3]
            ),
            Err(ListKOptError::FirstSuccessor)
        );
    }
    assert_eq!(a, p.initial_assignment());
    for n in 0..=3 {
        let p = ListProblem::new(
            (0..n).collect(),
            vec![ListEntity::new((0..n).collect()), ListEntity::new(vec![])],
        )
        .unwrap();
        assert!(
            sample_list_k_opt_repartitions(
                &p,
                &p.initial_assignment(),
                cfg(2, usize::MAX, 0, usize::MAX)
            )
            .unwrap()
            .next()
            .is_none()
        );
    }
    let p = ListProblem::new(
        (0..6).collect(),
        vec![ListEntity::new((0..6).collect()).with_pinned(true)],
    )
    .unwrap();
    assert!(
        sample_list_k_opt_repartitions(&p, &p.initial_assignment(), cfg(2, 6, 0, usize::MAX))
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn seeded_attempt_prefixes_and_large_lazy_repartitions_are_reproducible() {
    let p = ListProblem::new(
        (0..10).collect(),
        vec![
            ListEntity::new((0..5).collect()),
            ListEntity::new((5..10).collect()),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut distinct = BTreeSet::new();
    for seed in 0..16 {
        let run = |attempts| {
            sample_list_k_opt_repartitions(&p, &a, cfg(2, 6, seed, attempts))
                .unwrap()
                .map(|m| {
                    let mut trial = a.clone();
                    p.apply(&mut trial, &m).unwrap().lists().to_vec()
                })
                .collect::<Vec<_>>()
        };
        let full = run(100);
        assert_eq!(full, run(100));
        assert!(full.starts_with(&run(7)));
        assert!(run(0).is_empty());
        assert!(full.len() <= 100);
        distinct.insert(full);
        assert_eq!(
            sample_list_k_opt_repartitions(&p, &a, cfg(2, 6, seed, 100))
                .unwrap()
                .collect::<Vec<_>>(),
            list_moves(&p, &a, Neighborhood::RepartitionKOpt(cfg(2, 6, seed, 100)))
                .unwrap()
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(distinct.len(), 16);
    let p = ListProblem::new(
        (0..130).collect(),
        vec![
            ListEntity::new((0..65).collect()),
            ListEntity::new((65..130).collect()),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let original = edges(&(0..130).collect::<Vec<_>>());
    let moves: Vec<_> = sample_list_k_opt_repartitions(&p, &a, cfg(65, 65, u64::MAX, usize::MAX))
        .unwrap()
        .take(2)
        .collect();
    assert_eq!(moves.len(), 2);
    for m in moves {
        let mut trial = a.clone();
        let changed = p.apply(&mut trial, &m).unwrap();
        let flat = changed
            .lists()
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(original.difference(&edges(&flat)).count(), 65);
    }
}
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn relocated_identities_retain_the_assigned_payload() {
    let p = ListProblem::new(
        (0..6).map(|id| Value { id, payload: 0 }).collect(),
        vec![
            ListEntity::new(
                (0..3)
                    .map(|id| Value {
                        id,
                        payload: 100 + id,
                    })
                    .collect(),
            ),
            ListEntity::new(
                (3..6)
                    .map(|id| Value {
                        id,
                        payload: 100 + id,
                    })
                    .collect(),
            ),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    for m in sample_list_k_opt_repartitions(&p, &a, cfg(2, 5, 17, 200)).unwrap() {
        let mut trial = a.clone();
        {
            let changed = p.apply(&mut trial, &m).unwrap();
            for v in changed.lists().iter().flatten() {
                assert_eq!(v.payload, 100 + v.id);
            }
        }
        for v in trial.lists().iter().flatten() {
            assert_eq!(v.payload, 100 + v.id);
        }
    }
}
fn reward(entity: usize, row: &[usize]) -> SimpleScore {
    let wanted = if entity == 0 {
        &[0, 2][..]
    } else {
        &[1, 3][..]
    };
    SimpleScore::new(i64::from(row == wanted))
}
#[test]
fn cross_entity_search_improves_both_list_backends_and_obeys_score_limits() {
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap();
    let mut net = ListConstraintNetwork::new(2, 4);
    for e in 0..2 {
        net.add_constraint(format!("row{e}"), vec![ListScope::Entity(e)], move |f| {
            let ListFact::Entity(row) = f[0] else {
                unreachable!()
            };
            reward(e, row)
        })
        .unwrap();
    }
    for seed in [0, 3, 17] {
        for order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 11 },
            MoveOrder::ShuffledMoves {
                seed: 11,
                window: 7.try_into().unwrap(),
            },
        ] {
            for budget in [0, 1, 2, 17, 1000] {
                let config = LocalSearchConfig {
                    neighborhood: Neighborhood::RepartitionKOpt(cfg(2, 2, seed, 1000)),
                    max_evaluations: budget,
                    max_steps: 3,
                    acceptance: Acceptance::StrictImprovement,
                    selection: MoveSelection::BestAccepted,
                    move_order: order,
                    assert_scores: true,
                    ..Default::default()
                };
                let a = list_local_search_from(&p, &net, p.initial_assignment(), config).unwrap();
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("reward", s.entities.port(), |(e, row)| reward(*e, row))
                    .unwrap();
                let c =
                    list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                        .unwrap();
                assert_eq!(a, c);
                assert!(a.evaluations <= budget);
                if budget == 1000 {
                    assert_eq!(a.best.unwrap().score, SimpleScore::new(2));
                }
            }
        }
    }
}
#[test]
fn per_step_seed_derivation_matches_manual_first_improvement() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 5, 4]),
            ListEntity::new(vec![3, 2, 1]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    fn rank(e: usize, row: &[usize]) -> SimpleScore {
        SimpleScore::new(
            row.iter()
                .enumerate()
                .map(|(i, &v)| ((e * 7 + i) * v) as i64)
                .sum(),
        )
    }
    fn total(a: &ListAssignment<usize>) -> SimpleScore {
        SimpleScore::new(
            a.lists()
                .iter()
                .enumerate()
                .map(|(e, row)| rank(e, row).score())
                .sum(),
        )
    }
    let mut net = ListConstraintNetwork::new(3, 6);
    for e in 0..3 {
        net.add_constraint(format!("rank{e}"), vec![ListScope::Entity(e)], move |f| {
            let ListFact::Entity(row) = f[0] else {
                unreachable!()
            };
            rank(e, row)
        })
        .unwrap();
    }
    for seed in 0..16 {
        let sampling = cfg(2, 5, seed, 200);
        let mut manual = p.initial_assignment();
        let mut calls = 1;
        let mut steps = 0;
        for step in 0..3 {
            let snapshot = manual.clone();
            let old = total(&snapshot);
            let mut chosen = None;
            for m in sample_list_k_opt_repartitions(&p, &snapshot, sampling.for_step(step)).unwrap()
            {
                calls += 1;
                let trial = p.apply(&mut manual, &m).unwrap();
                if total(&trial) > old {
                    chosen = Some(trial.clone());
                    break;
                }
            }
            if let Some(next) = chosen {
                manual = next;
                steps += 1;
            } else {
                break;
            }
        }
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::RepartitionKOpt(sampling),
            max_evaluations: 10000,
            max_steps: 3,
            acceptance: Acceptance::StrictImprovement,
            selection: MoveSelection::FirstAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let a = list_local_search_from(&p, &net, p.initial_assignment(), config).unwrap();
        assert_eq!(a.best.unwrap().assignment, manual);
        assert_eq!(a.evaluations, calls);
        assert_eq!(a.completed_steps, steps);
    }
}
#[test]
fn coupled_mixed_search_needs_joint_repartition_and_matches_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            (0..4).collect(),
            vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
        )
        .unwrap(),
    );
    let mut net = MixedConstraintNetwork::new(1, 2, 4);
    for e in 0..2 {
        net.add_constraint(
            format!("joint{e}"),
            vec![MixedScope::Basic(0), MixedScope::ListEntity(e)],
            move |f| {
                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&f[0], &f[1]) else {
                    unreachable!()
                };
                if v.copied() == Some(1) {
                    reward(e, row)
                } else {
                    SimpleScore::ZERO
                }
            },
        )
        .unwrap();
    }
    for joint in [false, true] {
        for seed in [0, 5, 19] {
            for budget in [0, 1, 2, 17, 1000] {
                let config = MixedLocalSearchConfig {
                    joint_moves: joint,
                    search: LocalSearchConfig {
                        neighborhood: Neighborhood::RepartitionKOpt(cfg(2, 2, seed, 1000)),
                        max_evaluations: budget,
                        max_steps: 3,
                        acceptance: Acceptance::StrictImprovement,
                        selection: MoveSelection::BestAccepted,
                        assert_scores: true,
                        ..Default::default()
                    },
                };
                let a = mixed_local_search_from(&p, &net, p.initial_assignment(), config).unwrap();
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::new(&mut b);
                let joined = b
                    .join_equal(s.basic.port(), s.lists.entities.port(), |_| 0, |_| 0)
                    .unwrap();
                b.score("joint", &joined, |((_, v), (e, row))| {
                    if *v == Some(1) {
                        reward(*e, row)
                    } else {
                        SimpleScore::ZERO
                    }
                })
                .unwrap();
                let c = mixed_stream_local_search_from(
                    &p,
                    b.build(),
                    s,
                    p.initial_assignment(),
                    config,
                )
                .unwrap();
                // Scoped and joined stream graphs can perform different amounts
                // of constraint work while exposing the same search behavior.
                assert_eq!(a.best, c.best);
                assert_eq!(a.evaluations, c.evaluations);
                assert_eq!(a.accepted_moves, c.accepted_moves);
                assert_eq!(a.completed_steps, c.completed_steps);
                assert_eq!(a.unimproved_steps, c.unimproved_steps);
                assert_eq!(a.termination, c.termination);
                assert!(a.evaluations <= budget);
                if budget == 1000 {
                    assert_eq!(
                        a.best.unwrap().score,
                        SimpleScore::new(if joint { 2 } else { 0 })
                    );
                }
            }
        }
    }
}
