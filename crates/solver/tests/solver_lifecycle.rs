//! Original lifecycle ordering, termination, callbacks and backend equivalence.
mod lifecycle_support;
use lifecycle_support::*;
use timetuck::*;
fn ordered(events: &[SolverLifecycleEvent], expected_steps: u64) {
    use SolverLifecycleEvent::*;
    assert_eq!(events.first(), Some(&SolveStarted));
    assert_eq!(events.last(), Some(&SolveEnded));
    let mut phase = None;
    let mut step = None;
    let mut next = 0;
    let mut completed = 0;
    for event in events {
        match *event {
            SolveStarted | SolveEnded => assert!(phase.is_none()),
            PhaseStarted { phase: p, kind } => {
                assert!(phase.replace((p, kind)).is_none());
                next = 0;
            }
            PhaseEnded { phase: p, kind } => {
                assert_eq!(phase.take(), Some((p, kind)));
                assert!(step.is_none());
            }
            StepStarted {
                phase: p,
                kind,
                step: s,
            } => {
                assert_eq!(phase, Some((p, kind)));
                assert_eq!(s, next);
                assert!(step.replace(s).is_none());
            }
            StepEnded {
                phase: p,
                kind,
                step: s,
                completed: done,
            } => {
                assert_eq!(phase, Some((p, kind)));
                assert_eq!(step.take(), Some(s));
                next += 1;
                completed += u64::from(done);
            }
        }
    }
    assert_eq!(completed, expected_steps);
}
#[test]
fn all_models_and_backends_emit_same_order_without_changing_work() {
    let bp = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let lp = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    let mut reference = None;
    for backend in 0..4 {
        let mut all = vec![];
        macro_rules! run {
            ($call:expr,$plain:expr) => {{
                let mut events = vec![];
                let mut callback = |e| events.push(e);
                let r = $call(Some(&mut callback));
                let plain = $plain;
                assert_eq!(r.assignment, plain.assignment);
                assert_eq!(r.score, plain.score);
                assert_eq!(r.work, plain.work);
                ordered(&events, r.work.construction_steps + r.work.completed_steps);
                all.push(events);
            }};
        }
        run!(
            |cb| basic_run(&bp, backend, config(), cb),
            basic_run(&bp, backend, config(), None)
        );
        run!(
            |cb| list_run(&lp, backend, cb),
            list_run(&lp, backend, None)
        );
        run!(
            |cb| mixed_run(&mp, backend, cb),
            mixed_run(&mp, backend, None)
        );
        if let Some(previous) = &reference {
            assert_eq!(previous, &all);
        } else {
            reference = Some(all);
        }
    }
}
#[test]
fn cancellation_at_step_start_closes_attempt_and_skipped_phases_emit_nothing() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    for backend in 0..4 {
        let token = CancellationToken::new();
        let mut events = vec![];
        let mut c = config();
        c.limits.cancellation = Some(&token);
        let r = basic_run(
            &p,
            backend,
            c,
            Some(&mut |event| {
                events.push(event);
                if matches!(event, SolverLifecycleEvent::StepStarted { .. }) {
                    token.cancel();
                }
            }),
        );
        assert_eq!(r.termination, SolverTermination::Cancelled);
        ordered(&events, 0);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::PhaseStarted { .. }))
                .count(),
            1
        );
        assert!(events.iter().any(|e| matches!(
            e,
            SolverLifecycleEvent::StepEnded {
                completed: false,
                ..
            }
        )));
        events.clear();
        let mut c = config();
        c.limits.max_evaluations = 0;
        basic_run(&p, backend, c, Some(&mut |e| events.push(e)));
        ordered(&events, 0);
    }
}

#[test]
fn full_plan_and_provider_steps_are_not_duplicated_by_score_diagnostics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    for retained in [false, true] {
        let mut events = vec![];
        let mut callback = |e| events.push(e);
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            (0..p.variables().len())
                .flat_map(|variable| {
                    [1, 2].into_iter().filter_map(move |value| {
                        (a.values()[variable] != Some(value)).then_some(Move::Change {
                            variable,
                            value: Some(value),
                        })
                    })
                })
                .collect::<Vec<_>>()
        };
        let mut command = |_,
                           _: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            StreamScoringError,
        >| Ok(());
        let mut ext = SolverExtensions {
            lifecycle: Some(&mut callback),
            command: Some(&mut command),
            ..Default::default()
        };
        ext.move_providers
            .bind_shared([0, 3], &mut provider)
            .unwrap();
        let c = SolverSequence {
            limits: SolverLimits {
                assert_phase_scores: true,
                ..Default::default()
            },
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::Custom(CustomPhaseConfig::default()),
                SolverPhase::RuinRecreate(RuinRecreateConfig {
                    max_attempts: 4,
                    ..Default::default()
                }),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 2,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                }),
                SolverPhase::Exact(ExactPhaseConfig {
                    strategy: ExactSearchStrategy::Complete,
                    ..Default::default()
                }),
            ],
        };
        let r = if retained {
            let (g, s) = basic_graph();
            StreamScoreDirector::new(&p, g, s, p.initial_assignment())
                .unwrap()
                .run_solver_with_extensions(c, ext)
                .unwrap()
        } else {
            solve_stream_with_extensions(&p, || Ok(basic_graph()), c, ext).unwrap()
        };
        ordered(&events, r.work.construction_steps + r.work.completed_steps);
        assert_eq!(r.phases.len(), 5);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::PhaseStarted { .. }))
                .count(),
            5
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::StepStarted { phase: 2, .. }))
                .count() as u64,
            r.work.repair_attempts + r.work.fine_move_attempts
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, SolverLifecycleEvent::StepStarted { phase: 1 | 4, .. }))
        );
    }
}

#[test]
fn validation_errors_search_errors_and_panics_have_distinct_lifecycles() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
    for mode in 0..4 {
        let mut events = vec![];
        let mut callback = |e| {
            events.push(e);
            if mode == 3 && matches!(e, SolverLifecycleEvent::StepStarted { .. }) {
                panic!("callback failure");
            }
        };
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            assert_ne!(mode, 2, "provider failure");
            vec![Move::Change {
                variable: 99,
                value: Some(2),
            }]
        };
        let mut ext = SolverExtensions {
            lifecycle: Some(&mut callback),
            ..Default::default()
        };
        ext.move_providers
            .bind(if mode == 0 { 99 } else { 0 }, &mut provider);
        let c = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            })],
            ..Default::default()
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            solve_stream_with_extensions(&p, || Ok(basic_graph()), c, ext)
        }));
        if mode < 2 {
            assert!(outcome.unwrap().is_err());
            if mode == 0 {
                assert!(events.is_empty());
            } else {
                ordered(&events, 0);
                assert!(events.iter().any(|e| matches!(
                    e,
                    SolverLifecycleEvent::StepEnded {
                        completed: false,
                        ..
                    }
                )));
            }
        } else {
            assert!(outcome.is_err());
            assert!(matches!(
                events.last(),
                Some(SolverLifecycleEvent::StepStarted { .. })
            ));
        }
    }
}

#[test]
fn frozen_models_emit_only_solve_events_even_with_score_diagnostics() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    for backend in 0..4 {
        let mut c = config();
        c.limits.assert_phase_scores = true;
        let mut events = vec![];
        let r = basic_run(&p, backend, c, Some(&mut |e| events.push(e)));
        assert!(r.phases.is_empty());
        assert_eq!(
            events,
            [
                SolverLifecycleEvent::SolveStarted,
                SolverLifecycleEvent::SolveEnded
            ]
        );
    }
}
