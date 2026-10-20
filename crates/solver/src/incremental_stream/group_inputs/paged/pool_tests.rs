use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
fn pair_rows<V>() -> GroupInputs<(usize, usize), V> {
    let mut rows = GroupInputs::new();
    assert!(rows.bind_pair(3, 65));
    rows
}

#[test]
fn group_input_pool_paged_reuses_retired_values_with_padding_captures_and_outer_snapshot_installs()
{
    let id = (2usize, 64usize);
    let mut rows = pair_rows();
    rows.insert(id, 1usize);
    let first = rows.get(&id).unwrap() as *const usize;
    let outer = rows.begin_journal();
    rows.insert(id, 2);
    let second = rows.get(&id).unwrap() as *const usize;
    let inner = rows.begin_journal();
    rows.insert(id, 3);
    rows.commit_journal(inner);
    rows.insert((0, 0), 4);
    assert_ne!(rows.get(&(0, 0)).unwrap() as *const usize, first);
    assert_ne!(rows.get(&(0, 0)).unwrap() as *const usize, second);
    rows.rollback_journal(outer);
    assert_eq!(rows.get(&id), Some(&1));
    rows.prepare_change(&id, |_| ()).0.insert(id, 5);
    assert_eq!(rows.get(&id).unwrap() as *const usize, second);
    let snapshot = rows.clone();
    let outer = rows.begin_journal();
    rows.insert(id, 6);
    rows.insert((1, 64), 7);
    let capture = rows.capture_current(outer);
    let expected = rows.clone();
    rows.rollback_journal(outer);
    let enclosing = rows.begin_journal();
    rows.install_capture(capture);
    assert!(std::ptr::eq(
        rows.get(&id).unwrap(),
        expected.get(&id).unwrap()
    ));
    rows.install_snapshot(snapshot.clone());
    rows.rollback_journal(enclosing);
    assert!(std::ptr::eq(
        rows.get(&id).unwrap(),
        snapshot.get(&id).unwrap()
    ));
    assert!(rows.get(&(1, 64)).is_none());
    assert!(
        catch_unwind(AssertUnwindSafe(|| rows
            .prepare_change(&(3, 0), |_| ())
            .0
            .insert((3, 0), 9)))
        .is_err()
    );
    assert_eq!(rows.get(&id), Some(&5));
}

#[test]
fn group_input_pool_paged_preserves_page_snapshot_weak_lifetimes_and_custom_hooks() {
    let id = (2usize, 64usize);
    let mut rows = pair_rows();
    rows.insert(id, 1usize);
    let GroupInputs::Paged(paged) = &rows else {
        unreachable!()
    };
    let slot = paged.index(&id).unwrap();
    let page = paged.pages[slot / PAGE_SIZE].as_ref().unwrap();
    let old_weak = Arc::downgrade(page[slot % PAGE_SIZE].as_ref().unwrap());
    let page_address = page.as_ref() as *const Page<usize>;
    let snapshot = rows.clone();
    let GroupInputs::Paged(saved) = &snapshot else {
        unreachable!()
    };
    let saved_page = saved.pages[slot / PAGE_SIZE].as_ref().unwrap();
    // Only the private page representation changes: snapshots copy page boxes
    // and share exactly the same canonical value owners.
    assert_ne!(page_address, saved_page.as_ref() as *const Page<usize>);
    assert!(Arc::ptr_eq(
        page[slot % PAGE_SIZE].as_ref().unwrap(),
        saved_page[slot % PAGE_SIZE].as_ref().unwrap(),
    ));
    rows.insert(id, 2);
    assert!(old_weak.upgrade().is_some());

    assert_eq!(snapshot.get(&id), Some(&1));
    drop(snapshot);
    assert!(old_weak.upgrade().is_none());

    let GroupInputs::Paged(paged) = &rows else {
        unreachable!()
    };
    let weak = Arc::downgrade(
        paged.pages[slot / PAGE_SIZE].as_ref().unwrap()[slot % PAGE_SIZE]
            .as_ref()
            .unwrap(),
    );
    let old = rows.get(&id).unwrap() as *const usize;
    let outer = rows.begin_journal();
    rows.insert(id, 3);
    rows.commit_journal(outer);
    assert!(weak.upgrade().is_none());
    rows.insert((0, 0), 4);
    assert_ne!(
        rows.get(&(0, 0)).unwrap() as *const usize,
        old,
        "weak-observed allocation is never recycled"
    );
    struct NoDrop<'a>(usize, &'a Cell<usize>);
    impl Clone for NoDrop<'_> {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            panic!("mapped clone preflight");
        }
    }
    let clones = Cell::new(0);
    let mut rows = pair_rows();
    rows.insert(id, NoDrop(10, &clones));
    rows.insert((0, 0), NoDrop(11, &clones));
    let retired = rows.get(&(0, 0)).unwrap() as *const NoDrop<'_>;
    rows.remove(&(0, 0));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            rows.prepare_change(&id, |value| value.unwrap().clone());
        }))
        .is_err()
    );
    rows.insert((0, 0), NoDrop(12, &clones));
    assert_eq!(rows.get(&(0, 0)).unwrap() as *const NoDrop<'_>, retired);
    assert_eq!(rows.get(&id).unwrap().0, 10);
    assert_eq!(clones.get(), 1);
    struct Dropped(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut rows = pair_rows();
    rows.insert(id, Dropped(1, trace.clone()));
    let outer = rows.begin_journal();
    rows.insert(id, Dropped(2, trace.clone()));
    let inner = rows.begin_journal();
    rows.insert(id, Dropped(3, trace.clone()));
    rows.commit_journal(inner);
    assert!(trace.borrow().is_empty());
    rows.rollback_journal(outer);
    assert_eq!(*trace.borrow(), vec![3, 2]);
    rows.clear();
    assert_eq!(*trace.borrow(), vec![3, 2, 1]);
}
