// Adapted from Timefold Community SolverMetricsIT; Apache-2.0.
// Native snapshots and candidate families replace Micrometer names/tags.
// See compat/provenance.json for complete method attribution and adaptations.
use timetuck::*;
fn basic(initial: bool) -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        if initial {
            Variable::new(vec![1, 2, 3]).with_initial(3)
        } else {
            Variable::new(vec![1, 2, 3])
        },
        Variable::new(vec![1, 2, 3]).with_initial(2),
        Variable::new(vec![1, 2, 3]).with_initial(1),
    ])
    .unwrap()
}
#[test]
fn ch_with_custom_metrics() {
    let p = basic(false);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let metrics = SolverMetrics::default();
    let mut count = 0;
    let mut callback = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            count = metrics.snapshot().active[0]
                .move_evaluations
                .get(MoveEvaluationKind::Change);
        }
    };
    solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut callback),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(count > 0);
}
#[test]
fn ch_with_list_variable_and_custom_metrics() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![])]).unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 3);
    let metrics = SolverMetrics::default();
    let mut count = 0;
    let mut callback = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            count = metrics.snapshot().active[0]
                .move_evaluations
                .get(MoveEvaluationKind::ListAssign);
        }
    };
    solve_list_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut callback),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(count > 0);
}
#[test]
fn ls_with_custom_metrics() {
    let p = basic(true);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let metrics = SolverMetrics::default();
    let mut counts = MoveEvaluationCounts::default();
    let mut callback = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            counts = metrics.snapshot().active[0].move_evaluations;
        }
    };
    solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 20,
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut callback),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(counts.get(MoveEvaluationKind::Change) > 0);
    assert!(counts.get(MoveEvaluationKind::Swap) > 0);
}
#[test]
fn ls_with_list_variable_and_custom_metrics() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 3);
    let metrics = SolverMetrics::default();
    let mut counts = MoveEvaluationCounts::default();
    let mut callback = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            counts = metrics.snapshot().active[0].move_evaluations;
        }
    };
    // Explicit native union retains the source's change, swap and 2-opt families.
    // In a single list, 2-opt is represented by a reversed SubListChange.
    let union = NeighborhoodUnion {
        children: &[
            (Neighborhood::Change, 1),
            (Neighborhood::Swap, 1),
            (
                Neighborhood::TwoOpt {
                    cross_entity: false,
                },
                1,
            ),
        ],
        order: UnionOrder::Sequential,
    };
    solve_list_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: 20,
                neighborhood_union: Some(&union),
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut callback),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(counts.get(MoveEvaluationKind::ListChange) > 0);
    assert!(counts.get(MoveEvaluationKind::ListSwap) > 0);
    assert!(counts.get(MoveEvaluationKind::SubListChange) > 0);
}
#[test]
fn es_with_initialized_entities_and_metric() {
    let p = basic(false);
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let metrics = SolverMetrics::default();
    let mut best = None;
    let mut events = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if let SolverEvent::Best(u) = event {
            best = Some(u.assignment.clone());
        }
    };
    let solution = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::Complete,
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&metrics),
            events: Some(&mut events),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(best.is_some());
    // Stable native variable indices preserve source identities e1, e2, e3.
    assert!(solution.assignment.get(0).unwrap().is_some());
    assert_eq!(solution.assignment.get(1), Some(&Some(2)));
    assert_eq!(solution.assignment.get(2), Some(&Some(1)));
    let last = metrics.snapshot().latest.unwrap();
    assert!(last.work.evaluations > 0);
    assert!(last.work.move_evaluations > 0);
}
