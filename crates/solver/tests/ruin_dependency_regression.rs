//! Original dependency-preservation checks for reconstruction; no upstream credit.
use timetuck::*;

fn problem(optional: bool, pinned: bool) -> ListProblem<i32> {
    let entities = if pinned {
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2, 3]),
            ListEntity::new(vec![2, 3]).with_value_range(vec![1, 2, 3]),
        ]
    } else {
        vec![ListEntity::new(vec![0, 1, 2, 3])]
    };
    let p = ListProblem::new((0..4).collect(), entities)
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![], vec![2]])
        .unwrap();
    if optional { p.allowing_unassigned() } else { p }
}
// Independent transitive closure: fixed dependencies plus current predecessor
// edges must never reach back to their origin, including through missing values.
fn score(lists: &[Vec<i32>]) -> i64 {
    let mut reachable = [[false; 4]; 4];
    reachable[1][0] = true;
    reachable[3][2] = true;
    for list in lists {
        for pair in list.windows(2) {
            reachable[pair[1] as usize][pair[0] as usize] = true;
        }
    }
    for k in 0..4 {
        for i in 0..4 {
            for j in 0..4 {
                reachable[i][j] |= reachable[i][k] && reachable[k][j];
            }
        }
    }
    assert!(
        (0..4).all(|i| !reachable[i][i]),
        "cyclic repair reached scoring: {lists:?}"
    );
    // More assignments are better, but decreasing order is preferred. Without
    // structural filtering, the locally most attractive insertion makes a cycle.
    raw_score(lists)
}
fn raw_score(lists: &[Vec<i32>]) -> i64 {
    lists
        .iter()
        .flat_map(|list| list.iter().enumerate())
        .map(|(i, v)| 100 - i as i64 * i64::from(*v))
        .sum()
}
fn config(placement: ConstructionPlacement, budget: u64, seed: u64) -> RuinRecreateConfig<'static> {
    RuinRecreateConfig {
        minimum_ruined: 1,
        maximum_ruined: 4,
        max_attempts: 4,
        max_evaluations: budget,
        seed,
        placement,
        assert_scores: true,
        ..Default::default()
    }
}
fn list_network(count: usize) -> ListConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ListConstraintNetwork::new(count, 4);
    n.add_constraint(
        "dependency-safe preference",
        (0..count).map(ListScope::Entity).collect(),
        |facts| {
            let lists = facts
                .iter()
                .map(|f| {
                    let ListFact::Entity(list) = f else { panic!() };
                    list.to_vec()
                })
                .collect::<Vec<_>>();
            SimpleScore::new(score(&lists))
        },
    )
    .unwrap();
    n
}
fn list_graph(
    count: usize,
) -> Result<(StreamGraph<'static, SimpleScore>, ListStreamSources<i32>), GraphError> {
    list_graph_checked(count, true)
}
fn list_graph_checked(
    count: usize,
    check: bool,
) -> Result<(StreamGraph<'static, SimpleScore>, ListStreamSources<i32>), GraphError> {
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    let all = b.collect_list(sources.entities.port(), |row| row.clone())?;
    b.score("dependency-safe preference", &all, move |rows| {
        let mut lists = vec![vec![]; count];
        for (i, list) in rows.iter() {
            lists[*i] = list.clone();
        }
        SimpleScore::new(if check {
            score(&lists)
        } else {
            raw_score(&lists)
        })
    })?;
    Ok((b.build(), sources))
}
#[test]
fn list_repairs_keep_dependencies_for_every_placement_and_budget_prefix() {
    for optional in [false, true] {
        for pinned in [false, true] {
            let p = problem(optional, pinned);
            let original = p.initial_assignment();
            for placement in [
                ConstructionPlacement::Sequential,
                ConstructionPlacement::ValueQueue,
                ConstructionPlacement::Pooled,
            ] {
                for budget in [1, 2, 3, 5, 9, 17, 100] {
                    for seed in [0, 19] {
                        let c = config(placement, budget, seed);
                        let n = list_network(p.entities().len());
                        let a = ruin_recreate_list_search_from(&p, &n, original.clone(), c.clone())
                            .unwrap();
                        let b = ruin_recreate_list_stream_search_from(
                            &p,
                            || list_graph(p.entities().len()),
                            original.clone(),
                            c,
                        )
                        .unwrap();
                        assert_eq!(a.best, b.best);
                        assert_eq!(work(&a), work(&b));
                        let best = a.best.unwrap();
                        p.validate(&best.assignment).unwrap();
                        assert!(p.is_complete(&best.assignment));
                        assert_eq!(best.score, SimpleScore::new(score(best.assignment.lists())));
                        assert_eq!(p.initial_assignment(), original);
                    }
                }
            }
        }
    }
}

fn work<T>(r: &RuinRecreateOutcome<T>) -> (u64, u64, u64, u64, u64, RuinRecreateTermination) {
    (
        r.evaluations,
        r.repair_move_evaluations,
        r.attempts,
        r.complete_reconstructions,
        r.accepted_moves,
        r.termination,
    )
}

fn mixed_network(count: usize) -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(1, count, 4);
    n.add_constraint("basic", vec![MixedScope::Basic(0)], |facts| {
        let MixedFact::Basic(value) = facts[0] else {
            panic!()
        };
        SimpleScore::new(i64::from(value.copied().unwrap_or(0)))
    })
    .unwrap();
    n.add_constraint(
        "dependency-safe preference",
        (0..count).map(MixedScope::ListEntity).collect(),
        |facts| {
            let lists = facts
                .iter()
                .map(|f| {
                    let MixedFact::ListEntity(list) = f else {
                        panic!()
                    };
                    list.to_vec()
                })
                .collect::<Vec<_>>();
            SimpleScore::new(score(&lists))
        },
    )
    .unwrap();
    n
}
fn mixed_graph(
    count: usize,
) -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        MixedStreamSources<i32, i32>,
    ),
    GraphError,
> {
    mixed_graph_checked(count, true)
}
fn mixed_graph_checked(
    count: usize,
    check: bool,
) -> Result<
    (
        StreamGraph<'static, SimpleScore>,
        MixedStreamSources<i32, i32>,
    ),
    GraphError,
> {
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    b.score("basic", sources.basic.port(), |(_, value)| {
        SimpleScore::new(i64::from(value.unwrap_or(0)))
    })?;
    let all = b.collect_list(sources.lists.entities.port(), |row| row.clone())?;
    b.score("dependency-safe preference", &all, move |rows| {
        let mut lists = vec![vec![]; count];
        for (i, list) in rows.iter() {
            lists[*i] = list.clone();
        }
        SimpleScore::new(if check {
            score(&lists)
        } else {
            raw_score(&lists)
        })
    })?;
    Ok((b.build(), sources))
}
#[test]
fn mixed_repairs_preserve_dependencies_in_both_component_orders() {
    for optional in [false, true] {
        for pinned in [false, true] {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
                problem(optional, pinned),
            );
            let original = p.initial_assignment();
            for order in [
                MixedConstructionOrder::BasicThenLists,
                MixedConstructionOrder::ListsThenBasic,
            ] {
                for placement in [
                    ConstructionPlacement::Sequential,
                    ConstructionPlacement::ValueQueue,
                    ConstructionPlacement::Pooled,
                ] {
                    for budget in [1, 2, 3, 5, 9, 17, 100] {
                        let c = MixedRuinRecreateConfig {
                            search: config(placement, budget, 19),
                            order,
                        };
                        let n = mixed_network(p.lists().entities().len());
                        let a =
                            ruin_recreate_mixed_search_from(&p, &n, original.clone(), c.clone())
                                .unwrap();
                        let b = ruin_recreate_mixed_stream_search_from(
                            &p,
                            || mixed_graph(p.lists().entities().len()),
                            original.clone(),
                            c,
                        )
                        .unwrap();
                        assert_eq!(a.best, b.best);
                        assert_eq!(work(&a), work(&b));
                        let best = a.best.unwrap();
                        p.validate(&best.assignment).unwrap();
                        assert!(p.is_complete(&best.assignment));
                        assert_eq!(
                            best.score,
                            SimpleScore::new(
                                score(best.assignment.lists().lists())
                                    + i64::from(best.assignment.basic().get(0).unwrap().unwrap())
                            )
                        );
                        assert_eq!(p.initial_assignment(), original);
                    }
                }
            }
        }
    }
}
#[test]
fn repair_keeps_edges_through_unsampled_missing_values() {
    let p = ListProblem::new((0..4).collect(), vec![ListEntity::new(vec![0, 2, 3])])
        .unwrap()
        .allowing_unassigned()
        .with_dependencies(vec![vec![], vec![0], vec![1], vec![2]])
        .unwrap();
    let n = {
        let mut n = ListConstraintNetwork::new(1, 4);
        n.add_constraint("prefer decreasing", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(list) = facts[0] else {
                panic!()
            };
            assert!(!list.contains(&1), "unsampled missing value was assigned");
            assert!(
                list.windows(2).all(|w| w[0] < w[1]),
                "fixed path through missing value was lost"
            );
            SimpleScore::new(
                list.iter()
                    .enumerate()
                    .map(|(i, v)| 100 - i as i64 * i64::from(*v))
                    .sum(),
            )
        })
        .unwrap();
        n
    };
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let c = RuinRecreateConfig {
            ruin_candidates: Some(&[0, 2, 3]),
            ..config(placement, 100, 19)
        };
        let r = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c).unwrap();
        let best = r.best.unwrap();
        assert_eq!(best.assignment, p.initial_assignment());
        assert_eq!(r.attempts, 4);
        assert_eq!(r.complete_reconstructions, 4);
    }
}

#[test]
fn hybrid_fine_neighborhoods_preserve_dependencies_across_scoped_fresh_and_retained_search() {
    for optional in [false, true] {
        for pinned in [false, true] {
            let lp = problem(optional, pinned);
            let mp = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
                lp.clone(),
            );
            for seed in [0, 19, 37] {
                for budget in [1, 7, 50] {
                    for family in 0..4 {
                        let extra = [(
                            RepairNeighborhood::Standard(Neighborhood::SubListChange(
                                SubListChangeConfig::new(2, 3).unwrap(),
                            )),
                            1,
                        )];
                        let mixed_extra = [(
                            RepairNeighborhood::Mixed(MixedNeighborhood {
                                joint_moves: true,
                                ..Default::default()
                            }),
                            1,
                        )];
                        let mut c = config(ConstructionPlacement::Pooled, budget, seed);
                        c.max_attempts = 8;
                        c.move_weights = match family {
                            0 => RepairMoveWeights {
                                change: 1,
                                swap: 0,
                                repair: 0,
                            },
                            1 => RepairMoveWeights {
                                change: 0,
                                swap: 1,
                                repair: 0,
                            },
                            _ => RepairMoveWeights {
                                change: 1,
                                swap: 1,
                                repair: 2,
                            },
                        };
                        if family == 3 {
                            c.additional_neighborhoods = &extra;
                        }
                        let n = list_network(lp.entities().len());
                        let scoped = ruin_recreate_list_search_from(
                            &lp,
                            &n,
                            lp.initial_assignment(),
                            c.clone(),
                        )
                        .unwrap();
                        let fresh = ruin_recreate_list_stream_search_from(
                            &lp,
                            || list_graph(lp.entities().len()),
                            lp.initial_assignment(),
                            c.clone(),
                        )
                        .unwrap();
                        assert_eq!(scoped.best, fresh.best);
                        assert_eq!(work(&scoped), work(&fresh));
                        assert_eq!(scoped.fine_move_attempts, fresh.fine_move_attempts);
                        assert_eq!(scoped.fine_move_evaluations, fresh.fine_move_evaluations);
                        let (g, s) = list_graph_checked(lp.entities().len(), false).unwrap();
                        let mut d =
                            ListStreamScoreDirector::new(&lp, g, s, lp.initial_assignment())
                                .unwrap();
                        let retained = d
                            .run_solver_with_events(
                                SolverSequence {
                                    phases: vec![SolverPhase::RuinRecreate(c.clone())],
                                    ..Default::default()
                                },
                                |event| {
                                    if let SolverEvent::Best(update) = event {
                                        lp.validate(update.assignment).unwrap();
                                        assert_eq!(
                                            update.score,
                                            SimpleScore::new(score(update.assignment.lists()))
                                        );
                                    }
                                },
                            )
                            .unwrap();
                        let best = scoped.best.unwrap();
                        assert_eq!(retained.assignment, best.assignment);
                        assert_eq!(retained.score, Some(best.score));
                        assert_eq!(retained.work.evaluations, scoped.evaluations);
                        assert_eq!(retained.work.fine_move_attempts, scoped.fine_move_attempts);
                        assert_eq!(retained.work.repair_attempts, scoped.attempts);
                        assert_eq!(
                            retained.work.move_evaluations,
                            scoped.fine_move_evaluations + scoped.repair_move_evaluations
                        );
                        d.assert_consistent().unwrap();
                        if family == 3 {
                            c.additional_neighborhoods = &mixed_extra;
                        }
                        for order in [
                            MixedConstructionOrder::BasicThenLists,
                            MixedConstructionOrder::ListsThenBasic,
                        ] {
                            let c = MixedRuinRecreateConfig {
                                search: c.clone(),
                                order,
                            };
                            let n = mixed_network(mp.lists().entities().len());
                            let scoped = ruin_recreate_mixed_search_from(
                                &mp,
                                &n,
                                mp.initial_assignment(),
                                c.clone(),
                            )
                            .unwrap();
                            let fresh = ruin_recreate_mixed_stream_search_from(
                                &mp,
                                || mixed_graph(mp.lists().entities().len()),
                                mp.initial_assignment(),
                                c.clone(),
                            )
                            .unwrap();
                            assert_eq!(scoped.best, fresh.best);
                            assert_eq!(work(&scoped), work(&fresh));
                            assert_eq!(scoped.fine_move_attempts, fresh.fine_move_attempts);
                            assert_eq!(scoped.fine_move_evaluations, fresh.fine_move_evaluations);
                            let (g, s) =
                                mixed_graph_checked(mp.lists().entities().len(), false).unwrap();
                            let mut d =
                                MixedStreamScoreDirector::new(&mp, g, s, mp.initial_assignment())
                                    .unwrap();
                            let retained = d
                                .run_solver_with_events(
                                    MixedSolverSequence {
                                        phases: vec![MixedSolverPhase::RuinRecreate(c)],
                                        ..Default::default()
                                    },
                                    |event| {
                                        if let SolverEvent::Best(update) = event {
                                            mp.validate(update.assignment).unwrap();
                                            let basic =
                                                update.assignment.basic().values()[0].unwrap();
                                            assert_eq!(
                                                update.score,
                                                SimpleScore::new(
                                                    i64::from(basic)
                                                        + score(update.assignment.lists().lists())
                                                )
                                            );
                                        }
                                    },
                                )
                                .unwrap();
                            let best = scoped.best.unwrap();
                            assert_eq!(retained.assignment, best.assignment);
                            assert_eq!(retained.score, Some(best.score));
                            assert_eq!(retained.work.evaluations, scoped.evaluations);
                            assert_eq!(retained.work.fine_move_attempts, scoped.fine_move_attempts);
                            assert_eq!(retained.work.repair_attempts, scoped.attempts);
                            assert_eq!(
                                retained.work.move_evaluations,
                                scoped.fine_move_evaluations + scoped.repair_move_evaluations
                            );
                            d.assert_consistent().unwrap();
                        }
                    }
                }
            }
        }
    }
}
