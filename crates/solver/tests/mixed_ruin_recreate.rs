//! Original mixed repair checks; no upstream method credit.
use std::{cell::Cell, time::Duration};
use timetuck::*;
type Problem = MixedProblem<i32, i32>;
type Result = MixedRuinRecreateResult<i32, i32, SimpleScore>;
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];
fn config() -> MixedRuinRecreateConfig<'static> {
    MixedRuinRecreateConfig {
        search: RuinRecreateConfig {
            minimum_ruined: usize::MAX,
            maximum_ruined: usize::MAX,
            max_attempts: 3,
            seed: 917,
            assert_scores: true,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn run_both(
    p: &Problem,
    c: MixedRuinRecreateConfig,
    score: impl Fn(&[Option<i32>], &[Vec<i32>]) -> i64,
) -> Result {
    let nb = p.basic().variables().len();
    let ne = p.lists().entities().len();
    let mut n = MixedConstraintNetwork::new(nb, ne, p.lists().values().len());
    n.add_constraint(
        "coupled",
        (0..nb)
            .map(MixedScope::Basic)
            .chain((0..ne).map(MixedScope::ListEntity))
            .collect(),
        |f| {
            let basic: Vec<_> = f[..nb]
                .iter()
                .map(|v| {
                    let MixedFact::Basic(v) = v else {
                        unreachable!()
                    };
                    v.copied()
                })
                .collect();
            let lists: Vec<_> = f[nb..]
                .iter()
                .map(|v| {
                    let MixedFact::ListEntity(v) = v else {
                        unreachable!()
                    };
                    v.to_vec()
                })
                .collect();
            SimpleScore::new(score(&basic, &lists))
        },
    )
    .unwrap();
    let a = ruin_recreate_mixed_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
    let b = ruin_recreate_mixed_stream_search_from(
        p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::<i32, i32>::new(&mut b);
            let basics = b.collect_list(s.basic.port(), |row| *row)?;
            let lists = b.collect_list(s.lists.entities.port(), |row| row.clone())?;
            let all = b.join_equal(&basics, &lists, |_| 0, |_| 0)?;
            b.score("coupled", &all, |(bs, ls)| {
                let mut basic = vec![None; nb];
                let mut lists = vec![vec![]; ne];
                for (i, v) in bs.iter() {
                    basic[*i] = *v;
                }
                for (i, v) in ls.iter() {
                    lists[*i] = v.clone();
                }
                SimpleScore::new(score(&basic, &lists))
            })?;
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        c.clone(),
    )
    .unwrap();
    assert_eq!(a.best, b.best);
    assert_eq!(
        (
            a.evaluations,
            a.repair_move_evaluations,
            a.attempts,
            a.complete_reconstructions,
            a.accepted_moves,
            a.termination
        ),
        (
            b.evaluations,
            b.repair_move_evaluations,
            b.attempts,
            b.complete_reconstructions,
            b.accepted_moves,
            b.termination
        )
    );
    assert!(a.evaluations <= c.search.max_evaluations);
    if let Some(best) = &a.best {
        p.validate(&best.assignment).unwrap();
        assert!(p.is_complete(&best.assignment));
        assert_eq!(
            best.score,
            SimpleScore::new(score(
                best.assignment.basic().values(),
                best.assignment.lists().lists()
            ))
        );
    }
    a
}
fn simple() -> Problem {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![0],
            vec![ListEntity::new(vec![0]), ListEntity::new(vec![])],
        )
        .unwrap(),
    )
}
// Independent table-based repair, without native moves, construction or scoring.
fn reference(
    budget: u64,
    order: MixedConstructionOrder,
    table: &[[i64; 3]; 3],
) -> Option<(i32, usize, i64)> {
    if budget == 0 {
        return None;
    }
    let mut used = 1;
    let mut current = (0, 0);
    let mut best = current;
    for _ in 0..3 {
        if used == budget {
            break;
        }
        used += 1;
        let mut trial = [2, 2]; // index 2 denotes unassigned
        for part in if order == MixedConstructionOrder::BasicThenLists {
            [0, 1]
        } else {
            [1, 0]
        } {
            let mut winner = None;
            for value in 0..2 {
                if used == budget {
                    break;
                }
                let mut candidate = trial;
                candidate[part] = value;
                let score = table[candidate[0]][candidate[1]];
                used += 1;
                if winner.is_none_or(|(_, old)| score > old) {
                    winner = Some((value, score));
                }
            }
            if let Some((value, _)) = winner {
                trial[part] = value;
            }
            if used == budget {
                break;
            }
        }
        if trial[0] < 2 && trial[1] < 2 {
            if table[trial[0]][trial[1]] > table[best.0][best.1] {
                best = (trial[0], trial[1]);
            }
            if table[trial[0]][trial[1]] >= table[current.0][current.1] {
                current = (trial[0], trial[1]);
            }
        }
    }
    Some((best.0 as i32, best.1, table[best.0][best.1]))
}
#[test]
fn independent_coupled_tables_match_every_budget_prefix_and_both_orders() {
    let p = simple();
    for seed in 0..24 {
        let table: [[i64; 3]; 3] = std::array::from_fn(|b| {
            std::array::from_fn(|o| ((seed * 17 + b * 13 + o * 7 + b * o * seed) % 23) as i64 - 11)
        });
        for order in ORDERS {
            for budget in 0..=20 {
                let mut c = config();
                c.order = order;
                c.search.max_evaluations = budget;
                let r = run_both(&p, c, |b, l| {
                    table[b[0].map_or(2, |v| v as usize)]
                        [l.iter().position(|v| v.contains(&0)).unwrap_or(2)]
                });
                let actual = r.best.map(|s| {
                    (
                        s.assignment.basic().values()[0].unwrap(),
                        s.assignment.lists().location(&0).unwrap().entity,
                        s.score.score(),
                    )
                });
                assert_eq!(
                    actual,
                    reference(budget, order, &table),
                    "seed={seed} order={order:?} budget={budget}"
                );
            }
        }
    }
}
#[test]
fn coupled_order_and_global_pool_can_escape_different_valleys() {
    let p = simple();
    for order in ORDERS {
        for placement in [
            ConstructionPlacement::Sequential,
            ConstructionPlacement::Pooled,
            ConstructionPlacement::ValueQueue,
        ] {
            let mut c = config();
            c.order = order;
            c.search.placement = placement;
            let r = run_both(&p, c, |b, l| match (b[0], l[1].contains(&0)) {
                (Some(1), true) => 10,
                (Some(0), _) => 5,
                (None, true) => 4,
                _ => 0,
            });
            let expected = if order == MixedConstructionOrder::BasicThenLists
                || placement != ConstructionPlacement::Sequential
            {
                5
            } else {
                10
            };
            assert_eq!(r.best.unwrap().score, SimpleScore::new(expected));
        }
    }
}
#[test]
fn pins_prefixes_ranges_and_unsampled_optional_values_remain_valid() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
            Variable::new(vec![0, 1]).allowing_unassigned(),
            Variable::new(vec![0, 1]).with_initial(0),
        ])
        .unwrap(),
        ListProblem::new(
            vec![0, 1, 2, 3, 4],
            vec![
                ListEntity::new(vec![0, 1])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![0, 1, 2, 4]),
                ListEntity::new(vec![2]),
                ListEntity::new(vec![3]).with_pinned(true),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
    );
    for seed in 0..32 {
        for order in ORDERS {
            let mut c = config();
            c.order = order;
            c.search.seed = seed;
            c.search.minimum_ruined = 1;
            c.search.maximum_ruined = 1;
            c.search.max_attempts = 1;
            let r = run_both(&p, c, |b, l| {
                b.iter().flatten().map(|v| i64::from(*v)).sum::<i64>() + 10 * l[1].len() as i64
            });
            let a = r.best.unwrap().assignment;
            assert_eq!(a.basic().values()[0], Some(0));
            assert_eq!(a.lists().lists()[0][0], 0);
            assert_eq!(a.lists().lists()[2], vec![3]);
            let changed_basic = a
                .basic()
                .values()
                .iter()
                .zip(p.initial_assignment().basic().values())
                .filter(|(a, b)| a != b)
                .count();
            let changed_owners = p
                .lists()
                .values()
                .iter()
                .filter(|v| {
                    a.lists().location(v).map(|p| p.entity)
                        != p.initial_assignment().lists().location(v).map(|p| p.entity)
                })
                .count();
            assert!(changed_basic + changed_owners <= 1);
        }
    }
}
#[test]
fn all_empty_and_fully_pinned_components_stop_without_attempts() {
    for p in [
        MixedProblem::new(
            FiniteProblem::<i32>::new(vec![]).unwrap(),
            ListProblem::<i32>::new(vec![], vec![]).unwrap(),
        ),
        MixedProblem::new(
            FiniteProblem::new(vec![
                Variable::new(vec![0]).with_initial(0).with_pinned(true),
            ])
            .unwrap(),
            ListProblem::new(vec![0], vec![ListEntity::new(vec![0]).with_pinned(true)]).unwrap(),
        ),
    ] {
        let r = run_both(&p, config(), |_, _| 0);
        assert_eq!(r.termination, RuinRecreateTermination::NoMovableVariables);
        assert_eq!(r.evaluations, 1);
        assert_eq!(r.attempts, 0);
    }
}
#[test]
fn validation_and_skipped_work_precede_stream_factories() {
    let p = simple();
    let calls = Cell::new(0);
    let mut factory = || {
        calls.set(calls.get() + 1);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<i32, i32>::new(&mut b);
        Ok((b.build(), s))
    };
    let mut c = config();
    c.search.max_evaluations = 0;
    assert!(
        ruin_recreate_mixed_stream_search_from(&p, &mut factory, p.initial_assignment(), c.clone())
            .unwrap()
            .best
            .is_none()
    );
    c.search.minimum_ruined = 0;
    assert!(matches!(
        ruin_recreate_mixed_stream_search_from(&p, &mut factory, p.initial_assignment(), c),
        Err(RuinRecreateError::InvalidRuinRange)
    ));
    let token = CancellationToken::new();
    token.cancel();
    let mut c = config();
    c.search.cancellation = Some(&token);
    assert_eq!(
        ruin_recreate_mixed_stream_search_from(&p, &mut factory, p.initial_assignment(), c)
            .unwrap()
            .termination,
        RuinRecreateTermination::Cancelled
    );
    let incomplete = MixedAssignment::new(
        Assignment::new(vec![None]),
        p.initial_assignment().lists().clone(),
    );
    assert!(matches!(
        ruin_recreate_mixed_stream_search_from(&p, &mut factory, incomplete, config()),
        Err(RuinRecreateError::IncompleteInitialAssignment)
    ));
    assert_eq!(calls.get(), 0);
    let wrong = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(2, 2, 1);
    assert!(matches!(
        ruin_recreate_mixed_search_from(&p, &wrong, p.initial_assignment(), config()),
        Err(RuinRecreateError::MixedSearch(
            MixedLocalSearchError::Scoring(MixedScoringError::ModelShape { .. })
        ))
    ));
}
#[test]
fn factory_elapsed_time_stops_repairs_after_initial_score() {
    let p = simple();
    let mut c = config();
    c.search.max_time = Some(Duration::from_millis(1));
    let calls = Cell::new(0);
    let r = ruin_recreate_mixed_stream_search_from(
        &p,
        || {
            calls.set(calls.get() + 1);
            std::thread::sleep(Duration::from_millis(3));
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = MixedStreamSources::<i32, i32>::new(&mut b);
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        c,
    )
    .unwrap();
    assert_eq!(r.termination, RuinRecreateTermination::TimeLimit);
    assert_eq!(calls.get(), 1);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.attempts, 0);
    assert!(r.best.is_some());
}

#[test]
fn cancellation_retains_only_complete_joint_repairs() {
    for optional in [false, true] {
        for stream in [false, true] {
            let mut v = Variable::new(vec![0, 1]).with_initial(0);
            let mut l = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
            if optional {
                v = v.allowing_unassigned();
                l = l.allowing_unassigned();
            }
            let p = MixedProblem::new(FiniteProblem::new(vec![v]).unwrap(), l);
            let token = CancellationToken::new();
            let calls = Cell::new(0);
            let mut c = config();
            c.search.cancellation = Some(&token);
            c.search.assert_scores = false;
            let r = if stream {
                ruin_recreate_mixed_stream_search_from(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let s = MixedStreamSources::<i32, i32>::new(&mut b);
                        let all =
                            b.join_equal(s.basic.port(), s.lists.entities.port(), |_| 0, |_| 0)?;
                        b.score("assigned", &all, |((_, v), (_, l))| {
                            calls.set(calls.get() + 1);
                            if calls.get() == 2 {
                                token.cancel();
                            }
                            SimpleScore::new(-i64::from(v.is_some()) - l.len() as i64)
                        })?;
                        Ok((b.build(), s))
                    },
                    p.initial_assignment(),
                    c,
                )
                .unwrap()
            } else {
                let mut n = MixedConstraintNetwork::new(1, 1, 1);
                n.add_constraint(
                    "assigned",
                    vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
                    |f| {
                        calls.set(calls.get() + 1);
                        if calls.get() == 2 {
                            token.cancel();
                        }
                        let (MixedFact::Basic(v), MixedFact::ListEntity(l)) = (&f[0], &f[1]) else {
                            unreachable!()
                        };
                        SimpleScore::new(-i64::from(v.is_some()) - l.len() as i64)
                    },
                )
                .unwrap();
                ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c).unwrap()
            };
            assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
            assert_eq!(
                (r.evaluations, r.attempts, r.repair_move_evaluations),
                (2, 1, 0)
            );
            let best = r.best.unwrap();
            assert!(p.is_complete(&best.assignment));
            assert_eq!(best.score, SimpleScore::new(if optional { 0 } else { -2 }));
            if !optional {
                assert_eq!(best.assignment, p.initial_assignment());
            }
        }
    }
}

#[test]
fn graph_factory_errors_remain_typed_after_initialization() {
    let p = simple();
    let calls = Cell::new(0);
    let error = ruin_recreate_mixed_stream_search_from(
        &p,
        || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                return Err(GraphError::Dependency("repair factory".into()));
            }
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = MixedStreamSources::<i32, i32>::new(&mut b);
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        config(),
    )
    .unwrap_err();
    assert!(
        matches!(error,RuinRecreateError::MixedSearch(MixedLocalSearchError::StreamScoring(MixedStreamScoringError::Graph(GraphError::Dependency(ref message)))) if message=="repair factory")
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn seeded_joint_sampling_is_repeatable_across_placements_and_tabu_families() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]).with_initial(0),
            Variable::new(vec![0, 1]).with_initial(1),
        ])
        .unwrap(),
        ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    for acceptance in [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::MoveTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::ValueTabu {
            tenure: 2,
            aspiration: true,
        },
    ] {
        for order in ORDERS {
            for placement in [
                ConstructionPlacement::Sequential,
                ConstructionPlacement::Pooled,
                ConstructionPlacement::ValueQueue,
            ] {
                let mut c = config();
                c.order = order;
                c.search.acceptance = acceptance;
                c.search.placement = placement;
                c.search.minimum_ruined = 1;
                c.search.maximum_ruined = 3;
                c.search.max_attempts = 10;
                let score = |b: &[Option<i32>], l: &[Vec<i32>]| {
                    -l.iter()
                        .enumerate()
                        .map(|(i, l)| (l.len() as i64 - i64::from(b[i].unwrap_or(0))).pow(2))
                        .sum::<i64>()
                };
                let a = run_both(&p, c.clone(), score);
                let b = run_both(&p, c, score);
                assert_eq!(a, b);
                assert!(a.best.unwrap().score >= SimpleScore::new(-10));
            }
        }
    }
}
