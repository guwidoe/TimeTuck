//! Original candidate-accounting checks across phase kinds and exceptional exits.
use timetuck::*;
fn p() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1); 2]).unwrap()
}
fn counts<A>(
    m: &SolverMetrics<SimpleScore>,
    r: &SolverResult<A, SimpleScore>,
) -> MoveEvaluationCounts {
    let c = m.snapshot().latest.unwrap().move_evaluations;
    assert_eq!(c.total(), r.work.move_evaluations);
    c
}
#[test]
fn exact_enumeration_completion_and_bounded_nodes_are_counted_without_initial_rescore() {
    let p = p();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    for (strategy, kind) in [
        (
            ExactSearchStrategy::Enumerate,
            MoveEvaluationKind::ExactAssignment,
        ),
        (
            ExactSearchStrategy::Complete,
            MoveEvaluationKind::ExactAssignment,
        ),
        (
            ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            MoveEvaluationKind::ExactNode,
        ),
    ] {
        let m = SolverMetrics::default();
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                metrics: Some(&m),
                ..Default::default()
            },
        )
        .unwrap();
        let c = counts(&m, &r);
        assert!(c.get(kind) > 0);
        assert_eq!(c.get(kind), r.work.evaluations);
    }
}
#[test]
fn scored_rejections_duplicates_and_composites_count_once_and_are_live_before_step_end() {
    let p = p();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let m = SolverMetrics::default();
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Swap { left: 0, right: 1 },
            Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                Move::Change {
                    variable: 1,
                    value: Some(2),
                },
            ]),
        ]
    };
    let mut steps = 0;
    let mut lifecycle = |e| {
        if matches!(e, SolverLifecycleEvent::StepEnded { .. }) {
            assert_eq!(m.snapshot().active[0].move_evaluations.total(), 4);
            steps += 1;
        }
    };
    let mut ext = SolverExtensions {
        metrics: Some(&m),
        lifecycle: Some(&mut lifecycle),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    let c = counts(&m, &r);
    assert_eq!(steps, 1);
    assert_eq!(r.work.accepted_moves, 0);
    assert_eq!(c.get(MoveEvaluationKind::Change), 2);
    assert_eq!(c.get(MoveEvaluationKind::Swap), 1);
    assert_eq!(c.get(MoveEvaluationKind::Composite), 1);
}
#[test]
fn error_preserves_successful_candidate_counts_and_next_run_starts_empty() {
    let p = p();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let m = SolverMetrics::default();
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        vec![
            Move::Change {
                variable: 0,
                value: Some(2),
            },
            Move::Change {
                variable: 99,
                value: Some(2),
            },
        ]
    };
    let mut ext = SolverExtensions {
        metrics: Some(&m),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let config = SolverSequence {
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
        ..Default::default()
    };
    assert!(solve_with_extensions(&p, &n, config, ext).is_err());
    let s = m.snapshot().latest.unwrap();
    assert_eq!(s.outcome, Some(SolveMetricOutcome::Failed));
    assert_eq!(s.move_evaluations.get(MoveEvaluationKind::Change), 1);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 0,
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&m),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(counts(&m, &r).total(), 0);
}
#[test]
fn custom_trials_commits_and_limits_exclude_diagnostics_and_initialization() {
    let p = p();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let m = SolverMetrics::default();
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        ScoringError,
    >| {
        let movement = Move::Change {
            variable: 0,
            value: Some(2),
        };
        c.evaluate(&movement, |_, _| ())?;
        c.assert_consistent()?;
        c.execute(&movement)?;
        assert_eq!(
            m.snapshot().active[0]
                .move_evaluations
                .get(MoveEvaluationKind::Custom),
            2
        );
        assert!(c.execute(&movement)?.is_none());
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig {
                max_move_evaluations: Some(2),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&m),
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(counts(&m, &r).get(MoveEvaluationKind::Custom), 2);
    assert_eq!(r.work.evaluations, 3);
}
#[test]
fn reconstruction_and_fine_attempts_use_distinct_counts() {
    let p = p();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    for weights in [
        RepairMoveWeights {
            repair: 1,
            change: 0,
            swap: 0,
        },
        RepairMoveWeights {
            repair: 0,
            change: 1,
            swap: 0,
        },
    ] {
        let m = SolverMetrics::default();
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                    move_weights: weights,
                    seed: 7,
                    max_attempts: 3,
                    minimum_ruined: 1,
                    maximum_ruined: 1,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                metrics: Some(&m),
                ..Default::default()
            },
        )
        .unwrap();
        let c = counts(&m, &r);
        let kind = if weights.repair > 0 {
            MoveEvaluationKind::Reconstruction
        } else {
            MoveEvaluationKind::FineRepair
        };
        assert!(c.get(kind) > 0);
        assert_eq!(c.get(kind), c.total());
        let snapshot = m.snapshot().latest.unwrap();
        assert_eq!(snapshot.completed_step_count, r.work.completed_steps);
        if let Some(step) = snapshot.last_step {
            assert_eq!(step.kind, SolverPhaseKind::RuinRecreate);
            assert!(step.complete);
        }
    }
}
#[test]
fn provider_construction_skips_invalid_proposals_and_counts_composites_once() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let m = SolverMetrics::default();
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        vec![
            Move::Change {
                variable: 99,
                value: Some(2),
            },
            Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                Move::Change {
                    variable: 1,
                    value: Some(2),
                },
            ]),
        ]
    };
    let mut ext = SolverExtensions {
        metrics: Some(&m),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(counts(&m, &r).get(MoveEvaluationKind::Composite), 1);
    assert_eq!(counts(&m, &r).total(), 1);
    let snapshot = m.snapshot().latest.unwrap();
    assert_eq!(snapshot.completed_step_count, 1);
    let step = snapshot.last_step.unwrap();
    assert_eq!(step.score, r.score.unwrap());
    assert!(step.complete);
}
