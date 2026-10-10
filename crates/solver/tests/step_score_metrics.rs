//! Original step score observability and failure regressions.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn model() -> (
    FiniteProblem<i32>,
    ConstraintNetwork<'static, i32, SimpleScore>,
) {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(v[0].copied().unwrap_or(0)))
    })
    .unwrap();
    (p, n)
}
#[test]
fn worsening_custom_steps_remain_visible_after_best_restoration_and_exclude_previews() {
    let (p, n) = model();
    let m = SolverMetrics::default();
    let mut events = vec![];
    let mut lifecycle = |e| {
        if let SolverLifecycleEvent::StepEnded {
            phase,
            kind,
            step,
            completed,
        } = e
        {
            assert!(completed);
            let snapshot = m.snapshot();
            let run = &snapshot.active[0];
            let last = run.last_step.as_ref().unwrap();
            assert_eq!((last.phase, last.kind, last.step), (phase, kind, step));
            assert!(last.complete);
            assert_eq!(run.completed_step_count, step + 1);
            events.push(last.score);
        }
    };
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        ScoringError,
    >| {
        c.evaluate(
            &Move::Change {
                variable: 0,
                value: Some(2),
            },
            |_, _| (),
        )?;
        assert!(m.snapshot().active[0].last_step.is_none());
        for value in [1, 2, 1] {
            c.execute(&Move::Change {
                variable: 0,
                value: Some(value),
            })?;
        }
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        SolverExtensions {
            metrics: Some(&m),
            lifecycle: Some(&mut lifecycle),
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        events,
        vec![
            SimpleScore::new(1),
            SimpleScore::new(2),
            SimpleScore::new(1)
        ]
    );
    let last = m.snapshot().latest.unwrap();
    assert_eq!(last.best_score, Some(SimpleScore::new(2)));
    assert_eq!(last.last_step.unwrap().score, SimpleScore::new(1));
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(last.completed_step_count, 3);
}
#[test]
fn failed_and_panicking_attempts_keep_last_completed_step_and_new_runs_reset() {
    for panic in [false, true] {
        let (p, n) = model();
        let m = SolverMetrics::default();
        let mut lifecycle = |e| {
            if panic && matches!(e, SolverLifecycleEvent::StepStarted { step: 1, .. }) {
                panic!("step failure");
            }
        };
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| {
            c.execute(&Move::Change {
                variable: 0,
                value: Some(1),
            })?;
            c.execute(&Move::Change {
                variable: 99,
                value: Some(2),
            })?;
            Ok(())
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            solve_with_extensions(
                &p,
                &n,
                SolverSequence {
                    phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
                    ..Default::default()
                },
                SolverExtensions {
                    metrics: Some(&m),
                    lifecycle: Some(&mut lifecycle),
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let last = m.snapshot().latest.unwrap();
        assert_eq!(last.completed_step_count, 1);
        assert_eq!(last.last_step.unwrap().score, SimpleScore::new(1));
        assert_eq!(
            last.outcome,
            Some(if panic {
                SolveMetricOutcome::Panicked
            } else {
                SolveMetricOutcome::Failed
            })
        );
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                ..Default::default()
            },
            SolverExtensions {
                metrics: Some(&m),
                ..Default::default()
            },
        )
        .unwrap();
        let last = m.snapshot().latest.unwrap();
        assert_eq!(last.completed_step_count, 0);
        assert!(last.last_step.is_none());
    }
}

#[test]
fn catching_a_custom_lifecycle_panic_does_not_turn_the_phase_into_success() {
    for at_end in [false, true] {
        let (p, n) = model();
        let m = SolverMetrics::default();
        let mut lifecycle = |event| {
            if (at_end
                && matches!(
                    event,
                    SolverLifecycleEvent::StepEnded {
                        completed: true,
                        ..
                    }
                ))
                || (!at_end && matches!(event, SolverLifecycleEvent::StepStarted { .. }))
            {
                panic!("caught callback");
            }
        };
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| {
            assert!(
                catch_unwind(AssertUnwindSafe(|| c.execute(&Move::Change {
                    variable: 0,
                    value: Some(1)
                })))
                .is_err()
            );
            Ok(())
        };
        let result = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
                ..Default::default()
            },
            SolverExtensions {
                metrics: Some(&m),
                lifecycle: Some(&mut lifecycle),
                command: Some(&mut command),
                ..Default::default()
            },
        );
        assert!(result.is_err());
        let last = m.snapshot().latest.unwrap();
        assert_eq!(last.outcome, Some(SolveMetricOutcome::Failed));
        assert_eq!(last.completed_step_count, u64::from(at_end));
    }
}
