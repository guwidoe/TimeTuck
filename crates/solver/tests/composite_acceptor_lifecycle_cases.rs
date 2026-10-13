// Adapted from Community CompositeAcceptorTest.phaseLifecycle and its
// PlannerAssert.verifyPhaseLifecycle helper, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::RefCell;
use timetuck::*;
#[test]
fn phase_lifecycle() {
    let recorded: [RefCell<Vec<SolverLifecycleEvent>>; 3] =
        std::array::from_fn(|_| RefCell::new(vec![]));
    let children = recorded
        .iter()
        .map(|log| {
            ObservedAcceptorChild::new(
                Acceptance::HillClimbing
                    .start_with_values::<usize, usize>(SimpleScore::ZERO)
                    .unwrap(),
                |event| log.borrow_mut().push(event),
            )
        })
        .collect();
    let mut acceptor = ObservedCompositeAcceptor::new(children);
    let kind = SolverPhaseKind::LocalSearch;
    acceptor.lifecycle(SolverLifecycleEvent::SolveStarted);
    acceptor.lifecycle(SolverLifecycleEvent::PhaseStarted { phase: 0, kind });
    for step in 0..2 {
        acceptor.lifecycle(SolverLifecycleEvent::StepStarted {
            phase: 0,
            kind,
            step,
        });
        acceptor.lifecycle(SolverLifecycleEvent::StepEnded {
            phase: 0,
            kind,
            step,
            completed: true,
        });
    }
    acceptor.lifecycle(SolverLifecycleEvent::PhaseEnded { phase: 0, kind });
    acceptor.lifecycle(SolverLifecycleEvent::PhaseStarted { phase: 1, kind });
    acceptor.lifecycle(SolverLifecycleEvent::StepStarted {
        phase: 1,
        kind,
        step: 0,
    });
    acceptor.lifecycle(SolverLifecycleEvent::StepEnded {
        phase: 1,
        kind,
        step: 0,
        completed: true,
    });
    acceptor.lifecycle(SolverLifecycleEvent::PhaseEnded { phase: 1, kind });
    acceptor.lifecycle(SolverLifecycleEvent::SolveEnded);
    drop(acceptor);
    for events in recorded {
        let events = events.into_inner();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::SolveStarted))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::SolveEnded))
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::PhaseStarted { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::PhaseEnded { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::StepStarted { .. }))
                .count(),
            3
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, SolverLifecycleEvent::StepEnded { .. }))
                .count(),
            3
        );
    }
}
