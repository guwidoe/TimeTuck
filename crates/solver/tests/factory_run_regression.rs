//! Original solve-wide factory controls; no upstream-method credit.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::RefCell, io, rc::Rc, time::Duration};
use timetuck::*;

fn with_three_phases(
    d: &Domain,
    stream: bool,
    o: Rc<Observations>,
    limits: FactoryRunLimits,
) -> Factory<'_> {
    let mut c = configuration(d, Some(EnvironmentMode::NoAssert), vec![], 10);
    c.phases = vec![
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_evaluations: 1,
            ..Default::default()
        });
        3
    ];
    c.run_limits = limits;
    declaration(d, stream, o)
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap()
}
#[test]
fn logical_budget_is_shared_and_overrides_are_independent() {
    for stream in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let o = Rc::new(Observations::default());
        let f = with_three_phases(
            &d,
            stream,
            o.clone(),
            FactoryRunLimits {
                max_evaluations: Some(2),
                max_time: None,
            },
        );
        let mut a = f
            .assembled_solver_builder()
            .build_assembled_run(d.model.initial_assignment())
            .unwrap();
        let mut b = f
            .assembled_solver_builder()
            .with_factory_run_limits(FactoryRunLimits {
                max_evaluations: Some(1),
                max_time: None,
            })
            .build_assembled_run(d.model.initial_assignment())
            .unwrap();
        let mut c = f
            .assembled_solver_builder()
            .with_factory_run_limits(FactoryRunLimits::default())
            .build_assembled_run(d.model.initial_assignment())
            .unwrap();
        for _ in 0..2 {
            a.run_next_assembled_phase(|r, p| r.phase(p))
                .unwrap()
                .unwrap();
        }
        b.run_next_assembled_phase(|r, p| r.phase(p))
            .unwrap()
            .unwrap();
        assert!(
            a.run_next_assembled_phase::<Work>(|_, _| panic!("exhausted default budget"))
                .unwrap()
                .is_none()
        );
        assert!(
            b.run_next_assembled_phase::<Work>(|_, _| panic!("exhausted override budget"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            a.assembled_run_termination(),
            Some(FactoryRunTermination::EvaluationLimit)
        );
        assert_eq!(
            b.assembled_run_termination(),
            Some(FactoryRunTermination::EvaluationLimit)
        );
        for _ in 0..3 {
            c.run_next_assembled_phase(|r, p| r.phase(p))
                .unwrap()
                .unwrap();
        }
        assert!(
            c.run_next_assembled_phase::<Work>(|_, _| panic!("finished"))
                .unwrap()
                .is_none()
        );
        assert_eq!(a.finish_assembled_state().unwrap().2, 2);
        assert_eq!(b.finish_assembled_state().unwrap().2, 1);
        assert_eq!(c.finish_assembled_state().unwrap().2, 3);
        assert_eq!(o.instances.get(), 3);
        assert_eq!(o.releases.get(), 3);
        assert_eq!(
            f.assembled_solver_builder()
                .factory_run_limits()
                .max_evaluations,
            Some(2)
        );
    }
}
#[test]
fn remaining_budget_clamps_real_candidates_and_stricter_phase_limits() {
    for stream in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let mut c = configuration(&d, Some(EnvironmentMode::NoAssert), vec![], 10);
        c.phases = vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: 1,
                ..Default::default()
            }),
            SolverPhase::LocalSearch(Default::default()),
            SolverPhase::LocalSearch(Default::default()),
        ];
        c.run_limits = FactoryRunLimits {
            max_evaluations: Some(3),
            max_time: Some(Duration::from_secs(60)),
        };
        let f = declaration(&d, stream, Rc::new(Observations::default()))
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let mut seen = vec![];
        let result = f
            .solve_assembled_with(d.model.initial_assignment(), |r, p| {
                let SolverPhase::LocalSearch(c) = p else {
                    unreachable!()
                };
                seen.push(c.max_evaluations);
                assert!(c.max_time.unwrap() <= Duration::from_secs(60));
                r.phase(p)
            })
            .unwrap();
        assert_eq!(seen, [1, 2]);
        assert_eq!(result.phases.len(), 2);
        assert_eq!(result.logical_calculations, 3);
        assert_eq!(result.termination, FactoryRunTermination::EvaluationLimit);
        assert_ne!(
            result.working.values(),
            d.model.initial_assignment().values()
        );
        assert_eq!(result.score, result.phases.last().unwrap().0.score.unwrap());
    }
}
#[test]
fn zero_budgets_skip_native_phases_and_still_close_resources() {
    for stream in [false, true] {
        for (limits, reason) in [
            (
                FactoryRunLimits {
                    max_evaluations: Some(0),
                    max_time: None,
                },
                FactoryRunTermination::EvaluationLimit,
            ),
            (
                FactoryRunLimits {
                    max_time: Some(Duration::ZERO),
                    max_evaluations: None,
                },
                FactoryRunTermination::TimeLimit,
            ),
        ] {
            let d = Domain::new(3, 3, Objective::Weighted);
            let o = Rc::new(Observations::default());
            let f = with_three_phases(&d, stream, o.clone(), limits);
            let view = RefCell::<
                Option<FactoryRuntimeView<'_, Model, Prepared<'_>, Assignment<i64>, Resource<'_>>>,
            >::new(None);
            let events = RefCell::new(vec![]);
            let mut hook = |event, h| {
                *view.borrow_mut() = Some(h);
                events.borrow_mut().push(event);
                Ok::<_, io::Error>(())
            };
            let mut callbacks = FactoryRunCallbacks {
                lifecycle: Some(&mut hook),
                failure: None,
            };
            let result = f
                .solve_observed_assembled_with::<Work>(
                    d.model.initial_assignment(),
                    &mut callbacks,
                    |_, _| panic!("zero budget"),
                )
                .unwrap();
            assert!(result.phases.is_empty());
            assert_eq!(result.logical_calculations, 0);
            assert_eq!(result.termination, reason);
            assert_eq!(
                result.working.values(),
                d.model.initial_assignment().values()
            );
            assert_eq!(
                *events.borrow(),
                [FactoryRunEvent::Started, FactoryRunEvent::Finished]
            );
            assert!(view.borrow().as_ref().unwrap().runtime_view_closed());
            assert_eq!(o.releases.get(), 1);
        }
    }
}
#[test]
fn elapsed_hook_time_belongs_to_the_whole_solve() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let o = Rc::new(Observations::default());
    let f = with_three_phases(
        &d,
        true,
        o.clone(),
        FactoryRunLimits {
            max_time: Some(Duration::from_millis(1)),
            max_evaluations: None,
        },
    );
    let mut hook = |event, _| {
        if event == FactoryRunEvent::Started {
            std::thread::sleep(Duration::from_millis(3));
        }
        Ok::<_, io::Error>(())
    };
    let mut callbacks = FactoryRunCallbacks {
        lifecycle: Some(&mut hook),
        failure: None,
    };
    let result = f
        .solve_observed_assembled_with::<Work>(
            d.model.initial_assignment(),
            &mut callbacks,
            |_, _| panic!("deadline passed in hook"),
        )
        .unwrap();
    assert_eq!(result.termination, FactoryRunTermination::TimeLimit);
    assert!(result.phases.is_empty());
    assert_eq!(o.releases.get(), 1);
}
#[test]
fn unreachable_rejection_precedes_preparation_and_cannot_be_removed_by_override() {
    let d = Domain::new(2, 2, Objective::Zero);
    let o = Rc::new(Observations::default());
    let open = SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: u64::MAX,
        max_evaluations: u64::MAX,
        max_accepted_moves: u64::MAX,
        ..Default::default()
    });
    let mut c = configuration(&d, None, vec![], 10);
    c.phases = vec![open.clone(), open];
    let err = declaration(&d, false, o.clone())
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    assert!(matches!(err, FactoryAssemblyError::UnreachablePhase { .. }));
    assert_eq!(o.preparations.get(), 0);
    assert_eq!(o.instances.get(), 0);
    let mut c = configuration(&d, None, vec![], 10);
    c.phases = vec![
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: u64::MAX,
            max_evaluations: u64::MAX,
            max_accepted_moves: u64::MAX,
            ..Default::default()
        }),
        SolverPhase::LocalSearch(Default::default()),
    ];
    c.run_limits.max_evaluations = Some(0);
    let f = declaration(&d, false, o.clone())
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap();
    assert!(
        f.assembled_solver_builder()
            .with_factory_run_limits(FactoryRunLimits::default())
            .build_assembled_run(d.model.initial_assignment())
            .is_err()
    );
    assert_eq!(o.instances.get(), 0);
    let mut s = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    assert!(
        s.run_next_assembled_phase::<Work>(|_, _| panic!("zero"))
            .unwrap()
            .is_none()
    );
    assert_eq!(s.finish_assembled_state().unwrap().2, 0);
}

#[test]
fn ignoring_a_budget_is_a_typed_failure_with_resource_cleanup() {
    for observed in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let o = Rc::new(Observations::default());
        let f = with_three_phases(
            &d,
            false,
            o.clone(),
            FactoryRunLimits {
                max_evaluations: Some(1),
                max_time: None,
            },
        );
        let mut session = f.start_assembled_run(d.model.initial_assignment()).unwrap();
        let mut ignores_limit = |r: &mut Resource<'_>, _: &SolverPhase<'_, SimpleScore>| {
            r.phase(&SolverPhase::LocalSearch(Default::default()))
        };
        let error = if observed {
            session
                .run_next_observed_phase(&mut FactoryRunCallbacks::default(), &mut ignores_limit)
                .err()
                .unwrap()
        } else {
            session
                .run_next_assembled_phase(&mut ignores_limit)
                .err()
                .unwrap()
        };
        assert!(
            matches!(error.primary,FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::EvaluationBudgetExceeded{limit:1,actual})) if actual>1)
        );
        assert_eq!(o.releases.get(), 1);
        // Closure errors remain visible even after the failed binding used the budget.
        assert!(
            session
                .run_next_assembled_phase::<Work>(|_, _| panic!("closed"))
                .is_err()
        );
    }
}

#[test]
fn observed_override_uses_one_budget_and_preserves_mode_rebuild_counts() {
    for stream in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let o = Rc::new(Observations::default());
        let mut c = configuration(
            &d,
            Some(EnvironmentMode::NoAssert),
            vec![
                Some(EnvironmentMode::FullAssert),
                Some(EnvironmentMode::PhaseAssert),
            ],
            10,
        );
        c.phases = vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_evaluations: 1,
                ..Default::default()
            }),
            SolverPhase::LocalSearch(Default::default()),
        ];
        let f = declaration(&d, stream, o.clone())
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let mut session = f
            .assembled_solver_builder()
            .with_factory_run_limits(FactoryRunLimits {
                max_evaluations: Some(3),
                max_time: None,
            })
            .build_observed_assembled_run(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks::default(),
            )
            .unwrap();
        let view = session.assembled_runtime_view();
        let mut counts = vec![];
        for expected in [1, 2] {
            session
                .run_next_observed_phase(&mut FactoryRunCallbacks::default(), |r, p| {
                    let SolverPhase::LocalSearch(c) = p else {
                        unreachable!()
                    };
                    assert_eq!(c.max_evaluations, expected);
                    r.phase(p)
                })
                .unwrap()
                .unwrap();
            counts.push(session.assembled_runtime_view().runtime_view_calculations());
        }
        assert_eq!(counts, [1, 3]);
        assert!(
            session
                .run_next_observed_phase::<Work>(
                    &mut FactoryRunCallbacks::default(),
                    |_, _| panic!("finished")
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(
            session.assembled_run_termination(),
            Some(FactoryRunTermination::PhasesCompleted)
        );
        assert_eq!(
            session
                .finish_observed_assembled_state(&mut FactoryRunCallbacks::default())
                .unwrap()
                .2,
            3
        );
        assert!(view.runtime_view_closed());
        assert_eq!(o.instances.get(), 3);
        assert_eq!(o.releases.get(), 3);
        assert_eq!(
            f.assembled_solver_builder().factory_run_limits(),
            FactoryRunLimits::default()
        );
    }
}
