//! Original pin, reconstruction-reference and resource-control checks.
use std::{cell::Cell, time::Duration};
use timetuck::*;
fn config() -> RuinRecreateConfig<'static> {
    RuinRecreateConfig {
        minimum_ruined: usize::MAX,
        maximum_ruined: usize::MAX,
        max_attempts: 3,
        seed: 917,
        assert_scores: true,
        ..Default::default()
    }
}
fn run_both(
    p: &ListProblem<i32>,
    c: RuinRecreateConfig,
    score: impl Fn(&[Vec<i32>]) -> i64,
) -> Vec<ListRuinRecreateResult<i32, SimpleScore>> {
    let count = p.entities().len();
    let mut n = ListConstraintNetwork::new(count, p.values().len());
    n.add_constraint("all", (0..count).map(ListScope::Entity).collect(), |v| {
        SimpleScore::new(score(
            &v.iter()
                .map(|v| {
                    let ListFact::Entity(l) = v else {
                        unreachable!()
                    };
                    l.to_vec()
                })
                .collect::<Vec<_>>(),
        ))
    })
    .unwrap();
    let a = ruin_recreate_list_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
    let b = ruin_recreate_list_stream_search_from(
        p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            let lists = b.collect_list(s.entities.port(), |row| row.clone())?;
            b.score("all", &lists, |rows| {
                let mut all = vec![Vec::new(); count];
                for (i, v) in rows.iter() {
                    all[*i] = v.clone();
                }
                SimpleScore::new(score(&all))
            })?;
            Ok((b.build(), s))
        },
        p.initial_assignment(),
        c,
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
    vec![a, b]
}
fn reference(
    p: &ListProblem<i32>,
    budget: u64,
    score: impl Fn(&[Vec<i32>]) -> i64,
) -> Option<ScoredListAssignment<i32, SimpleScore>> {
    if budget == 0 {
        return None;
    }
    let mut best = p.initial_assignment().lists().to_vec();
    let mut current = best.clone();
    let mut used = 1;
    for _ in 0..3 {
        if used == budget {
            break;
        }
        let mut trial: Vec<_> = p
            .entities()
            .iter()
            .enumerate()
            .map(|(e, m)| {
                if m.is_pinned() {
                    current[e].clone()
                } else {
                    current[e][..m.pinned_prefix()].to_vec()
                }
            })
            .collect();
        used += 1;
        for v in p.values() {
            if trial.iter().any(|l| l.contains(v)) {
                continue;
            }
            let before = score(&trial);
            let mut winner = None;
            'positions: for (e, m) in p.entities().iter().enumerate() {
                if m.is_pinned() || m.value_range().is_some_and(|r| !r.contains(v)) {
                    continue;
                }
                for index in m.pinned_prefix()..=trial[e].len() {
                    if used == budget {
                        break 'positions;
                    }
                    let mut candidate = trial.clone();
                    candidate[e].insert(index, *v);
                    let s = score(&candidate);
                    used += 1;
                    if (!p.allows_unassigned() || s >= before)
                        && winner.as_ref().is_none_or(|(_, old)| s > *old)
                    {
                        winner = Some((candidate, s));
                    }
                }
            }
            if let Some((candidate, _)) = winner {
                trial = candidate;
            }
            if used == budget {
                break;
            }
        }
        if p.allows_unassigned()
            || p.values()
                .iter()
                .all(|v| trial.iter().any(|l| l.contains(v)))
        {
            if score(&trial) > score(&best) {
                best = trial.clone();
            }
            if score(&trial) >= score(&current) {
                current = trial;
            }
        }
    }
    Some(ScoredListAssignment {
        score: SimpleScore::new(score(&best)),
        assignment: ListAssignment::new(best),
    })
}
#[test]
fn all_budget_prefixes_match_independent_list_insertion_reference() {
    for case in 0..24 {
        let size = 2 + case % 4;
        let values: Vec<_> = (0..size).collect();
        let mut entities = vec![
            ListEntity::new(values.iter().step_by(2).copied().collect()),
            ListEntity::new(values.iter().skip(1).step_by(2).copied().collect()),
        ];
        if case % 3 == 0 {
            entities[0] = entities[0].clone().with_pinned_prefix(1);
        }
        if case % 4 == 0 {
            entities[1] = entities[1].clone().with_pinned(true);
        }
        if case % 5 == 0 {
            entities[0] = entities[0]
                .clone()
                .with_value_range(values.iter().step_by(2).copied().collect());
        }
        let p = ListProblem::new(values, entities).unwrap();
        let p = if case % 2 == 0 {
            p.allowing_unassigned()
        } else {
            p
        };
        let score = |lists: &[Vec<i32>]| {
            lists
                .iter()
                .enumerate()
                .map(|(e, l)| {
                    l.iter()
                        .enumerate()
                        .map(|(i, v)| {
                            ((e as i64 + 2) * (i as i64 + 3) * (*v as i64 + 1) + case as i64) % 17
                                - 8
                        })
                        .sum::<i64>()
                        - l.len().pow(2) as i64
                })
                .sum()
        };
        for budget in 0..=32 {
            let expected = reference(&p, budget, score);
            for r in run_both(
                &p,
                RuinRecreateConfig {
                    max_evaluations: budget,
                    ..config()
                },
                score,
            ) {
                assert_eq!(r.best, expected, "case {case}, budget {budget}");
                assert!(r.evaluations <= budget);
                if let Some(b) = r.best {
                    assert!(p.is_complete(&b.assignment));
                    assert_eq!(b.score, SimpleScore::new(score(b.assignment.lists())));
                }
            }
        }
    }
}
#[test]
fn subsets_preserve_pins_unsampled_order_and_unselected_missing_values() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4, 5, 6],
        vec![
            ListEntity::new(vec![0, 1, 2, 3])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2, 3, 5]),
            ListEntity::new(vec![4]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![1, 2, 3, 5]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    for seed in 0..64 {
        for r in run_both(
            &p,
            RuinRecreateConfig {
                minimum_ruined: 1,
                maximum_ruined: 1,
                max_attempts: 1,
                seed,
                ..config()
            },
            |l| {
                l.iter()
                    .enumerate()
                    .map(|(e, l)| l.len() as i64 * (e as i64 + 1))
                    .sum()
            },
        ) {
            let a = r.best.unwrap().assignment;
            assert!(p.is_complete(&a));
            assert_eq!(a.list(0).unwrap()[0], 0);
            assert_eq!(a.list(1), Some(&[4][..]));
            assert_eq!(a.location(&6), None);
            // Removing the single selected identity restores the old subsequences;
            // or selecting missing 5 leaves every old identity in place.
            assert!((1..=5).any(|removed| {
                if removed == 4 {
                    return false;
                }
                (0..3).all(|e| {
                    a.lists()[e]
                        .iter()
                        .filter(|&&v| v != removed)
                        .copied()
                        .collect::<Vec<_>>()
                        == p.entities()[e]
                            .initial()
                            .iter()
                            .filter(|&&v| v != removed)
                            .copied()
                            .collect::<Vec<_>>()
                })
            }));
        }
    }
}
#[test]
fn registry_value_scopes_and_stream_shadows_survive_partial_removal() {
    let p = ListProblem::new(
        vec![9, 2, 7],
        vec![ListEntity::new(vec![7, 9]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 3);
    let contribution =
        |value: i32, position: Option<ListPosition>, previous: Option<i32>, next: Option<i32>| {
            SimpleScore::new(
                position.map_or(0, |p| {
                    value as i64 * (p.entity as i64 + 2) - (p.index as i64) * 3
                }) + previous.unwrap_or(0) as i64
                    - next.unwrap_or(0) as i64,
            )
        };
    for i in 0..3 {
        let f = contribution;
        n.add_constraint(format!("v{i}"), vec![ListScope::Value(i)], move |v| {
            let ListFact::Value { value, element } = &v[0] else {
                unreachable!()
            };
            f(
                **value,
                element.as_ref().map(|v| v.position),
                element.as_ref().and_then(|v| v.previous.copied()),
                element.as_ref().and_then(|v| v.next.copied()),
            )
        })
        .unwrap();
    }
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::Pooled,
        ConstructionPlacement::ValueQueue,
    ] {
        let c = RuinRecreateConfig {
            placement,
            ..config()
        };
        let a = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
        let b = ruin_recreate_list_stream_search_from(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("value", s.values.port(), |v| {
                    contribution(v.value, v.position, v.previous, v.next)
                })?;
                Ok((b.build(), s))
            },
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(a.evaluations, b.evaluations);
    }
}
#[test]
fn validation_empty_models_and_initial_limits_preserve_callback_contracts() {
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0]).with_pinned(true)]).unwrap();
    for r in run_both(&p, config(), |_| 0) {
        assert_eq!(r.termination, RuinRecreateTermination::NoMovableVariables);
        assert_eq!(r.evaluations, 1);
    }
    let empty = ListProblem::new(Vec::<i32>::new(), vec![ListEntity::new(vec![])]).unwrap();
    for r in run_both(&empty, config(), |_| 0) {
        assert_eq!(r.termination, RuinRecreateTermination::NoMovableVariables);
    }
    let token = CancellationToken::new();
    token.cancel();
    for c in [
        RuinRecreateConfig {
            max_evaluations: 0,
            ..config()
        },
        RuinRecreateConfig {
            cancellation: Some(&token),
            ..config()
        },
    ] {
        let calls = Cell::new(0);
        let mut n = ListConstraintNetwork::new(1, 1);
        n.add_constraint("never", vec![ListScope::Entity(0)], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
        let r = ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
        assert_eq!(r.evaluations, 0);
        assert_eq!(calls.get(), 0);
        let r = ruin_recreate_list_stream_search_from(
            &p,
            || panic!("factory ran"),
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(r.evaluations, 0);
    }
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 1);
    assert!(matches!(
        ruin_recreate_list_search_from(
            &p,
            &n,
            ListAssignment::new(vec![vec![]]),
            RuinRecreateConfig {
                max_evaluations: 0,
                ..config()
            }
        ),
        Err(RuinRecreateError::ListSearch(_))
    ));
    let required = ListProblem::new(vec![0], vec![ListEntity::new(vec![])]).unwrap();
    assert!(matches!(
        ruin_recreate_list_search_from(&required, &n, required.initial_assignment(), config()),
        Err(RuinRecreateError::IncompleteInitialAssignment)
    ));
    for (c, reason) in [
        (
            RuinRecreateConfig {
                max_time: Some(Duration::ZERO),
                ..config()
            },
            RuinRecreateTermination::TimeLimit,
        ),
        (
            RuinRecreateConfig {
                max_attempts: 0,
                ..config()
            },
            RuinRecreateTermination::AttemptLimit,
        ),
    ] {
        for r in run_both(&p, c, |_| 0) {
            assert_eq!(r.termination, reason);
            assert_eq!(r.evaluations, 1);
        }
    }
}

#[test]
fn cancellation_keeps_complete_optional_repairs_but_discards_required_partial_ones() {
    for optional in [false, true] {
        let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])]).unwrap();
        let p = if optional { p.allowing_unassigned() } else { p };
        for stream in [false, true] {
            let calls = Cell::new(0);
            let token = CancellationToken::new();
            let score = |len: usize| {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    token.cancel();
                }
                SimpleScore::new(-(len as i64))
            };
            let c = RuinRecreateConfig {
                cancellation: Some(&token),
                assert_scores: false,
                ..config()
            };
            let r = if stream {
                ruin_recreate_list_stream_search_from(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let s = ListStreamSources::new(&mut b);
                        b.score("count", s.entities.port(), |(_, l)| score(l.len()))?;
                        Ok((b.build(), s))
                    },
                    p.initial_assignment(),
                    c,
                )
                .unwrap()
            } else {
                let mut n = ListConstraintNetwork::new(1, 3);
                n.add_constraint("count", vec![ListScope::Entity(0)], |v| {
                    let ListFact::Entity(l) = v[0] else {
                        unreachable!()
                    };
                    score(l.len())
                })
                .unwrap();
                ruin_recreate_list_search_from(&p, &n, p.initial_assignment(), c).unwrap()
            };
            assert_eq!(
                (r.evaluations, r.repair_move_evaluations, r.attempts),
                (2, 0, 1)
            );
            assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
            let b = r.best.unwrap();
            assert!(p.is_complete(&b.assignment));
            assert_eq!(
                b.assignment.list(0).unwrap().len(),
                if optional { 0 } else { 3 }
            );
            assert_eq!(r.complete_reconstructions, u64::from(optional));
        }
    }
}
#[test]
fn factory_delays_errors_and_cancellation_are_preserved_by_list_wrapper() {
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
    let make = || {
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("count", s.entities.port(), |(_, l)| {
            SimpleScore::new(l.len() as i64)
        })?;
        Ok((b.build(), s))
    };
    let r = ruin_recreate_list_stream_search_from(
        &p,
        || {
            std::thread::sleep(Duration::from_millis(20));
            make()
        },
        p.initial_assignment(),
        RuinRecreateConfig {
            max_time: Some(Duration::from_millis(1)),
            ..config()
        },
    )
    .unwrap();
    assert_eq!(r.termination, RuinRecreateTermination::TimeLimit);
    assert_eq!((r.evaluations, r.attempts), (1, 0));
    let token = CancellationToken::new();
    let calls = Cell::new(0);
    let r = ruin_recreate_list_stream_search_from(
        &p,
        || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                token.cancel();
            }
            make()
        },
        p.initial_assignment(),
        RuinRecreateConfig {
            cancellation: Some(&token),
            ..config()
        },
    )
    .unwrap();
    assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
    assert_eq!((r.evaluations, r.attempts, calls.get()), (1, 1, 2));
    assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
    assert!(matches!(
        ruin_recreate_list_stream_search_from(
            &p,
            || Err(GraphError::ForeignPort),
            p.initial_assignment(),
            config()
        ),
        Err(RuinRecreateError::ListSearch(
            ListLocalSearchError::StreamScoring(ListStreamScoringError::Graph(
                GraphError::ForeignPort
            ))
        ))
    ));
}
#[test]
fn all_placements_and_tabu_identity_families_remain_deterministic() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 1, 2, 3]), ListEntity::new(vec![])],
    )
    .unwrap();
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
        for placement in [
            ConstructionPlacement::Sequential,
            ConstructionPlacement::Pooled,
            ConstructionPlacement::ValueQueue,
        ] {
            let c = RuinRecreateConfig {
                acceptance,
                placement,
                minimum_ruined: 1,
                maximum_ruined: 3,
                max_attempts: 10,
                ..config()
            };
            let score = |l: &[Vec<i32>]| -(l.iter().map(|l| l.len().pow(2)).sum::<usize>() as i64);
            let a = run_both(&p, c.clone(), score);
            assert_eq!(a, run_both(&p, c, score));
            assert!(a[0].best.as_ref().unwrap().score >= SimpleScore::new(-16));
        }
    }
}
#[test]
fn owned_decimal_list_reconstruction_preserves_tiny_improvements() {
    let p = ListProblem::new(
        vec![0],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![])],
    )
    .unwrap();
    let scores: Vec<SimpleDecimalScore> = [
        "9007199254740992.000000000000000001",
        "9007199254740992.000000000000000002",
    ]
    .iter()
    .map(|s| s.parse().unwrap())
    .collect();
    let mut n = ListConstraintNetwork::new(2, 1);
    n.add_constraint("value", vec![ListScope::Value(0)], |v| {
        let ListFact::Value { element, .. } = &v[0] else {
            unreachable!()
        };
        element.as_ref().map_or(SimpleDecimalScore::ZERO, |e| {
            scores[e.position.entity].clone()
        })
    })
    .unwrap();
    let r = ruin_recreate_list_search_from(
        &p,
        &n,
        p.initial_assignment(),
        RuinRecreateConfig {
            max_attempts: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.best.unwrap().score, scores[1]);
}
