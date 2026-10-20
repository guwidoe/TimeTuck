//! Original equal-input controls; no upstream method credit.
use super::*;
use crate::{GroupNode, SimpleScore, StreamGraphBuilder, TupleChange};
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
};
type Key = (usize, usize);
const KEY: Key = (usize::MAX, 1);
fn node(reuse: bool) -> GroupNode<usize, Key> {
    let mut n = GroupNode::new();
    n.bind_primitive_group_keys();
    n.bind_primitive_inputs();
    assert!(n.bind_fixed_inputs(2));
    if reuse {
        n.enable_equal_input_reuse();
    }
    n
}
fn pointer(n: &GroupNode<usize, Key>) -> *const (Key, i64) {
    n.inputs.get(&0).unwrap() as *const _
}
#[test]
fn equal_group_input_duplicates_keep_all_emissions_and_singleton_rebirth_work() {
    for singleton in [false, true] {
        let mut n = node(true);
        let mut old = node(false);
        n.apply(TupleChange::put(0, (KEY, 1)));
        old.apply(TupleChange::put(0, (KEY, 1)));
        if !singleton {
            n.apply(TupleChange::put(1, (KEY, 1)));
            old.apply(TupleChange::put(1, (KEY, 1)));
        }
        let before = pointer(&n);
        let old_before = pointer(&old);
        let group_keys = n.groups.shared_entries();
        let key_pointer = Arc::as_ptr(&group_keys[0].0);
        let key_weak = Arc::downgrade(&group_keys[0].0);
        drop(group_keys);
        let outer = n.begin_journal_checkpoint();
        let old_outer = old.begin_journal_checkpoint();
        for _ in 0..3 {
            let actual = n.apply(TupleChange::put(0, (KEY, 1)));
            assert_eq!(actual, old.apply(TupleChange::put(0, (KEY, 1))));
            assert_eq!(actual.len(), 1, "identical mapped inputs still emit");
            assert_eq!(pointer(&n), before);
        }
        assert_ne!(
            pointer(&old),
            old_before,
            "original stores remain journaled"
        );
        if singleton {
            assert_ne!(
                Arc::as_ptr(&n.groups.shared_entries()[0].0),
                key_pointer,
                "singleton group still removed/reborn"
            );
        }
        n.restore_checkpoint(outer);
        old.restore_checkpoint(old_outer);
        assert_eq!(pointer(&n), before);
        assert_eq!(pointer(&old), old_before);
        assert!(key_weak.upgrade().is_some());
    }
}
#[test]
fn equal_group_input_held_full_current_weak_and_changed_rows_force_original_replacement() {
    let mut n = node(true);
    n.apply(TupleChange::put(0, (KEY, 1)));
    let full = n.checkpoint();
    let first = pointer(&n);
    n.apply(TupleChange::put(0, (KEY, 1)));
    assert_ne!(pointer(&n), first);
    n.restore_checkpoint(full);
    assert_eq!(pointer(&n), first);
    let outer = n.begin_journal_checkpoint();
    n.apply(TupleChange::put(0, (KEY, 2)));
    let changed = pointer(&n);
    assert_ne!(changed, first);
    let current = n.capture_checkpoint(outer.as_ref());
    n.apply(TupleChange::put(0, (KEY, 2)));
    assert_ne!(pointer(&n), changed, "capture owns current value");
    n.restore_checkpoint(current);
    assert_eq!(pointer(&n), changed);
    n.restore_checkpoint(outer);
    assert_eq!(pointer(&n), first);
    let GroupInputs::Dense(rows) = &n.inputs else {
        unreachable!()
    };
    let weak = Arc::downgrade(rows.values[0].as_ref().unwrap());
    let outer = n.begin_journal_checkpoint();
    n.apply(TupleChange::put(0, (KEY, 1)));
    assert_ne!(pointer(&n), first);
    assert!(weak.upgrade().is_some());
    n.commit_journal_checkpoint(outer);
    assert!(weak.upgrade().is_none());
}
#[test]
fn equal_group_input_nested_children_changed_values_invalid_bounds_and_clear_restore() {
    let mut n = node(true);
    let mut old = node(false);
    for x in [&mut n, &mut old] {
        x.apply(TupleChange::put(0, (KEY, 1)));
    }
    let outer = n.begin_journal_checkpoint();
    let old_outer = old.begin_journal_checkpoint();
    for (key, value) in [
        (KEY, 1),
        (KEY, 3),
        ((0, usize::MAX), 3),
        ((0, usize::MAX), 3),
    ] {
        let inner = n.begin_journal_checkpoint();
        let old_inner = old.begin_journal_checkpoint();
        assert_eq!(
            n.apply(TupleChange::put(0, (key, value))),
            old.apply(TupleChange::put(0, (key, value)))
        );
        n.commit_journal_checkpoint(inner);
        old.commit_journal_checkpoint(old_inner);
    }
    let middle = n.checkpoint();
    let old_middle = old.checkpoint();
    n.clear();
    old.clear();
    n.restore_checkpoint(middle);
    old.restore_checkpoint(old_middle);
    assert_eq!(
        n.groups().map(|(k, v)| (*k, v)).collect::<Vec<_>>(),
        old.groups().map(|(k, v)| (*k, v)).collect::<Vec<_>>()
    );
    let a = catch_unwind(AssertUnwindSafe(|| {
        n.apply(TupleChange::put(usize::MAX, (KEY, 1)))
    }));
    let b = catch_unwind(AssertUnwindSafe(|| {
        old.apply(TupleChange::put(usize::MAX, (KEY, 1)))
    }));
    assert!(a.is_err() && b.is_err());
    assert!(n.is_poisoned() && old.is_poisoned());
    n.restore_checkpoint(outer);
    old.restore_checkpoint(old_outer);
    assert_eq!(n.inputs.get(&0), Some(&(KEY, 1)));
    assert_eq!(old.inputs.get(&0), Some(&(KEY, 1)));
}
#[test]
fn equal_group_input_captures_taken_after_omission_replay_logical_state_after_rollback() {
    for reuse in [false, true] {
        let mut n = node(reuse);
        n.apply(TupleChange::put(0, (KEY, 1)));
        let outer = n.begin_journal_checkpoint();
        n.apply(TupleChange::put(0, (KEY, 1)));
        let current = n.capture_checkpoint(outer.as_ref());
        let full = n.checkpoint();
        n.apply(TupleChange::put(0, ((0, usize::MAX), 9)));
        n.restore_checkpoint(outer);
        n.restore_checkpoint(current);
        assert_eq!(n.inputs.get(&0), Some(&(KEY, 1)));
        let summaries: Vec<_> = n.groups().map(|(k, s)| (*k, s)).collect();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].0, KEY);
        assert_eq!((summaries[0].1.count, summaries[0].1.sum), (1, 1));
        n.apply(TupleChange::retract(0));
        assert!(n.groups().next().is_none());
        n.restore_checkpoint(full);
        assert_eq!(n.inputs.get(&0), Some(&(KEY, 1)));
        assert_eq!(n.groups().next().unwrap().1.sum, 1);
    }
}
thread_local! {static CLONES:RefCell<Vec<usize>>=const {RefCell::new(Vec::new())}; static FAIL_KEY:Cell<bool>=const {Cell::new(false)};}
#[derive(Debug)]
struct Custom(usize, usize);
impl Clone for Custom {
    fn clone(&self) -> Self {
        CLONES.with(|c| c.borrow_mut().push(self.1));
        assert!(
            !(FAIL_KEY.with(Cell::get) && self.1 == 2),
            "custom key Clone panic"
        );
        Self(self.0, self.1)
    }
}
impl PartialEq for Custom {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl Eq for Custom {}
impl Ord for Custom {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.cmp(&o.0)
    }
}
impl PartialOrd for Custom {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
#[test]
fn equal_group_input_standalone_custom_no_drop_borrowed_and_unbound_keep_original_stores() {
    fn custom(enable: bool) -> Vec<usize> {
        CLONES.with(|c| c.borrow_mut().clear());
        let mut n = GroupNode::<usize, Custom>::new();
        assert!(!n.bind_primitive_group_keys());
        n.bind_primitive_inputs();
        n.bind_fixed_inputs(2);
        if enable {
            n.enable_equal_input_reuse();
        }
        assert!(!n.equal_input_reuse);
        n.apply(TupleChange::put(0, (Custom(0, 1), 1)));
        let before = n.inputs.get(&0).unwrap() as *const _;
        let outer = n.begin_journal_checkpoint();
        n.apply(TupleChange::put(0, (Custom(0, 2), 1)));
        assert_ne!(n.inputs.get(&0).unwrap() as *const _, before);
        assert_eq!(n.inputs.get(&0).unwrap().0.1, 2);
        n.restore_checkpoint(outer);
        FAIL_KEY.with(|f| f.set(true));
        let result = catch_unwind(AssertUnwindSafe(|| {
            n.apply(TupleChange::put(0, (Custom(0, 2), 1)))
        }));
        FAIL_KEY.with(|f| f.set(false));
        assert!(result.is_err());
        assert!(!n.is_poisoned());
        assert_eq!(n.inputs.get(&0).unwrap().0.1, 1);
        CLONES.with(|c| c.take())
    }
    assert_eq!(custom(false), custom(true));
    let mut standalone = node(false);
    standalone.apply(TupleChange::put(0, (KEY, 1)));
    let ptr = pointer(&standalone);
    let outer = standalone.begin_journal_checkpoint();
    standalone.apply(TupleChange::put(0, (KEY, 1)));
    assert_ne!(pointer(&standalone), ptr);
    standalone.restore_checkpoint(outer);
    let text = "borrowed".to_owned();
    let mut borrowed = GroupNode::<&str, usize>::new();
    borrowed.bind_primitive_group_keys();
    borrowed.enable_equal_input_reuse();
    borrowed.apply(TupleChange::put(text.as_str(), (0, 1)));
    let before = borrowed.inputs.get(&text.as_str()).unwrap() as *const _;
    borrowed.apply(TupleChange::put(text.as_str(), (0, 1)));
    assert_ne!(
        borrowed.inputs.get(&text.as_str()).unwrap() as *const _,
        before
    );
    let mut unbound = GroupNode::<usize, Key>::new();
    unbound.bind_primitive_group_keys();
    unbound.bind_primitive_inputs();
    unbound.enable_equal_input_reuse();
    unbound.apply(TupleChange::put(0, (KEY, 1)));
    let ptr = pointer(&unbound);
    let outer = unbound.begin_journal_checkpoint();
    unbound.apply(TupleChange::put(0, (KEY, 1)));
    assert_ne!(
        pointer(&unbound),
        ptr,
        "Tree stays original even after graph opt-in"
    );
    unbound.restore_checkpoint(outer);
    let GroupInputs::Tree(rows) = &unbound.inputs else {
        unreachable!()
    };
    let held = rows.materialized().clone();
    let before = pointer(&unbound);
    unbound.apply(TupleChange::put(0, (KEY, 1)));
    assert_ne!(pointer(&unbound), before);
    assert_eq!(&held[&0] as *const _, before);
}
#[test]
fn equal_group_input_actual_graph_keeps_mapping_and_score_callbacks_for_duplicate_payloads() {
    let calls = RefCell::new(Vec::new());
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, usize)>();
    let grouped = b
        .group_collect(source.port(), |row| {
            calls.borrow_mut().push(("map", row.1));
            (row.0, 1)
        })
        .unwrap();
    let c = b
        .score("count", &grouped, |(_, s)| {
            calls.borrow_mut().push(("score", s.count));
            assert!(!fail.get(), "score callback panic");
            SimpleScore::new(s.sum)
        })
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..2 {
        tx.apply(&source, TupleChange::put(id, (0, id))).unwrap();
    }
    tx.commit().unwrap();
    g.bind_fixed_planning_source(&source, 2).unwrap();
    calls.borrow_mut().clear();
    for value in 10..14 {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, (0, value))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(2));
        assert_eq!(tx.score_evaluations(), 1);
        tx.commit().unwrap();
    }
    assert_eq!(
        *calls.borrow(),
        vec![
            ("map", 10),
            ("score", 2),
            ("map", 11),
            ("score", 2),
            ("map", 12),
            ("score", 2),
            ("map", 13),
            ("score", 2)
        ]
    );
    assert_eq!(g.constraint_matches(&c).unwrap()[&0].0.1.count, 2);
    calls.borrow_mut().clear();
    fail.set(true);
    {
        let mut tx = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                tx.apply(&source, TupleChange::put(0, (0, 99)))
            }))
            .is_err()
        );
        assert!(tx.score().is_err());
    }
    fail.set(false);
    assert_eq!(*calls.borrow(), [("map", 99), ("score", 2)]);
    assert_eq!(g.score(), SimpleScore::new(2));
    assert_eq!(g.constraint_matches(&c).unwrap()[&0].0.1.count, 2);
    g.assert_consistent().unwrap();
}
