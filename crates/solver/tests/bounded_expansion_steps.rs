//! Original bounded-exact expansion controls, lifecycle and metrics regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn exact(completion: bool, budget: u64, bound: PartialScoreBound) -> SolverPhase<'static> {
    let c = BoundedPhaseConfig {
        exploration: NodeExploration::InputOrderDepthFirst,
        bound,
        ..Default::default()
    };
    SolverPhase::Exact(ExactPhaseConfig {
        strategy: if completion {
            ExactSearchStrategy::CompleteBranchAndBound(c)
        } else {
            ExactSearchStrategy::BranchAndBound(c)
        },
        max_evaluations: budget,
        ..Default::default()
    })
}
fn plan(phases: Vec<SolverPhase<'_>>) -> SolverSequence<'_> {
    SolverSequence {
        phases,
        ..Default::default()
    }
}
fn problem(n: usize) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]); n]).unwrap()
}
fn events_summary(events: &[SolverLifecycleEvent]) -> (usize, usize) {
    let mut open = None;
    let mut started = 0;
    let mut completed = 0;
    for &e in events {
        match e {
            SolverLifecycleEvent::StepStarted { phase, kind, step } => {
                assert_eq!(kind, SolverPhaseKind::Exact);
                assert!(open.replace((phase, step)).is_none());
                started += 1;
            }
            SolverLifecycleEvent::StepEnded {
                phase,
                kind,
                step,
                completed: done,
            } => {
                assert_eq!(kind, SolverPhaseKind::Exact);
                assert_eq!(open.take(), Some((phase, step)));
                completed += usize::from(done);
            }
            SolverLifecycleEvent::PhaseEnded { .. } => assert!(open.is_none()),
            _ => {}
        }
    }
    assert!(open.is_none());
    (started, completed)
}
#[test]
fn every_evaluation_prefix_closes_only_fully_generated_expansions_and_preserves_best() {
    let p = problem(3);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    for budget in 0..=15 {
        let mut events = vec![];
        let r = solve_with_extensions(
            &p,
            &n,
            plan(vec![exact(false, budget, PartialScoreBound::Unknown)]),
            SolverExtensions {
                lifecycle: Some(&mut |e| events.push(e)),
                ..Default::default()
            },
        )
        .unwrap();
        // Binary tree: root evaluation, then pairs of scored children for each
        // expansion. An odd remainder starts one incomplete expansion.
        let generated = budget.saturating_sub(1);
        assert_eq!(r.work.evaluations, budget);
        assert_eq!(r.work.completed_steps, generated / 2);
        assert_eq!(
            events_summary(&events),
            (generated.div_ceil(2) as usize, (generated / 2) as usize)
        );
        assert_eq!(r.work.accepted_moves, 0);
        assert_eq!(r.is_optimal(), budget == 15);
        if budget >= 6 {
            assert_eq!(r.assignment.values(), &[Some(0), Some(0), Some(0)]);
            assert_eq!(r.score, Some(SimpleScore::ZERO));
        }
    }
}
#[test]
fn nested_phase_steps_stop_then_next_phase_resumes_while_global_steps_stop_the_solve() {
    let p = problem(3);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let leaves = [
        SearchTermination::Condition(SearchLimit::Steps(2)),
        SearchTermination::Condition(SearchLimit::ScoreEvaluations(5)),
    ];
    let condition = SearchTermination::All(&leaves);
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    let mut events = vec![];
    let r = solve_with_extensions(
        &p,
        &n,
        plan(vec![
            exact(true, 100, PartialScoreBound::Unknown),
            exact(false, 100, PartialScoreBound::Unknown),
        ]),
        SolverExtensions {
            phase_terminations: &bindings,
            lifecycle: Some(&mut |e| events.push(e)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.phases[0].work.completed_steps, 2);
    assert_eq!(r.phases[0].work.evaluations, 5);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Exact(ExactPhaseTermination::Condition)
    );
    assert_eq!(r.phases[1].work.completed_steps, 7);
    assert_eq!(events_summary(&events), (9, 9));
    assert!(r.is_optimal());
    let global = SearchTermination::Condition(SearchLimit::Steps(3));
    let mut c = plan(vec![exact(false, 100, PartialScoreBound::Unknown)]);
    c.limits.termination = Some(&global);
    let r = solve(&p, &n, c).unwrap();
    assert_eq!(r.work.completed_steps, 3);
    assert_eq!(r.work.evaluations, 7);
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.assignment.values(), &[Some(0); 3]);
}
#[test]
fn shared_counts_accumulate_across_completion_phases_and_phase_counts_reset() {
    let p = problem(3);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let phase = SearchTermination::Condition(SearchLimit::Steps(2));
    let global = SearchTermination::Condition(SearchLimit::Steps(3));
    let bindings = [
        PhaseTerminationBinding::search(0, &phase),
        PhaseTerminationBinding::search(1, &phase),
    ];
    let mut c = plan(vec![
        exact(true, 100, PartialScoreBound::Unknown),
        exact(true, 100, PartialScoreBound::Unknown),
    ]);
    c.limits.termination = Some(&global);
    let r = solve_with_extensions(
        &p,
        &n,
        c,
        SolverExtensions {
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        r.phases
            .iter()
            .map(|p| p.work.completed_steps)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(r.work.completed_steps, 3);
    assert_eq!(r.termination, SolverTermination::Condition);
}
#[test]
fn stagnation_counts_completed_expansions_and_best_discovery_resets_it() {
    let p = problem(3);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let condition = SearchTermination::Condition(SearchLimit::UnimprovedSteps(3));
    let mut c = plan(vec![exact(false, 100, PartialScoreBound::Unknown)]);
    c.limits.termination = Some(&condition);
    let r = solve(&p, &n, c).unwrap();
    // Root and first depth1 expansion have no leaf; third expansion discovers
    // the best and resets stagnation. Three subsequent equal-score expansions stop.
    assert_eq!(r.work.completed_steps, 6);
    assert_eq!(r.work.evaluations, 13);
    assert_eq!(r.termination, SolverTermination::Condition);
    let mut c = plan(vec![exact(false, 100, PartialScoreBound::Unknown)]);
    c.limits.max_unimproved_steps = Some(2);
    let r = solve(&p, &n, c).unwrap();
    assert_eq!(r.work.completed_steps, 2);
    assert_eq!(r.work.evaluations, 5);
    assert_eq!(r.termination, SolverTermination::UnimprovedStepLimit);
}
#[test]
fn metric_only_observation_records_parent_scores_without_rescoring() {
    let p = problem(2);
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("count", vec![0, 1], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let metrics = SolverMetrics::default();
    let r = solve_with_extensions(
        &p,
        &n,
        plan(vec![exact(false, 100, PartialScoreBound::Unknown)]),
        SolverExtensions {
            metrics: Some(&metrics),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 7);
    assert_eq!(r.work.completed_steps, 3);
    let s = metrics.snapshot().latest.unwrap();
    assert_eq!(s.completed_step_count, 3);
    assert_eq!(s.work, r.work);
    let last = s.last_step.unwrap();
    assert_eq!(last.kind, SolverPhaseKind::Exact);
    assert_eq!(last.step, 2);
    assert!(!last.complete);
    assert_eq!(last.score.structural(), -1);
}
#[test]
fn cancellation_at_expansion_start_and_score_errors_close_incomplete_steps() {
    let p = problem(2);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let token = CancellationToken::new();
    let mut events = vec![];
    let mut c = plan(vec![exact(false, 100, PartialScoreBound::Unknown)]);
    c.limits.cancellation = Some(&token);
    let r = solve_with_extensions(
        &p,
        &n,
        c,
        SolverExtensions {
            lifecycle: Some(&mut |e| {
                events.push(e);
                if matches!(e, SolverLifecycleEvent::StepStarted { .. }) {
                    token.cancel();
                }
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.work.completed_steps, 0);
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(events_summary(&events), (1, 0));
    // Checked score aggregation fails only after the root, within an expansion.
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("first", vec![0], |v| {
        SimpleScore::new(if v[0].is_some() { i64::MAX } else { 0 })
    })
    .unwrap();
    n.add_constraint("second", vec![0], |v| {
        SimpleScore::new(i64::from(v[0].is_some()))
    })
    .unwrap();
    events.clear();
    let r = solve_with_extensions(
        &p,
        &n,
        plan(vec![exact(false, 100, PartialScoreBound::Unknown)]),
        SolverExtensions {
            lifecycle: Some(&mut |e| events.push(e)),
            ..Default::default()
        },
    );
    assert!(r.is_err());
    assert_eq!(events_summary(&events), (1, 0));
    assert_eq!(events.last(), Some(&SolverLifecycleEvent::SolveEnded));
}
#[test]
fn callback_panics_propagate_without_emitting_callbacks_during_unwind() {
    let p = problem(2);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let mut events = vec![];
    let r = catch_unwind(AssertUnwindSafe(|| {
        solve_with_extensions(
            &p,
            &n,
            plan(vec![exact(false, 100, PartialScoreBound::Unknown)]),
            SolverExtensions {
                lifecycle: Some(&mut |e| {
                    events.push(e);
                    assert!(
                        !matches!(e, SolverLifecycleEvent::StepStarted { .. }),
                        "listener panic"
                    );
                }),
                ..Default::default()
            },
        )
    }));
    assert!(r.is_err());
    assert!(matches!(
        events.last(),
        Some(SolverLifecycleEvent::StepStarted { .. })
    ));
}
#[test]
fn complete_roots_pruned_roots_zero_limits_and_leaf_enumeration_do_not_invent_steps() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let condition = SearchTermination::Condition(SearchLimit::Steps(1));
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
    ] {
        let mut c = plan(vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            ..Default::default()
        })]);
        c.limits.termination = Some(&condition);
        let mut events = vec![];
        let r = solve_with_extensions(
            &p,
            &n,
            c,
            SolverExtensions {
                lifecycle: Some(&mut |e| events.push(e)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.work.completed_steps, 0);
        assert_eq!(events_summary(&events), (0, 0));
    }
    let zero = SearchTermination::Condition(SearchLimit::Steps(0));
    let bindings = [PhaseTerminationBinding::search(0, &zero)];
    let r = solve_with_extensions(
        &p,
        &n,
        plan(vec![exact(false, 100, PartialScoreBound::Unknown)]),
        SolverExtensions {
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 0);
    assert_eq!(r.work.completed_steps, 0);
    // Completion root supplies the incumbent; an equal admissible bound prunes
    // the second root before expansion. No step is synthesized for pruning.
    let r = solve(
        &p,
        &n,
        plan(vec![
            exact(true, 100, PartialScoreBound::Unknown),
            exact(false, 100, PartialScoreBound::NonIncreasing),
        ]),
    )
    .unwrap();
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.work.completed_steps, 0);
    assert_eq!(r.work.evaluations, 2);
    assert!(r.is_optimal());
}
