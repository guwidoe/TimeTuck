//! Original end-to-end lifecycle/context regressions; no source method credit.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
#[path = "fixtures/observed_list_factory_fixture.rs"]
mod lists;
use EnvironmentMode::{FullAssert, NoAssert, PhaseAssert};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};
use timetuck::*;
type BasicView<'p> =
    FactoryRuntimeView<'p, basic::Model, basic::Prepared<'p>, Assignment<i64>, basic::Resource<'p>>;

#[test]
fn list_solve_observers_read_real_counts_and_do_not_retain_resources() {
    for stream in [false, true] {
        let d = lists::Domain::new(5, 3, true);
        let o = Rc::new(basic::Observations::default());
        let f = lists::prepare(&d, stream, o.clone(), vec![], 10);
        let events = RefCell::new(vec![]);
        let views = RefCell::<Vec<lists::View<'_>>>::new(vec![]);
        let mut callback = |event, view| {
            events.borrow_mut().push(event);
            views.borrow_mut().push(view);
            Ok(())
        };
        for _ in 0..2 {
            let result = f
                .solve_observed_assembled_with(
                    d.model.initial_assignment(),
                    &mut FactoryRunCallbacks {
                        lifecycle: Some(&mut callback),
                        failure: None,
                    },
                    |r, p| r.phase(p, false),
                )
                .unwrap();
            assert_eq!(result.score, SimpleScore::new(15));
            assert_eq!(result.working.list(2).unwrap().len(), 5);
            assert_eq!(
                result.logical_calculations,
                result
                    .phases
                    .iter()
                    .map(|w| u128::from(w.context_calculations()))
                    .sum()
            );
        }
        let expected = vec![
            FactoryRunEvent::Started,
            FactoryRunEvent::PhaseEntered { index: 0 },
            FactoryRunEvent::PhaseCompleted { index: 0 },
            FactoryRunEvent::PhaseEntered { index: 1 },
            FactoryRunEvent::PhaseCompleted { index: 1 },
            FactoryRunEvent::Finished,
        ];
        assert_eq!(*events.borrow(), [expected.clone(), expected].concat());
        let views = views.borrow();
        for run in views.chunks(6) {
            assert!(
                run.iter()
                    .all(|v| run[0].same_runtime_view(v) && v.runtime_view_closed())
            );
            assert!(run[2].runtime_view_calculations() > 0);
            assert_eq!(
                run[0].runtime_view_calculations(),
                run[5].runtime_view_calculations()
            );
        }
        assert!(!views[0].same_runtime_view(&views[6]));
        assert_eq!(o.preparations.get(), 1);
        assert_eq!(o.instances.get(), 2);
        assert_eq!(o.releases.get(), 2);
        assert!(
            d.model
                .initial_assignment()
                .lists()
                .iter()
                .all(Vec::is_empty)
        );
    }
}

#[test]
fn returning_modes_reuse_preparation_but_close_each_actual_resource() {
    for stream in [false, true] {
        let d = basic::Domain::new(3, 3, basic::Objective::Weighted);
        let o = Rc::new(basic::Observations::default());
        let mut c = basic::configuration(
            &d,
            Some(PhaseAssert),
            vec![None, Some(FullAssert), None],
            10,
        );
        c.phases.push(SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }));
        let f = basic::declaration(&d, stream, o.clone())
            .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
            .unwrap();
        let views = RefCell::<Vec<BasicView<'_>>>::new(vec![]);
        let mut callback = |event, view| {
            if matches!(event, FactoryRunEvent::PhaseEntered { .. }) {
                let mut views = views.borrow_mut();
                if let Some(old) = views.last() {
                    assert!(old.runtime_view_closed());
                    assert!(!old.same_runtime_view(&view));
                }
                views.push(view);
            }
            Ok(())
        };
        let result = f
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: Some(&mut callback),
                    failure: None,
                },
                |r, p| r.phase(p),
            )
            .unwrap();
        assert_eq!(result.score, SimpleScore::new(12));
        assert_eq!(o.preparations.get(), if stream { 2 } else { 1 });
        assert_eq!(o.releases.get(), 3);
        assert_eq!(o.instances.get(), 3);
        let views = views.borrow();
        assert_eq!(views.len(), 3);
        assert!(views.iter().all(FactoryRuntimeView::runtime_view_closed));
        assert_eq!(
            views[2].runtime_view_calculations(),
            result.logical_calculations
        );
        assert!(views[2].runtime_view_calculations() >= views[1].runtime_view_calculations());
    }
}

#[test]
fn phase_failure_preserves_primary_and_listener_then_release_errors() {
    let d = basic::Domain::new(2, 2, basic::Objective::Weighted);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            basic::configuration(&d, Some(NoAssert), vec![], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    o.fail_release.set(true);
    let notified = Cell::new(0);
    let saved = RefCell::<Option<BasicView<'_>>>::new(None);
    let mut listener = |primary: &FactoryExecutionError<io::Error>, view| {
        saved.replace(Some(view));
        let saved = saved.borrow();
        let view = saved.as_ref().unwrap();
        assert!(
            matches!(primary,FactoryExecutionError::Context(ScoringContextError::Scoring(FactoryAssemblyError::Resource(e))) if e.kind()==io::ErrorKind::Interrupted)
        );
        assert!(!view.runtime_view_closed());
        assert_eq!(o.releases.get(), 0);
        notified.set(notified.get() + 1);
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "listener failure",
        ))
    };
    let error = f
        .solve_observed_assembled_with::<basic::Work>(
            d.model.initial_assignment(),
            &mut FactoryRunCallbacks {
                lifecycle: None,
                failure: Some(&mut listener),
            },
            |_, _| Err(io::Error::new(io::ErrorKind::Interrupted, "phase failure")),
        )
        .err()
        .unwrap();
    assert_eq!(
        lists::primary_message(&error).kind(),
        io::ErrorKind::Interrupted
    );
    assert!(
        matches!(error.suppressed.as_slice(),[FactoryAssemblyError::Resource(a),FactoryAssemblyError::Resource(b)] if a.kind()==io::ErrorKind::PermissionDenied && b.to_string()=="release fixture")
    );
    assert_eq!(notified.get(), 1);
    assert_eq!(o.releases.get(), 1);
    assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
}

#[test]
fn lifecycle_failure_at_each_boundary_stops_callbacks_and_closes_once() {
    for target in [
        FactoryRunEvent::Started,
        FactoryRunEvent::PhaseEntered { index: 0 },
        FactoryRunEvent::PhaseCompleted { index: 0 },
        FactoryRunEvent::Finished,
    ] {
        let d = basic::Domain::new(2, 2, basic::Objective::Weighted);
        let o = Rc::new(basic::Observations::default());
        let f = basic::declaration(&d, false, o.clone())
            .assemble_solver_factory(
                basic::configuration(&d, Some(NoAssert), vec![], 10),
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
            )
            .unwrap();
        let events = RefCell::new(vec![]);
        let saved = RefCell::<Option<BasicView<'_>>>::new(None);
        let failed = Cell::new(0);
        let mut callback = |event, view| {
            events.borrow_mut().push(event);
            if event == target {
                saved.replace(Some(view));
                Err(io::Error::other("boundary failure"))
            } else {
                Ok(())
            }
        };
        let mut listener = |_: &FactoryExecutionError<io::Error>, view: BasicView<'_>| {
            assert!(!view.runtime_view_closed());
            failed.set(failed.get() + 1);
            Ok(())
        };
        let error = f
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: Some(&mut callback),
                    failure: Some(&mut listener),
                },
                |r, p| r.phase(p),
            )
            .err()
            .unwrap();
        assert_eq!(
            lists::primary_message(&error).to_string(),
            "boundary failure"
        );
        assert_eq!(events.borrow().last(), Some(&target));
        assert_eq!(failed.get(), 1);
        assert_eq!(o.releases.get(), 1);
        assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
    }
}

#[test]
fn panic_releases_current_context_without_invoking_more_hooks() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let o = Rc::new(basic::Observations::default());
    let f = basic::declaration(&d, false, o.clone())
        .assemble_solver_factory(
            basic::configuration(&d, None, vec![None, Some(FullAssert)], 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let saved = RefCell::<Option<BasicView<'_>>>::new(None);
    let failures = Cell::new(0);
    let mut callback = |event, view| {
        if event == (FactoryRunEvent::PhaseEntered { index: 1 }) {
            saved.replace(Some(view));
            panic!("listener panic");
        }
        Ok(())
    };
    let mut listener = |_: &FactoryExecutionError<io::Error>, _: BasicView<'_>| {
        failures.set(failures.get() + 1);
        Ok(())
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.solve_observed_assembled_with(
            d.model.initial_assignment(),
            &mut FactoryRunCallbacks {
                lifecycle: Some(&mut callback),
                failure: Some(&mut listener),
            },
            |r, p| r.phase(p),
        )
    }));
    assert!(result.is_err());
    assert_eq!(failures.get(), 0);
    assert_eq!(o.releases.get(), 2);
    assert!(saved.borrow().as_ref().unwrap().runtime_view_closed());
}

#[test]
fn list_context_trait_uses_real_scores_and_typed_validation_errors() {
    let d = lists::Domain::new(3, 2, true);
    let mut dense =
        ListScoreDirector::new(&d.model, &d.network, d.model.initial_assignment()).unwrap();
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("weighted", sources.entities.port(), |(i, values)| {
        SimpleScore::new((*i as i64 + 1) * values.len() as i64)
    })
    .unwrap();
    let mut stream =
        ListStreamScoreDirector::new(&d.model, b.build(), sources, d.model.initial_assignment())
            .unwrap();
    for (i, v) in d.model.values().iter().enumerate() {
        let movement = ListMove::Assign {
            value: v.clone(),
            destination: ListPosition {
                entity: 1,
                index: i,
            },
        };
        dense.apply(&movement).unwrap().commit();
        stream.apply(&movement).unwrap().commit();
    }
    let dense_check: Result<(), ListScoringError> = dense.check_context();
    let stream_check: Result<(), ListStreamScoringError> = stream.check_context();
    assert!(dense_check.is_ok() && stream_check.is_ok());
    assert_eq!(dense.context_score(), SimpleScore::new(6));
    assert_eq!(stream.context_score(), dense.context_score());
    assert_eq!(
        dense
            .context_totals()
            .iter()
            .map(|(_, s)| s.score())
            .sum::<i64>(),
        6
    );
    assert_eq!(
        stream.context_totals(),
        vec![("weighted".into(), SimpleScore::new(6))]
    );
    let bad = ListMove::Assign {
        value: "bad value".into(),
        destination: ListPosition {
            entity: 1,
            index: 3,
        },
    };
    assert!(matches!(dense.apply(&bad), Err(ListScoringError::Model(_))));
    assert!(matches!(
        stream.apply(&bad),
        Err(ListStreamScoringError::Model(_))
    ));
    assert_eq!(dense.context_score(), SimpleScore::new(6));
    assert_eq!(stream.context_score(), SimpleScore::new(6));
}
