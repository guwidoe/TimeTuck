// Adapted from pinned Community SolverMetricsIT.solveBestScoreMetrics and
// esCustomMetrics, Apache-2.0. See compat/provenance.json for substitutions.
use timetuck::*;
#[test]
fn solve_best_score_metrics() {
    let p = FiniteProblem::new(vec![Variable::new(vec!["none", "reward"]); 2]).unwrap();
    let mut n = ConstraintNetwork::<&'static str, HardSoftScore>::new(2);
    n.add_constraint("reward", vec![0, 1], |values| {
        HardSoftScore::new(
            0,
            values
                .iter()
                .filter(|v| v.is_some_and(|v| v.starts_with("reward")))
                .count() as i64,
        )
    })
    .unwrap();
    let metrics = SolverMetrics::<HardSoftScore>::default().with_best_metrics_after_callbacks();
    let mut provider = |_: &FiniteProblem<&str>, a: &Assignment<&str>, _: u64| {
        a.values()
            .iter()
            .position(Option::is_none)
            .map(|variable| Move::Change {
                variable,
                value: Some("none"),
            })
            .into_iter()
            .collect::<Vec<_>>()
    };
    let mut step = -1;
    let mut observed = false;
    let mut events = |event: SolverEvent<'_, Assignment<&str>, HardSoftScore>| {
        let SolverEvent::Best(update) = event else {
            return;
        };
        // Native partial improvements are separate events; this adapter observes
        // initialized solutions, matching the source listener's three deliveries.
        if !update.complete {
            return;
        }
        let snapshot = metrics.snapshot();
        let best = snapshot.active[0].best_score;
        if step != -1 {
            assert_eq!(best.unwrap().hard(), 0);
        }
        match step {
            0 => assert_eq!(best.unwrap().soft(), 0),
            1 => assert_eq!(best.unwrap().soft(), 1),
            2 => assert_eq!(best.unwrap().soft(), 2),
            -1 => {}
            _ => panic!("extra initialized best event"),
        }
        step += 1;
        observed = true;
    };
    let mut extensions = SolverExtensions {
        metrics: Some(&metrics),
        events: Some(&mut events),
        ..Default::default()
    };
    extensions.move_providers.bind(0, &mut provider);
    let solution = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                }),
            ],
            limits: SolverLimits {
                best_score_limit: Some(HardSoftScore::new(0, 2)),
                ..Default::default()
            },
        },
        extensions,
    )
    .unwrap();
    assert!(observed);
    assert_eq!(step, 2);
    assert!(solution.complete);
    assert_eq!(solution.score, Some(HardSoftScore::new(0, 2)));
    let best = metrics.snapshot().latest.unwrap().best_score.unwrap();
    assert_eq!(best.hard(), 0);
    assert_eq!(best.soft(), 2);
}
#[test]
fn es_custom_metrics() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2, 3]),
        Variable::new(vec![1, 2, 3]).with_initial(2),
        Variable::new(vec![1, 2, 3]).with_initial(1),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let metrics = SolverMetrics::default();
    let mut count = 0;
    let mut lifecycle = |event| {
        if event == SolverLifecycleEvent::SolveEnded {
            count = metrics.snapshot().active[0]
                .move_evaluations
                .get(MoveEvaluationKind::ExactAssignment);
        }
    };
    solve_with_extensions(
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
            lifecycle: Some(&mut lifecycle),
            ..Default::default()
        },
    )
    .unwrap();
    // Native exhaustive completion scores whole assignments; that native family
    // replaces Java ChangeMove(TestdataEntity.value), without a trace/count claim.
    assert!(count > 0);
}
