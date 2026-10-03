//! Original construction-step budgets; no upstream method credit.
use super::*;
#[test]
fn construction_steps_finish_each_scan_and_share_one_mixed_budget() {
    for mode in 0..6 {
        for order in ORDERS {
            for early in [false, true] {
                for diagnostics in [false, true] {
                    for steps in [0, 1, 2, 3, u64::MAX] {
                        let config = ConstructionConfig {
                            max_steps: Some(steps),
                            pick_early: if early {
                                ConstructionPickEarly::FirstFeasibleScore
                            } else {
                                ConstructionPickEarly::Never
                            },
                            assert_scores: diagnostics,
                            ..Default::default()
                        };
                        let r = run(mode, order, config, || {});
                        let assigned = steps.min(2) as usize;
                        let first = usize::from(mode >= 4 && order == ORDERS[1]);
                        let mut values = vec![None; 2];
                        if assigned > 0 {
                            values[first] = Some(usize::from(!early));
                        }
                        if assigned > 1 {
                            values[1 - first] = Some(usize::from(!early));
                        }
                        assert_eq!(r.values, values, "{mode} {order:?} {early} {steps}");
                        assert_eq!(r.assigned, assigned);
                        assert_eq!(r.complete, steps >= 2);
                        let trials = if early {
                            assigned as u64
                        } else {
                            2 * assigned as u64 + u64::from((2..4).contains(&mode) && assigned == 2)
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
                        if steps >= 2 {
                            assert_eq!(
                                r,
                                run(
                                    mode,
                                    order,
                                    ConstructionConfig {
                                        max_steps: None,
                                        ..config
                                    },
                                    || {}
                                )
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn construction_steps_compose_with_every_small_score_and_candidate_budget() {
    for mode in 0..6 {
        for order in ORDERS {
            for early in [false, true] {
                for steps in 0..=2 {
                    for scores in 0..=7 {
                        for moves in 0..=6 {
                            let c = ConstructionConfig {
                                max_steps: Some(steps),
                                max_evaluations: scores,
                                max_move_evaluations: Some(moves),
                                assert_scores: true,
                                pick_early: if early {
                                    ConstructionPickEarly::FirstFeasibleScore
                                } else {
                                    ConstructionPickEarly::Never
                                },
                                ..Default::default()
                            };
                            let r = run(mode, order, c, || {});
                            // A step cap stops before the next scan, not partway through
                            // the current scan. Compare the same evaluated prefix with
                            // the independently established evaluation-budget contract.
                            let step_scores = if steps >= 2 {
                                u64::MAX
                            } else {
                                1 + steps * if early { 1 } else { 2 }
                            };
                            let mut prefix = run(
                                mode,
                                order,
                                ConstructionConfig {
                                    max_steps: None,
                                    max_evaluations: scores.min(step_scores),
                                    ..c
                                },
                                || {},
                            );
                            let available = scores.min(moves + 1);
                            if steps < 2 && available >= step_scores {
                                prefix.reason = ConstructionTermination::StepLimit;
                            }
                            assert_eq!(
                                r, prefix,
                                "{mode} {order:?} early={early} steps={steps} scores={scores} moves={moves}"
                            );
                            assert!(r.assigned as u64 <= steps);
                            assert!(r.evaluations <= scores);
                            assert!(r.evaluations.saturating_sub(1) <= moves);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn construction_step_boundaries_preserve_cancellation_and_zero_score_precedence() {
    for mode in 0..6 {
        for order in ORDERS {
            for cancel_at in [0, 1, 3] {
                let token = CancellationToken::new();
                if cancel_at == 0 {
                    token.cancel();
                }
                let calls = Cell::new(0);
                let r = run(
                    mode,
                    order,
                    ConstructionConfig {
                        max_steps: Some(u64::from(cancel_at == 3)),
                        cancellation: Some(&token),
                        ..Default::default()
                    },
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == cancel_at {
                            token.cancel();
                        }
                    },
                );
                assert_eq!(r.reason, ConstructionTermination::Cancelled);
                assert_eq!(r.evaluations, cancel_at);
                assert_eq!(r.assigned, usize::from(cancel_at == 3));
                assert_eq!(calls.get(), cancel_at);
            }
            let token = CancellationToken::new();
            token.cancel();
            let r = run(
                mode,
                order,
                ConstructionConfig {
                    max_steps: Some(0),
                    max_evaluations: 0,
                    cancellation: Some(&token),
                    ..Default::default()
                },
                || panic!("zero score cap must not score"),
            );
            assert_eq!(r.reason, ConstructionTermination::EvaluationLimit);
            assert_eq!(r.evaluations, 0);
            assert_eq!(r.assigned, 0);
        }
    }
}
