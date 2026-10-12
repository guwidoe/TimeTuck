// Adapted pinned Community BestSolutionHolderTest and TEST-only TestdataSolution
// generateSolution fixture. Apache-2.0; see compat/provenance.json and notices.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
fn generated_solution() -> Arc<FiniteProblem<i32>> {
    Arc::new(
        FiniteProblem::new(
            (0..7)
                .map(|i| Variable::new((0..5).collect()).with_initial(i % 5))
                .collect(),
        )
        .unwrap(),
    )
}
fn producer(phase: usize, kind: SolverPhaseKind) -> SolverEventProducer {
    SolverEventProducer { phase, kind }
}
fn add_problem_change<T>(slot: &mut BestDeliverySlot<T>) -> BestDeliveryReceipt {
    let change = Arc::new(());
    let expected = change.clone();
    let calls = Cell::new(0);
    let receipt = slot
        .submit_changes(vec![change], |changes| {
            calls.set(calls.get() + 1);
            assert_eq!(changes.len(), 1);
            assert!(Arc::ptr_eq(&changes[0], &expected));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    receipt
}
#[test]
fn set_best_solution() {
    let mut slot = BestDeliverySlot::default();
    assert!(slot.take_best().is_none());
    let s1 = generated_solution();
    let s2 = generated_solution();
    assert!(!Arc::ptr_eq(&s1, &s2));
    slot.set_if(
        s1.clone(),
        producer(0, SolverPhaseKind::Construction),
        || true,
    );
    let delivery = slot.take_best().unwrap();
    assert!(Arc::ptr_eq(&delivery.solution, &s1));
    assert_eq!(
        delivery.producer,
        producer(0, SolverPhaseKind::Construction)
    );
    assert!(slot.take_best().is_none());
    slot.set_if(
        s1.clone(),
        producer(1, SolverPhaseKind::Construction),
        || true,
    );
    slot.set_if(
        s2.clone(),
        producer(2, SolverPhaseKind::LocalSearch),
        || false,
    );
    let delivery = slot.take_best().unwrap();
    assert!(Arc::ptr_eq(&delivery.solution, &s1));
    assert_eq!(
        delivery.producer,
        producer(1, SolverPhaseKind::Construction)
    );
    slot.set_if(s1, producer(3, SolverPhaseKind::Custom), || true);
    slot.set_if(s2.clone(), producer(4, SolverPhaseKind::Custom), || true);
    let delivery = slot.take_best().unwrap();
    assert!(Arc::ptr_eq(&delivery.solution, &s2));
    assert_eq!(delivery.producer, producer(4, SolverPhaseKind::Custom));
}
#[test]
fn complete_problem_changes() {
    let mut slot = BestDeliverySlot::default();
    let change1 = add_problem_change(&mut slot);
    slot.set_if(
        generated_solution(),
        producer(0, SolverPhaseKind::Construction),
        || true,
    );
    let change2 = add_problem_change(&mut slot);
    slot.take_best().unwrap().complete_problem_changes();
    assert_eq!(change1.delivery_status(), BestDeliveryStatus::Delivered);
    assert_eq!(change2.delivery_status(), BestDeliveryStatus::Pending);
    let change3 = add_problem_change(&mut slot);
    slot.set_if(
        generated_solution(),
        producer(1, SolverPhaseKind::Construction),
        || true,
    );
    slot.set_if(
        generated_solution(),
        producer(2, SolverPhaseKind::LocalSearch),
        || true,
    );
    let change4 = add_problem_change(&mut slot);
    slot.take_best().unwrap().complete_problem_changes();
    assert_eq!(change2.delivery_status(), BestDeliveryStatus::Delivered);
    assert_eq!(change3.delivery_status(), BestDeliveryStatus::Delivered);
    assert_eq!(change4.delivery_status(), BestDeliveryStatus::Pending);
}
#[test]
fn cancel_pending_changes_no_changes_retrieved() {
    let mut slot = BestDeliverySlot::default();
    let change = add_problem_change(&mut slot);
    slot.set_if(
        generated_solution(),
        producer(0, SolverPhaseKind::Construction),
        || true,
    );
    slot.cancel_pending_changes();
    slot.take_best().unwrap().complete_problem_changes();
    assert_eq!(change.delivery_status(), BestDeliveryStatus::Cancelled);
}
