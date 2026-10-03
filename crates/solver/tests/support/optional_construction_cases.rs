//! Original optional-construction checks; independent of source-method coverage.
use super::*;
fn policy() -> ConstructionConfig<'static> {
    ConstructionConfig {
        optional: OptionalConstruction::NonWorsening,
        ..Default::default()
    }
}
#[test]
fn optional_scans_choose_improvements_and_ties_but_keep_unassignment_over_worse_scores() {
    for mode in 0..6 {
        for order in ORDERS {
            for slope in [-1, 0, 1] {
                for early in [false, true] {
                    for diagnostics in [false, true] {
                        for steps in 0..=3 {
                            let c = ConstructionConfig {
                                max_steps: Some(steps),
                                assert_scores: diagnostics,
                                pick_early: if early {
                                    ConstructionPickEarly::FirstFeasibleScore
                                } else {
                                    ConstructionPickEarly::Never
                                },
                                ..policy()
                            };
                            let score = |v: &[Option<usize>]| {
                                SimpleScore::new(
                                    slope * v.iter().flatten().map(|v| (v + 1) as i64).sum::<i64>(),
                                )
                            };
                            let r = run_problem(mode, order, c, true, || {}, score);
                            let mut expected = vec![None; 2];
                            let first = usize::from(mode >= 4 && order == ORDERS[1]);
                            let count = steps.min(2) as usize;
                            if slope >= 0 {
                                let value = usize::from(slope > 0 && !early);
                                if count > 0 {
                                    expected[first] = Some(value);
                                }
                                if count > 1 {
                                    expected[1 - first] = Some(value);
                                }
                            }
                            assert_eq!(
                                r.values, expected,
                                "{mode} {order:?} slope={slope} early={early} steps={steps}"
                            );
                            assert!(r.complete);
                            assert_eq!(r.steps, steps.min(2));
                            assert_eq!(r.assigned, if slope >= 0 { count } else { 0 });
                            let trials = if early && slope >= 0 {
                                count as u64
                            } else {
                                2 * count as u64
                                    + u64::from((2..4).contains(&mode) && count == 2 && slope >= 0)
                            };
                            assert_eq!(r.evaluations, 1 + trials);
                            assert_eq!(
                                r.reason,
                                if steps < 2 {
                                    ConstructionTermination::StepLimit
                                } else {
                                    ConstructionTermination::Completed
                                }
                            );
                            assert_eq!(r.score, Some(score(&expected)));
                            assert!(r.score.unwrap() >= SimpleScore::ZERO);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn optional_construction_preserves_defaults_and_bounds_every_small_budget_prefix() {
    for mode in 0..6 {
        for order in ORDERS {
            for slope in [-1, 0, 1] {
                let score = |v: &[Option<usize>]| {
                    SimpleScore::new(
                        slope * v.iter().flatten().map(|v| (v + 1) as i64).sum::<i64>(),
                    )
                };
                let preserved = run_problem(
                    mode,
                    order,
                    ConstructionConfig::default(),
                    true,
                    || {},
                    score,
                );
                assert_eq!(preserved.values, vec![None; 2]);
                assert_eq!(preserved.steps, 0);
                assert_eq!(preserved.evaluations, 1);
                for scores in 0..=7 {
                    for moves in 0..=6 {
                        for steps in 0..=2 {
                            let c = ConstructionConfig {
                                max_evaluations: scores,
                                max_move_evaluations: Some(moves),
                                max_steps: Some(steps),
                                ..policy()
                            };
                            let r = run_problem(mode, order, c, true, || {}, score);
                            let same_prefix = run_problem(
                                mode,
                                order,
                                ConstructionConfig {
                                    max_evaluations: scores.min(moves + 1),
                                    max_move_evaluations: None,
                                    ..c
                                },
                                true,
                                || {},
                                score,
                            );
                            assert_eq!(r.values, same_prefix.values);
                            assert_eq!(r.score, same_prefix.score);
                            assert_eq!(r.evaluations, same_prefix.evaluations);
                            assert_eq!(r.steps, same_prefix.steps);
                            assert!(r.complete);
                            assert!(r.steps <= steps);
                            assert!(r.evaluations <= scores);
                            assert!(r.evaluations.saturating_sub(1) <= moves);
                            if scores == 0 {
                                assert_eq!(r.score, None);
                            } else {
                                assert!(r.score.unwrap() >= SimpleScore::ZERO);
                            }
                            if slope < 0 {
                                assert_eq!(r.assigned, 0);
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn optional_scans_retain_only_committed_nonworsening_prefixes_on_cancellation() {
    for mode in 0..6 {
        for order in ORDERS {
            for slope in [-1, 0, 1] {
                let score = |v: &[Option<usize>]| {
                    SimpleScore::new(
                        slope * v.iter().flatten().map(|v| (v + 1) as i64).sum::<i64>(),
                    )
                };
                for stop in 1..=3 {
                    let token = CancellationToken::new();
                    let calls = Cell::new(0);
                    let r = run_problem(
                        mode,
                        order,
                        ConstructionConfig {
                            cancellation: Some(&token),
                            ..policy()
                        },
                        true,
                        || {
                            calls.set(calls.get() + 1);
                            if calls.get() == stop {
                                token.cancel();
                            }
                        },
                        score,
                    );
                    let prefix = run_problem(
                        mode,
                        order,
                        ConstructionConfig {
                            max_evaluations: stop,
                            ..policy()
                        },
                        true,
                        || {},
                        score,
                    );
                    assert_eq!(r.reason, ConstructionTermination::Cancelled);
                    assert_eq!(r.values, prefix.values);
                    assert_eq!(r.score, prefix.score);
                    assert_eq!(r.steps, prefix.steps);
                    assert_eq!(r.evaluations, stop);
                    assert!(r.complete);
                }
            }
        }
    }
}
