// Adapted from Timefold Community SolverMetricsIT.solveStepScoreMetrics.
// Apache-2.0; provenance and helper attribution in compat/provenance.json.
// Native StepStarted observes the previous step, corresponding to the source's
// StepEnded listener which runs before the metric listener publishes that step.
use timetuck::*;
#[test]
fn solve_step_score_metrics() {
    let p = FiniteProblem::new(vec![Variable::new(vec!["none", "reward"]); 3]).unwrap();
    let mut n = ConstraintNetwork::<&'static str, HardSoftScore>::new(3);
    n.add_constraint("reward", vec![0, 1, 2], |values| {
        HardSoftScore::new(
            0,
            values
                .iter()
                .filter(|v| v.is_some_and(|v| v.starts_with("reward")))
                .count() as i64,
        )
    })
    .unwrap();
    let metrics = SolverMetrics::<HardSoftScore>::default();
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
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<&str>,
        Move<&str>,
        HardSoftScore,
        ScoringError,
    >| {
        for (variable, value) in [
            (0, "reward"),
            (1, "reward"),
            (0, "none"),
            (0, "reward"),
            (2, "reward"),
        ] {
            c.execute(&Move::Change {
                variable,
                value: Some(value),
            })?;
        }
        Ok(())
    };
    let mut step = -1;
    let mut lifecycle = |event| {
        if !matches!(event, SolverLifecycleEvent::StepStarted { .. }) {
            return;
        }
        if step < 2 {
            step += 1;
            return;
        }
        let score = metrics.snapshot().active[0]
            .last_step
            .as_ref()
            .unwrap()
            .score;
        assert_eq!(score.hard(), 0);
        match step {
            2 => assert_eq!(score.soft(), 0),
            3 => assert_eq!(score.soft(), 1),
            4 => assert_eq!(score.soft(), 2),
            5 => assert_eq!(score.soft(), 1),
            6 => assert_eq!(score.soft(), 2),
            _ => unreachable!(),
        }
        step += 1;
    };
    let mut ext = SolverExtensions {
        metrics: Some(&metrics),
        lifecycle: Some(&mut lifecycle),
        command: Some(&mut command),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let solution = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                best_score_limit: Some(HardSoftScore::new(0, 3)),
                ..Default::default()
            },
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::Custom(CustomPhaseConfig::default()),
            ],
        },
        ext,
    )
    .unwrap();
    assert_eq!(step, 7);
    assert!(solution.complete);
    let score = metrics.snapshot().latest.unwrap().last_step.unwrap().score;
    assert_eq!(score.hard(), 0);
    assert_eq!(score.soft(), 3);
}
