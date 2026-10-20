//! Original page-level ownership controls; no upstream method credit.
use super::*;
use crate::{GroupNode, TupleChange};
type Id = (usize, usize);
fn node() -> GroupNode<Id, usize> {
    let mut n = GroupNode::new();
    n.bind_primitive_group_keys();
    n.bind_primitive_inputs();
    n.enable_equal_input_reuse();
    assert!(n.bind_pair_inputs(2, 65));
    n
}
fn ptr(n: &GroupNode<Id, usize>) -> *const (usize, i64) {
    n.inputs.get(&(0, 0)).unwrap() as *const _
}
#[test]
fn equal_group_input_pair_copied_pages_snapshots_and_weak_owners_block_omission() {
    let mut n = node();
    n.apply(TupleChange::put((0, 0), (usize::MAX, 1)));
    let before = ptr(&n);
    let snapshot = n.inputs.clone();
    let GroupInputs::Paged(rows) = &n.inputs else {
        unreachable!()
    };
    assert_eq!(
        Arc::strong_count(rows.pages[0].as_ref().unwrap()[0].as_ref().unwrap()),
        2,
        "Box snapshot copies the container and shares the exact row owner"
    );
    let GroupInputs::Paged(saved) = &snapshot else {
        unreachable!()
    };
    assert!(!std::ptr::eq(
        rows.pages[0].as_ref().unwrap().as_ref(),
        saved.pages[0].as_ref().unwrap().as_ref()
    ));
    n.apply(TupleChange::put((0, 0), (usize::MAX, 1)));
    assert_ne!(ptr(&n), before);
    assert_eq!(snapshot.get(&(0, 0)).unwrap() as *const _, before);
    drop(snapshot);
    let before = ptr(&n);
    let GroupInputs::Paged(rows) = &n.inputs else {
        unreachable!()
    };
    let page_pointer = rows.pages[0].as_ref().unwrap().as_ref() as *const _;
    let value_weak = Arc::downgrade(rows.pages[0].as_ref().unwrap()[0].as_ref().unwrap());
    n.apply(TupleChange::put((0, 0), (usize::MAX, 1)));
    assert_ne!(ptr(&n), before);
    let GroupInputs::Paged(rows) = &n.inputs else {
        unreachable!()
    };
    assert_eq!(
        rows.pages[0].as_ref().unwrap().as_ref() as *const _,
        page_pointer,
        "unique Box page remains in place"
    );
    assert!(value_weak.upgrade().is_none());
}
#[test]
fn equal_group_input_pair_unique_duplicates_current_captures_and_nested_rollback() {
    let mut n = node();
    n.apply(TupleChange::put((0, 0), (usize::MAX, 1)));
    let first = ptr(&n);
    let outer = n.begin_journal_checkpoint();
    n.apply(TupleChange::put((0, 0), (usize::MAX, 1)));
    assert_eq!(ptr(&n), first);
    let inner = n.begin_journal_checkpoint();
    n.apply(TupleChange::put((0, 0), (usize::MAX, 2)));
    let changed = ptr(&n);
    n.commit_journal_checkpoint(inner);
    let current = n.capture_checkpoint(outer.as_ref());
    n.apply(TupleChange::put((0, 0), (usize::MAX, 2)));
    assert_ne!(ptr(&n), changed);
    n.restore_checkpoint(current);
    assert_eq!(ptr(&n), changed);
    let full = n.checkpoint();
    n.clear();
    n.restore_checkpoint(full);
    assert_eq!(ptr(&n), changed);
    n.restore_checkpoint(outer);
    assert_eq!(ptr(&n), first);
    assert_eq!(n.inputs.get(&(0, 0)), Some(&(usize::MAX, 1)));
    let before = ptr(&n);
    let outer = n.begin_journal_checkpoint();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || n.apply(TupleChange::put((usize::MAX, 0), (0, 1)))
        ))
        .is_err()
    );
    assert!(n.is_poisoned());
    n.restore_checkpoint(outer);
    assert_eq!(ptr(&n), before);
}
