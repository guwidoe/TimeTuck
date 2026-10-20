//! Original primitive-cache controls; no upstream method credits.
use super::*;
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn primitive_expansion_tails_duplicates_and_maximum_coordinates_match_staged_storage() {
    for primitive in [false, true] {
        let mut node = FlatMapNode::new(|values: &Vec<i64>| values.clone());
        assert!(
            !node.lengths.is_primitive(),
            "standalone construction retains tree storage"
        );
        if primitive {
            assert!(node.bind_primitive_inputs());
        }
        let id = (usize::MAX, usize::MAX);
        assert_eq!(
            node.apply(TupleChange::put(id, vec![7, 7, 3])),
            vec![
                TupleChange::put((id, 0), 7),
                TupleChange::put((id, 1), 7),
                TupleChange::put((id, 2), 3)
            ]
        );
        assert_eq!(
            node.apply(TupleChange::put(id, vec![3, 7])),
            vec![
                TupleChange::retract((id, 2)),
                TupleChange::put((id, 0), 3),
                TupleChange::put((id, 1), 7)
            ]
        );
        assert_eq!(
            node.apply(TupleChange::put(id, vec![])),
            vec![TupleChange::retract((id, 0)), TupleChange::retract((id, 1))]
        );
        assert_eq!(
            node.lengths.get(&id),
            Some(&0),
            "empty expansion is a present input"
        );
        assert!(node.apply(TupleChange::retract(id)).is_empty());
        assert!(node.lengths.get(&id).is_none());
        assert!(node.apply(TupleChange::retract(id)).is_empty());
        assert_eq!(
            node.apply(TupleChange::put((0, usize::MAX), vec![1])),
            vec![TupleChange::put(((0, usize::MAX), 0), 1)]
        );
        assert_eq!(
            node.apply(TupleChange::put((usize::MAX, 0), vec![2])),
            vec![TupleChange::put(((usize::MAX, 0), 0), 2)]
        );
    }
}

#[test]
fn primitive_expansion_captures_nested_full_saves_alternate_trials_and_outer_undo_without_mapping()
{
    let calls = Cell::new(0);
    let mut node = FlatMapNode::new(|n: &usize| {
        calls.set(calls.get() + 1);
        0..*n
    });
    assert!(node.bind_primitive_inputs());
    for (id, count) in [
        ((0usize, 0usize), 3),
        ((usize::MAX, usize::MAX), 0),
        ((1, 16), 2),
    ] {
        node.apply(TupleChange::put(id, count));
    }
    let contents = |node: &FlatMapNode<'_, (usize, usize), usize, usize>| {
        node.lengths
            .iter()
            .map(|(key, length)| (*key, *length))
            .collect::<Vec<_>>()
    };
    let base = contents(&node);
    let full_base = node.checkpoint();
    let original = node.begin_checkpoint();
    node.apply(TupleChange::put((0, 0), 1));
    node.apply(TupleChange::retract((usize::MAX, usize::MAX)));
    let inner = node.begin_checkpoint();
    node.apply(TupleChange::put((1, 16), 4));
    node.apply(TupleChange::put((usize::MAX, usize::MAX), 0));
    node.apply(TupleChange::put((16, 31), 5));
    let wanted = contents(&node);
    let before = calls.get();
    let saved = node.capture_checkpoint(original.as_ref());
    let nested = node.capture_checkpoint(original.as_ref());
    assert!(saved.is::<Capture<(usize, usize)>>());
    node.commit_checkpoint(inner);
    node.restore(original);
    assert_eq!(contents(&node), base);
    let other = node.begin_checkpoint();
    node.apply(TupleChange::put((0, 0), 8));
    node.clear();
    node.restore(full_base);
    node.restore(other);
    assert_eq!(contents(&node), base);
    let before_install = calls.get();
    let outer = node.begin_checkpoint();
    node.restore(nested);
    assert_eq!(contents(&node), wanted);
    node.restore(outer);
    assert_eq!(contents(&node), base);
    node.restore(saved);
    assert_eq!(contents(&node), wanted);
    assert_eq!(calls.get(), before_install);
    assert_eq!(before_install, before + 1);
    assert_eq!(
        node.apply(TupleChange::retract((1, 16))),
        (0..4)
            .map(|position| TupleChange::retract(((1, 16), position)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        calls.get(),
        before_install,
        "retraction uses captured length without mapper replay"
    );
}

#[test]
fn primitive_expansion_mapping_and_iterator_failures_do_not_publish_lengths() {
    for primitive in [false, true] {
        let fault = Cell::new(0);
        let mut node = FlatMapNode::new(|n: &usize| {
            assert_ne!(fault.get(), 1, "mapper failure");
            let fault = &fault;
            (0..*n).inspect(move |&position| {
                assert!(!(fault.get() == 2 && position == 2), "iterator failure")
            })
        });
        if primitive {
            assert!(node.bind_primitive_inputs());
        }
        node.apply(TupleChange::put(usize::MAX, 4));
        for mode in [1, 2] {
            fault.set(mode);
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || node.apply(TupleChange::put(usize::MAX, 6))
                ))
                .is_err()
            );
            assert_eq!(node.lengths.get(&usize::MAX), Some(&4));
        }
        assert_eq!(
            node.apply(TupleChange::retract(usize::MAX)),
            (0..4)
                .map(|position| TupleChange::retract((usize::MAX, position)))
                .collect::<Vec<_>>()
        );
        fault.set(0);
        assert_eq!(
            node.apply(TupleChange::put(usize::MAX, 1)),
            vec![TupleChange::put((usize::MAX, 0), 0)]
        );
    }
}

struct Id {
    value: usize,
    clone_failure: Rc<Cell<usize>>,
    comparison_failure: Rc<Cell<usize>>,
}
impl Clone for Id {
    fn clone(&self) -> Self {
        let remaining = self.clone_failure.get();
        if remaining > 0 {
            self.clone_failure.set(remaining - 1);
            assert_ne!(remaining, 1, "identity Clone failure");
        }
        Self {
            value: self.value,
            clone_failure: self.clone_failure.clone(),
            comparison_failure: self.comparison_failure.clone(),
        }
    }
}
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        let remaining = self.comparison_failure.get();
        if remaining > 0 {
            self.comparison_failure.set(remaining - 1);
            assert_ne!(remaining, 1, "identity comparison failure");
        }
        self.value.cmp(&other.value)
    }
}
#[test]
fn custom_identity_clone_and_comparison_failures_keep_original_staged_path() {
    let clones = Rc::new(Cell::new(0));
    let comparisons = Rc::new(Cell::new(0));
    let id = || Id {
        value: 0,
        clone_failure: clones.clone(),
        comparison_failure: comparisons.clone(),
    };
    let fresh = || {
        let mut node = FlatMapNode::new(|n: &usize| 0..*n);
        node.apply(TupleChange::put(id(), 4));
        assert!(!node.bind_primitive_inputs());
        node
    };
    // Shrinking 4->2 emits two tail removals, then two positional puts.
    for failure in 1..=4 {
        let mut node = fresh();
        clones.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| node.apply(TupleChange::put(id(), 2)))).is_err());
        clones.set(0);
        assert_eq!(node.lengths.get(&id()), Some(&4));
    }
    let mut node = fresh();
    comparisons.set(usize::MAX);
    node.apply(TupleChange::put(id(), 2));
    let comparison_count = usize::MAX - comparisons.get();
    comparisons.set(0);
    assert!(comparison_count > 0);
    for failure in 1..=comparison_count {
        let mut node = fresh();
        comparisons.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| node.apply(TupleChange::put(id(), 2)))).is_err());
        comparisons.set(0);
        assert_eq!(node.lengths.get(&id()), Some(&4));
    }
    let mut node = fresh();
    let original = node.begin_checkpoint();
    node.apply(TupleChange::put(id(), 1));
    let saved = node.capture_checkpoint(original.as_ref());
    assert!(
        saved.is::<PrimitiveRows<Id, usize>>(),
        "custom identity captures retain full state"
    );
    node.restore(original);
    node.restore(saved);
    assert_eq!(node.lengths.get(&id()), Some(&1));
}

#[test]
fn borrowed_identity_standalone_nodes_keep_the_original_nonstatic_api() {
    let owned = String::from("borrowed");
    let mut node = FlatMapNode::new(|n: &usize| 0..*n);
    assert!(!node.lengths.is_primitive());
    assert_eq!(
        node.apply(TupleChange::put(owned.as_str(), 2)),
        vec![
            TupleChange::put((owned.as_str(), 0), 0),
            TupleChange::put((owned.as_str(), 1), 1)
        ]
    );
    assert_eq!(
        node.apply(TupleChange::retract(owned.as_str())),
        vec![
            TupleChange::retract((owned.as_str(), 0)),
            TupleChange::retract((owned.as_str(), 1))
        ]
    );
}
