//! Original list-stream binding/search regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn random(s: &mut u64) -> usize {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*s >> 32) as usize
}
fn graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("loads", s.entities.port(), |(_, v)| {
        SimpleScore::new(-(v.len() as i64).pow(2))
    })
    .unwrap();
    b.score("views", s.values.port(), |r| {
        SimpleScore::new(
            r.position.map_or(-7, |p| {
                i64::from(r.value) * (p.entity as i64 + 1) + p.index as i64
            }) + i64::from(r.previous.unwrap_or(0))
                - i64::from(r.next.unwrap_or(0)),
        )
    })
    .unwrap();
    let joined = b
        .join_equal(
            s.entities.port(),
            s.values.port(),
            |(e, _)| Some(*e),
            |r| r.position.map(|p| p.entity),
        )
        .unwrap();
    b.score("joined", &joined, |((_, v), r)| {
        SimpleScore::new(-(((r.range_index + 1) * v.len()) as i64))
    })
    .unwrap();
    let groups = b
        .group_collect_with::<_, _, _, _, CountCollector>(s.values.port(), |r| {
            (r.position.map(|p| p.entity), ())
        })
        .unwrap();
    b.score("groups", &groups, |(_, n)| {
        SimpleScore::new((*n as i64).pow(2))
    })
    .unwrap();
    (b.build(), s)
}
fn reference(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
) -> (SimpleScore, Vec<ListValueRow<i32>>) {
    let mut score = 0;
    let mut groups = BTreeMap::<Option<usize>, i64>::new();
    for list in a.lists() {
        score -= (list.len() as i64).pow(2);
    }
    let mut rows = vec![];
    for (range_index, &value) in p.values().iter().enumerate() {
        let mut row = ListValueRow {
            range_index,
            value,
            position: None,
            previous: None,
            next: None,
        };
        for (e, list) in a.lists().iter().enumerate() {
            for (i, &v) in list.iter().enumerate() {
                if v != value {
                    continue;
                }
                row.position = Some(ListPosition {
                    entity: e,
                    index: i,
                });
                row.previous = if i == 0 { None } else { Some(list[i - 1]) };
                row.next = list.get(i + 1).copied();
                score += i64::from(value) * (e as i64 + 1) + i as i64
                    - ((range_index + 1) * list.len()) as i64;
            }
        }
        if row.position.is_none() {
            score -= 7;
        }
        score += i64::from(row.previous.unwrap_or(0)) - i64::from(row.next.unwrap_or(0));
        *groups.entry(row.position.map(|p| p.entity)).or_default() += 1;
        rows.push(row);
    }
    score += groups.values().map(|n| n * n).sum::<i64>();
    let missing = if p.allows_unassigned() {
        0
    } else {
        rows.iter().filter(|r| r.position.is_none()).count()
    };
    (
        SimpleScore::new(score).with_structural(-(missing as i64)),
        rows,
    )
}
fn inspect(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    g: &StreamGraph<'_, SimpleScore>,
    s: &ListStreamSources<i32>,
    score: SimpleScore,
) {
    let (expected, rows) = reference(p, a);
    assert_eq!(score, expected);
    assert_eq!(
        g.source_rows(&s.values)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        rows
    );
    assert_eq!(
        g.source_rows(&s.entities)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>(),
        a.lists().iter().cloned().enumerate().collect::<Vec<_>>()
    );
}
#[test]
fn seeded_composites_restore_lists_views_joins_and_groups() {
    let p = ListProblem::new(
        vec![40, 10, 90, 20, 70, 30],
        vec![
            ListEntity::new(vec![40, 10]),
            ListEntity::new(vec![90]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let (g, s) = graph();
    let inspect_source = s.clone();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let mut seed = 20261024;
    for step in 0..1200 {
        let base = d.assignment().clone();
        let mut expected = base.clone();
        let mut parts = vec![];
        for _ in 0..1 + random(&mut seed) % 3 {
            let moves = list_moves(&p, &expected, Neighborhood::ChangeAndSwap)
                .unwrap()
                .collect::<Vec<_>>();
            let movement = moves[random(&mut seed) % moves.len()].clone();
            p.apply(&mut expected, &movement).unwrap().commit();
            parts.push(movement);
        }
        let movement = ListMove::Composite(parts);
        let tx = d.apply(&movement).unwrap();
        assert_eq!(tx.assignment(), &expected);
        inspect(&p, tx.assignment(), tx.graph(), &inspect_source, tx.score());
        if step % 3 == 0 {
            drop(tx);
            assert_eq!(d.assignment(), &base);
        } else {
            tx.commit();
        }
        inspect(&p, d.assignment(), d.graph(), &inspect_source, d.score());
        if step % 40 == 0 {
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn required_unassignment_span_and_composite_final_state_are_bound() {
    let p = ListProblem::new(
        vec![10, 30, 20],
        vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![30])],
    )
    .unwrap();
    let (g, s) = graph();
    let saved = s.clone();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let movement = ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 2,
        },
        ListMove::Assign {
            value: 20,
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    ]);
    let t = d.apply(&movement).unwrap();
    inspect(&p, t.assignment(), t.graph(), &saved, t.score());
    assert_eq!(t.score().structural(), -1);
    t.commit();
    assert_eq!(
        d.graph().source_rows(&saved.values).unwrap()[&0].position,
        None
    );
    d.assert_consistent().unwrap();
}
#[test]
fn unrelated_rows_and_constant_facts_do_not_replay_on_moves_or_commit() {
    let p = ListProblem::new(
        vec![10, 20, 30, 40],
        vec![
            ListEntity::new(vec![10, 20]),
            ListEntity::new(vec![30]),
            ListEntity::new(vec![40]),
        ],
    )
    .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    b.score("values", s.values.port(), |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let constants = b.source::<usize, i32>();
    b.score("constant", constants.port(), |_| {
        calls.set(calls.get() + 100);
        SimpleScore::new(5)
    })
    .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&constants, TupleChange::put(0, 8)).unwrap();
    t.commit().unwrap();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(calls.get(), 104);
    let m = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 1,
        },
    };
    let t = d.apply(&m).unwrap();
    assert_eq!(t.constraint_evaluations(), 3);
    assert_eq!(calls.get(), 107);
    drop(t);
    assert_eq!(calls.get(), 107);
    let t = d.apply(&m).unwrap();
    t.commit();
    assert_eq!(calls.get(), 110);
    assert_eq!(d.score(), SimpleScore::new(5));
}
#[test]
fn failures_after_entity_updates_restore_every_source_and_assignment() {
    let p = ListProblem::new(
        vec![10, 20],
        vec![ListEntity::new(vec![10]), ListEntity::new(vec![20])],
    )
    .unwrap();
    for panic in [false, true] {
        let enabled = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut b);
        let inspect = s.clone();
        b.score("entity", s.entities.port(), |(_, v)| {
            SimpleScore::new(v.len() as i64)
        })
        .unwrap();
        b.score("value", s.values.port(), |r| {
            if enabled.get() && r.value == 20 {
                assert!(!panic, "injected value failure");
                return SimpleScore::ZERO.with_structural(-1);
            }
            SimpleScore::new(r.position.unwrap().index as i64)
        })
        .unwrap();
        let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let old = d.score();
        enabled.set(true);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let t = d.apply(&ListMove::Change {
                source: ListPosition {
                    entity: 1,
                    index: 0,
                },
                destination: ListPosition {
                    entity: 0,
                    index: 1,
                },
            })?;
            t.commit();
            Ok::<(), ListStreamScoringError>(())
        }));
        if panic {
            assert!(outcome.is_err());
        } else {
            assert!(outcome.unwrap().is_err());
        }
        enabled.set(false);
        assert_eq!(d.assignment(), &p.initial_assignment());
        assert_eq!(d.score(), old);
        assert_eq!(
            d.graph().source_rows(&inspect.entities).unwrap()[&1],
            (1, vec![20])
        );
        assert_eq!(
            d.graph().source_rows(&inspect.values).unwrap()[&1].position,
            Some(ListPosition {
                entity: 1,
                index: 0
            })
        );
        d.assert_consistent().unwrap();
    }
}
fn route_score(a: &ListAssignment<i32>) -> HardSoftScore {
    let mut distance = 0;
    let mut overflow = 0;
    for (e, r) in a.lists().iter().enumerate() {
        let points = std::iter::once(e as i32 * 10)
            .chain(r.iter().copied())
            .chain(std::iter::once(e as i32 * 10))
            .collect::<Vec<_>>();
        distance += points
            .windows(2)
            .map(|w| i64::from((w[0] - w[1]).abs()))
            .sum::<i64>();
        overflow += r.len().saturating_sub(3);
    }
    HardSoftScore::new(-(overflow as i64), -distance)
}
fn route_graph() -> (StreamGraph<'static, HardSoftScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    b.score("capacity", s.entities.port(), |(_, r)| {
        HardSoftScore::new(-(r.len().saturating_sub(3) as i64), 0)
    })
    .unwrap();
    let assigned = b.filter(s.values.port(), |r| r.position.is_some()).unwrap();
    b.score("legs", &assigned, |r| {
        let depot = r.position.unwrap().entity as i32 * 10;
        let before = r.previous.unwrap_or(depot);
        let distance = i64::from((r.value - before).abs())
            + if r.next.is_none() {
                i64::from((r.value - depot).abs())
            } else {
                0
            };
        HardSoftScore::new(0, -distance)
    })
    .unwrap();
    (b.build(), s)
}
fn route_network(p: &ListProblem<i32>) -> ListConstraintNetwork<'static, i32, HardSoftScore> {
    let mut n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
    n.add_constraint(
        "full",
        (0..p.entities().len()).map(ListScope::Entity).collect(),
        |f| {
            route_score(&ListAssignment::new(
                f.iter()
                    .map(|f| match f {
                        ListFact::Entity(v) => v.to_vec(),
                        _ => unreachable!(),
                    })
                    .collect(),
            ))
        },
    )
    .unwrap();
    n
}
#[test]
fn construction_and_search_match_full_scoring_across_policies_and_budgets() {
    let p = ListProblem::new(
        vec![1, 9, 2, 8, 3, 7],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let n = route_network(&p);
    for pick_early in [
        ConstructionPickEarly::Never,
        ConstructionPickEarly::FirstNonDeterioratingScore,
        ConstructionPickEarly::FirstFeasibleScore,
        ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard,
    ] {
        for max_evaluations in [0, 1, 2, 9, 50, 1000] {
            let config = ConstructionConfig {
                max_steps: None,
                optional: timetuck::OptionalConstruction::Preserve,
                cancellation: None,
                max_move_evaluations: None,
                max_time: None,
                max_evaluations,
                pick_early,
                assert_scores: true,
            };
            let expected = greedy_list_construction(&p, &n, config).unwrap();
            let (g, s) = route_graph();
            let actual = greedy_list_stream_construction(&p, g, s, config).unwrap();
            assert_eq!(actual.assignment, expected.assignment);
            assert_eq!(actual.score, expected.score);
            assert_eq!(
                (
                    actual.evaluations,
                    actual.termination,
                    actual.complete,
                    actual.assigned_values
                ),
                (
                    expected.evaluations,
                    expected.termination,
                    expected.complete,
                    expected.assigned_values
                )
            );
        }
    }
    let initial = ListAssignment::new(vec![vec![9, 1, 8], vec![2, 7, 3]]);
    let extras = [
        Acceptance::LateAcceptance {
            history_size: 2,
            hill_climbing: true,
        },
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
    ];
    for (acceptance, additional_acceptance) in [
        Acceptance::DiversifiedLateAcceptance { history_size: 3 },
        Acceptance::SizedVariableTabu {
            size: TabuSize::ratio(0.5).unwrap(),
            aspiration: true,
        },
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::ValueTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::FadingValueTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::MoveTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::FadingMoveTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::FadingVariableTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 20261229,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: HardSoftScore::new(1, 20),
            seed: 73,
            cooling_steps: 10,
        },
    ]
    .into_iter()
    .flat_map(|a| [(a, &[][..]), (a, &extras[..])])
    {
        for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
            for max_evaluations in [0, 1, 2, 37, 1000] {
                for shuffled in [false, true] {
                    let config = LocalSearchConfig {
                        max_evaluations,
                        max_steps: 6,
                        acceptance,
                        additional_acceptance,
                        selection,
                        neighborhood: Neighborhood::ChangeAndSwap,
                        move_order: if shuffled {
                            MoveOrder::ShuffledVariables { seed: 15 }
                        } else {
                            MoveOrder::Original
                        },
                        rejected_moves: RejectedMovePolicy::BestScore,
                        assert_scores: true,
                        ..Default::default()
                    };
                    let expected = list_local_search_from(&p, &n, initial.clone(), config).unwrap();
                    let (g, s) = route_graph();
                    let actual =
                        list_stream_local_search_from(&p, g, s, initial.clone(), config).unwrap();
                    assert_eq!(actual.best, expected.best);
                    assert_eq!(
                        (
                            actual.evaluations,
                            actual.accepted_moves,
                            actual.completed_steps,
                            actual.termination
                        ),
                        (
                            expected.evaluations,
                            expected.accepted_moves,
                            expected.completed_steps,
                            expected.termination
                        )
                    );
                    if let Some(best) = actual.best {
                        assert_eq!(best.score, route_score(&best.assignment));
                    }
                }
            }
        }
    }
}
#[test]
fn model_validation_precedes_callbacks_and_zero_budgets_preserve_validation() {
    let p = ListProblem::new(
        vec![10, 20, 30],
        vec![
            ListEntity::new(vec![10])
                .with_pinned_prefix(1)
                .with_value_range(vec![10, 30]),
            ListEntity::new(vec![20]).with_pinned(true),
        ],
    )
    .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    b.score("rows", s.values.port(), |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    for movement in [
        ListMove::Assign {
            value: 30,
            destination: ListPosition {
                entity: 1,
                index: 1,
            },
        },
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 999,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
        ListMove::Composite(vec![
            ListMove::Assign {
                value: 30,
                destination: ListPosition {
                    entity: 0,
                    index: 1,
                },
            },
            ListMove::Assign {
                value: 30,
                destination: ListPosition {
                    entity: 0,
                    index: 0,
                },
            },
        ]),
    ] {
        assert!(d.apply(&movement).is_err());
        assert_eq!(calls.get(), 3);
        assert_eq!(d.assignment(), &p.initial_assignment());
    }
    let zero = LocalSearchConfig {
        max_evaluations: 0,
        ..Default::default()
    };
    let (g, s) = graph();
    assert!(list_stream_local_search_from(&p, g, s, p.initial_assignment(), zero).is_err());
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = ListStreamSources::<i32>::new(&mut other);
    let (g, _) = graph();
    assert!(
        greedy_list_stream_construction(
            &p,
            g,
            foreign,
            ConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        )
        .is_err()
    );
    for entity_source in [false, true] {
        let (mut g, s) = graph();
        let mut t = g.transaction();
        if entity_source {
            t.apply(&s.entities, TupleChange::put(0, (0, vec![])))
                .unwrap();
        } else {
            t.apply(
                &s.values,
                TupleChange::put(
                    0,
                    ListValueRow {
                        range_index: 0,
                        value: 10,
                        position: None,
                        previous: None,
                        next: None,
                    },
                ),
            )
            .unwrap();
        }
        t.commit().unwrap();
        assert!(matches!(
            greedy_list_stream_construction(
                &p,
                g,
                s,
                ConstructionConfig {
                    max_evaluations: 0,
                    ..Default::default()
                }
            ),
            Err(ListStreamScoringError::PlanningSourceNotEmpty)
        ));
    }
}
#[test]
fn empty_optional_impossible_and_pinned_range_models_have_matching_results() {
    let models = [
        ListProblem::new(vec![], vec![]).unwrap(),
        ListProblem::new(vec![1], vec![]).unwrap(),
        ListProblem::new(vec![1], vec![])
            .unwrap()
            .allowing_unassigned(),
        ListProblem::new(
            vec![1, 2, 8, 9],
            vec![
                ListEntity::new(vec![1])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![1, 2]),
                ListEntity::new(vec![9]).with_value_range(vec![8, 9]),
            ],
        )
        .unwrap(),
        ListProblem::new(
            vec![1, 2, 8, 9],
            vec![
                ListEntity::new(vec![1]).with_pinned(true),
                ListEntity::new(vec![9]).with_value_range(vec![8, 9]),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
    ];
    for p in models {
        let n = route_network(&p);
        let config = ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        };
        let expected = greedy_list_construction(&p, &n, config).unwrap();
        let (g, s) = route_graph();
        let actual = greedy_list_stream_construction(&p, g, s, config).unwrap();
        assert_eq!(actual.assignment, expected.assignment);
        assert_eq!(actual.score, expected.score);
        assert_eq!(actual.termination, expected.termination);
        if actual.complete {
            let config = LocalSearchConfig {
                max_evaluations: 1000,
                max_steps: 8,
                assert_scores: true,
                ..Default::default()
            };
            let expected =
                list_local_search_from(&p, &n, actual.assignment.clone(), config).unwrap();
            let (g, s) = route_graph();
            let actual =
                list_stream_local_search_from(&p, g, s, actual.assignment, config).unwrap();
            assert_eq!(actual.best, expected.best);
            assert_eq!(actual.termination, expected.termination);
        }
    }
}
#[test]
fn search_counters_exclude_diagnostics_and_winner_installation() {
    let p = ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![])]).unwrap();
    for diagnostic in [false, true] {
        let calls = Cell::new(0u128);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut b);
        b.score("lists", s.entities.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v.len() as i64)
        })
        .unwrap();
        let r = greedy_list_stream_construction(
            &p,
            b.build(),
            s,
            ConstructionConfig {
                max_evaluations: 3,
                assert_scores: diagnostic,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.evaluations, 3);
        assert_eq!(r.constraint_evaluations, 3);
        assert!(r.complete);
        assert_eq!(r.termination, ConstructionTermination::EvaluationLimit);
        if !diagnostic {
            assert_eq!(calls.get(), 3);
        } else {
            assert!(calls.get() > 3);
        }
    }
}

#[derive(Debug)]
struct Fragile(i32, std::rc::Rc<Cell<Option<usize>>>);
impl PartialEq for Fragile {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Fragile {}
impl Clone for Fragile {
    fn clone(&self) -> Self {
        if let Some(n) = self.1.get() {
            assert!(n > 0, "clone failure");
            self.1.set(Some(n - 1));
        }
        Self(self.0, self.1.clone())
    }
}
#[test]
fn value_clone_failures_restore_model_and_both_planning_sources() {
    let mut failed = 0;
    let mut succeeded = 0;
    for budget in 0..160 {
        let counter = std::rc::Rc::new(Cell::new(None));
        let values = (0..4)
            .map(|i| Fragile(i, counter.clone()))
            .collect::<Vec<_>>();
        let p = ListProblem::new(
            values.clone(),
            vec![
                ListEntity::new(values[..2].to_vec()),
                ListEntity::new(values[2..].to_vec()),
            ],
        )
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut b);
        let inspect = s.clone();
        b.score("entities", s.entities.port(), |(_, v)| {
            SimpleScore::new(v.len() as i64)
        })
        .unwrap();
        b.score("values", s.values.port(), |r| {
            SimpleScore::new(r.position.unwrap().index as i64)
        })
        .unwrap();
        let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let old = d.score();
        counter.set(Some(budget));
        let result = catch_unwind(AssertUnwindSafe(|| {
            d.apply(&ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 0,
                },
                destination: ListPosition {
                    entity: 1,
                    index: 1,
                },
            })
            .unwrap()
            .commit();
        }));
        counter.set(None);
        let expected = if result.is_ok() {
            succeeded += 1;
            vec![vec![1], vec![2, 0, 3]]
        } else {
            failed += 1;
            assert_eq!(d.score(), old);
            vec![vec![0, 1], vec![2, 3]]
        };
        assert_eq!(
            d.assignment()
                .lists()
                .iter()
                .map(|r| r.iter().map(|v| v.0).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            d.graph()
                .source_rows(&inspect.entities)
                .unwrap()
                .values()
                .map(|(_, r)| r.iter().map(|v| v.0).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected
        );
        for (e, row) in expected.iter().enumerate() {
            for (i, v) in row.iter().enumerate() {
                let actual = &d.graph().source_rows(&inspect.values).unwrap()[&(*v as usize)];
                assert_eq!(
                    actual.position,
                    Some(ListPosition {
                        entity: e,
                        index: i
                    })
                );
                assert_eq!(
                    actual.previous.as_ref().map(|v| v.0),
                    i.checked_sub(1).map(|j| row[j])
                );
                assert_eq!(actual.next.as_ref().map(|v| v.0), row.get(i + 1).copied());
            }
        }
        d.assert_consistent().unwrap();
    }
    assert!(failed > 20 && succeeded > 0);
}
