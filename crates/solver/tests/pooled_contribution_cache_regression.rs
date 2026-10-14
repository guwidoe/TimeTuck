//! Original opt-in pooled contribution reuse contracts; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

fn pooled() -> ConstructionConfig<'static> {
    ConstructionConfig {
        placement: ConstructionPlacement::Pooled,
        ..Default::default()
    }
}
fn same_outcome<V: Eq + Clone + std::fmt::Debug, S: Eq + Clone + std::fmt::Debug>(
    cached: &ConstructionResult<V, S>,
    plain: &ConstructionResult<V, S>,
) {
    let mut cached = cached.clone();
    let mut plain = plain.clone();
    cached.constraint_evaluations = 0;
    plain.constraint_evaluations = 0;
    assert_eq!(cached, plain);
}
#[test]
fn zero_delta_edge_still_invalidates_other_endpoint_candidates() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 2]).unwrap();
    for cap in [0, 1, 128] {
        let mut n = ConstraintNetwork::new(2).with_pooled_contribution_cache(cap);
        n.add_constraint("first placement", vec![0], |v| {
            SimpleScore::new(if v[0] == Some(&0) { 10 } else { 0 })
        })
        .unwrap();
        n.add_constraint("edge", vec![0, 1], |v| {
            SimpleScore::new(if v[0] == Some(&0) && v[1] == Some(&0) {
                -100
            } else {
                0
            })
        })
        .unwrap();
        let result = greedy_construction(&p, &n, pooled()).unwrap();
        // First endpoint changes the edge's dependencies even though its current
        // contribution stays zero. Reusing the earlier other-endpoint score loses.
        assert_eq!(result.assignment.values(), &[Some(0), Some(1)]);
        assert_eq!(result.score, Some(SimpleScore::new(10)));
        assert!(result.complete);
    }
}
fn scoped_score(values: &[Option<i64>], weight: i64) -> SimpleScore {
    let unary: i64 = values
        .iter()
        .enumerate()
        .map(|(id, v)| v.map_or(0, |v| (id as i64 + 1) * (3 - v)))
        .sum();
    let mut extra = 0;
    for scope in [
        vec![0, 1],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 1],
        vec![2, 3],
    ] {
        let sum: i64 = scope.into_iter().filter_map(|id| values[id]).sum();
        extra -= sum * sum;
    }
    SimpleScore::new(weight * unary + extra)
        .with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
}
fn scoped_run(
    cap: usize,
    config: ConstructionConfig<'_>,
    weight: i64,
    optional: bool,
) -> ConstructionResult<i64, SimpleScore> {
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| {
                let v = Variable::new(vec![0i64, 1, 2]);
                if optional { v.allowing_unassigned() } else { v }
            })
            .collect(),
    )
    .unwrap();
    let calls = Cell::new(0u128);
    let mut n = ConstraintNetwork::new(4).with_pooled_contribution_cache(cap);
    for id in 0..4 {
        let calls = &calls;
        n.add_constraint(format!("unary-{id}"), vec![id], move |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v[0].map_or(0, |v| weight * (id as i64 + 1) * (3 - v)))
        })
        .unwrap();
    }
    for (id, scope) in [
        vec![0, 1],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 1],
        vec![2, 3],
    ]
    .into_iter()
    .enumerate()
    {
        let calls = &calls;
        n.add_constraint(format!("edge-{id}"), scope, move |v| {
            calls.set(calls.get() + 1);
            let sum: i64 = v.iter().filter_map(|v| v.copied()).sum();
            SimpleScore::new(-sum * sum)
        })
        .unwrap();
    }
    let result = greedy_construction(&p, &n, config).unwrap();
    if !config.assert_scores && !config.assert_step_scores {
        assert_eq!(result.constraint_evaluations, calls.get());
    }
    if let Some(score) = result.score {
        let expected = scoped_score(result.assignment.values(), weight);
        assert_eq!(
            score,
            if optional {
                expected.with_structural(0)
            } else {
                expected
            }
        );
    }
    result
}
#[test]
fn related_scopes_and_unary_contributions_rebase_to_current_total() {
    let plain = scoped_run(0, pooled(), 1, false);
    let cached = scoped_run(512, pooled(), 1, false);
    same_outcome(&cached, &plain);
    assert!(cached.constraint_evaluations < plain.constraint_evaluations);
    // Repeated solves and different immutable weights cannot inherit phase data.
    for weight in [7, -3, 1] {
        let plain = scoped_run(0, pooled(), weight, false);
        let cached = scoped_run(512, pooled(), weight, false);
        same_outcome(&cached, &plain);
    }
}
#[test]
fn every_score_move_step_prefix_and_atomic_discard_matches_uncached() {
    for interruption in [
        ConstructionInterruption::KeepBestPrefix,
        ConstructionInterruption::DiscardIncompleteStep,
    ] {
        for early in [
            ConstructionPickEarly::Never,
            ConstructionPickEarly::FirstNonDeterioratingScore,
            ConstructionPickEarly::FirstFeasibleScore,
        ] {
            for budget in 0..=34 {
                for steps in [Some(0), Some(1), Some(2), None] {
                    let config = ConstructionConfig {
                        interruption,
                        pick_early: early,
                        max_evaluations: budget,
                        max_steps: steps,
                        ..pooled()
                    };
                    let plain = scoped_run(0, config, 1, false);
                    for cap in [1, 512] {
                        same_outcome(&scoped_run(cap, config, 1, false), &plain);
                    }
                }
                let config = ConstructionConfig {
                    interruption,
                    pick_early: early,
                    max_move_evaluations: Some(budget),
                    ..pooled()
                };
                same_outcome(
                    &scoped_run(512, config, 1, false),
                    &scoped_run(0, config, 1, false),
                );
            }
        }
    }
}
#[test]
fn optional_and_sequential_fallback_and_assertion_modes_preserve_contracts() {
    for config in [
        pooled(),
        ConstructionConfig::default(),
        ConstructionConfig {
            assert_scores: true,
            ..pooled()
        },
        ConstructionConfig {
            assert_step_scores: true,
            ..pooled()
        },
    ] {
        for optional in [false, true] {
            let plain = scoped_run(0, config, 1, optional);
            let cached = scoped_run(512, config, 1, optional);
            same_outcome(&cached, &plain);
            if optional || config.placement == ConstructionPlacement::Sequential {
                assert_eq!(cached.constraint_evaluations, plain.constraint_evaluations);
            }
        }
    }
}
#[test]
fn pre_cancel_and_initial_callback_cancel_do_not_commit_or_reuse_stale_work() {
    for cancel_initial in [false, true] {
        let mut outcomes = Vec::new();
        for cap in [0, 512] {
            let token = CancellationToken::default();
            if !cancel_initial {
                token.cancel();
            }
            let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 2]).unwrap();
            let mut n = ConstraintNetwork::new(2).with_pooled_contribution_cache(cap);
            n.add_constraint("cancel", vec![0, 1], |_| {
                if cancel_initial {
                    token.cancel();
                }
                SimpleScore::ZERO
            })
            .unwrap();
            let result = greedy_construction(
                &p,
                &n,
                ConstructionConfig {
                    cancellation: Some(&token),
                    ..pooled()
                },
            )
            .unwrap();
            assert_eq!(result.assignment, p.initial_assignment());
            assert_eq!(result.termination, ConstructionTermination::Cancelled);
            outcomes.push(result);
        }
        same_outcome(&outcomes[0], &outcomes[1]);
    }
}
#[test]
fn structural_error_or_panic_aborts_phase_and_same_network_retries_fresh() {
    for cap in [0, 512] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 2]).unwrap();
        let mode = Cell::new(0);
        let mut n = ConstraintNetwork::new(2).with_pooled_contribution_cache(cap);
        n.add_constraint("earlier", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
        n.add_constraint("failure", vec![0, 1], |v| {
            let candidate = v.iter().any(Option::is_some);
            assert!(!(candidate && mode.get() == 2), "injected panic");
            SimpleScore::ZERO.with_structural(if candidate && mode.get() == 1 { -1 } else { 0 })
        })
        .unwrap();
        let baseline = greedy_construction(&p, &n, pooled()).unwrap();
        for failure in [1, 2] {
            mode.set(failure);
            let failed = catch_unwind(AssertUnwindSafe(|| greedy_construction(&p, &n, pooled())));
            if failure == 1 {
                assert!(matches!(
                    failed.unwrap(),
                    Err(ScoringError::StructuralContribution { constraint: 1 })
                ));
            } else {
                assert!(failed.is_err());
            }
            assert_eq!(p.initial_assignment().values(), &[None, None]);
            mode.set(0);
            same_outcome(&greedy_construction(&p, &n, pooled()).unwrap(), &baseline);
        }
    }
}
#[test]
fn total_overflow_is_not_hidden_by_saved_contributions() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1i64]); 2]).unwrap();
    for cap in [0, 1, 512] {
        let mut n = ConstraintNetwork::new(2).with_pooled_contribution_cache(cap);
        n.add_constraint("base near maximum", vec![], |_| {
            SimpleScore::new(i64::MAX - 1)
        })
        .unwrap();
        for id in 0..2 {
            n.add_constraint(format!("unit-{id}"), vec![id], |v| {
                SimpleScore::new(i64::from(v[0].is_some()))
            })
            .unwrap();
        }
        assert!(matches!(
            greedy_construction(&p, &n, pooled()),
            Err(ScoringError::ScoreOverflow { .. })
        ));
    }
}
#[derive(Clone, Debug)]
struct EqualPayload {
    key: u8,
    payload: i64,
}
impl PartialEq for EqualPayload {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for EqualPayload {}
#[test]
fn eq_equal_domain_payloads_are_rejected_before_cache_admission() {
    let values = vec![
        EqualPayload { key: 1, payload: 3 },
        EqualPayload { key: 1, payload: 9 },
    ];
    assert_ne!(values[0].payload, values[1].payload);
    assert!(matches!(
        FiniteProblem::new(vec![Variable::new(values)]),
        Err(ModelError::DuplicateValue { variable: 0 })
    ));
}

#[test]
fn independent_unaries_reuse_contributions_but_rebase_scores_after_each_selection() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 3]).unwrap();
    let mut outcomes = Vec::new();
    for cap in [0, 128] {
        let mut n = ConstraintNetwork::new(3).with_pooled_contribution_cache(cap);
        for id in 0..3 {
            n.add_constraint(format!("independent-{id}"), vec![id], move |v| {
                SimpleScore::new(v[0].map_or(0, |v| 9 + id as i64 + v))
            })
            .unwrap();
        }
        let result = greedy_construction(&p, &n, pooled()).unwrap();
        assert_eq!(result.assignment.values(), &[Some(1), Some(1), Some(1)]);
        assert_eq!(result.score, Some(SimpleScore::new(33)));
        outcomes.push(result);
    }
    same_outcome(&outcomes[0], &outcomes[1]);
    assert!(outcomes[1].constraint_evaluations < outcomes[0].constraint_evaluations);
}
#[test]
fn default_and_explicit_zero_keep_identical_callback_trace() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 3]).unwrap();
    let mut traces = Vec::new();
    let mut outcomes = Vec::new();
    for explicit_zero in [false, true] {
        let trace = std::cell::RefCell::new(Vec::new());
        let mut n = ConstraintNetwork::new(3);
        if explicit_zero {
            n = n.with_pooled_contribution_cache(0);
        }
        for (id, scope) in [vec![0], vec![1, 2], vec![2, 1, 2]].into_iter().enumerate() {
            let trace = &trace;
            n.add_constraint(format!("trace-{id}"), scope, move |v| {
                let values: Vec<_> = v.iter().map(|v| v.copied()).collect();
                trace.borrow_mut().push((id, values.clone()));
                SimpleScore::new(values.into_iter().flatten().sum())
            })
            .unwrap();
        }
        outcomes.push(greedy_construction(&p, &n, pooled()).unwrap());
        drop(n);
        traces.push(trace.into_inner());
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(traces[0], traces[1]);
}
#[test]
fn cancellation_during_later_pool_scan_preserves_each_interruption_policy() {
    for interruption in [
        ConstructionInterruption::KeepBestPrefix,
        ConstructionInterruption::DiscardIncompleteStep,
    ] {
        let mut outcomes = Vec::new();
        for cap in [0, 512] {
            let token = CancellationToken::default();
            let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 3]).unwrap();
            let mut n = ConstraintNetwork::new(3).with_pooled_contribution_cache(cap);
            for id in 0..3 {
                n.add_constraint(format!("value-{id}"), vec![id], |v| {
                    SimpleScore::new(v[0].copied().unwrap_or(0))
                })
                .unwrap();
            }
            // The whole-model dependency makes this watchdog observe the same
            // actual candidate states even when independent unaries are reused.
            n.add_constraint("cancel on second assigned variable", vec![0, 1, 2], |v| {
                if v.iter().filter(|v| v.is_some()).count() >= 2 {
                    token.cancel();
                }
                SimpleScore::ZERO
            })
            .unwrap();
            let result = greedy_construction(
                &p,
                &n,
                ConstructionConfig {
                    interruption,
                    cancellation: Some(&token),
                    ..pooled()
                },
            )
            .unwrap();
            assert_eq!(result.termination, ConstructionTermination::Cancelled);
            assert!(
                result
                    .assignment
                    .values()
                    .iter()
                    .filter(|v| v.is_some())
                    .count()
                    >= 1
            );
            outcomes.push(result);
        }
        same_outcome(&outcomes[0], &outcomes[1]);
    }
}

#[test]
fn disabled_weights_skip_callbacks_and_fresh_phases_apply_live_weight_changes() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 2]).unwrap();
    let mut runs = Vec::new();
    for cap in [0, 512] {
        let direct_calls = Cell::new(0u128);
        let factor_calls = Cell::new(0u128);
        let current_weight = Cell::new(0);
        let mut n = ConstraintNetwork::new(2).with_pooled_contribution_cache(cap);
        n.add_weighted_constraint("always disabled", vec![1], SimpleScore::ZERO, |_| {
            panic!("zero weighted callback must not execute")
        })
        .unwrap();
        for id in 0..2 {
            let calls = &direct_calls;
            n.add_constraint(format!("direct-{id}"), vec![id], move |values| {
                calls.set(calls.get() + 1);
                SimpleScore::new(if values[0] == Some(&1) { 4 } else { 0 })
            })
            .unwrap();
        }
        n.add_weighted_constraint("changing weight", vec![0, 0], SimpleScore::ZERO, |values| {
            assert_ne!(current_weight.get(), 0, "disabled factor must not execute");
            assert_eq!(values[0], values[1]);
            factor_calls.set(factor_calls.get() + 1);
            match values[0] {
                None => 0,
                Some(&0) => 5,
                Some(_) => -3,
            }
        })
        .unwrap();
        let mut phases = Vec::new();
        for weight in [0, 2, 0] {
            current_weight.set(weight);
            n.apply_weight_overrides(&ConstraintWeightOverrides::from_iter([(
                "changing weight".to_string(),
                SimpleScore::new(weight),
            )]))
            .unwrap();
            direct_calls.set(0);
            factor_calls.set(0);
            let result = greedy_construction(&p, &n, pooled()).unwrap();
            assert_eq!(
                result.constraint_evaluations,
                direct_calls.get() + factor_calls.get()
            );
            assert_eq!(factor_calls.get() == 0, weight == 0);
            assert_eq!(
                result.assignment.values(),
                if weight == 0 {
                    &[Some(1), Some(1)]
                } else {
                    &[Some(0), Some(1)]
                }
            );
            assert_eq!(
                result.score,
                Some(SimpleScore::new(if weight == 0 { 8 } else { 14 }))
            );
            phases.push(result);
        }
        // Switching back to zero restores the first solve's exact logical and
        // callback work, rather than retaining a previous phase's cached scores.
        assert_eq!(phases[0], phases[2]);
        runs.push(phases);
    }
    for (plain, cached) in runs[0].iter().zip(&runs[1]) {
        same_outcome(plain, cached);
        assert!(cached.constraint_evaluations < plain.constraint_evaluations);
    }
}
