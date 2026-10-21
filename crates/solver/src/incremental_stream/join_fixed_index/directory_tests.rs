use super::*;
use crate::incremental_stream::{JoinNode, TupleChange};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Key = (usize, usize);
type Rows = Vec<(Key, Vec<usize>)>;
fn contents(index: &FixedIndex<usize, Key>) -> Rows {
    index
        .buckets
        .shared_entries()
        .into_iter()
        .map(|(key, members)| (*key, members.ids().collect()))
        .collect()
}
fn numeric_index() -> FixedIndex<usize, Key> {
    let mut groups: BTreeMap<Key, SnapshotMap<usize, ()>> = BTreeMap::new();
    for id in 0..129 {
        let key = if id == 128 {
            (usize::MAX, usize::MAX)
        } else {
            (0, id % 3)
        };
        groups.entry(key).or_default().insert(id, ());
    }
    let mut index = FixedIndex::new(
        (0..129usize).map(Arc::new).collect(),
        groups
            .into_iter()
            .map(|(key, ids)| (Arc::new(key), Arc::new(ids)))
            .collect(),
        |id| *id,
    );
    index.enable_delta_capture();
    index
}

#[test]
fn directory_buckets_keep_full_current_nested_undo_and_canonical_rebirth_owners() {
    fn run(paged: bool) -> Vec<Rows> {
        let mut index = numeric_index();
        let high = (usize::MAX, usize::MAX);
        let original = index.buckets.shared_entries();
        let old = original.iter().find(|(key, _)| **key == high).unwrap();
        let key_weak = Arc::downgrade(&old.0);
        let value_weak = Arc::downgrade(&old.1);
        let words = old.1.words.as_ptr();
        // A full tree save may precede graph activation. Conversion must transfer
        // the exact Arcs, rather than rebuilding values or key representatives.
        let legacy_full = index.clone();
        if paged {
            assert!(index.enable_directory_buckets());
        }
        let owners = index.buckets.shared_entries();
        let current = owners.iter().find(|(key, _)| **key == high).unwrap();
        assert!(Arc::ptr_eq(&old.0, &current.0));
        assert!(Arc::ptr_eq(&old.1, &current.1));
        assert_eq!(current.1.words.as_ptr(), words);
        drop(owners);
        let base = contents(&index);
        let mut trace = vec![base.clone()];
        let outer = index.begin();
        index.remove(&high, &128);
        index.insert(&high, &128);
        let owners = index.buckets.shared_entries();
        let reborn = owners.iter().find(|(key, _)| **key == high).unwrap();
        assert!(!Arc::ptr_eq(&old.0, &reborn.0));
        assert!(!Arc::ptr_eq(&old.1, &reborn.1));
        let reborn_key = Arc::downgrade(&reborn.0);
        let reborn_value = Arc::downgrade(&reborn.1);
        drop(owners);
        let child = index.begin();
        for id in [0usize, 63, 64] {
            let key = (0, id % 3);
            index.remove(&key, &id);
            index.insert(&(4096, 1024), &id);
        }
        index.commit(child);
        let discarded = index.begin();
        index.remove(&(0, 1), &1);
        index.insert(&(usize::MAX, 0), &1);
        index.rollback(discarded);
        let wanted = contents(&index);
        trace.push(wanted.clone());
        let input_owners: Vec<_> = index.keys.iter().map(Arc::strong_count).collect();
        let capture = index.capture_current(outer);
        assert_eq!(
            index.keys.iter().map(Arc::strong_count).collect::<Vec<_>>(),
            input_owners
        );
        let late_full = index.clone();
        index.rollback(outer);
        assert_eq!(contents(&index), base);
        assert!(Arc::ptr_eq(
            &key_weak.upgrade().unwrap(),
            &index
                .buckets
                .shared_entries()
                .into_iter()
                .find(|(key, _)| **key == high)
                .unwrap()
                .0,
        ));
        assert_eq!(value_weak.upgrade().unwrap().words.as_ptr(), words);
        let alternate = index.begin();
        index.clear();
        index.insert(&(0, 15), &128);
        index.rollback(alternate);
        let install = index.begin();
        index.install_capture(capture);
        assert_eq!(contents(&index), wanted);
        let owners = index.buckets.shared_entries();
        let installed = owners.iter().find(|(key, _)| **key == high).unwrap();
        assert!(Arc::ptr_eq(&reborn_key.upgrade().unwrap(), &installed.0));
        assert!(Arc::ptr_eq(&reborn_value.upgrade().unwrap(), &installed.1));
        drop(owners);
        index.rollback(install);
        assert_eq!(contents(&index), base);
        let install = index.begin();
        index.install(late_full);
        assert_eq!(contents(&index), wanted);
        index.install(legacy_full);
        assert_eq!(contents(&index), base);
        assert_eq!(index.buckets.is_paged(), paged);
        index.rollback(install);
        assert_eq!(contents(&index), base);
        assert!(reborn_key.upgrade().is_none());
        assert!(reborn_value.upgrade().is_none());
        trace.push(contents(&index));
        drop(original);
        trace
    }
    assert_eq!(run(true), run(false));

    let mut empty = FixedIndex::<usize, Key>::new(Vec::new(), Vec::new(), |id| *id);
    empty.enable_delta_capture();
    assert!(empty.enable_directory_buckets());
    let outer = empty.begin();
    let capture = empty.capture_current(outer);
    empty.clear();
    empty.rollback(outer);
    empty.install_capture(capture);
    assert!(contents(&empty).is_empty());
}

type Row = (Key, i64);
type Node<'a> = JoinNode<'a, usize, usize, Row, Row, Key>;
type State = (Vec<(usize, (Key, Row))>, Vec<(usize, (Key, Row))>);
fn state(node: &Node<'_>) -> State {
    (
        node.left.row_iter().map(|(id, row)| (*id, *row)).collect(),
        node.right.row_iter().map(|(id, row)| (*id, *row)).collect(),
    )
}
fn force_tree(index: &mut FixedIndex<usize, Key>) {
    if let Buckets::Paged(rows) = &index.buckets {
        index.buckets = Buckets::Tree(rows.materialized().clone());
    }
}
fn node<'a>(
    trace: &'a RefCell<Vec<(bool, Row)>>,
    bindings: usize,
    late: bool,
    paged: bool,
) -> Node<'a> {
    let mut node = JoinNode::new(
        move |row: &Row| {
            trace.borrow_mut().push((false, *row));
            assert_ne!(row.1, -90001, "projection panic");
            row.0
        },
        move |row: &Row| {
            trace.borrow_mut().push((true, *row));
            assert_ne!(row.1, -90001, "projection panic");
            row.0
        },
    );
    if !late {
        assert!(node.bind_primitive_graph_journals());
    }
    for id in 0..129 {
        let key = if id == 128 {
            (usize::MAX, usize::MAX)
        } else {
            (0, id % 3)
        };
        node.apply_left(TupleChange::put(id, (key, id as i64)));
        node.apply_right(TupleChange::put(id, (key, -(id as i64))));
    }
    for right in [false, true] {
        let mask = if right { 2 } else { 1 };
        if bindings & mask != 0 {
            assert!(node.bind_fixed_input(right, 129));
        }
    }
    if late {
        assert!(node.bind_primitive_graph_journals());
    }
    for side in [&mut node.left, &mut node.right] {
        if let Some(index) = &mut side.fixed_index {
            assert!(index.buckets.is_paged());
            if !paged {
                force_tree(index);
            }
        }
    }
    node
}

#[test]
fn directory_buckets_join_both_binding_handoffs_preserve_outputs_projection_panics_and_restores() {
    for bindings in [1, 2, 3] {
        for late in [false, true] {
            let fast_trace = RefCell::new(Vec::new());
            let tree_trace = RefCell::new(Vec::new());
            let mut fast = node(&fast_trace, bindings, late, true);
            let mut tree = node(&tree_trace, bindings, late, false);
            assert_eq!(*fast_trace.borrow(), *tree_trace.borrow());
            let base = state(&fast);
            let full_fast = fast.checkpoint();
            let full_tree = tree.checkpoint();
            let outer_fast = fast.begin_bound_checkpoint().unwrap();
            let outer_tree = tree.begin_bound_checkpoint().unwrap();
            for (right, id, value) in [
                (false, 0, Some(((0, 1), 901))),
                (true, 64, Some(((0, 2), -902))),
                (false, 128, None),
                (false, 128, Some(((usize::MAX, usize::MAX), i64::MAX))),
                (true, 128, Some(((usize::MAX, usize::MAX), i64::MIN))),
            ] {
                let change = TupleChange { id, value };
                let a = if right {
                    fast.apply_right(change.clone())
                } else {
                    fast.apply_left(change.clone())
                };
                let b = if right {
                    tree.apply_right(change)
                } else {
                    tree.apply_left(change)
                };
                assert_eq!(a, b);
                assert_eq!(state(&fast), state(&tree));
            }
            let child_fast = fast.begin_bound_checkpoint().unwrap();
            let child_tree = tree.begin_bound_checkpoint().unwrap();
            assert_eq!(
                fast.apply_left(TupleChange::put(63, ((4096, 1024), -3))),
                tree.apply_left(TupleChange::put(63, ((4096, 1024), -3))),
            );
            fast.commit_bound_checkpoint(child_fast);
            tree.commit_bound_checkpoint(child_tree);
            let expected = state(&fast);
            let calls = fast_trace.borrow().len();
            let capture_fast = fast.capture_checkpoint(outer_fast.as_ref());
            let capture_tree = tree.capture_checkpoint(outer_tree.as_ref());
            assert_eq!(fast_trace.borrow().len(), calls, "capture does not project");
            fast.restore_bound_checkpoint(outer_fast);
            tree.restore_bound_checkpoint(outer_tree);
            assert_eq!(state(&fast), base);
            let install_fast = fast.begin_bound_checkpoint().unwrap();
            let install_tree = tree.begin_bound_checkpoint().unwrap();
            fast.restore_bound_checkpoint(capture_fast);
            tree.restore_bound_checkpoint(capture_tree);
            assert_eq!(state(&fast), expected);
            assert_eq!(state(&fast), state(&tree));
            let before_panic = state(&fast);
            for node in [&mut fast, &mut tree] {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| {
                        node.apply_right(TupleChange::put(0, ((0, 0), -90001)))
                    }))
                    .is_err()
                );
                assert!(!node.is_poisoned());
            }
            assert_eq!(state(&fast), before_panic);
            assert_eq!(*fast_trace.borrow(), *tree_trace.borrow());
            fast.restore_bound_checkpoint(install_fast);
            tree.restore_bound_checkpoint(install_tree);
            assert_eq!(state(&fast), base);
            fast.clear();
            tree.clear();
            assert_eq!(
                fast.apply_right(TupleChange::put(128, ((usize::MAX, 15), 7))),
                tree.apply_right(TupleChange::put(128, ((usize::MAX, 15), 7))),
            );
            fast.restore_bound_checkpoint(full_fast);
            tree.restore_bound_checkpoint(full_tree);
            assert_eq!(state(&fast), base);
            assert_eq!(state(&fast), state(&tree));
        }
    }
}

#[test]
fn directory_buckets_keep_ordered_custom_drop_and_actual_borrowed_fallbacks() {
    for comparison in [
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        for late in [false, true] {
            let mut node = JoinNode::comparing(comparison, |row: &usize| *row, |row: &usize| *row);
            if !late {
                assert!(node.bind_primitive_graph_journals());
            }
            for id in 0..4 {
                node.apply_left(TupleChange::put(id, id));
                node.apply_right(TupleChange::put(id, id));
            }
            assert!(node.bind_fixed_input(false, 4));
            assert!(node.bind_fixed_input(true, 4));
            if late {
                assert!(node.bind_primitive_graph_journals());
            }
            assert!(!node.left.fixed_index.as_ref().unwrap().buckets.is_paged());
            assert!(!node.right.fixed_index.as_ref().unwrap().buckets.is_paged());
        }
    }
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Key(usize);
    thread_local! { static CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    impl Clone for Key {
        fn clone(&self) -> Self {
            CLONES.with(|n| n.set(n.get() + 1));
            Self(self.0)
        }
    }
    assert!(!std::mem::needs_drop::<Key>());
    let mut custom = JoinNode::new(|row: &usize| Key(*row), |row: &usize| Key(*row));
    assert!(!custom.bind_primitive_graph_journals());
    for id in 0..2usize {
        custom.apply_left(TupleChange::put(id, id));
        custom.apply_right(TupleChange::put(id, id));
    }
    assert!(custom.bind_fixed_input(false, 2));
    assert!(!custom.left.fixed_index.as_ref().unwrap().buckets.is_paged());
    let before = CLONES.with(std::cell::Cell::get);
    let saved = custom.begin_bound_checkpoint().unwrap();
    custom.apply_left(TupleChange::put(0, 1));
    assert!(CLONES.with(std::cell::Cell::get) > before);
    custom.restore_bound_checkpoint(saved);
    #[derive(Clone)]
    struct Row(std::rc::Rc<()>);
    let mut drop_row = JoinNode::new(
        |row: &Row| {
            let _ = &row.0;
            0usize
        },
        |row: &Row| {
            let _ = &row.0;
            0usize
        },
    );
    assert!(!drop_row.bind_primitive_graph_journals());
    drop_row.apply_left(TupleChange::put(0usize, Row(std::rc::Rc::new(()))));
    drop_row.apply_right(TupleChange::put(0usize, Row(std::rc::Rc::new(()))));
    drop_row.apply_right(TupleChange::put(0usize, Row(std::rc::Rc::new(()))));
    assert!(drop_row.bind_fixed_input(false, 1));
    assert!(
        !drop_row
            .left
            .fixed_index
            .as_ref()
            .unwrap()
            .buckets
            .is_paged()
    );
    fn borrowed<'a>(left: &'a str, right: &'a str, key: &'a str) {
        let mut node = JoinNode::new(|_: &i32| key, |_: &i32| key);
        assert!(node.apply_left(TupleChange::put(left, 3)).is_empty());
        assert_eq!(
            node.apply_right(TupleChange::put(right, 4)),
            vec![TupleChange::put((left, right), (3, 4))]
        );
    }
    let left = String::from("left");
    let right = String::from("right");
    let key = String::from("key");
    borrowed(&left, &right, &key);
}
