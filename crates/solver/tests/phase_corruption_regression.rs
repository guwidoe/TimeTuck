//! Original real phase-boundary corruption/cleanup regressions; no method credit.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};
use timetuck::*;
type View<'a> =
    FactoryRuntimeView<'a, basic::Model, basic::Prepared<'a>, Assignment<i64>, basic::Resource<'a>>;
fn domain(offset: Rc<Cell<i64>>) -> basic::Domain {
    let mut d = basic::Domain::new(3, 2, basic::Objective::Weighted);
    d.network
        .add_constraint("external offset", vec![0], move |_| {
            SimpleScore::new(offset.get())
        })
        .unwrap();
    d
}
fn declaration<'a>(
    d: &'a basic::Domain,
    offset: Rc<Cell<i64>>,
    stream: bool,
    o: Rc<basic::Observations>,
) -> basic::Declaration<'a> {
    if !stream {
        return basic::declaration(d, false, o);
    }
    let prep = o.clone();
    let recipe = ScoringFactoryRecipe::stream_recipe(
        move |mode| {
            prep.preparations.set(prep.preparations.get() + 1);
            Ok(basic::Prepared { domain: d, mode })
        },
        move |_: &basic::Prepared<'a>, settings, input| {
            o.instances.set(o.instances.get() + 1);
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i64>)>();
            b.score("fixture", source.port(), |(i, v)| {
                SimpleScore::new((*i as i64 + 1) * v.unwrap_or(0))
            })
            .unwrap();
            let flag = offset.clone();
            b.score("external offset", source.port(), move |(i, _)| {
                SimpleScore::new(if *i == 0 { flag.get() } else { 0 })
            })
            .unwrap();
            let director = StreamScoreDirector::new(&d.model, b.build(), source, input)
                .map_err(|e| io::Error::other(e.to_string()))?;
            Ok(basic::Resource {
                engine: basic::Engine::Stream(Box::new(director)),
                observations: o.clone(),
                settings,
            })
        },
        |r: &basic::Resource<'a>| Ok(r.input().clone()),
    );
    ScoringFactoryDeclaration::default().with_scoring_recipe(recipe)
}
fn configuration(
    d: &basic::Domain,
    mode: EnvironmentMode,
) -> SolverAssemblyConfiguration<'_, basic::Model, SimpleScore> {
    let mut c = basic::configuration(d, Some(mode), vec![], 1);
    c.phases.remove(0); // Only the final local-search phase: no later phase can detect this.
    c
}
fn assertion_phase(e: &FactoryExecutionFailure<io::Error>) -> usize {
    match &e.primary {
        FactoryExecutionError::Context(ScoringContextError::Scoring(
            FactoryAssemblyError::PhaseAssertion { phase, error },
        )) => {
            assert!(error.to_string().to_lowercase().contains("corruption"));
            *phase
        }
        other => panic!("unexpected primary {other:?}"),
    }
}
#[test]
fn final_phase_detects_real_corruption_and_retains_completed_work_before_observed_cleanup() {
    for stream in [false, true] {
        let offset = Rc::new(Cell::new(0));
        let d = domain(offset.clone());
        let o = Rc::new(basic::Observations::default());
        let factory = declaration(&d, offset.clone(), stream, o.clone())
            .assemble_solver_factory(
                configuration(&d, EnvironmentMode::PhaseAssert),
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
            )
            .unwrap();
        o.fail_release.set(true);
        let events = RefCell::new(vec![]);
        let saved = RefCell::<Option<View<'_>>>::new(None);
        let native_count = Cell::new(0);
        let notified = Cell::new(0);
        let mut callback = |event, _| {
            events.borrow_mut().push(event);
            Ok(())
        };
        let mut failed = |primary: &FactoryExecutionError<io::Error>, view| {
            saved.replace(Some(view));
            let saved = saved.borrow();
            let view = saved.as_ref().unwrap();
            assert!(matches!(
                primary,
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::PhaseAssertion { phase: 0, .. }
                ))
            ));
            assert!(!view.runtime_view_closed());
            assert_eq!(view.runtime_view_calculations(), native_count.get());
            assert!(native_count.get() > 0);
            assert_eq!(o.releases.get(), 0);
            notified.set(notified.get() + 1);
            Err(io::Error::other("failure listener"))
        };
        let error = factory
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: Some(&mut callback),
                    failure: Some(&mut failed),
                },
                |resource, phase| {
                    let work = resource.phase(phase)?;
                    native_count.set(u128::from(work.context_calculations()));
                    offset.set(99); // Cached scores disagree with genuine independent rescoring.
                    Ok(work)
                },
            )
            .err()
            .unwrap();
        assert_eq!(assertion_phase(&error), 0);
        assert_eq!(
            *events.borrow(),
            vec![
                FactoryRunEvent::Started,
                FactoryRunEvent::PhaseEntered { index: 0 }
            ]
        );
        assert_eq!(notified.get(), 1);
        assert_eq!(o.releases.get(), 1);
        assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
        assert!(
            matches!(error.suppressed.as_slice(), [FactoryAssemblyError::Resource(a), FactoryAssemblyError::Resource(b)] if a.to_string() == "failure listener" && b.to_string() == "release fixture")
        );
    }
}
#[test]
fn unobserved_end_check_is_mode_gated_and_preserves_native_counts() {
    for stream in [false, true] {
        for mode in [EnvironmentMode::PhaseAssert, EnvironmentMode::NoAssert] {
            let offset = Rc::new(Cell::new(0));
            let d = domain(offset.clone());
            let o = Rc::new(basic::Observations::default());
            let factory = declaration(&d, offset.clone(), stream, o.clone())
                .assemble_solver_factory(
                    configuration(&d, mode),
                    Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                )
                .unwrap();
            let mut run = factory
                .start_assembled_run(d.model.initial_assignment())
                .unwrap();
            let view = run.assembled_runtime_view();
            let actual = Cell::new(0);
            let result = run.run_next_assembled_phase(|resource, phase| {
                let work = resource.phase(phase)?;
                actual.set(u128::from(work.context_calculations()));
                offset.set(99);
                Ok(work)
            });
            assert_eq!(view.runtime_view_calculations(), actual.get());
            assert!(actual.get() > 0);
            if mode == EnvironmentMode::PhaseAssert {
                assert_eq!(assertion_phase(&result.err().unwrap()), 0);
                assert!(view.runtime_view_closed());
            } else {
                assert!(result.unwrap().is_some());
                assert_eq!(o.checks.get(), 0);
                assert!(
                    run.run_next_assembled_phase(|r, p| r.phase(p))
                        .unwrap()
                        .is_none()
                );
                run.finish_assembled_state().unwrap();
            }
            assert_eq!(o.releases.get(), 1);
        }
    }
}
#[test]
fn arbitrary_phase_failure_is_never_relabelled_as_corruption() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            configuration(&d, EnvironmentMode::PhaseAssert),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let e = f
        .solve_assembled_with::<basic::Work>(d.model.initial_assignment(), |_, _| {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "application failure",
            ))
        })
        .err()
        .unwrap();
    assert!(
        matches!(e.primary, FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(ref e))) if e.kind() == io::ErrorKind::Interrupted)
    );
    assert_eq!(o.releases.get(), 1);
}
