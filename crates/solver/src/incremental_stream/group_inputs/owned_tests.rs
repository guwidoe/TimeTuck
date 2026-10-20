//! Original graph-private builtin input controls; no upstream credit.
use super::*;
use crate::{GroupNode, TupleChange};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Key = (usize, usize);
const KEY: Key = (usize::MAX, 1);
fn dense(owned: bool) -> GroupNode<usize, Key> {
    let mut node = GroupNode::new();
    node.bind_primitive_group_keys();
    node.bind_primitive_inputs();
    node.enable_equal_input_reuse();
    if owned {
        node.enable_owned_inputs();
    }
    // Binding after initial population must retain every logical captured value.
    node.apply(TupleChange::put(0, (KEY, 1)));
    node.apply(TupleChange::put(1, (KEY, 2)));
    assert!(node.bind_fixed_inputs(3));
    assert_eq!(matches!(node.inputs, GroupInputs::Owned(_)), owned);
    node
}
fn groups<I: Ord>(node: &GroupNode<I, Key>) -> Vec<(Key, crate::IntSummary)> {
    node.groups().map(|(k, v)| (*k, v)).collect()
}
#[test]
fn owned_group_inputs_dense_changes_nested_captures_full_install_and_clear_match_original() {
    let mut fast = dense(true);
    let mut old = dense(false);
    let outer = fast.begin_journal_checkpoint();
    let old_outer = old.begin_journal_checkpoint();
    for change in [
        TupleChange::put(0, (KEY, 1)),
        TupleChange::put(0, (KEY, 9)),
        TupleChange::put(2, ((0, usize::MAX), -4)),
        TupleChange::retract(1),
        TupleChange::retract(1),
    ] {
        let child = fast.begin_journal_checkpoint();
        let old_child = old.begin_journal_checkpoint();
        assert_eq!(fast.apply(change.clone()), old.apply(change));
        fast.commit_journal_checkpoint(child);
        old.commit_journal_checkpoint(old_child);
    }
    let captured = fast.capture_checkpoint(outer.as_ref());
    let old_captured = old.capture_checkpoint(old_outer.as_ref());
    let full = fast.checkpoint();
    let old_full = old.checkpoint();
    fast.restore_checkpoint(outer);
    old.restore_checkpoint(old_outer);
    assert_eq!(groups(&fast), groups(&old));
    let parent = fast.begin_journal_checkpoint();
    let old_parent = old.begin_journal_checkpoint();
    fast.restore_checkpoint(captured);
    old.restore_checkpoint(old_captured);
    assert_eq!(groups(&fast), groups(&old));
    for id in 0..3 {
        assert_eq!(fast.inputs.get(&id), old.inputs.get(&id));
    }
    fast.clear();
    old.clear();
    fast.restore_checkpoint(full);
    old.restore_checkpoint(old_full);
    assert_eq!(groups(&fast), groups(&old));
    fast.restore_checkpoint(parent);
    old.restore_checkpoint(old_parent);
    for id in 0..3 {
        assert_eq!(fast.inputs.get(&id), old.inputs.get(&id));
    }
    assert_eq!(groups(&fast), groups(&old));
}
#[test]
fn owned_group_inputs_pair_sparse_boundaries_absence_rebirth_and_bounds_match_original() {
    fn pair(owned: bool) -> GroupNode<(usize, usize), Key> {
        let mut n = GroupNode::new();
        n.bind_primitive_group_keys();
        n.bind_primitive_inputs();
        if owned {
            n.enable_owned_inputs();
        }
        for id in [(0, 0), (0, 63), (0, 64), (2, 64)] {
            n.apply(TupleChange::put(id, (KEY, 1)));
        }
        assert!(n.bind_pair_inputs(3, 65));
        assert_eq!(matches!(n.inputs, GroupInputs::Owned(_)), owned);
        n
    }
    let mut n = pair(true);
    let mut old = pair(false);
    let outer = n.begin_journal_checkpoint();
    let old_outer = old.begin_journal_checkpoint();
    let child = n.begin_journal_checkpoint();
    let old_child = old.begin_journal_checkpoint();
    for change in [
        TupleChange::put((0, 63), ((0, usize::MAX), 7)),
        TupleChange::retract((0, 0)),
        TupleChange::retract((1, 64)),
        TupleChange::put((2, 64), (KEY, 1)),
        TupleChange::retract((2, 64)),
        TupleChange::put((2, 64), (KEY, -3)),
    ] {
        assert_eq!(n.apply(change.clone()), old.apply(change));
    }
    let captured = n.capture_checkpoint(outer.as_ref());
    let old_captured = old.capture_checkpoint(old_outer.as_ref());
    n.commit_journal_checkpoint(child);
    old.commit_journal_checkpoint(old_child);
    n.restore_checkpoint(outer);
    old.restore_checkpoint(old_outer);
    let outer = n.begin_journal_checkpoint();
    let old_outer = old.begin_journal_checkpoint();
    n.restore_checkpoint(captured);
    old.restore_checkpoint(old_captured);
    assert_eq!(groups(&n), groups(&old));
    for id in [(0, 0), (0, 63), (0, 64), (1, 64), (2, 64)] {
        assert_eq!(n.inputs.get(&id), old.inputs.get(&id));
    }
    let full = n.checkpoint();
    let old_full = old.checkpoint();
    n.clear();
    old.clear();
    n.restore_checkpoint(full);
    old.restore_checkpoint(old_full);
    assert_eq!(groups(&n), groups(&old));
    let child = n.begin_journal_checkpoint();
    let old_child = old.begin_journal_checkpoint();
    assert_eq!(
        n.apply(TupleChange::put((0, 64), (KEY, 20))),
        old.apply(TupleChange::put((0, 64), (KEY, 20)))
    );
    n.restore_checkpoint(child);
    old.restore_checkpoint(old_child);
    assert_eq!(n.inputs.get(&(0, 64)), old.inputs.get(&(0, 64)));
    let a = catch_unwind(AssertUnwindSafe(|| {
        n.apply(TupleChange::put((usize::MAX, 0), (KEY, 1)))
    }));
    let b = catch_unwind(AssertUnwindSafe(|| {
        old.apply(TupleChange::put((usize::MAX, 0), (KEY, 1)))
    }));
    assert!(a.is_err() && b.is_err());
    assert!(n.is_poisoned() && old.is_poisoned());
    n.restore_checkpoint(outer);
    old.restore_checkpoint(old_outer);
    assert_eq!(groups(&n), groups(&old));
    assert_eq!(n.inputs.get(&(usize::MAX, usize::MAX)), None);
}
#[test]
fn owned_group_inputs_preparation_failure_abandonment_and_unwind_restore_values() {
    let mut node = dense(true);
    let before = node.inputs.get(&0).copied();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = node
                .inputs
                .prepare_change(&0, |_| -> () { panic!("inspect") });
        }))
        .is_err()
    );
    {
        let (_prepared, observed) = node.inputs.prepare_change(&0, |v| v.copied());
        assert_eq!(observed, before);
    }
    assert_eq!(node.inputs.get(&0).copied(), before);
    {
        let mut tx = node.inputs.journal_transaction();
        tx.insert(0, ((0, usize::MAX), -1));
        assert!(tx.remove(&1));
    }
    assert_eq!(node.inputs.get(&0).copied(), before);
    assert_eq!(node.inputs.get(&1), Some(&(KEY, 2)));
    let outer = node.begin_journal_checkpoint();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply(TupleChange::retract(usize::MAX))
        ))
        .is_err()
    );
    assert!(node.is_poisoned());
    node.restore_checkpoint(outer);
    assert_eq!(node.inputs.get(&0).copied(), before);
}
#[test]
fn owned_group_inputs_failed_discovery_keeps_open_values_until_valid_bounded_conversion() {
    let mut n = GroupNode::<usize, Key>::new();
    n.bind_primitive_group_keys();
    n.bind_primitive_inputs();
    n.enable_owned_inputs();
    n.apply(TupleChange::put(usize::MAX, (KEY, 1)));
    assert!(!n.bind_fixed_inputs(4096));
    assert!(matches!(n.inputs, GroupInputs::Tree(_)));
    assert_eq!(n.inputs.get(&usize::MAX), Some(&(KEY, 1)));
    n.apply(TupleChange::retract(usize::MAX));
    n.apply(TupleChange::put(0, (KEY, 2)));
    assert!(!n.bind_fixed_inputs(4097));
    assert!(n.bind_fixed_inputs(1));
    assert!(matches!(n.inputs, GroupInputs::Owned(_)));
    let full = n.checkpoint();
    n.apply(TupleChange::put(0, (KEY, 9)));
    n.restore_checkpoint(full);
    assert_eq!(n.inputs.get(&0), Some(&(KEY, 2)));
    let mut pairs = GroupNode::<(usize, usize), Key>::new();
    pairs.bind_primitive_group_keys();
    pairs.enable_owned_inputs();
    pairs.apply(TupleChange::put((usize::MAX, usize::MAX), (KEY, -1)));
    assert!(!pairs.bind_pair_inputs(usize::MAX, usize::MAX));
    assert!(!pairs.bind_pair_inputs(2, 2));
    assert_eq!(
        pairs.inputs.get(&(usize::MAX, usize::MAX)),
        Some(&(KEY, -1))
    );
    pairs.apply(TupleChange::retract((usize::MAX, usize::MAX)));
    assert!(pairs.bind_pair_inputs(2, 2));
}
thread_local! {static TRACE:RefCell<Vec<usize>>=const{RefCell::new(Vec::new())};static FAIL:Cell<bool>=const{Cell::new(false)};}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Custom(usize);
impl Clone for Custom {
    fn clone(&self) -> Self {
        TRACE.with(|v| v.borrow_mut().push(self.0));
        assert!(!FAIL.with(Cell::get), "Clone panic");
        Self(self.0)
    }
}
#[test]
fn owned_group_inputs_exact_certificate_rejects_custom_clone_borrowed_unbound_and_standalone() {
    fn run(enable: bool) -> Vec<usize> {
        TRACE.with(|v| v.borrow_mut().clear());
        let mut n = GroupNode::<usize, Custom>::new();
        n.bind_primitive_group_keys();
        if enable {
            n.enable_owned_inputs();
        }
        assert!(n.owned_input_copy.is_none());
        n.apply(TupleChange::put(0, (Custom(0), 1)));
        assert!(n.bind_fixed_inputs(1));
        assert!(matches!(n.inputs, GroupInputs::Dense(_)));
        n.apply(TupleChange::put(0, (Custom(0), 2)));
        FAIL.with(|f| f.set(true));
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply(TupleChange::put(0, (Custom(0), 3)))
            ))
            .is_err()
        );
        FAIL.with(|f| f.set(false));
        assert!(!n.is_poisoned());
        TRACE.with(|v| v.take())
    }
    assert_eq!(run(false), run(true));
    let mut standalone = GroupNode::<usize, Key>::new();
    standalone.bind_primitive_group_keys();
    assert!(standalone.bind_fixed_inputs(1));
    assert!(matches!(standalone.inputs, GroupInputs::Dense(_)));
    let mut unbound = GroupNode::<usize, Key>::new();
    unbound.bind_primitive_group_keys();
    unbound.bind_primitive_inputs();
    unbound.enable_owned_inputs();
    unbound.apply(TupleChange::put(usize::MAX, (KEY, 1)));
    assert!(matches!(unbound.inputs, GroupInputs::Tree(_)));
    let text = "key".to_owned();
    let mut borrowed = GroupNode::<&str, &str>::new();
    borrowed.apply(TupleChange::put(text.as_str(), (text.as_str(), 1)));
    borrowed.apply(TupleChange::retract(text.as_str()));
    assert!(borrowed.groups().next().is_none());
    assert!(owned::group_copier::<usize>().is_some());
    assert!(owned::group_copier::<(usize, usize, usize, usize)>().is_some());
    assert!(owned::group_copier::<((usize, usize), (usize, usize))>().is_some());
    assert!(owned::group_copier::<Custom>().is_none());
}

#[test]
fn owned_group_inputs_custom_drop_keys_and_custom_id_types_keep_original_ownership_paths() {
    #[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
    struct Dropped(usize);
    impl Clone for Dropped {
        fn clone(&self) -> Self {
            TRACE.with(|t| t.borrow_mut().push(self.0));
            Self(self.0)
        }
    }
    impl Drop for Dropped {
        fn drop(&mut self) {
            TRACE.with(|t| t.borrow_mut().push(1000 + self.0));
        }
    }
    fn run(enable: bool) -> Vec<usize> {
        TRACE.with(|t| t.borrow_mut().clear());
        let mut n = GroupNode::<usize, Dropped>::new();
        n.bind_primitive_group_keys();
        if enable {
            n.enable_owned_inputs();
        }
        assert!(n.owned_input_copy.is_none());
        n.apply(TupleChange::put(0, (Dropped(2), 1)));
        assert!(n.bind_fixed_inputs(1));
        let full = n.checkpoint();
        let outer = n.begin_journal_checkpoint();
        n.apply(TupleChange::put(0, (Dropped(2), 3)));
        n.restore_checkpoint(outer);
        n.restore_checkpoint(full);
        drop(n);
        TRACE.with(|t| t.take())
    }
    assert_eq!(run(false), run(true));
    let mut ids = GroupNode::<Custom, Key>::new();
    ids.bind_primitive_group_keys();
    ids.enable_owned_inputs();
    assert!(ids.owned_input_copy.is_none());
    ids.apply(TupleChange::put(Custom(1), (KEY, 1)));
    assert!(!ids.bind_fixed_inputs(2));
    assert!(matches!(ids.inputs, GroupInputs::Tree(_)));
}

#[test]
fn owned_group_inputs_captures_after_equal_store_omission_keep_both_current_and_full_values() {
    for use_owned in [false, true] {
        let mut n = dense(use_owned);
        let outer = n.begin_journal_checkpoint();
        n.apply(TupleChange::put(0, (KEY, 1)));
        let captured = n.capture_checkpoint(outer.as_ref());
        let full = n.checkpoint();
        n.apply(TupleChange::put(0, ((0, usize::MAX), 9)));
        n.restore_checkpoint(outer);
        n.restore_checkpoint(captured);
        assert_eq!(n.inputs.get(&0), Some(&(KEY, 1)));
        assert_eq!(n.inputs.get(&1), Some(&(KEY, 2)));
        n.apply(TupleChange::retract(0));
        n.restore_checkpoint(full);
        assert_eq!(n.inputs.get(&0), Some(&(KEY, 1)));
        assert_eq!(n.groups().next().unwrap().1.sum, 3);
    }
}
