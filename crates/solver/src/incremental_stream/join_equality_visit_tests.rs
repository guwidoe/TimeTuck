//! Fixed equality traversal controls; no upstream-method credit.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

fn fixed() -> join_fixed_index::FixedIndex<usize, (usize, usize)> {
    let keys: Vec<_> = (0usize..131).map(Arc::new).collect();
    let mut groups = BTreeMap::new();
    for id in 0usize..131 {
        groups
            .entry((id % 3, usize::MAX))
            .or_insert_with(SnapshotMap::new)
            .insert(id, ());
    }
    let buckets = groups
        .into_iter()
        .map(|(key, ids)| (Arc::new(key), Arc::new(ids)))
        .collect();
    let mut index = join_fixed_index::FixedIndex::new(keys, buckets, |id| *id);
    index.enable_delta_capture();
    index
}
fn compare(index: &join_fixed_index::FixedIndex<usize, (usize, usize)>, key: (usize, usize)) {
    let (count, old) = index.matching(&key, JoinComparison::Equal);
    let expected: Vec<_> = old.map(|id| (*id, id as *const usize)).collect();
    let mut actual = vec![(usize::MAX, std::ptr::null())];
    index.for_each_equal(&key, &mut actual, |output, id| {
        assert!(
            output.capacity() > count,
            "reserve must precede every callback"
        );
        output.push((*id, id as *const usize));
    });
    assert_eq!(&actual[1..], expected.as_slice());
    assert_eq!(count, expected.len());
}
#[test]
fn equality_visit_reserves_first_and_uses_identical_canonical_refs_across_words_missing_keys_and_saved_indexes()
 {
    let mut index = fixed();
    assert!(index.supports_equality_visit());
    for key in [0, 1, 2, 3, usize::MAX] {
        compare(&index, (key, usize::MAX));
    }
    let full = index.clone();
    let outer = index.begin();
    for id in [0usize, 1, 63, 64, 65, 127, 128, 130] {
        index.remove(&(id % 3, usize::MAX), &id);
        index.insert(&(3, usize::MAX), &id);
    }
    let child = index.begin();
    index.remove(&(3, usize::MAX), &63);
    index.insert(&(4, usize::MAX), &63);
    index.commit(child);
    let saved = index.capture_current(outer);
    for key in 0..5 {
        compare(&index, (key, usize::MAX));
    }
    index.rollback(outer);
    for key in 0..5 {
        compare(&index, (key, usize::MAX));
    }
    let install = index.begin();
    index.install_capture(saved);
    for key in 0..5 {
        compare(&index, (key, usize::MAX));
    }
    index.rollback(install);
    index.install(full);
    for key in 0..5 {
        compare(&index, (key, usize::MAX));
    }
}

thread_local! {
    static TRACE:RefCell<Vec<(&'static str,usize,i64)>>=const{RefCell::new(Vec::new())};
    static FAIL_CLONE:Cell<usize>=const{Cell::new(0)};
    static CLONES:Cell<usize>=const{Cell::new(0)};
}
#[derive(Debug)]
struct Row {
    key: usize,
    value: i64,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|t| t.borrow_mut().push(("clone", self.key, self.value)));
        let count = CLONES.with(|n| {
            n.set(n.get() + 1);
            n.get()
        });
        assert_ne!(count, FAIL_CLONE.with(Cell::get), "payload clone failure");
        Self {
            key: self.key,
            value: self.value,
        }
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        TRACE.with(|t| t.borrow_mut().push(("drop", self.key, self.value)));
    }
}
type Node = JoinNode<'static, usize, usize, Row, Row, (usize, usize)>;
type Changes = Vec<((usize, usize), Option<(i64, i64)>)>;
fn normalize(changes: Vec<TupleChange<(usize, usize), (Row, Row)>>) -> Changes {
    changes
        .iter()
        .map(|c| (c.id, c.value.as_ref().map(|(l, r)| (l.value, r.value))))
        .collect()
}
fn node(legacy: bool) -> Node {
    let mut node = JoinNode::new(
        |r: &Row| {
            TRACE.with(|t| t.borrow_mut().push(("left", r.key, r.value)));
            (r.key, usize::MAX)
        },
        |r: &Row| {
            TRACE.with(|t| t.borrow_mut().push(("right", r.key, r.value)));
            (r.key, usize::MAX)
        },
    );
    for id in 0usize..131 {
        node.apply_left(TupleChange::put(
            id,
            Row {
                key: id % 3,
                value: id as i64,
            },
        ));
        node.apply_right(TupleChange::put(
            id,
            Row {
                key: id % 3,
                value: -(id as i64),
            },
        ));
    }
    assert!(node.bind_fixed_input(false, 131));
    assert!(node.bind_fixed_input(true, 131));
    if legacy {
        node.left
            .fixed_index
            .as_mut()
            .unwrap()
            .disable_equality_visit_for_tests();
        node.right
            .fixed_index
            .as_mut()
            .unwrap()
            .disable_equality_visit_for_tests();
    }
    assert_eq!(
        node.left
            .fixed_index
            .as_ref()
            .unwrap()
            .supports_equality_visit(),
        !legacy
    );
    assert_eq!(
        node.right
            .fixed_index
            .as_ref()
            .unwrap()
            .supports_equality_visit(),
        !legacy
    );
    node
}
#[test]
fn equality_visit_clone_drop_projection_retract_put_order_and_nested_full_restore_match_original_iterator()
 {
    fn run(legacy: bool) -> (Vec<(&'static str, usize, i64)>, Vec<Changes>) {
        FAIL_CLONE.with(|n| n.set(0));
        CLONES.with(|n| n.set(0));
        let mut node = node(legacy);
        TRACE.with(|t| t.borrow_mut().clear());
        let full = node.checkpoint();
        let outer = node.begin_bound_checkpoint().unwrap();
        let mut changes = Vec::new();
        changes.push(normalize(
            node.apply_left(TupleChange::put(0usize, Row { key: 0, value: 900 })),
        ));
        changes.push(normalize(
            node.apply_left(TupleChange::put(0usize, Row { key: 2, value: 901 })),
        ));
        changes.push(normalize(
            node.apply_right(TupleChange::put(64usize, Row { key: 2, value: 902 })),
        ));
        let inner = node.begin_bound_checkpoint().unwrap();
        changes.push(normalize(node.apply_right(TupleChange::retract(127usize))));
        changes.push(normalize(node.apply_left(TupleChange::retract(63usize))));
        changes.push(normalize(
            node.apply_left(TupleChange::put(63usize, Row { key: 9, value: 903 })),
        ));
        node.commit_bound_checkpoint(inner);
        let saved = node.capture_checkpoint(outer.as_ref());
        node.restore_bound_checkpoint(outer);
        let install = node.begin_bound_checkpoint().unwrap();
        node.restore_bound_checkpoint(saved);
        node.restore_bound_checkpoint(install);
        node.restore(full);
        drop(node);
        (TRACE.with(|t| t.borrow().clone()), changes)
    }
    assert_eq!(run(false), run(true));
}
#[test]
fn equality_visit_output_clone_panics_keep_staging_poison_bounds_and_rollback_phases() {
    fn run(legacy: bool, after: usize) -> Vec<(&'static str, usize, i64)> {
        FAIL_CLONE.with(|n| n.set(0));
        CLONES.with(|n| n.set(0));
        let mut node = node(legacy);
        TRACE.with(|t| t.borrow_mut().clear());
        let checkpoint = node.begin_bound_checkpoint().unwrap();
        FAIL_CLONE.with(|n| n.set(CLONES.with(Cell::get) + after));
        assert!(
            catch_unwind(AssertUnwindSafe(
                || node.apply_left(TupleChange::put(0usize, Row { key: 2, value: 900 }))
            ))
            .is_err()
        );
        FAIL_CLONE.with(|n| n.set(0));
        assert!(!node.is_poisoned(), "outputs still stage before poisoning");
        node.restore_bound_checkpoint(checkpoint);
        let checkpoint = node.begin_bound_checkpoint().unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || node.apply_right(TupleChange::put(131usize, Row { key: 2, value: 901 }))
            ))
            .is_err()
        );
        assert!(node.is_poisoned());
        node.restore_bound_checkpoint(checkpoint);
        assert!(!node.is_poisoned());
        drop(node);
        TRACE.with(|t| t.borrow().clone())
    }
    for after in [1usize, 2, 3, 40] {
        assert_eq!(run(false, after), run(true, after));
    }
}

#[test]
fn equality_visit_custom_key_falls_back_and_all_ordered_relations_preserve_matching_order() {
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Custom(usize);
    let mut custom = JoinNode::new(|r: &usize| Custom(*r % 3), |r: &usize| Custom(*r % 3));
    for id in 0usize..8 {
        custom.apply_left(TupleChange::put(id, id));
        custom.apply_right(TupleChange::put(id, id));
    }
    assert!(custom.bind_fixed_input(false, 8));
    assert!(custom.bind_fixed_input(true, 8));
    assert!(
        !custom
            .left
            .fixed_index
            .as_ref()
            .unwrap()
            .supports_equality_visit()
    );
    assert!(
        !custom
            .right
            .fixed_index
            .as_ref()
            .unwrap()
            .supports_equality_visit()
    );
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let mut fast =
            JoinNode::comparing(comparison, |r: &(usize, i64)| r.0, |r: &(usize, i64)| r.0);
        let mut old =
            JoinNode::comparing(comparison, |r: &(usize, i64)| r.0, |r: &(usize, i64)| r.0);
        for id in 0usize..8 {
            for node in [&mut fast, &mut old] {
                node.apply_left(TupleChange::put(id, (id % 3, id as i64)));
                node.apply_right(TupleChange::put(id, (id % 3, -(id as i64))));
            }
        }
        for node in [&mut fast, &mut old] {
            assert!(node.bind_fixed_input(false, 8));
            assert!(node.bind_fixed_input(true, 8));
        }
        old.left
            .fixed_index
            .as_mut()
            .unwrap()
            .disable_equality_visit_for_tests();
        old.right
            .fixed_index
            .as_mut()
            .unwrap()
            .disable_equality_visit_for_tests();
        for (id, key) in [(0usize, 2usize), (1, 4), (2, 0)] {
            assert_eq!(
                fast.apply_left(TupleChange::put(id, (key, 99i64))),
                old.apply_left(TupleChange::put(id, (key, 99i64)))
            );
            assert_eq!(
                fast.apply_right(TupleChange::retract(id)),
                old.apply_right(TupleChange::retract(id))
            );
        }
    }
}

fn tree_node(legacy: bool) -> Node {
    let mut node = JoinNode::new(
        |r: &Row| {
            TRACE.with(|t| t.borrow_mut().push(("left", r.key, r.value)));
            (r.key, usize::MAX)
        },
        |r: &Row| {
            TRACE.with(|t| t.borrow_mut().push(("right", r.key, r.value)));
            (r.key, usize::MAX)
        },
    );
    for id in 0usize..131 {
        node.apply_left(TupleChange::put(
            id,
            Row {
                key: id % 3,
                value: id as i64,
            },
        ));
        node.apply_right(TupleChange::put(
            id,
            Row {
                key: id % 3,
                value: -(id as i64),
            },
        ));
    }
    node.left.disable_tree_visit = legacy;
    node.right.disable_tree_visit = legacy;
    assert!(node.left.fixed_index.is_none() && node.right.fixed_index.is_none());
    assert_eq!(node.left.uses_tree_equality_visit(), !legacy);
    node
}

fn tree_history_scenario(legacy: bool) -> (Vec<(&'static str, usize, i64)>, Vec<Changes>) {
    FAIL_CLONE.with(|n| n.set(0));
    CLONES.with(|n| n.set(0));
    let mut node = tree_node(legacy);
    TRACE.with(|t| t.borrow_mut().clear());
    let full = node.checkpoint();
    let outer = node.begin_journal_checkpoint();
    let mut changes = Vec::new();
    changes.push(normalize(
        node.apply_left(TupleChange::put(0usize, Row { key: 0, value: 900 })),
    ));
    changes.push(normalize(
        node.apply_left(TupleChange::put(0usize, Row { key: 2, value: 901 })),
    ));
    changes.push(normalize(
        node.apply_right(TupleChange::put(64usize, Row { key: 2, value: 902 })),
    ));
    let inner = node.begin_journal_checkpoint();
    changes.push(normalize(node.apply_right(TupleChange::retract(127usize))));
    changes.push(normalize(node.apply_left(TupleChange::retract(63usize))));
    changes.push(normalize(
        node.apply_left(TupleChange::put(63usize, Row { key: 9, value: 903 })),
    ));
    node.commit_journal_checkpoint(inner);
    let saved = node.capture_journal_checkpoint(outer.as_ref()).unwrap();
    node.restore_checkpoint(outer);
    let install = node.begin_journal_checkpoint();
    node.restore_checkpoint(saved);
    node.restore_checkpoint(install);
    node.restore(full);
    drop(node);
    (TRACE.with(|t| t.borrow().clone()), changes)
}
#[test]
fn tree_equality_visit_keeps_clone_drop_projection_order_nested_captures_and_full_restore() {
    assert_eq!(tree_history_scenario(false), tree_history_scenario(true));
}
#[test]
fn tree_equality_visit_clone_failures_leave_staging_healthy_and_preserve_rollback() {
    fn run(legacy: bool, after: usize) -> Vec<(&'static str, usize, i64)> {
        FAIL_CLONE.with(|n| n.set(0));
        CLONES.with(|n| n.set(0));
        let mut node = tree_node(legacy);
        TRACE.with(|t| t.borrow_mut().clear());
        let saved = node.begin_journal_checkpoint();
        FAIL_CLONE.with(|n| n.set(CLONES.with(Cell::get) + after));
        assert!(
            catch_unwind(AssertUnwindSafe(
                || node.apply_left(TupleChange::put(0, Row { key: 2, value: 900 }))
            ))
            .is_err()
        );
        FAIL_CLONE.with(|n| n.set(0));
        assert!(!node.is_poisoned());
        node.restore_checkpoint(saved);
        drop(node);
        TRACE.with(|t| t.borrow().clone())
    }
    for after in [1, 2, 3, 40] {
        assert_eq!(run(false, after), run(true, after));
    }
}
#[test]
fn tree_equality_visit_preserves_full_pair_ids_canonical_refs_and_reservation_before_delivery() {
    let mut side = JoinSide::<(usize, usize), i64, (usize, usize)>::new();
    for (id, key) in [
        ((0, 0), (0, usize::MAX)),
        ((0, usize::MAX), (0, usize::MAX)),
        ((usize::MAX, 0), (1, usize::MAX)),
        ((usize::MAX, usize::MAX), (0, usize::MAX)),
        ((63, 64), (1, usize::MAX)),
    ] {
        side.install_projected(id, Some((key, 7)), false);
    }
    let saved = side.clone();
    for side in [&side, &saved] {
        for key in [(0, usize::MAX), (1, usize::MAX), (2, usize::MAX)] {
            let (count, ids) = side.matching_ids_sized(&key, JoinComparison::Equal);
            let expected: Vec<_> = ids.map(|id| (*id, id as *const (usize, usize))).collect();
            let mut actual = vec![((7, 7), std::ptr::null())];
            side.for_each_tree_equal(&key, &mut actual, |out, id| {
                assert!(out.capacity() > count);
                out.push((*id, id as *const (usize, usize)));
            });
            assert_eq!(&actual[1..], expected.as_slice());
        }
    }
}
