use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn update(
    phase: usize,
    kind: SolverPhaseKind,
    value: &i32,
    score: i64,
) -> SolverUpdate<'_, i32, SimpleScore> {
    SolverUpdate {
        phase,
        kind,
        assignment: value,
        score: SimpleScore::new(score),
        complete: true,
    }
}
#[test]
fn retains_latest_per_origin_and_flushes_without_resetting_the_baseline() {
    let mut b = PhaseBestBuffer::new(SimpleScore::new(0), true);
    for value in 1..=4 {
        assert!(
            b.observe(update(
                0,
                SolverPhaseKind::Construction,
                &value,
                value as i64
            ))
            .is_none()
        );
    }
    assert!(b.has_pending());
    let ready = b
        .observe(update(1, SolverPhaseKind::LocalSearch, &5, 5))
        .unwrap();
    assert_eq!(
        (ready.phase, ready.kind, ready.assignment, ready.score),
        (0, SolverPhaseKind::Construction, 4, SimpleScore::new(4))
    );
    assert!(
        b.observe(update(1, SolverPhaseKind::LocalSearch, &999, 5))
            .is_none()
    );
    assert!(
        b.observe(update(1, SolverPhaseKind::LocalSearch, &999, 3))
            .is_none()
    );
    let ready = b.flush().unwrap();
    assert_eq!(ready.assignment, 5);
    assert!(!b.has_pending());
    assert!(b.flush().is_none());
    assert!(
        b.observe(update(1, SolverPhaseKind::LocalSearch, &999, 4))
            .is_none()
    );
    assert!(!b.has_pending());
    assert!(
        b.observe(update(1, SolverPhaseKind::LocalSearch, &6, 6))
            .is_none()
    );
    // Origin includes phase kind even if callers reuse a numeric phase ID.
    assert_eq!(
        b.observe(update(1, SolverPhaseKind::Custom, &7, 7))
            .unwrap()
            .assignment,
        6
    );
    assert_eq!(b.flush().unwrap().assignment, 7);
}
#[test]
fn an_unimproved_new_phase_still_delivers_the_previous_pending_snapshot() {
    let mut b = PhaseBestBuffer::new(SimpleScore::ZERO.with_structural(-2), false);
    let mut incomplete = update(0, SolverPhaseKind::Construction, &1, 0);
    incomplete.score = SimpleScore::ZERO.with_structural(-1);
    incomplete.complete = false;
    b.observe(incomplete);
    let mut equal = update(1, SolverPhaseKind::Custom, &99, 0);
    equal.score = SimpleScore::ZERO.with_structural(-1);
    equal.complete = false;
    let previous = b.observe(equal).unwrap();
    assert_eq!(previous.assignment, 1);
    assert!(!previous.complete);
    assert!(!b.has_pending());
    b.observe(update(2, SolverPhaseKind::Construction, &2, 0));
    assert!(b.flush().unwrap().complete);
}
#[derive(Debug, PartialEq, Eq)]
struct FallibleClone(bool);
impl Clone for FallibleClone {
    fn clone(&self) -> Self {
        assert!(!self.0, "clone failure");
        Self(self.0)
    }
}
#[test]
fn failed_snapshot_clone_preserves_previous_phase_delivery() {
    let mut b = PhaseBestBuffer::new(SimpleScore::new(0), true);
    b.observe(SolverUpdate {
        phase: 0,
        kind: SolverPhaseKind::Construction,
        assignment: &FallibleClone(false),
        score: SimpleScore::new(1),
        complete: true,
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| b.observe(SolverUpdate {
            phase: 1,
            kind: SolverPhaseKind::LocalSearch,
            assignment: &FallibleClone(true),
            score: SimpleScore::new(2),
            complete: true
        })))
        .is_err()
    );
    let previous = b.flush().unwrap();
    assert_eq!(previous.phase, 0);
    assert_eq!(previous.score, SimpleScore::new(1));
    assert_eq!(previous.assignment, FallibleClone(false));
}
#[test]
fn baseline_and_rejected_updates_do_not_clone_assignments() {
    struct Counted(Rc<Cell<usize>>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    let count = Rc::new(Cell::new(0));
    let value = Counted(count.clone());
    let mut b = PhaseBestBuffer::new(SimpleScore::new(5), true);
    for score in [5, 4, 3] {
        b.observe(SolverUpdate {
            phase: 0,
            kind: SolverPhaseKind::LocalSearch,
            assignment: &value,
            score: SimpleScore::new(score),
            complete: true,
        });
    }
    assert_eq!(count.get(), 0);
    assert!(!b.has_pending());
    b.observe(SolverUpdate {
        phase: 0,
        kind: SolverPhaseKind::LocalSearch,
        assignment: &value,
        score: SimpleScore::new(6),
        complete: true,
    });
    assert_eq!(count.get(), 1);
    assert!(b.flush().is_some());
    assert_eq!(count.get(), 1);
}
