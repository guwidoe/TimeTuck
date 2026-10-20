use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
fn dense<V>() -> GroupInputs<usize, V> {
    let mut rows = GroupInputs::new();
    assert!(rows.bind_fixed(2));
    rows
}

#[test]
fn group_input_pool_dense_reuses_only_retired_owners_after_nested_commit_and_rollback() {
    let mut rows = dense();
    rows.insert(0, 1usize);
    let first = rows.get(&0).unwrap() as *const usize;
    let outer = rows.begin_journal();
    rows.insert(0, 2);
    let second = rows.get(&0).unwrap() as *const usize;
    let inner = rows.begin_journal();
    rows.insert(0, 3);
    rows.commit_journal(inner);
    rows.insert(1, 4);
    let fourth = rows.get(&1).unwrap() as *const usize;
    assert_ne!(fourth, first);
    assert_ne!(fourth, second, "active undo owners cannot be reused");
    rows.rollback_journal(outer);
    assert_eq!(rows.get(&0), Some(&1));
    assert!(rows.get(&1).is_none());
    rows.insert(0, 5);
    assert_eq!(rows.get(&0).unwrap() as *const usize, second);
    let outer = rows.begin_journal();
    rows.insert(0, 6);
    rows.commit_journal(outer);
    rows.insert(1, 7);
    assert_eq!(rows.get(&1).unwrap() as *const usize, second);
    rows.remove(&1);
    let mut snapshot = rows.clone();
    snapshot.insert(1, 8);
    assert_ne!(
        snapshot.get(&1).unwrap() as *const usize,
        second,
        "cloned caches start with empty pools"
    );
    rows.insert(1, 9);
    assert_eq!(rows.get(&1).unwrap() as *const usize, second);
    assert_eq!(snapshot.get(&1), Some(&8));
}

#[test]
fn group_input_pool_dense_snapshot_capture_and_weak_observers_keep_their_original_owners() {
    let mut rows = dense();
    rows.insert(0, 10usize);
    let first = rows.get(&0).unwrap() as *const usize;
    let original = rows.clone();
    let outer = rows.begin_journal();
    rows.insert(0, 20);
    let candidate = rows.get(&0).unwrap() as *const usize;
    let capture = rows.capture_current(outer);
    rows.commit_journal(outer);
    let GroupInputs::Dense(dense) = &rows else {
        unreachable!()
    };
    let weak = Arc::downgrade(dense.values[0].as_ref().unwrap());
    rows.insert(1, 30);
    assert_ne!(rows.get(&1).unwrap() as *const usize, first);
    assert_ne!(rows.get(&1).unwrap() as *const usize, candidate);
    rows.remove(&0);
    assert!(weak.upgrade().is_some(), "capture still owns candidate");
    drop(capture);
    assert!(
        weak.upgrade().is_none(),
        "pool cannot extend weak-observed lifetime"
    );
    rows.insert(0, 40);
    assert_ne!(rows.get(&0).unwrap() as *const usize, candidate);
    assert_eq!(original.get(&0), Some(&10));
    assert_eq!(original.get(&0).unwrap() as *const usize, first);
    let base = rows.clone();
    let outer = rows.begin_journal();
    rows.insert(0, 50);
    rows.install_snapshot(original.clone());
    rows.rollback_journal(outer);
    assert!(std::ptr::eq(rows.get(&0).unwrap(), base.get(&0).unwrap()));
    assert!(std::ptr::eq(rows.get(&1).unwrap(), base.get(&1).unwrap()));
}

#[test]
fn group_input_pool_dense_preserves_user_clone_preflight_bounds_and_drop_phases() {
    struct NoDrop<'a>(usize, &'a Cell<usize>);
    impl Clone for NoDrop<'_> {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            panic!("inspect clone");
        }
    }
    let clones = Cell::new(0);
    let mut rows = dense();
    rows.insert(0, NoDrop(1, &clones));
    rows.insert(1, NoDrop(2, &clones));
    let retired = rows.get(&1).unwrap() as *const NoDrop<'_>;
    rows.remove(&1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            rows.prepare_change(&0, |value| value.unwrap().clone());
        }))
        .is_err()
    );
    assert_eq!(rows.get(&0).unwrap().0, 1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| rows
            .prepare_change(&2, |_| ())
            .0
            .insert(2, NoDrop(9, &clones))))
        .is_err()
    );
    {
        let (_prepared, old) = rows.prepare_change(&1, |row| row.is_some());
        assert!(!old);
    }
    rows.prepare_change(&1, |_| ())
        .0
        .insert(1, NoDrop(3, &clones));
    assert_eq!(rows.get(&1).unwrap() as *const NoDrop<'_>, retired);
    assert_eq!(clones.get(), 1);
    let snapshot = rows.clone();
    let outer = rows.begin_journal();
    rows.insert(0, NoDrop(4, &clones));
    let captured = rows.capture_current(outer);
    rows.rollback_journal(outer);
    rows.install_capture(captured);
    rows.install_snapshot(snapshot);
    assert_eq!(clones.get(), 1);
    struct Dropped(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut rows = dense();
    rows.insert(0, Dropped(1, trace.clone()));
    let outer = rows.begin_journal();
    rows.insert(0, Dropped(2, trace.clone()));
    let inner = rows.begin_journal();
    rows.insert(0, Dropped(3, trace.clone()));
    rows.commit_journal(inner);
    assert!(trace.borrow().is_empty());
    rows.rollback_journal(outer);
    assert_eq!(*trace.borrow(), vec![3, 2]);
    let outer = rows.begin_journal();
    rows.insert(0, Dropped(4, trace.clone()));
    rows.commit_journal(outer);
    assert_eq!(*trace.borrow(), vec![3, 2, 1]);
    rows.clear();
    assert_eq!(*trace.borrow(), vec![3, 2, 1, 4]);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || rows.insert(2, Dropped(9, trace.clone()))
        ))
        .is_err()
    );
    assert_eq!(*trace.borrow(), vec![3, 2, 1, 4, 9]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut trial = rows.journal_transaction();
            trial.insert(0, Dropped(5, trace.clone()));
            panic!("transaction");
        }))
        .is_err()
    );
    assert_eq!(*trace.borrow(), vec![3, 2, 1, 4, 9, 5]);
}
