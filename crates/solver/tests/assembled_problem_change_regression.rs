//! Original native change-context lifecycle cases; no source-method credit.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
use std::{cell::RefCell, io, rc::Rc};
use timetuck::*;
type View<'a> =
    FactoryRuntimeView<'a, basic::Model, basic::Prepared<'a>, Assignment<i64>, basic::Resource<'a>>;
fn edit(resource: &mut basic::Resource<'_>) -> Result<(), io::Error> {
    let movement = Move::Change {
        variable: 0,
        value: Some(1),
    };
    match &mut resource.engine {
        basic::Engine::Dense(d) => d.apply(&movement).map_err(io::Error::other)?.commit(),
        basic::Engine::Stream(d) => d.apply(&movement).map_err(io::Error::other)?.commit(),
    }
    Ok(())
}
#[test]
fn change_uses_completed_input_fresh_real_resource_and_owned_output_across_backends_and_modes() {
    for stream in [false, true] {
        for mode in [
            EnvironmentMode::NoAssert,
            EnvironmentMode::PhaseAssert,
            EnvironmentMode::FullAssert,
        ] {
            let d = basic::Domain::new(3, 2, basic::Objective::Weighted);
            let o = Rc::new(basic::Observations::default());
            let f = basic::declaration(&d, stream, o.clone())
                .assemble_solver_factory(
                    basic::configuration(&d, Some(mode), vec![], 10),
                    Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                )
                .unwrap();
            let old = RefCell::<Option<View<'_>>>::new(None);
            let mut callback = |_, view| {
                old.replace(Some(view));
                Ok(())
            };
            let first = f
                .solve_observed_assembled_with(
                    d.model.initial_assignment(),
                    &mut FactoryRunCallbacks {
                        lifecycle: Some(&mut callback),
                        failure: None,
                    },
                    |r, p| r.phase(p),
                )
                .unwrap();
            assert_eq!(first.working.values(), &[Some(2), Some(2)]);
            assert!(old.borrow().as_ref().unwrap().runtime_view_closed());
            let changed = RefCell::<Option<View<'_>>>::new(None);
            let result = f
                .apply_observed_assembled_problem_change(
                    first.working.clone(),
                    |view| {
                        changed.replace(Some(view));
                        let changed = changed.borrow();
                        let view = changed.as_ref().unwrap();
                        assert!(!view.runtime_view_closed());
                        assert!(!old.borrow().as_ref().unwrap().same_runtime_view(view));
                        assert_eq!(
                            view.read_runtime_view(|r| r.input().clone()).unwrap(),
                            first.working
                        );
                        Ok(())
                    },
                    |resource| {
                        edit(resource)?;
                        Ok("applied")
                    },
                )
                .unwrap();
            assert_eq!(result.working.values(), &[Some(1), Some(2)]);
            assert_eq!(result.score, SimpleScore::new(5));
            assert_eq!(result.value, "applied");
            assert_eq!(result.logical_calculations, 1);
            assert_eq!(first.working.values(), &[Some(2), Some(2)]);
            assert!(changed.borrow().as_ref().unwrap().runtime_view_closed());
            assert!(old.borrow().as_ref().unwrap().runtime_view_closed());
            let second = f
                .solve_assembled_with(result.working, |r, p| r.phase(p))
                .unwrap();
            assert_eq!(second.score, SimpleScore::new(6));
            assert_eq!(o.instances.get(), 3);
            assert_eq!(o.releases.get(), 3);
        }
    }
}
#[test]
fn change_failure_retains_primary_and_suppresses_release_failure_once() {
    let d = basic::Domain::new(3, 2, basic::Objective::Weighted);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            basic::configuration(&d, Some(EnvironmentMode::NoAssert), vec![], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    o.fail_release.set(true);
    let saved = RefCell::<Option<View<'_>>>::new(None);
    let error = f
        .apply_observed_assembled_problem_change::<()>(
            d.model.initial_assignment(),
            |view| {
                saved.replace(Some(view));
                Ok(())
            },
            |resource| {
                edit(resource)?;
                Err(io::Error::new(io::ErrorKind::Interrupted, "change failure"))
            },
        )
        .err()
        .unwrap();
    assert!(
        matches!(error.primary, FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.kind() == io::ErrorKind::Interrupted)
    );
    assert!(
        matches!(error.suppressed.as_slice(), [FactoryAssemblyError::Resource(e)] if e.to_string() == "release fixture")
    );
    assert_eq!(o.releases.get(), 1);
    assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
}
#[test]
fn panic_releases_the_actual_change_resource_and_closes_its_saved_view() {
    let d = basic::Domain::new(3, 2, basic::Objective::Weighted);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            basic::configuration(&d, Some(EnvironmentMode::NoAssert), vec![], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let saved = RefCell::<Option<View<'_>>>::new(None);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<FactoryProblemChange<_, _, ()>, _> = f
            .apply_observed_assembled_problem_change(
                d.model.initial_assignment(),
                |view| {
                    saved.replace(Some(view));
                    Ok(())
                },
                |_| {
                    panic!("change panic");
                },
            );
    }));
    assert!(panic.is_err());
    assert_eq!(o.releases.get(), 1);
    assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
}
#[test]
fn observer_failure_stops_the_edit_and_closes_the_fresh_resource() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            basic::configuration(&d, Some(EnvironmentMode::NoAssert), vec![], 1),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let saved = RefCell::<Option<View<'_>>>::new(None);
    let failure = f
        .apply_observed_assembled_problem_change::<()>(
            d.model.initial_assignment(),
            |view| {
                saved.replace(Some(view));
                assert!(!saved.borrow().as_ref().unwrap().runtime_view_closed());
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "observer failure",
                ))
            },
            |_| panic!("edit must not run"),
        )
        .err()
        .unwrap();
    assert!(
        matches!(failure.primary, FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.kind() == io::ErrorKind::PermissionDenied)
    );
    assert_eq!(o.releases.get(), 1);
    assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
}
#[test]
fn change_scope_checks_real_corruption_in_phase_and_full_modes() {
    for mode in [EnvironmentMode::PhaseAssert, EnvironmentMode::FullAssert] {
        let offset = Rc::new(std::cell::Cell::new(0));
        let flag = offset.clone();
        let mut d = basic::Domain::new(3, 2, basic::Objective::Weighted);
        d.network
            .add_constraint("external fact", vec![0], move |_| {
                SimpleScore::new(flag.get())
            })
            .unwrap();
        let o = Rc::new(basic::Observations::default());
        let f = basic::declaration(&d, false, o.clone())
            .assemble_solver_factory(
                basic::configuration(&d, Some(mode), vec![], 1),
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
            )
            .unwrap();
        let saved = RefCell::<Option<View<'_>>>::new(None);
        let failure = f
            .apply_observed_assembled_problem_change(
                d.model.initial_assignment(),
                |view| {
                    saved.replace(Some(view));
                    Ok(())
                },
                |_| {
                    offset.set(99);
                    Ok(())
                },
            )
            .err()
            .unwrap();
        assert!(
            matches!(failure.primary, FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.to_string().to_lowercase().contains("corruption"))
        );
        assert_eq!(o.releases.get(), 1);
        assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
        assert_eq!(
            saved.borrow().as_ref().unwrap().runtime_view_calculations(),
            0
        );
    }
}
