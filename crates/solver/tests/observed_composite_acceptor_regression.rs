// Original live policy/observer controls; no upstream method credit.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};
use timetuck::*;
#[test]
fn rejection_does_not_suppress_other_child_lifecycle_or_selected_step_memory() {
    let order = RefCell::new(vec![]);
    let children = [
        Acceptance::StrictImprovement,
        Acceptance::ValueTabu {
            tenure: 2,
            aspiration: false,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(i, p)| {
        let order = &order;
        ObservedAcceptorChild::new(
            p.start_with_values::<usize, usize>(SimpleScore::ZERO)
                .unwrap(),
            move |e| order.borrow_mut().push((i, e)),
        )
    })
    .collect();
    let mut a = ObservedCompositeAcceptor::new(children);
    assert!(!a.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[0], &0, &[7]));
    a.lifecycle(SolverLifecycleEvent::SolveStarted);
    a.step_started(Duration::ZERO);
    a.termination_step_started(TerminationProgressScope::Phase, Some(0.5))
        .unwrap();
    assert_eq!(
        a.termination_step_started(TerminationProgressScope::Phase, Some(f64::NAN)),
        Err(AcceptanceError::InvalidGradient)
    );
    a.step_ended(SimpleScore::ZERO, &[0], &0, &[7]);
    assert!(!a.child_mut(1).unwrap().accepts(
        SimpleScore::ZERO,
        SimpleScore::new(1),
        &[1],
        &1,
        &[7]
    ));
    a.lifecycle(SolverLifecycleEvent::SolveEnded);
    assert_eq!(
        *order.borrow(),
        [
            (0, SolverLifecycleEvent::SolveStarted),
            (1, SolverLifecycleEvent::SolveStarted),
            (0, SolverLifecycleEvent::SolveEnded),
            (1, SolverLifecycleEvent::SolveEnded)
        ]
    );
    assert!(a.child_mut(2).is_none());
}
#[test]
fn empty_conjunction_and_inline_observer_panic_keep_defined_order() {
    let mut empty = ObservedCompositeAcceptor::<SimpleScore, usize, usize>::new(vec![]);
    assert!(empty.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[], &0, &[]));
    empty.lifecycle(SolverLifecycleEvent::SolveStarted);
    let seen = RefCell::new(vec![]);
    let children = (0..3)
        .map(|i| {
            let seen = &seen;
            ObservedAcceptorChild::new(
                Acceptance::HillClimbing
                    .start_with_values::<usize, usize>(SimpleScore::ZERO)
                    .unwrap(),
                move |e| {
                    seen.borrow_mut().push((i, e));
                    assert_ne!(i, 1, "observer");
                },
            )
        })
        .collect();
    let mut a = ObservedCompositeAcceptor::new(children);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || a.lifecycle(SolverLifecycleEvent::SolveStarted)
        ))
        .is_err()
    );
    assert_eq!(seen.borrow().len(), 2);
    assert_eq!(seen.borrow()[0].0, 0);
    assert_eq!(seen.borrow()[1].0, 1);
}
