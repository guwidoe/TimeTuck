// Original exhaustive permutation/edge-set references and native integration checks.
use std::collections::BTreeSet;
use timetuck::*;
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn visit(n: usize, row: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if row.len() == n {
            out.push(row.clone());
            return;
        }
        for i in 1..n {
            if !row.contains(&i) {
                row.push(i);
                visit(n, row, out);
                row.pop();
            }
        }
    }
    let mut out = vec![];
    visit(n, &mut vec![0], &mut out);
    out
}
fn edges(row: &[usize]) -> BTreeSet<(usize, usize)> {
    (0..row.len())
        .map(|i| {
            let a = row[i];
            let b = row[(i + 1) % row.len()];
            (a.min(b), a.max(b))
        })
        .collect()
}
#[test]
fn all_small_reconnections_match_independent_permutation_edge_differences() {
    let mut checked = 0;
    for n in 3..=7 {
        for prefix in 0..=2 {
            let row: Vec<_> = (0..n + prefix).collect();
            let p = ListProblem::new(
                (0..n + prefix + 2).collect(),
                vec![
                    ListEntity::new(row.clone())
                        .with_pinned_prefix(prefix)
                        .with_value_range(row.clone()),
                    ListEntity::new(vec![n + prefix]).with_pinned(true),
                    ListEntity::new(vec![]),
                ],
            )
            .unwrap()
            .allowing_unassigned();
            let a = p.initial_assignment();
            let initial = edges(&(0..n).collect::<Vec<_>>());
            let reference = permutations(n);
            for k in 2..=n + 1 {
                let config = ListKOptConfig::new(k, k).unwrap();
                let expected: BTreeSet<_> = reference
                    .iter()
                    .filter(|r| initial.difference(&edges(r)).count() == k)
                    .map(|r| {
                        (0..prefix)
                            .chain(r.iter().map(|i| i + prefix))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                let mut actual = BTreeSet::new();
                let mut state = a.clone();
                for m in list_k_opt_moves(&p, &a, config).unwrap() {
                    let trial = p.apply(&mut state, &m).unwrap();
                    assert!(p.is_complete(&trial));
                    assert_eq!(&trial.lists()[1..], &a.lists()[1..]);
                    assert!(
                        actual.insert(trial.lists()[0].clone()),
                        "duplicate n={n} k={k} prefix={prefix}"
                    );
                    checked += 1;
                }
                assert_eq!(state, a);
                assert_eq!(actual, expected, "n={n} k={k} prefix={prefix}");
                let integrated: BTreeSet<_> = list_moves(&p, &a, Neighborhood::KOpt(config))
                    .unwrap()
                    .map(|m| p.apply(&mut state, &m).unwrap().lists()[0].clone())
                    .collect();
                assert_eq!(actual, integrated);
            }
            // The graph constructor is checked independently of path enumeration.
            for target in reference {
                let final_edges = edges(&target);
                let removed: Vec<_> = initial
                    .difference(&final_edges)
                    .map(|&(a, b)| (a + prefix, b + prefix))
                    .collect();
                if removed.len() < 2 {
                    continue;
                }
                let added: Vec<_> = final_edges
                    .difference(&initial)
                    .map(|&(a, b)| (a + prefix, b + prefix))
                    .collect();
                let m = p
                    .k_opt_move(&a, 0, &removed, &added, target[1] + prefix)
                    .unwrap();
                let mut state = a.clone();
                let expected: Vec<_> = (0..prefix)
                    .chain(target.iter().map(|i| i + prefix))
                    .collect();
                {
                    let trial = p.apply(&mut state, &m).unwrap();
                    assert_eq!(trial.lists()[0], expected);
                }
                assert_eq!(state, a);
            }
            let all: BTreeSet<_> =
                list_k_opt_moves(&p, &a, ListKOptConfig::new(2, usize::MAX).unwrap())
                    .unwrap()
                    .map(|m| {
                        let mut state = a.clone();
                        let trial = p.apply(&mut state, &m).unwrap();
                        trial.lists()[0].clone()
                    })
                    .collect();
            assert_eq!(all.len(), permutations(n).len() - 2);
        }
    }
    assert_eq!(checked, 2586);
}
#[test]
fn invalid_edges_subtours_and_configurations_are_rejected_without_mutation() {
    for (min, max) in [(0, 3), (1, 2), (4, 3)] {
        assert!(ListKOptConfig::new(min, max).is_err());
    }
    let p = ListProblem::new((0..8).collect(), vec![ListEntity::new((0..8).collect())]).unwrap();
    let a = p.initial_assignment();
    for (removed, added, successor, error) in [
        (vec![(0, 1)], vec![(0, 1)], 1, ListKOptError::EdgeCount),
        (
            vec![(0, 2), (3, 4)],
            vec![(0, 4), (2, 3)],
            4,
            ListKOptError::InvalidEdge,
        ),
        (
            vec![(0, 1), (1, 0)],
            vec![(0, 1), (2, 3)],
            1,
            ListKOptError::DuplicateEdge,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 0), (1, 4)],
            4,
            ListKOptError::InvalidEdge,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 8), (1, 4)],
            4,
            ListKOptError::InvalidEdge,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 3), (3, 0)],
            3,
            ListKOptError::DuplicateEdge,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(1, 2), (0, 4)],
            4,
            ListKOptError::DuplicateEdge,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 3), (0, 4)],
            3,
            ListKOptError::Degree,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 3), (1, 4)],
            5,
            ListKOptError::FirstSuccessor,
        ),
        (
            vec![(0, 1), (3, 4)],
            vec![(0, 4), (1, 3)],
            4,
            ListKOptError::Disconnected,
        ),
    ] {
        assert_eq!(p.k_opt_move(&a, 0, &removed, &added, successor), Err(error));
    }
    assert_eq!(a, p.initial_assignment());
    assert!(matches!(
        p.k_opt_move(&a, 1, &[], &[], 0),
        Err(ListKOptError::List(ListError::EntityIndex { .. }))
    ));
    let pinned = ListProblem::new(
        (0..8).collect(),
        vec![ListEntity::new((0..8).collect()).with_pinned(true)],
    )
    .unwrap();
    assert_eq!(
        pinned.k_opt_move(&a, 0, &[(0, 1), (3, 4)], &[(0, 3), (1, 4)], 3),
        Err(ListKOptError::List(ListError::PinChanged { entity: 0 }))
    );
    for n in 0..=3 {
        let p =
            ListProblem::new((0..n).collect(), vec![ListEntity::new((0..n).collect())]).unwrap();
        assert_eq!(
            list_k_opt_moves(
                &p,
                &p.initial_assignment(),
                ListKOptConfig::new(2, usize::MAX).unwrap()
            )
            .unwrap()
            .count(),
            0
        );
        if n < 3 {
            assert_eq!(
                p.k_opt_move(&p.initial_assignment(), 0, &[], &[], 0),
                Err(ListKOptError::TooFewValues)
            );
        }
    }
    let p = ListProblem::new(
        (0..8).collect(),
        vec![ListEntity::new((0..8).collect()).with_pinned_prefix(1)],
    )
    .unwrap();
    assert_eq!(
        p.k_opt_move(&a, 0, &[(0, 1), (3, 4)], &[(0, 3), (1, 4)], 3),
        Err(ListKOptError::InvalidEdge)
    );
    assert!(
        list_k_opt_moves(
            &p,
            &ListAssignment::new(vec![]),
            ListKOptConfig::new(2, 3).unwrap()
        )
        .is_err()
    );
}
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [0, 1, 4, 5, 3, 2]))
}
#[test]
fn k_opt_escapes_two_opt_plateau_in_both_list_scoring_backends() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 6);
    n.add_constraint("target", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::TwoOpt {
            cross_entity: false,
        },
        Neighborhood::KOpt(ListKOptConfig::new(3, 3).unwrap()),
        Neighborhood::SampledKOpt(ListKOptSampling::new(
            ListKOptConfig::new(3, 3).unwrap(),
            11,
            2000,
        )),
    ] {
        for order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 9 },
            MoveOrder::ShuffledMoves {
                seed: 9,
                window: 7.try_into().unwrap(),
            },
        ] {
            for budget in [0, 1, 2, 17, 1000] {
                let config = LocalSearchConfig {
                    neighborhood: neighborhood.clone(),
                    move_order: order,
                    max_evaluations: budget,
                    max_steps: 2,
                    acceptance: Acceptance::StrictImprovement,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                };
                let result =
                    list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("target", s.entities.port(), |(_, row)| reward(row))
                    .unwrap();
                let streamed =
                    list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                        .unwrap();
                assert_eq!(result.best, streamed.best);
                assert_eq!(result.evaluations, streamed.evaluations);
                assert_eq!(result.termination, streamed.termination);
                assert!(result.evaluations <= budget);
                if budget == 1000 {
                    assert_eq!(
                        result.best.unwrap().score,
                        SimpleScore::new(i64::from(matches!(
                            neighborhood,
                            Neighborhood::KOpt(_) | Neighborhood::SampledKOpt(_)
                        )))
                    );
                }
            }
        }
    }
}
#[test]
fn large_groups_are_lazy_and_have_no_machine_word_cardinality_limit() {
    let p = ListProblem::new(
        (0..130).collect(),
        vec![ListEntity::new((0..130).collect())],
    )
    .unwrap();
    let a = p.initial_assignment();
    let mut moves = list_k_opt_moves(&p, &a, ListKOptConfig::new(2, 2).unwrap()).unwrap();
    for _ in 0..2 {
        let m = moves.next().unwrap();
        let mut state = a.clone();
        assert!(p.is_complete(&p.apply(&mut state, &m).unwrap()));
    }
    // Supply 65 disjoint cuts and reconnect the resulting two-value segments.
    let target: Vec<_> = (0..65)
        .step_by(2)
        .chain((1..65).step_by(2))
        .flat_map(|i| [2 * i, 2 * i + 1])
        .collect();
    let original = edges(&(0..130).collect::<Vec<_>>());
    let after = edges(&target);
    let removed: Vec<_> = original.difference(&after).copied().collect();
    let added: Vec<_> = after.difference(&original).copied().collect();
    assert_eq!(removed.len(), 65);
    let m = p.k_opt_move(&a, 0, &removed, &added, target[1]).unwrap();
    let mut state = a.clone();
    assert_eq!(p.apply(&mut state, &m).unwrap().lists()[0], target);
}

#[test]
fn k_opt_integrates_joint_moves_orders_acceptors_and_both_mixed_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap(),
    );
    let mut net = MixedConstraintNetwork::new(1, 1, 6);
    net.add_constraint(
        "joint",
        vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
        |f| {
            let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&f[0], &f[1]) else {
                unreachable!()
            };
            if v.copied() == Some(1) {
                reward(row)
            } else {
                SimpleScore::ZERO
            }
        },
    )
    .unwrap();
    for neighborhood in [
        Neighborhood::KOpt(ListKOptConfig::new(3, 3).unwrap()),
        Neighborhood::SampledKOpt(ListKOptSampling::new(
            ListKOptConfig::new(3, 3).unwrap(),
            11,
            2000,
        )),
    ] {
        for joint_moves in [false, true] {
            for order in [
                MoveOrder::Original,
                MoveOrder::ShuffledVariables { seed: 13 },
                MoveOrder::ShuffledMoves {
                    seed: 13,
                    window: 5.try_into().unwrap(),
                },
            ] {
                for acceptance in [
                    Acceptance::StrictImprovement,
                    Acceptance::ValueTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                    Acceptance::MoveTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                ] {
                    for budget in [0, 1, 2, 17, 1000] {
                        let cfg = MixedLocalSearchConfig {
                            joint_moves,
                            search: LocalSearchConfig {
                                neighborhood: neighborhood.clone(),
                                move_order: order,
                                max_evaluations: budget,
                                max_steps: 3,
                                acceptance,
                                selection: MoveSelection::BestAccepted,
                                assert_scores: true,
                                ..Default::default()
                            },
                        };
                        let a =
                            mixed_local_search_from(&p, &net, p.initial_assignment(), cfg.clone())
                                .unwrap();
                        let mut b = StreamGraphBuilder::new();
                        let s = MixedStreamSources::new(&mut b);
                        let joined = b
                            .join_equal(s.basic.port(), s.lists.entities.port(), |_| 0, |_| 0)
                            .unwrap();
                        b.score("joint", &joined, |((_, v), (_, row))| {
                            if *v == Some(1) {
                                reward(row)
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
                            cfg,
                        )
                        .unwrap();
                        assert_eq!(a.best, c.best);
                        assert_eq!(a.evaluations, c.evaluations);
                        assert_eq!(a.termination, c.termination);
                        assert!(a.evaluations <= budget);
                        if budget == 1000 && acceptance == Acceptance::StrictImprovement {
                            assert_eq!(
                                a.best.unwrap().score,
                                SimpleScore::new(i64::from(joint_moves))
                            );
                        }
                    }
                }
            }
        }
    }
}
