//! Original prepared-group controls; no upstream-method credit.
use super::*;
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};

fn contents<I: Ord, K: Ord + Clone>(node: &GroupNode<I, K>) -> Vec<(K, IntSummary)> {
    node.groups()
        .map(|(key, summary)| (key.clone(), summary))
        .collect()
}

#[test]
fn prepared_entries_keep_singleton_reinsertion_and_equal_multiset_owners() {
    let mut node = GroupNode::<usize, usize>::new();
    assert!(node.bind_primitive_group_keys());
    node.apply(TupleChange::put(0, (usize::MAX, 7)));
    let singleton = node.groups.shared_entries();
    assert_eq!(node.apply(TupleChange::put(0, (usize::MAX, 7))).len(), 1);
    assert!(
        !std::ptr::eq(singleton[0].0.as_ref(), node.groups().next().unwrap().0),
        "singleton replacement still removes and reinserts its representative"
    );
    node.apply(TupleChange::put(1, (usize::MAX, i64::MAX)));
    let multiple = node.groups.shared_entries();
    let output = node.apply(TupleChange::put(0, (usize::MAX, 7)));
    assert_eq!(output.len(), 1, "an equal replacement still emits");
    let equal = node.groups.shared_entries();
    assert!(std::sync::Arc::ptr_eq(&multiple[0].0, &equal[0].0));
    assert!(
        std::sync::Arc::ptr_eq(&multiple[0].1, &equal[0].1),
        "equal multiset retains the collector owner"
    );
    node.apply(TupleChange::put(0, (usize::MAX, i64::MIN)));
    let changed = node.groups.shared_entries();
    assert!(std::sync::Arc::ptr_eq(&multiple[0].0, &changed[0].0));
    assert_eq!(
        changed[0].1.summary(),
        [i64::MIN, i64::MAX]
            .into_iter()
            .collect::<IntCollector>()
            .summary()
    );
    assert_eq!(
        singleton[0].1.summary().sum,
        7,
        "full owners remain immutable"
    );
    assert_eq!(multiple[0].1.summary().min, Some(7));
}

#[test]
fn prepared_group_emissions_match_tree_across_same_page_transfers_and_saved_trials() {
    type Key = (usize, usize, usize, usize);
    let keys: [Key; 4] = [
        (0, 0, 0, 15),
        (0, 0, 0, 14),
        (0, 0, 0, 16),
        (usize::MAX, usize::MAX, usize::MAX, usize::MAX),
    ];
    for fixed in [false, true] {
        let mut node = GroupNode::<usize, Key>::new();
        let mut tree = GroupNode::<usize, Key>::new();
        assert!(node.bind_primitive_group_keys());
        if fixed {
            assert!(node.bind_fixed_inputs(4));
            assert!(tree.bind_fixed_inputs(4));
        } else {
            assert!(node.bind_primitive_inputs());
            assert!(tree.bind_primitive_inputs());
        }
        for (id, key, value) in [
            (0, keys[0], i64::MAX),
            (1, keys[0], 7),
            (2, keys[1], -7),
            (3, keys[2], i64::MIN),
        ] {
            assert_eq!(
                node.apply_inline(TupleChange::put(id, (key, value))),
                tree.apply_inline(TupleChange::put(id, (key, value)))
            );
        }
        let base = contents(&node);
        let outer = node.begin_journal_checkpoint();
        let old_outer = tree.begin_journal_checkpoint();
        for change in [
            TupleChange::put(0, (keys[1], 7)),
            TupleChange::put(1, (keys[1], -7)),
            TupleChange::put(2, (keys[1], -7)),
            TupleChange::retract(3),
            TupleChange::put(3, (keys[3], i64::MIN)),
            TupleChange::put(3, (keys[3], i64::MAX)),
        ] {
            assert_eq!(node.apply_inline(change.clone()), tree.apply_inline(change));
            assert_eq!(contents(&node), contents(&tree));
        }
        let full = node.checkpoint();
        let old_full = tree.checkpoint();
        let inner = node.begin_journal_checkpoint();
        let old_inner = tree.begin_journal_checkpoint();
        assert_eq!(
            node.apply_inline(TupleChange::put(2, (keys[3], 7))),
            tree.apply_inline(TupleChange::put(2, (keys[3], 7)))
        );
        assert_eq!(
            node.apply_inline(TupleChange::retract(0)),
            tree.apply_inline(TupleChange::retract(0))
        );
        let wanted = contents(&node);
        let saved = node.capture_checkpoint(outer.as_ref());
        let old_saved = tree.capture_checkpoint(old_outer.as_ref());
        node.commit_journal_checkpoint(inner);
        tree.commit_journal_checkpoint(old_inner);
        node.restore_checkpoint(full);
        tree.restore_checkpoint(old_full);
        assert_eq!(contents(&node), contents(&tree));
        node.restore_checkpoint(outer);
        tree.restore_checkpoint(old_outer);
        assert_eq!(contents(&node), base);
        let other = node.begin_journal_checkpoint();
        let old_other = tree.begin_journal_checkpoint();
        node.apply(TupleChange::put(0, (keys[2], 9)));
        tree.apply(TupleChange::put(0, (keys[2], 9)));
        node.restore_checkpoint(other);
        tree.restore_checkpoint(old_other);
        let install = node.begin_journal_checkpoint();
        let old_install = tree.begin_journal_checkpoint();
        node.restore_checkpoint(saved);
        tree.restore_checkpoint(old_saved);
        assert_eq!(contents(&node), wanted);
        assert_eq!(contents(&node), contents(&tree));
        node.restore_checkpoint(install);
        tree.restore_checkpoint(old_install);
        assert_eq!(contents(&node), base);
        assert_eq!(contents(&node), contents(&tree));
    }
}

thread_local! {
    static INPUT_EVENTS: RefCell<Vec<(&'static str, usize, usize)>> = const { RefCell::new(Vec::new()) };
    static FAIL_COMPARE: Cell<bool> = const { Cell::new(false) };
    static FAIL_DROP: Cell<bool> = const { Cell::new(false) };
}
#[derive(Debug)]
struct InputId(usize, usize);
impl Clone for InputId {
    fn clone(&self) -> Self {
        INPUT_EVENTS.with(|events| events.borrow_mut().push(("clone", self.0, self.1)));
        Self(self.0, self.1)
    }
}
impl Drop for InputId {
    fn drop(&mut self) {
        INPUT_EVENTS.with(|events| events.borrow_mut().push(("drop", self.0, self.1)));
        assert!(
            !FAIL_DROP.with(|fail| fail.replace(false)),
            "input representative drop failure"
        );
    }
}
impl PartialEq for InputId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for InputId {}
impl PartialOrd for InputId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for InputId {
    fn cmp(&self, other: &Self) -> Ordering {
        INPUT_EVENTS.with(|events| events.borrow_mut().push(("compare", self.0, other.0)));
        assert!(
            !FAIL_COMPARE.with(|fail| fail.replace(false)),
            "input lookup failure"
        );
        self.0.cmp(&other.0)
    }
}
type Observed = (
    Result<[Option<TupleChange<usize, IntSummary>>; 2], ()>,
    Vec<(&'static str, usize, usize)>,
);
fn attempt(
    node: &mut GroupNode<InputId, usize>,
    id: usize,
    tag: usize,
    value: Option<(usize, i64)>,
    compare_failure: bool,
    drop_failure: bool,
) -> Observed {
    INPUT_EVENTS.with(|events| events.borrow_mut().clear());
    FAIL_COMPARE.with(|fail| fail.set(compare_failure));
    FAIL_DROP.with(|fail| fail.set(drop_failure));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        node.apply_inline(TupleChange {
            id: InputId(id, tag),
            value,
        })
    }))
    .map_err(|_| ());
    FAIL_COMPARE.with(|fail| fail.set(false));
    FAIL_DROP.with(|fail| fail.set(false));
    (result, INPUT_EVENTS.with(|events| events.borrow().clone()))
}
#[test]
fn primitive_group_entries_keep_custom_input_hooks_poison_and_restore_boundaries() {
    let mut node = GroupNode::<InputId, usize>::new();
    let mut tree = GroupNode::<InputId, usize>::new();
    assert!(node.bind_primitive_group_keys());
    for (id, tag, value) in [
        (0, 0, Some((0, 5))),
        (1, 1, Some((0, 7))),
        (0, 2, Some((1, 8))),
        (0, 3, Some((1, 8))),
        (0, 4, Some((0, -2))),
    ] {
        assert_eq!(
            attempt(&mut node, id, tag, value, false, false),
            attempt(&mut tree, id, tag, value, false, false)
        );
        assert_eq!(contents(&node), contents(&tree));
    }
    let before = contents(&node);
    let got = attempt(&mut node, 0, 5, Some((2, 9)), true, false);
    assert_eq!(got, attempt(&mut tree, 0, 5, Some((2, 9)), true, false));
    assert!(got.0.is_err());
    assert!(
        !node.is_poisoned(),
        "input lookup preflight precedes poison and all group changes"
    );
    assert_eq!(contents(&node), before);
    let saved = node.checkpoint();
    let old_saved = tree.checkpoint();
    let got = attempt(&mut node, 0, 6, Some((2, 9)), false, true);
    assert_eq!(got, attempt(&mut tree, 0, 6, Some((2, 9)), false, true));
    assert!(got.0.is_err());
    assert!(
        node.is_poisoned(),
        "input publication still follows group mutation"
    );
    assert!(tree.is_poisoned());
    node.restore_checkpoint(saved);
    tree.restore_checkpoint(old_saved);
    assert_eq!(contents(&node), before);
    assert_eq!(contents(&node), contents(&tree));
    assert!(!node.is_poisoned());
}

#[test]
fn group_static_mapper_matches_stored_nested_captures_and_singleton_canonical_owners() {
    use crate::primitive_rows::{Exact, LookupPolicy, Stored};
    use std::sync::Arc;
    type Key = (usize, usize);
    type Trace = Vec<(Vec<TupleChange<Key, IntSummary>>, Vec<(Key, IntSummary)>)>;
    fn step<P: LookupPolicy<Key>>(
        node: &mut GroupNode<usize, Key>,
        change: TupleChange<usize, (Key, i64)>,
        trace: &mut Trace,
    ) {
        let output = node.apply_inline_with_policy::<P, _>(change, |old, new| [old, new]);
        trace.push((output.into_iter().flatten().collect(), contents(node)));
    }
    fn run<P: LookupPolicy<Key>>() -> Trace {
        let mut node = GroupNode::<usize, Key>::new();
        assert!(node.bind_primitive_group_keys());
        let low = (0, 15);
        let high = (usize::MAX, usize::MAX);
        let mut trace = Vec::new();
        for (id, key, value) in [(0, low, 1), (1, low, 2), (usize::MAX, high, 7)] {
            step::<P>(&mut node, TupleChange::put(id, (key, value)), &mut trace);
        }
        let base = contents(&node);
        let owners = node.groups.shared_entries();
        let old = owners.iter().find(|(key, _)| **key == high).unwrap();
        let old_key = Arc::downgrade(&old.0);
        let old_value = Arc::downgrade(&old.1);
        let full = node.checkpoint();
        let outer = node.begin_journal_checkpoint();
        step::<P>(&mut node, TupleChange::put(0, (low, 4)), &mut trace);
        step::<P>(&mut node, TupleChange::put(0, (low, 5)), &mut trace);
        step::<P>(
            &mut node,
            TupleChange::put(usize::MAX, (high, i64::MAX)),
            &mut trace,
        );
        let current = node.groups.shared_entries();
        let replacement = current.iter().find(|(key, _)| **key == high).unwrap();
        assert!(
            !Arc::ptr_eq(&old.0, &replacement.0),
            "singleton physically reborn"
        );
        assert!(old_key.upgrade().is_some());
        assert_eq!(old_value.upgrade().unwrap().summary().sum, 7);
        let new_key = Arc::downgrade(&replacement.0);
        drop(current);
        let child = node.begin_journal_checkpoint();
        step::<P>(&mut node, TupleChange::put(1, (high, i64::MIN)), &mut trace);
        step::<P>(&mut node, TupleChange::retract(0), &mut trace);
        let wanted = contents(&node);
        let capture = node.capture_checkpoint(outer.as_ref());
        node.commit_journal_checkpoint(child);
        node.restore_checkpoint(outer);
        assert_eq!(contents(&node), base);
        assert!(std::ptr::eq(
            node.groups().find(|(key, _)| **key == high).unwrap().0,
            old.0.as_ref(),
        ));
        assert!(new_key.upgrade().is_some(), "capture retains candidate key");
        let alternate = node.begin_journal_checkpoint();
        step::<P>(
            &mut node,
            TupleChange::put(0, ((4096, 1024), -7)),
            &mut trace,
        );
        node.restore_checkpoint(alternate);
        let install = node.begin_journal_checkpoint();
        node.restore_checkpoint(capture);
        assert_eq!(contents(&node), wanted);
        node.restore_checkpoint(install);
        assert_eq!(contents(&node), base);
        assert!(
            new_key.upgrade().is_none(),
            "retired Weak-observed key is not pooled"
        );
        node.restore_checkpoint(full);
        assert_eq!(contents(&node), base);
        trace
    }
    assert_eq!(run::<Exact>(), run::<Stored>());
}

#[test]
fn group_static_mapper_keeps_actual_borrowed_public_ids_and_keys() {
    // Neither borrow can satisfy 'static. This compiles only if public GroupNode
    // entrypoints keep the shared Stored policy without adding static bounds.
    fn run<'a>(ids: &'a [String], keys: &'a [String]) {
        let mut node = GroupNode::<&'a str, &'a str>::new();
        node.apply(TupleChange::put(ids[0].as_str(), (keys[0].as_str(), 3)));
        node.apply(TupleChange::put(ids[1].as_str(), (keys[0].as_str(), 5)));
        assert_eq!(node.groups().next().unwrap().1.sum, 8);
        node.apply(TupleChange::put(ids[0].as_str(), (keys[1].as_str(), -7)));
        node.apply(TupleChange::retract(ids[1].as_str()));
        assert_eq!(
            contents(&node),
            vec![(
                keys[1].as_str(),
                [-7].into_iter().collect::<IntCollector>().summary()
            )]
        );
    }
    let ids = [String::from("left"), String::from("right")];
    let keys = [String::from("first"), String::from("second")];
    run(&ids, &keys);
}
