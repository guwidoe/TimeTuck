use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn rows<V>() -> GroupInputs<(usize, usize), V> {
    let mut rows = GroupInputs::new();
    assert!(rows.bind_pair(3, 65));
    rows
}

#[test]
fn pair_box_snapshots_share_exact_value_owners_across_page_and_partial_page_boundaries() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Paged<(usize, usize), i64>>();
    struct NoClone(i64);
    let mut rows = rows();
    for (n, id) in [(0usize, 0usize), (0, 63), (0, 64), (1, 64), (2, 64)]
        .into_iter()
        .enumerate()
    {
        rows.insert(id, NoClone(n as i64));
    }
    let snapshot = rows.clone();
    let (GroupInputs::Paged(current), GroupInputs::Paged(saved)) = (&rows, &snapshot) else {
        unreachable!()
    };
    for (current, saved) in current.pages.iter().zip(&saved.pages) {
        match (current, saved) {
            (Some(current), Some(saved)) => {
                assert!(!std::ptr::eq(current.as_ref(), saved.as_ref()));
                for (current, saved) in current.iter().zip(saved.iter()) {
                    match (current, saved) {
                        (Some(current), Some(saved)) => assert!(Arc::ptr_eq(current, saved)),
                        (None, None) => {}
                        _ => panic!("snapshot changed occupancy"),
                    }
                }
            }
            (None, None) => {}
            _ => panic!("snapshot changed page occupancy"),
        }
    }
    let untouched = rows.get(&(0, 63)).unwrap() as *const NoClone;
    let original = rows.get(&(2, 64)).unwrap() as *const NoClone;
    let outer = rows.begin_journal();
    rows.insert((2, 64), NoClone(40));
    rows.remove(&(0, 0));
    let inner = rows.begin_journal();
    rows.insert((2, 63), NoClone(50));
    rows.commit_journal(inner);
    let candidate = rows.get(&(2, 64)).unwrap() as *const NoClone;
    let capture = rows.capture_current(outer);
    let super::super::Capture::Paged(capture) = capture else {
        unreachable!()
    };
    assert_eq!(capture.rows.len(), 3);
    assert!(capture.rows.iter().all(|(index, _)| *index < 3 * 65));
    rows.rollback_journal(outer);
    assert_eq!(rows.get(&(2, 64)).unwrap() as *const _, original);
    rows.install_capture(super::super::Capture::Paged(capture));
    assert_eq!(rows.get(&(2, 64)).unwrap() as *const _, candidate);
    assert_eq!(rows.get(&(0, 63)).unwrap() as *const _, untouched);
    assert_eq!(snapshot.get(&(2, 64)).unwrap().0, 4);
    assert_eq!(rows.get(&(2, 64)).unwrap().0, 40);
    assert!(rows.get(&(0, 0)).is_none());
    assert_eq!(rows.get(&(2, 63)).unwrap().0, 50);
    rows.install_snapshot(snapshot);
    assert_eq!(rows.get(&(2, 64)).unwrap() as *const _, original);
    assert!(rows.get(&(2, 63)).is_none());
}

#[test]
fn pair_box_full_install_journals_only_changed_canonical_cells_and_undo_remains_nested() {
    let mut rows = rows();
    rows.insert((0usize, 63usize), 1i64);
    rows.insert((2, 64), 2);
    let baseline = rows.clone();
    let outer = rows.begin_journal();
    rows.install_snapshot(baseline.clone());
    let GroupInputs::Paged(paged) = &rows else {
        unreachable!()
    };
    assert!(
        paged.frames[outer].is_empty(),
        "independent page boxes retain identical values"
    );
    rows.insert((2, 64), 20);
    rows.insert((0, 64), 30);
    let candidate = rows.clone();
    let original_owner = baseline.get(&(2, 64)).unwrap() as *const i64;
    let candidate_owner = candidate.get(&(2, 64)).unwrap() as *const i64;
    let inner = rows.begin_journal();
    rows.install_snapshot(baseline.clone());
    let GroupInputs::Paged(paged) = &rows else {
        unreachable!()
    };
    assert_eq!(paged.frames[inner].len(), 2);
    assert!(paged.frames[inner].iter().all(|(index, _)| *index < 195));
    rows.rollback_journal(inner);
    assert_eq!(rows.get(&(2, 64)).unwrap() as *const _, candidate_owner);
    assert_eq!(rows.get(&(0, 64)), Some(&30));
    rows.install_snapshot(baseline);
    rows.rollback_journal(outer);
    assert_eq!(rows.get(&(2, 64)).unwrap() as *const _, original_owner);
    assert!(rows.get(&(0, 64)).is_none());
    assert_eq!(
        candidate.get(&(2, 64)).unwrap() as *const _,
        candidate_owner
    );
}

#[test]
fn pair_box_snapshots_preflight_and_transaction_failure_preserve_borrowed_clone_and_drop_phases() {
    struct Value<'a>(usize, &'a Cell<usize>, Rc<RefCell<Vec<usize>>>);
    impl Clone for Value<'_> {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            panic!("explicit inspection clone")
        }
    }
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.2.borrow_mut().push(self.0);
        }
    }
    let clones = Cell::new(0);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut rows = rows();
    rows.insert((2usize, 64usize), Value(1, &clones, trace.clone()));
    let snapshot = rows.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            rows.prepare_change(&(2, 64), |value| value.unwrap().clone());
        }))
        .is_err()
    );
    assert_eq!(clones.get(), 1);
    assert!(trace.borrow().is_empty());
    let outer = rows.begin_journal();
    rows.insert((2, 64), Value(2, &clones, trace.clone()));
    let inner = rows.begin_journal();
    rows.insert((2, 64), Value(3, &clones, trace.clone()));
    rows.commit_journal(inner);
    let captured = rows.capture_current(outer);
    rows.rollback_journal(outer);
    assert_eq!(*trace.borrow(), [2]); // Captured final value is still alive.
    rows.install_capture(captured);
    assert_eq!(rows.get(&(2, 64)).unwrap().0, 3);
    assert_eq!(*trace.borrow(), [2]); // Full snapshot still owns original value.
    rows.install_snapshot(snapshot);
    assert_eq!(*trace.borrow(), [2, 3]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut trial = rows.journal_transaction();
            trial.remove(&(2, 64));
            trial.insert((0, 64), Value(4, &clones, trace.clone()));
            panic!("transaction failure");
        }))
        .is_err()
    );
    assert_eq!(*trace.borrow(), [2, 3, 4]);
    assert_eq!(rows.get(&(2, 64)).unwrap().0, 1);
    assert!(rows.get(&(0, 64)).is_none());
    assert_eq!(clones.get(), 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            rows.prepare_change(&(3, 0), |_| ())
                .0
                .insert((3, 0), Value(5, &clones, trace.clone()));
        }))
        .is_err()
    );
    assert_eq!(*trace.borrow(), [2, 3, 4, 5]);
    rows.clear();
    assert_eq!(*trace.borrow(), [2, 3, 4, 5, 1]);
}
