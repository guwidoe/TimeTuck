//! Original integration regressions; no upstream method credit.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use EnvironmentMode::{FullAssert, NoAssert, PhaseAssert};
use fixture::*;
use std::{io, rc::Rc};
use timetuck::*;

#[test]
fn real_search_reuses_preparation_but_not_runtime_state() {
    for stream in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let o = Rc::new(Observations::default());
        let f = declaration(&d, stream, o.clone())
            .assemble_solver_factory(
                configuration(
                    &d,
                    Some(NoAssert),
                    vec![Some(FullAssert), Some(FullAssert)],
                    10,
                ),
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
            )
            .unwrap();
        for _ in 0..2 {
            let result = f
                .solve_assembled_with(d.model.initial_assignment(), |r, p| {
                    match p {
                        SolverPhase::Construction(c) => assert!(c.assert_scores),
                        SolverPhase::LocalSearch(c) => assert!(c.assert_scores),
                        _ => unreachable!(),
                    }
                    r.phase(p)
                })
                .unwrap();
            assert_eq!(result.score, SimpleScore::new(12));
            assert_eq!(result.working.values(), &[Some(2), Some(2), Some(2)]);
            assert_eq!(result.phases.len(), 2);
            assert_eq!(
                result.logical_calculations,
                result
                    .phases
                    .iter()
                    .map(|w| u128::from(w.context_calculations()))
                    .sum()
            );
        }
        assert_eq!(o.preparations.get(), if stream { 2 } else { 1 });
        assert_eq!(o.instances.get(), 4); // global and first mode, same-mode second phase
        assert_eq!(o.releases.get(), 4);
        assert!(o.checks.get() > 0);
        assert_eq!(
            *o.modes.borrow(),
            vec![NoAssert, FullAssert, NoAssert, FullAssert]
        );
        assert_eq!(
            d.model.initial_assignment().values(),
            &[Some(0), Some(1), Some(2)]
        );
    }
}

#[test]
fn concurrent_sessions_keep_independent_assignments_and_live_mode_handoffs() {
    for stream in [false, true] {
        let d = Domain::new(3, 3, Objective::Weighted);
        let mut c = configuration(
            &d,
            Some(NoAssert),
            vec![Some(PhaseAssert), Some(FullAssert)],
            10,
        );
        c.phases[0] = SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            ..Default::default()
        });
        let f = declaration(&d, stream, Rc::new(Observations::default()))
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let mut a = f.start_assembled_run(d.model.initial_assignment()).unwrap();
        let b = f.start_assembled_run(d.model.initial_assignment()).unwrap();
        assert_eq!(
            a.assembled_runtime_settings().unwrap().matching,
            ContextMatchPolicy::Disabled
        );
        a.run_next_assembled_phase(|r, p| {
            assert_eq!(r.settings.requested_mode, PhaseAssert);
            assert_eq!(r.settings.matching, ContextMatchPolicy::Enabled);
            r.phase(p)
        })
        .unwrap();
        let edited = a
            .inspect_assembled_resource(|r| r.input().values().to_vec())
            .unwrap();
        assert_ne!(edited, d.model.initial_assignment().values());
        a.run_next_assembled_phase(|r, p| {
            assert_eq!(r.settings.requested_mode, FullAssert);
            assert_eq!(r.input().values(), edited);
            r.phase(p)
        })
        .unwrap();
        assert_eq!(
            a.inspect_assembled_resource(ContextScoreResource::context_score),
            Some(SimpleScore::new(12))
        );
        assert_eq!(
            b.inspect_assembled_resource(ContextScoreResource::context_score),
            Some(SimpleScore::new(8))
        );
        let (input, score, count) = b.finish_assembled_state().unwrap();
        assert_eq!(input.values(), &[Some(0), Some(1), Some(2)]);
        assert_eq!(score, SimpleScore::new(8));
        assert_eq!(count, 0);
        assert!(
            a.run_next_assembled_phase(|r, p| r.phase(p))
                .unwrap()
                .is_none()
        );
        assert_eq!(a.finish_assembled_state().unwrap().1, SimpleScore::new(12));
    }
}

#[test]
fn phase_preparation_failure_closes_old_resource_and_remains_retryable() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let o = Rc::new(Observations::default());
    let f = declaration(&d, true, o.clone())
        .assemble_solver_factory(
            configuration(&d, Some(NoAssert), vec![None, Some(FullAssert)], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let mut run = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    run.run_next_assembled_phase(|r, p| r.phase(p)).unwrap();
    o.fail_prepare.set(true);
    let error = run
        .run_next_assembled_phase(|r, p| r.phase(p))
        .err()
        .unwrap();
    assert!(
        matches!(error.primary, FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Scoring(ScoreFactoryBuildError::Preparation { error: ref e, .. }))) if e.to_string()=="prepare fixture")
    );
    assert!(error.suppressed.is_empty());
    assert!(run.inspect_assembled_resource(|_| ()).is_none());
    drop(run);
    assert_eq!(o.releases.get(), 1);
    o.fail_prepare.set(false);
    assert_eq!(
        f.solve_assembled_with(d.model.initial_assignment(), |r, p| r.phase(p))
            .unwrap()
            .score,
        SimpleScore::new(12)
    );
    assert_eq!(o.preparations.get(), 3); // failed mode was not cached
    assert_eq!(o.releases.get(), 3);
}

#[test]
fn phase_and_cleanup_failures_keep_both_typed_errors() {
    let d = Domain::new(2, 2, Objective::Weighted);
    let o = Rc::new(Observations::default());
    let f = declaration(&d, false, o.clone())
        .assemble_solver_factory(
            configuration(&d, Some(NoAssert), vec![], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    o.fail_release.set(true);
    let error = f
        .solve_assembled_with::<Work>(d.model.initial_assignment(), |_, _| {
            Err(io::Error::new(io::ErrorKind::Interrupted, "phase fixture"))
        })
        .err()
        .unwrap();
    assert!(
        matches!(error.primary,FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.kind()==io::ErrorKind::Interrupted && e.to_string()=="phase fixture")
    );
    assert!(
        matches!(error.suppressed.as_slice(),[FactoryAssemblyError::Resource(e)] if e.to_string()=="release fixture")
    );
    assert_eq!(o.releases.get(), 1);
}

#[test]
fn registration_and_phase_shape_errors_precede_recipe_callbacks() {
    let d = Domain::new(2, 2, Objective::Zero);
    let o = Rc::new(Observations::default());
    let mut c = configuration(&d, None, vec![None], 10);
    assert!(matches!(
        declaration(&d, false, o.clone())
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>())),
        Err(FactoryAssemblyError::PhaseModes {
            phases: 2,
            modes: 1
        })
    ));
    c = configuration(&d, None, vec![], 10);
    c.schema = Some(Rc::new(
        PlanningSchema::new::<String>(vec![PlanningEntityDefinition::of::<Entity>(vec![])])
            .unwrap(),
    ));
    assert!(matches!(
        declaration(&d, false, o.clone())
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>())),
        Err(FactoryAssemblyError::ModelSchemaType)
    ));
    assert_eq!(o.preparations.get(), 0);
    assert_eq!(o.instances.get(), 0);
}

#[test]
fn registered_zero_runtime_entities_and_empty_plan_are_valid() {
    for stream in [false, true] {
        let d = Domain::new(2, 0, Objective::Zero);
        let o = Rc::new(Observations::default());
        let mut c = configuration(&d, None, vec![], 10);
        c.phases.clear();
        let f = declaration(&d, stream, o.clone())
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let result = f
            .solve_assembled_with::<Work>(d.model.initial_assignment(), |_, _| panic!("empty plan"))
            .unwrap();
        assert!(result.working.values().is_empty());
        assert_eq!(result.score, SimpleScore::ZERO);
        assert_eq!(result.logical_calculations, 0);
        assert!(result.phases.is_empty());
        assert_eq!(o.releases.get(), 1);
    }
}

#[test]
fn factory_seed_reaches_actual_search_and_repeated_openings() {
    let d = Domain::new(3, 3, Objective::Weighted);
    let mut c = configuration(&d, Some(NoAssert), vec![], 10);
    c.seed = u64::MAX;
    if let SolverPhase::LocalSearch(search) = &mut c.phases[1] {
        search.move_order = MoveOrder::ShuffledVariables { seed: 7 };
    }
    let f = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap();
    let result = f
        .solve_assembled_with(d.model.initial_assignment(), |r, p| {
            if let SolverPhase::LocalSearch(search) = p {
                assert_eq!(search.move_order, MoveOrder::ShuffledVariables { seed: 0 });
            }
            r.phase(p)
        })
        .unwrap();
    assert_eq!(result.score, SimpleScore::new(12));
    let mut a = f.open_assembled_random();
    let mut b = f.open_assembled_random();
    for _ in 0..100 {
        assert_eq!(a.next_factory_word(), b.next_factory_word());
    }
    assert_eq!(
        resolve_factory_workers(FactoryWorkerRequest::Auto, 3).unwrap(),
        None
    );
    assert!(FactoryWorkerRequest::parse_factory_workers("0").is_err());
    assert!(FactoryWorkerRequest::parse_factory_workers("typo").is_err());
}

#[test]
fn totals_metric_enables_matching_in_unasserted_actual_resources() {
    let d = Domain::new(2, 2, Objective::Weighted);
    let mut c = configuration(&d, Some(NoAssert), vec![], 10);
    c.metrics.best_constraint_totals = true;
    let f = declaration(&d, true, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap();
    let mut run = f.start_assembled_run(d.model.initial_assignment()).unwrap();
    assert_eq!(
        run.assembled_runtime_settings().unwrap().matching,
        ContextMatchPolicy::Enabled
    );
    run.run_next_assembled_phase(|r, p| r.phase(p)).unwrap();
    run.run_next_assembled_phase(|r, p| r.phase(p)).unwrap();
    assert_eq!(
        run.inspect_assembled_resource(ContextScoreResource::context_totals),
        Some(vec![("fixture".into(), SimpleScore::new(3))])
    );
    assert_eq!(run.finish_assembled_state().unwrap().1, SimpleScore::new(3));
}
