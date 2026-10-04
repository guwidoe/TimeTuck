//! Original reconstruction references and solver integration checks.
use std::{cell::Cell, time::Duration};
use timetuck::*;
fn run_both(
    p: &FiniteProblem<i32>,
    c: RuinRecreateConfig<SimpleScore>,
    score: impl Fn(&[Option<i32>]) -> i64,
) -> Vec<RuinRecreateResult<i32, SimpleScore>> {
    let size = p.variables().len();
    let mut n = ConstraintNetwork::new(size);
    n.add_constraint("all", (0..size).collect(), |v| {
        SimpleScore::new(score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()))
    })
    .unwrap();
    let a = ruin_recreate_search_from(p, &n, p.initial_assignment(), c.clone()).unwrap();
    let b = ruin_recreate_stream_search_from(
        p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i32>)>();
            let all = b.collect_list(s.port(), |row| *row)?;
            b.score("all", &all, |v| {
                let mut values = vec![None; size];
                for &(i, value) in v.iter() {
                    values[i] = value;
                }
                SimpleScore::new(score(&values))
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
fn valley(v: &[Option<i32>]) -> i64 {
    match (v[0], v[1]) {
        (Some(0), Some(0)) => 10,
        (Some(1), Some(1)) => 20,
        (Some(_), Some(_)) => 0,
        (a, b) => a.unwrap_or(0) as i64 * 5 + b.unwrap_or(0) as i64 * 5,
    }
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap()
}
fn config() -> RuinRecreateConfig<'static> {
    RuinRecreateConfig {
        minimum_ruined: 2,
        maximum_ruined: 2,
        max_attempts: 3,
        seed: 719,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn reconstructing_a_group_escapes_a_single_move_local_optimum() {
    let p = problem();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("valley", vec![0, 1], |v| {
        SimpleScore::new(valley(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()))
    })
    .unwrap();
    let local = local_search(&p, &n, LocalSearchConfig::default()).unwrap();
    assert_eq!(local.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(local.best.unwrap().score, SimpleScore::new(10));
    for r in run_both(&p, config(), valley) {
        assert_eq!(r.best.unwrap().assignment.values(), &[Some(1), Some(1)]);
        assert_eq!(
            (
                r.attempts,
                r.accepted_moves,
                r.evaluations,
                r.repair_move_evaluations
            ),
            (3, 1, 16, 12)
        );
        assert_eq!(r.termination, RuinRecreateTermination::AttemptLimit);
    }
    assert_eq!(p.initial_assignment().values(), &[Some(0), Some(0)]);
}
#[test]
fn every_budget_retains_complete_best_and_counts_reconstruction_work() {
    for budget in 0..=18 {
        for r in run_both(
            &problem(),
            RuinRecreateConfig {
                max_evaluations: budget,
                ..config()
            },
            valley,
        ) {
            assert!(r.evaluations <= budget);
            if budget == 0 {
                assert!(r.best.is_none());
                assert_eq!(r.attempts, 0);
                continue;
            }
            assert_eq!(r.evaluations, budget.min(16));
            let b = r.best.unwrap();
            assert_eq!(b.score, SimpleScore::new(if budget >= 6 { 20 } else { 10 }));
            assert!(problem().is_complete(&b.assignment));
            assert_eq!(b.score, SimpleScore::new(valley(b.assignment.values())));
            assert_eq!(r.repair_move_evaluations, r.evaluations - 1 - r.attempts);
            assert_eq!(
                r.termination,
                if budget <= 16 {
                    RuinRecreateTermination::EvaluationLimit
                } else {
                    RuinRecreateTermination::AttemptLimit
                }
            );
        }
    }
}
#[test]
fn optional_nulls_outside_the_sample_and_original_pins_remain_frozen() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1).with_pinned(true),
        Variable::new(vec![])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    for seed in 0..64 {
        let c = RuinRecreateConfig {
            minimum_ruined: 1,
            maximum_ruined: 1,
            max_attempts: 1,
            seed,
            ..config()
        };
        for r in run_both(&p, c, |v| {
            v.iter().flatten().map(|v| i64::from(*v) + 1).sum()
        }) {
            let a = r.best.unwrap().assignment;
            assert_eq!(a.get(2), Some(&Some(1)));
            assert_eq!(a.get(3), Some(&None));
            assert!(p.is_complete(&a));
            assert!(
                a.values()[..2]
                    .iter()
                    .zip(&p.initial_assignment().values()[..2])
                    .filter(|(a, b)| a != b)
                    .count()
                    <= 1
            );
        }
    }
}
#[test]
fn all_placements_and_acceptance_families_are_seeded_and_bounded() {
    let policies = [
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
    ];
    for acceptance in policies {
        for placement in [
            ConstructionPlacement::Sequential,
            ConstructionPlacement::Pooled,
            ConstructionPlacement::ValueQueue,
        ] {
            let c = RuinRecreateConfig {
                acceptance,
                placement,
                max_attempts: 12,
                ..config()
            };
            let a = run_both(&problem(), c.clone(), valley);
            assert_eq!(a, run_both(&problem(), c, valley));
            assert_eq!(a[0].attempts, 12);
            assert_eq!(
                a[0].best.as_ref().unwrap().score,
                SimpleScore::new(if placement == ConstructionPlacement::ValueQueue {
                    10
                } else {
                    20
                })
            );
        }
    }
}
#[test]
fn zero_limits_pins_and_cancellation_avoid_unnecessary_callbacks() {
    let token = CancellationToken::new();
    token.cancel();
    let p = problem();
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
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("never", vec![0], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
        let r = ruin_recreate_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
        assert_eq!(r.evaluations, 0);
        assert_eq!(calls.get(), 0);
        let r = ruin_recreate_stream_search_from(
            &p,
            || panic!("factory must not run"),
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(r.evaluations, 0);
    }
    for (c, reason) in [
        (
            RuinRecreateConfig {
                max_attempts: 0,
                ..config()
            },
            RuinRecreateTermination::AttemptLimit,
        ),
        (
            RuinRecreateConfig {
                max_time: Some(Duration::ZERO),
                ..config()
            },
            RuinRecreateTermination::TimeLimit,
        ),
    ] {
        for r in run_both(&p, c, valley) {
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.attempts, 0);
            assert_eq!(r.termination, reason);
        }
    }
    let pinned = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0).with_pinned(true);
        2
    ])
    .unwrap();
    for r in run_both(&pinned, config(), valley) {
        assert_eq!(r.termination, RuinRecreateTermination::NoMovableVariables);
        assert_eq!(r.evaluations, 1);
    }
}
#[test]
fn validation_and_factory_failures_propagate_without_mutating_input() {
    let p = problem();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    for c in [
        RuinRecreateConfig {
            minimum_ruined: 0,
            ..config()
        },
        RuinRecreateConfig {
            maximum_ruined: 1,
            ..config()
        },
    ] {
        assert!(matches!(
            ruin_recreate_search_from(&p, &n, p.initial_assignment(), c),
            Err(RuinRecreateError::InvalidRuinRange)
        ));
    }
    assert!(matches!(
        ruin_recreate_search_from(&p, &n, Assignment::new(vec![None, None]), config()),
        Err(RuinRecreateError::IncompleteInitialAssignment)
    ));
    assert!(
        ruin_recreate_search_from(
            &p,
            &n,
            Assignment::new(vec![]),
            RuinRecreateConfig {
                max_evaluations: 0,
                ..config()
            }
        )
        .is_err()
    );
    let r = ruin_recreate_stream_search_from(
        &p,
        || Err(GraphError::ForeignPort),
        p.initial_assignment(),
        config(),
    );
    assert!(matches!(
        r,
        Err(RuinRecreateError::Search(LocalSearchError::StreamScoring(
            StreamScoringError::Graph(GraphError::ForeignPort)
        )))
    ));
    assert_eq!(p.initial_assignment().values(), &[Some(0), Some(0)]);
}

fn reference(
    p: &FiniteProblem<i32>,
    budget: u64,
    score: impl Fn(&[Option<i32>]) -> i64,
) -> Option<ScoredAssignment<i32, SimpleScore>> {
    if budget == 0 {
        return None;
    }
    let mut best = p.initial_assignment().values().to_vec();
    let mut current = best.clone();
    let mut used = 1;
    for _ in 0..3 {
        if used == budget {
            break;
        }
        let mut trial: Vec<_> = p
            .variables()
            .iter()
            .zip(&current)
            .map(|(v, x)| if v.is_pinned() { *x } else { None })
            .collect();
        used += 1;
        for (i, v) in p.variables().iter().enumerate() {
            if v.is_pinned() {
                continue;
            }
            let before = score(&trial);
            let mut winner = None;
            for value in v.domain() {
                if used == budget {
                    break;
                }
                let mut candidate = trial.clone();
                candidate[i] = Some(*value);
                let scored = score(&candidate);
                used += 1;
                if (!v.allows_unassigned() || scored >= before)
                    && winner.as_ref().is_none_or(|(_, s)| scored > *s)
                {
                    winner = Some((*value, scored));
                }
            }
            if let Some((value, _)) = winner {
                trial[i] = Some(value);
            }
            if used == budget {
                break;
            }
        }
        if p.variables()
            .iter()
            .zip(&trial)
            .all(|(v, x)| v.allows_unassigned() || x.is_some())
        {
            if score(&trial) > score(&best) {
                best = trial.clone();
            }
            if score(&trial) >= score(&current) {
                current = trial;
            }
        }
    }
    Some(ScoredAssignment {
        score: SimpleScore::new(score(&best)),
        assignment: Assignment::new(best),
    })
}
#[test]
fn heterogeneous_optional_repairs_match_independent_full_score_reconstruction() {
    for case in 0..32 {
        let size = 2 + case % 4;
        let p = FiniteProblem::new(
            (0..size)
                .map(|i| {
                    let count = 1 + (i + case) % 3;
                    let mut v = Variable::new((0..count as i32).collect());
                    if (i + case) % 3 == 0 {
                        v = v.allowing_unassigned();
                    }
                    if (i + case) % 4 != 0 || (i + case) % 3 != 0 {
                        v = v.with_initial((i % count) as i32);
                    }
                    v.with_pinned((i + case) % 5 == 0)
                })
                .collect(),
        )
        .unwrap();
        let score = |values: &[Option<i32>]| {
            let unary: i64 = values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    v.map_or(0, |v| {
                        ((i as i64 + 3) * (v as i64 + 2) + case as i64) % 13 - 6
                    })
                })
                .sum();
            unary
                - (0..size)
                    .flat_map(|i| (i + 1..size).map(move |j| (i, j)))
                    .filter(|&(i, j)| values[i].is_some() && values[i] == values[j])
                    .count() as i64
                    * 7
        };
        for budget in 0..=24 {
            let expected = reference(&p, budget, score);
            for r in run_both(
                &p,
                RuinRecreateConfig {
                    minimum_ruined: usize::MAX,
                    maximum_ruined: usize::MAX,
                    max_evaluations: budget,
                    ..config()
                },
                score,
            ) {
                assert_eq!(r.best, expected, "case {case}, budget {budget}");
                assert!(r.evaluations <= budget);
            }
        }
    }
}

#[test]
fn cancellation_inside_repair_retains_a_complete_prefix_and_discards_partial_work() {
    for cancel_at in 1..=6 {
        let token = CancellationToken::new();
        let calls = Cell::new(0);
        let p = problem();
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("cancel", vec![0, 1], |v| {
            calls.set(calls.get() + 1);
            if calls.get() == cancel_at {
                token.cancel();
            }
            SimpleScore::new(valley(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()))
        })
        .unwrap();
        let r = ruin_recreate_search_from(
            &p,
            &n,
            p.initial_assignment(),
            RuinRecreateConfig {
                cancellation: Some(&token),
                assert_scores: false,
                ..config()
            },
        )
        .unwrap();
        assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
        assert_eq!(r.evaluations, calls.get());
        let b = r.best.unwrap();
        assert!(p.is_complete(&b.assignment));
        assert_eq!(
            b.score,
            SimpleScore::new(if cancel_at == 6 { 20 } else { 10 })
        );
    }
}

#[test]
fn owned_decimal_repairs_retain_sub_float_improvements() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
    let scores: Vec<SimpleDecimalScore> = [
        "9007199254740992.000000000000000001",
        "9007199254740992.000000000000000002",
    ]
    .iter()
    .map(|s| s.parse().unwrap())
    .collect();
    let mut n = ConstraintNetwork::<usize, SimpleDecimalScore>::new(1);
    n.add_constraint("decimal", vec![0], |v| {
        v[0].map_or(SimpleDecimalScore::ZERO, |v| scores[*v].clone())
    })
    .unwrap();
    let r = ruin_recreate_search_from(
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

#[test]
fn stream_factory_time_and_mid_factory_cancellation_belong_to_the_search() {
    let p = problem();
    let token = CancellationToken::new();
    let factories = Cell::new(0);
    let make = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        b.score("score", s.port(), |(_, v)| {
            SimpleScore::new(v.map_or(0, i64::from))
        })?;
        Ok((b.build(), s))
    };
    let r = ruin_recreate_stream_search_from(
        &p,
        || {
            factories.set(factories.get() + 1);
            if factories.get() == 2 {
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
    assert_eq!((factories.get(), r.evaluations, r.attempts), (2, 1, 1));
    assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
    let r = ruin_recreate_stream_search_from(
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
    assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
}
