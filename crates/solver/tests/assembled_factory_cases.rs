// Adapted from pinned Community DefaultSolverFactoryTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use EnvironmentMode::{
    FullAssert, NoAssert, NonIntrusiveFullAssert, NonReproducible, PhaseAssert, StepAssert,
    TrackedFullAssert,
};
use fixture::*;
use std::rc::Rc;
use timetuck::*;
#[test]
fn move_thread_count_auto_is_correctly_resolved_when_cpu_count_is_positive() {
    for (cpu, n) in [
        (1, None),
        (2, None),
        (4, Some(2)),
        (5, Some(3)),
        (6, Some(4)),
        (100, Some(4)),
    ] {
        assert_eq!(
            resolve_factory_workers(FactoryWorkerRequest::Auto, cpu).unwrap(),
            n
        );
    }
}
#[test]
fn move_thread_count_auto_is_resolved_to_null_when_cpu_count_is_negative() {
    assert_eq!(
        resolve_factory_workers(FactoryWorkerRequest::Auto, -1).unwrap(),
        None
    );
}
#[test]
fn move_thread_count_is_correctly_resolved_when_value_is_positive() {
    let r = FactoryWorkerRequest::parse_factory_workers("2").unwrap();
    assert_eq!(resolve_factory_workers(r, 0).unwrap(), Some(2));
}
#[test]
fn move_thread_count_throws_exception_when_value_is_negative() {
    assert!(FactoryWorkerRequest::parse_factory_workers("-1").is_err());
}
#[test]
fn move_thread_count_is_resolved_to_null_when_value_is_none() {
    assert_eq!(
        resolve_factory_workers(
            FactoryWorkerRequest::parse_factory_workers("NONE").unwrap(),
            0
        )
        .unwrap(),
        None
    );
}
#[test]
fn caches_score_director_factory() {
    let d = Domain::new(5, 7, Objective::Collisions);
    let f = prepare(&d, false, None, vec![]);
    let model1 = f.assembled_model();
    let scoring1 = f.assembled_scoring_factory();
    assert!(Rc::ptr_eq(model1, scoring1.factory_model()));
    assert!(Rc::ptr_eq(model1, f.assembled_model()));
    assert!(std::ptr::eq(scoring1, f.assembled_scoring_factory()));
    assert!(Rc::ptr_eq(f.assembled_schema(), f.assembled_schema()));
}
#[test]
fn use_correct_random_seed() {
    let d = Domain::new(5, 7, Objective::FirstPresence);
    let mut c = configuration(&d, None, vec![], 10);
    c.seed = 123456;
    let f = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap();
    let mut actual = f.open_assembled_random();
    let mut expected = FactoryRandomStream::seeded_factory_stream(123456);
    assert_eq!(actual.factory_stream_seed(), expected.factory_stream_seed());
    assert_eq!(actual.next_factory_word(), expected.next_factory_word());
}
#[test]
fn test_no_solution_configuration() {
    let d = Domain::new(2, 2, Objective::Zero);
    let mut c = configuration(&d, None, vec![], 10);
    c.schema = None;
    let e = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    assert!(matches!(
        e,
        FactoryAssemblyError::MissingSolutionRegistration
    ));
    assert!(
        e.to_string()
            .contains("must have a planning solution schema")
    );
    assert!(
        e.to_string()
            .contains("if using a model-binding integration")
    );
}
#[test]
fn test_no_entity_configuration() {
    let d = Domain::new(2, 2, Objective::Zero);
    let mut c = configuration(&d, None, vec![], 10);
    c.schema = Some(Rc::new(PlanningSchema::new::<Model>(vec![]).unwrap()));
    let e = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    assert!(matches!(e, FactoryAssemblyError::MissingEntityRegistration));
    assert!(
        e.to_string()
            .contains("must have at least 1 registered entity type")
    );
    assert!(
        e.to_string()
            .contains("if using a model-binding integration")
    );
}
#[test]
fn test_invalid_move_thread_count_configuration() {
    let d = Domain::new(2, 2, Objective::Collisions);
    let mut c = configuration(&d, None, vec![], 10);
    c.workers = FactoryWorkerRequest::Count(-1);
    let e = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    for part in [
        "moveThreadCount",
        "resulted in a resolvedMoveThreadCount",
        "that is lower than 1",
    ] {
        assert!(e.to_string().contains(part));
    }
}
#[test]
fn test_invalid_constraint_profiling_without_enterprise() {
    let d = Domain::new(2, 2, Objective::AlwaysAssigned);
    let mut c = configuration(&d, None, vec![], 10);
    c.reference_constraint_profiling = true;
    let e = declaration(&d, true, Rc::new(Observations::default()))
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .err()
        .unwrap();
    for part in [
        "Constraint profiling",
        "remove constraintStreamProfilingEnabled from the solver configuration",
    ] {
        assert!(e.to_string().contains(part));
    }
}
#[test]
fn assert_environment_mode_without_phases() {
    let d = Domain::new(2, 2, Objective::FirstPresence);
    let mut c = configuration(&d, Some(FullAssert), vec![], 10);
    c.phases.clear();
    assert!(
        declaration(&d, false, Rc::new(Observations::default()))
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .is_ok()
    );
}
#[test]
fn assert_environment_mode_with_valid_phases() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(
        &d,
        false,
        Some(FullAssert),
        vec![Some(FullAssert), Some(TrackedFullAssert)],
    );
    assert_eq!(f.assembled_global_mode(), FullAssert);
}
fn invalid_modes(global: EnvironmentMode, modes: Vec<Option<EnvironmentMode>>, message: &str) {
    let d = Domain::new(2, 2, Objective::Zero);
    let e = declaration(&d, false, Rc::new(Observations::default()))
        .assemble_solver_factory(
            configuration(&d, Some(global), modes, 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .err()
        .unwrap();
    assert!(e.to_string().contains(message));
}
#[test]
fn assert_environment_with_non_reproducible_and_mismatching_phase() {
    invalid_modes(
        NonReproducible,
        vec![Some(NonReproducible), Some(NoAssert)],
        "is only possible when global environmentMode is reproducible",
    );
}
#[test]
fn assert_environment_mode_with_global_not_used_by_any_phase() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(
        &d,
        false,
        Some(StepAssert),
        vec![Some(FullAssert), Some(NonIntrusiveFullAssert)],
    );
    assert_eq!(f.assembled_global_mode(), StepAssert);
}
#[test]
fn every_phase_overriding_leaves_the_global_environment_mode_alone() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(&d, false, None, vec![Some(FullAssert), Some(FullAssert)]);
    assert_eq!(f.assembled_global_mode(), PhaseAssert);
}
#[test]
fn differing_phase_environment_modes_leave_the_global_environment_mode_alone() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(
        &d,
        false,
        Some(StepAssert),
        vec![Some(FullAssert), Some(NonIntrusiveFullAssert)],
    );
    assert_eq!(f.assembled_global_mode(), StepAssert);
}
#[test]
fn one_phase_left_on_the_global_environment_mode() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(&d, false, None, vec![None, Some(FullAssert)]);
    assert_eq!(f.assembled_global_mode(), PhaseAssert);
}
#[test]
fn phase_environment_mode_cannot_make_the_global_non_reproducible() {
    invalid_modes(
        PhaseAssert,
        vec![Some(NonReproducible), Some(NonReproducible)],
        "must have an assertion level higher than or equal to the global environment level",
    );
}
#[test]
fn solves_with_every_phase_overriding_an_unset_global_environment_mode() {
    let d = Domain::new(2, 2, Objective::Zero);
    let f = prepare(&d, false, None, vec![Some(FullAssert), Some(FullAssert)]);
    let result = f
        .solve_assembled_with(d.model.initial_assignment(), |r, p| r.phase(p))
        .unwrap();
    assert_eq!(result.working.values().len(), 2);
    assert_eq!(result.score, SimpleScore::ZERO);
    assert_eq!(result.phases.len(), 2);
}
#[test]
fn solves_with_constraint_streams_and_a_phase_overriding_the_environment_mode() {
    let d = Domain::new(2, 2, Objective::AlwaysAssigned);
    let f = declaration(&d, true, Rc::new(Observations::default()))
        .assemble_solver_factory(
            configuration(&d, None, vec![None, Some(FullAssert)], 5),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    assert_eq!(
        f.assembled_scoring_factory()
            .factory_score_trend()
            .levels()
            .len(),
        1
    );
    let result = f
        .solve_assembled_with(d.model.initial_assignment(), |r, p| r.phase(p))
        .unwrap();
    assert_eq!(result.score, SimpleScore::new(-2));
    assert!(result.phases[1].0.work.completed_steps <= 5);
}
#[test]
fn assert_environment_mode_with_phase_less_strict_than_default() {
    invalid_modes(
        StepAssert,
        vec![Some(StepAssert), Some(NoAssert)],
        "must have an assertion level higher than or equal to the global environment level",
    );
}
