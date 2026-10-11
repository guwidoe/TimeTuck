// Adapted from pinned Community DefaultSolverTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
#[path = "fixtures/observed_list_factory_fixture.rs"]
mod lists;
use EnvironmentMode::{FullAssert, PhaseAssert, TrackedFullAssert};
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};
use timetuck::*;
type BasicView<'p> =
    FactoryRuntimeView<'p, basic::Model, basic::Prepared<'p>, Assignment<i64>, basic::Resource<'p>>;
#[test]
fn assert_default_environment_mode() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![]);
    assert_eq!(f.assembled_phase_modes(), &[PhaseAssert, PhaseAssert]);
}
#[test]
fn assert_updated_default_environment_mode() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, Some(FullAssert), vec![]);
    assert_eq!(f.assembled_phase_modes(), &[FullAssert, FullAssert]);
}
#[test]
fn assert_phase_environment_mode() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(
        &d,
        false,
        Some(FullAssert),
        vec![None, Some(TrackedFullAssert)],
    );
    assert_eq!(f.assembled_phase_modes(), &[FullAssert, TrackedFullAssert]);
}
#[test]
fn assert_default_phase_environment_mode() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![None, Some(FullAssert)]);
    assert_eq!(f.assembled_phase_modes(), &[PhaseAssert, FullAssert]);
}
#[test]
fn solve_with_phase_environment_mode_override() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![None, Some(TrackedFullAssert)]);
    let result = f
        .solve_observed_assembled_with(
            d.model.initial_assignment(),
            &mut FactoryRunCallbacks::default(),
            |r, p| r.phase(p),
        )
        .unwrap();
    assert_eq!(result.working.values().len(), 2);
    assert_eq!(result.score, SimpleScore::ZERO);
}
#[test]
fn replaced_score_director_is_closed_when_a_phase_overrides_the_environment_mode() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![None, Some(FullAssert)]);
    let views = RefCell::<Vec<BasicView<'_>>>::new(vec![]);
    let closed_on_swap = Cell::new(false);
    let mut callback = |event, view| {
        if matches!(event, FactoryRunEvent::PhaseEntered { .. }) {
            let mut views = views.borrow_mut();
            if let Some(old) = views.first() {
                closed_on_swap.set(old.read_runtime_view(|r| r.input().clone()).is_none());
            }
            views.push(view);
        }
        Ok(())
    };
    f.solve_observed_assembled_with(
        d.model.initial_assignment(),
        &mut FactoryRunCallbacks {
            lifecycle: Some(&mut callback),
            failure: None,
        },
        |r, p| r.phase(p),
    )
    .unwrap();
    let views = views.borrow();
    assert_eq!(views.len(), 2);
    assert!(!views[0].same_runtime_view(&views[1]));
    assert!(closed_on_swap.get());
}
#[test]
fn ensure_score_calculation_count_consistent() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![None, Some(FullAssert)]);
    let after = Cell::new(None);
    let start = Cell::new(None);
    let mut callback = |event, view: BasicView<'_>| {
        match event {
            FactoryRunEvent::PhaseCompleted { index: 0 } => {
                after.set(Some(view.runtime_view_calculations()))
            }
            FactoryRunEvent::PhaseEntered { index: 1 } => {
                start.set(Some(view.runtime_view_calculations()))
            }
            _ => {}
        }
        Ok(())
    };
    f.solve_observed_assembled_with(
        d.model.initial_assignment(),
        &mut FactoryRunCallbacks {
            lifecycle: Some(&mut callback),
            failure: None,
        },
        |r, p| r.phase(p),
    )
    .unwrap();
    assert!(after.get().unwrap() > 0);
    assert!(start.get().unwrap() >= after.get().unwrap());
}
#[test]
fn solve_list_variable_with_phase_environment_mode_override() {
    for stream in [false, true] {
        let d = lists::Domain::new(20, 5, false);
        let f = lists::prepare(
            &d,
            stream,
            Rc::new(basic::Observations::default()),
            vec![None, Some(TrackedFullAssert)],
            50,
        );
        let result = f
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks::default(),
                |r, p| r.phase(p, false),
            )
            .unwrap();
        assert_eq!(result.working.lists().len(), 5);
        assert_eq!(
            result.working.lists().iter().map(Vec::len).sum::<usize>(),
            20
        );
        assert_eq!(result.score, SimpleScore::ZERO);
    }
}
#[test]
fn solving_error_closes_score_director_when_solving_started_fails() {
    let d = basic::Domain::new(2, 2, basic::Objective::Zero);
    let f = basic::prepare(&d, false, None, vec![]);
    let saved = RefCell::<Option<BasicView<'_>>>::new(None);
    let mut callback = |event, view| {
        if event == FactoryRunEvent::Started {
            saved.replace(Some(view));
            return Err(io::Error::other("Boom from a listener"));
        }
        Ok(())
    };
    let error = f
        .solve_observed_assembled_with(
            d.model.initial_assignment(),
            &mut FactoryRunCallbacks {
                lifecycle: Some(&mut callback),
                failure: None,
            },
            |r, p| r.phase(p),
        )
        .err()
        .unwrap();
    assert!(
        lists::primary_message(&error)
            .to_string()
            .contains("Boom from a listener")
    );
    assert!(
        saved
            .borrow()
            .as_ref()
            .unwrap()
            .read_runtime_view(|r| r.input().clone())
            .is_none()
    );
}
#[test]
fn solving_error_notifies_listeners_before_closing_the_score_director() {
    for stream in [false, true] {
        let d = lists::Domain::new(2, 2, false);
        let f = lists::prepare(
            &d,
            stream,
            Rc::new(basic::Observations::default()),
            vec![None, Some(FullAssert)],
            10,
        );
        let seen = RefCell::new(None);
        let mut failure = |_: &FactoryExecutionError<io::Error>, view: lists::View<'_>| {
            seen.replace(view.read_runtime_view(|r| r.input().clone()));
            Ok(())
        };
        let error = f
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: None,
                    failure: Some(&mut failure),
                },
                |r, p| r.phase(p, true),
            )
            .err()
            .unwrap();
        assert!(
            lists::primary_message(&error)
                .to_string()
                .contains("value bad value outside allowed range")
        );
        assert!(
            lists::primary_message(&error)
                .to_string()
                .contains("variable valueList:")
        );
        assert!(seen.borrow().is_some());
    }
}
#[test]
fn solving_error_closes_score_director_when_phase_fails() {
    for stream in [false, true] {
        let d = lists::Domain::new(2, 2, false);
        let f = lists::prepare(
            &d,
            stream,
            Rc::new(basic::Observations::default()),
            vec![None, Some(FullAssert)],
            10,
        );
        let saved = RefCell::<Option<lists::View<'_>>>::new(None);
        let mut callback = |event, view| {
            if event == (FactoryRunEvent::PhaseEntered { index: 1 }) {
                saved.replace(Some(view));
            }
            Ok(())
        };
        let error = f
            .solve_observed_assembled_with(
                d.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: Some(&mut callback),
                    failure: None,
                },
                |r, p| r.phase(p, true),
            )
            .err()
            .unwrap();
        assert!(
            lists::primary_message(&error)
                .to_string()
                .contains("value bad value outside allowed range")
        );
        assert!(
            lists::primary_message(&error)
                .to_string()
                .contains("variable valueList:")
        );
        let saved = saved.borrow();
        let view = saved.as_ref().unwrap();
        assert!(view.read_runtime_view(|r| r.input().clone()).is_none());
    }
}
