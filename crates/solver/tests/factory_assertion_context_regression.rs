//! Original typed checker context and failure-preservation controls.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, error::Error, io, rc::Rc};
use timetuck::*;
fn domain(offset: Rc<Cell<i64>>) -> Domain {
    let mut d = Domain::new(3, 2, Objective::Weighted);
    d.network
        .add_constraint("external offset", vec![0], move |_| {
            SimpleScore::new(offset.get())
        })
        .unwrap();
    d
}
fn factory<'a>(d: &'a Domain, mode: EnvironmentMode, o: Rc<Observations>) -> Factory<'a> {
    let mut c = configuration(d, Some(mode), vec![None], 1);
    c.phases.remove(0);
    declaration(d, false, o)
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap()
}
#[test]
fn declared_phase_checker_keeps_mode_phase_cause_and_release_failures() {
    use EnvironmentMode::*;
    for mode in [
        StepAssert,
        FullAssert,
        NonIntrusiveFullAssert,
        TrackedFullAssert,
    ] {
        let offset = Rc::new(Cell::new(0));
        let d = domain(offset.clone());
        let o = Rc::new(Observations::default());
        o.fail_release.set(true);
        let f = factory(&d, mode, o.clone());
        let failure = f
            .solve_assembled_with(d.model.initial_assignment(), |r, p| {
                let w = r.phase(p)?;
                offset.set(20);
                Ok(w)
            })
            .err()
            .unwrap();
        let FactoryExecutionError::Context(ScoringContextError::Scoring(error)) = failure.primary
        else {
            panic!()
        };
        assert!(
            error
                .source()
                .unwrap()
                .downcast_ref::<io::Error>()
                .is_some()
        );
        let text = error.to_string();
        let FactoryAssemblyError::ScoreAssertion {
            phase,
            mode: actual,
            error,
        } = error
        else {
            panic!("wrong checker context {text}")
        };
        assert_eq!(phase, 0);
        assert_eq!(actual, mode);
        assert!(error.to_string().contains("external offset"));
        assert!(text.contains("resource consistency check failed"));
        assert!(
            matches!(failure.suppressed.as_slice(),[FactoryAssemblyError::Resource(e)] if e.to_string()=="release fixture")
        );
        assert_eq!(o.releases.get(), 1);
    }
}
#[test]
fn arbitrary_work_errors_keep_their_type_and_are_never_called_assertions() {
    use EnvironmentMode::*;
    for mode in [
        NoAssert,
        NonReproducible,
        PhaseAssert,
        StepAssert,
        FullAssert,
        NonIntrusiveFullAssert,
        TrackedFullAssert,
    ] {
        let d = Domain::new(3, 2, Objective::Weighted);
        let o = Rc::new(Observations::default());
        let f = factory(&d, mode, o.clone());
        let e = f
            .solve_assembled_with::<Work>(d.model.initial_assignment(), |_, _| {
                Err(io::Error::new(io::ErrorKind::Interrupted, "work aborted"))
            })
            .err()
            .unwrap();
        assert!(
            matches!(e.primary,FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.kind()==io::ErrorKind::Interrupted)
        );
        assert_eq!(o.releases.get(), 1);
    }
}
#[test]
fn phase_checks_keep_existing_guidance_and_unasserted_modes_do_not_recompute() {
    use EnvironmentMode::*;
    for mode in [PhaseAssert, NoAssert, NonReproducible] {
        let offset = Rc::new(Cell::new(0));
        let d = domain(offset.clone());
        let o = Rc::new(Observations::default());
        let f = factory(&d, mode, o.clone());
        let result = f.solve_assembled_with(d.model.initial_assignment(), |r, p| {
            let w = r.phase(p)?;
            offset.set(20);
            Ok(w)
        });
        if mode == PhaseAssert {
            let e = result.err().unwrap();
            let FactoryExecutionError::Context(ScoringContextError::Scoring(error)) = e.primary
            else {
                panic!()
            };
            assert!(matches!(
                error,
                FactoryAssemblyError::PhaseAssertion { phase: 0, .. }
            ));
            let text = error.to_string();
            assert!(text.contains("FULL_ASSERT"));
            assert!(text.contains("NO_ASSERT"));
        } else {
            assert!(result.is_ok());
            assert_eq!(o.checks.get(), 0);
        }
        assert_eq!(o.releases.get(), 1);
    }
}
#[test]
fn observed_checker_failure_exposes_actual_context_before_closing_the_resource() {
    let offset = Rc::new(Cell::new(0));
    let d = domain(offset.clone());
    let o = Rc::new(Observations::default());
    let f = factory(&d, EnvironmentMode::FullAssert, o.clone());
    let seen = Cell::new(false);
    let mut failed =
        |error: &FactoryExecutionError<io::Error>,
         view: FactoryRuntimeView<'_, Model, Prepared<'_>, Assignment<i64>, Resource<'_>>| {
            assert!(matches!(
                error,
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::ScoreAssertion {
                        phase: 0,
                        mode: EnvironmentMode::FullAssert,
                        ..
                    }
                ))
            ));
            assert!(!view.runtime_view_closed());
            assert!(view.runtime_view_calculations() > 0);
            assert_eq!(o.releases.get(), 0);
            seen.set(true);
            Ok(())
        };
    let result = f.solve_observed_assembled_with(
        d.model.initial_assignment(),
        &mut FactoryRunCallbacks {
            lifecycle: None,
            failure: Some(&mut failed),
        },
        |r, p| {
            let w = r.phase(p)?;
            offset.set(20);
            Ok(w)
        },
    );
    assert!(result.is_err());
    assert!(seen.get());
    assert_eq!(o.releases.get(), 1);
}
