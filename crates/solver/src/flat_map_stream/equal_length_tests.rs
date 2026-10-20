//! Original controls for the private graph-only cardinality cache shortcut.
use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn equal_lengths_graph_opt_in_preserves_zero_presence_tails_duplicates_and_max_ids() {
    for optimized in [false, true] {
        let mut node = FlatMapNode::new(|rows: &Vec<i64>| rows.clone());
        assert!(!node.omit_equal_lengths);
        if optimized {
            assert!(node.bind_graph_inputs());
        } else {
            assert!(node.bind_primitive_inputs());
        }
        let id = (usize::MAX, usize::MAX);
        for values in [
            vec![],
            vec![],
            vec![7, 7, 3],
            vec![2, 2, 2],
            vec![8],
            vec![],
            vec![],
        ] {
            let old = node.lengths.get(&id).copied().unwrap_or(0);
            let held = node.lengths.shared_entries();
            let expected = (values.len()..old)
                .map(|p| TupleChange::retract((id, p)))
                .chain(
                    values
                        .iter()
                        .enumerate()
                        .map(|(p, v)| TupleChange::put((id, p), *v)),
                )
                .collect::<Vec<_>>();
            let new = values.len();
            assert_eq!(node.apply(TupleChange::put(id, values)), expected);
            assert_eq!(node.lengths.get(&id), Some(&new));
            if let Some((key, value)) = held.first() {
                let current = node.lengths.shared_entries();
                assert!(std::sync::Arc::ptr_eq(key, &current[0].0));
                assert_eq!(
                    std::sync::Arc::ptr_eq(value, &current[0].1),
                    optimized && old == new
                );
            }
        }
        node.apply(TupleChange::retract(id));
        assert!(node.lengths.get(&id).is_none());
        node.apply(TupleChange::put(id, vec![]));
        assert_eq!(node.lengths.get(&id), Some(&0));
        node.apply(TupleChange::retract(id));
        assert!(node.lengths.get(&id).is_none());
    }
}

#[test]
fn equal_lengths_captures_after_omission_nested_full_alternate_trials_and_outer_restore() {
    let calls = Cell::new(0);
    let mut node = FlatMapNode::new(|n: &usize| {
        calls.set(calls.get() + 1);
        0..*n
    });
    assert!(node.bind_graph_inputs());
    node.apply(TupleChange::put(0usize, 2usize));
    node.apply(TupleChange::put(usize::MAX, 0usize));
    let full = node.checkpoint();
    let outer = node.begin_checkpoint();
    node.apply(TupleChange::put(0, 2)); // Omitted: capture must still install the correct length.
    let equal = node.capture_checkpoint(outer.as_ref());
    let child = node.begin_checkpoint();
    node.apply(TupleChange::put(0, 5));
    node.apply(TupleChange::put(0, 5));
    node.apply(TupleChange::retract(usize::MAX));
    let changed = node.capture_checkpoint(outer.as_ref());
    node.commit_checkpoint(child);
    node.restore(outer);
    assert_eq!(node.lengths.get(&0), Some(&2));
    assert_eq!(node.lengths.get(&usize::MAX), Some(&0));
    let other = node.begin_checkpoint();
    node.apply(TupleChange::put(0, 9));
    node.restore(other);
    let enclosing = node.begin_checkpoint();
    node.restore(equal);
    assert_eq!(node.lengths.get(&0), Some(&2));
    node.restore(changed);
    assert_eq!(node.lengths.get(&0), Some(&5));
    assert_eq!(node.lengths.get(&usize::MAX), None);
    node.apply(TupleChange::put(0, 5));
    node.restore(enclosing);
    assert_eq!(node.lengths.get(&0), Some(&2));
    assert_eq!(node.lengths.get(&usize::MAX), Some(&0));
    node.clear();
    node.restore(full);
    let before = calls.get();
    assert_eq!(
        node.apply(TupleChange::retract(0)),
        vec![TupleChange::retract((0, 0)), TupleChange::retract((0, 1))]
    );
    assert_eq!(calls.get(), before);
}

struct FaultIter<'a> {
    next: usize,
    end: usize,
    mode: &'a Cell<usize>,
    trace: &'a RefCell<Vec<&'static str>>,
}
impl Iterator for FaultIter<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        self.trace.borrow_mut().push("next");
        assert_ne!(self.mode.get(), 3, "next panic");
        if self.next == self.end {
            None
        } else {
            let n = self.next;
            self.next += 1;
            Some(n)
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.trace.borrow_mut().push("size_hint");
        assert_ne!(self.mode.get(), 2, "hint panic");
        (0, None)
    }
}
impl Drop for FaultIter<'_> {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("iterator drop");
        if self.mode.get() == 4 {
            self.mode.set(0);
            panic!("iterator drop panic");
        }
    }
}
fn iterator_trace(optimized: bool) -> Vec<&'static str> {
    let trace = RefCell::new(Vec::new());
    let mode = Cell::new(0);
    let mut node = FlatMapNode::new(|n: &usize| {
        trace.borrow_mut().push("mapping");
        assert_ne!(mode.get(), 1, "mapping panic");
        FaultIter {
            next: 0,
            end: *n,
            mode: &mode,
            trace: &trace,
        }
    });
    if optimized {
        node.bind_graph_inputs();
    } else {
        node.bind_primitive_inputs();
    }
    node.apply(TupleChange::put(0usize, 3usize));
    for fault in 0..=4 {
        mode.set(fault);
        let result = catch_unwind(AssertUnwindSafe(|| node.apply(TupleChange::put(0, 3))));
        assert_eq!(result.is_ok(), fault == 0);
        assert_eq!(node.lengths.get(&0), Some(&3));
    }
    mode.set(0);
    drop(node);
    trace.into_inner()
}
#[test]
fn equal_lengths_mapper_hint_iteration_and_iterator_drop_phases_are_unchanged() {
    assert_eq!(iterator_trace(true), iterator_trace(false));
}

#[test]
fn equal_lengths_standalone_borrowed_and_custom_no_drop_identity_stay_original() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CLONES: AtomicUsize = AtomicUsize::new(0);
    #[derive(Eq, PartialEq, Ord, PartialOrd)]
    struct Id(usize);
    impl Clone for Id {
        fn clone(&self) -> Self {
            CLONES.fetch_add(1, Ordering::Relaxed);
            panic!("custom identity clone");
        }
    }
    let mut custom = FlatMapNode::new(|n: &usize| 0..*n);
    assert!(!custom.bind_graph_inputs());
    assert!(!custom.omit_equal_lengths);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || custom.apply(TupleChange::put(Id(0), 1usize))
        ))
        .is_err()
    );
    assert_eq!(CLONES.load(Ordering::Relaxed), 1);
    assert!(custom.lengths.is_empty());
    static COMPARE_PANIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    #[derive(Clone, Eq, PartialEq)]
    struct OrdId(usize);
    impl PartialOrd for OrdId {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for OrdId {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            assert!(
                !COMPARE_PANIC.load(Ordering::Relaxed),
                "custom comparison panic"
            );
            self.0.cmp(&other.0)
        }
    }
    let mut comparisons = FlatMapNode::new(|n: &usize| 0..*n);
    assert!(!comparisons.bind_graph_inputs());
    comparisons.apply(TupleChange::put(OrdId(0), 0usize));
    COMPARE_PANIC.store(true, Ordering::Relaxed);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || comparisons.apply(TupleChange::put(OrdId(0), 0usize))
        ))
        .is_err()
    );
    COMPARE_PANIC.store(false, Ordering::Relaxed);
    assert_eq!(comparisons.lengths.get(&OrdId(0)), Some(&0));
    let key = String::from("borrowed");
    let mut borrowed = FlatMapNode::new(|n: &usize| 0..*n);
    borrowed.apply(TupleChange::put(key.as_str(), 0usize));
    borrowed.apply(TupleChange::put(key.as_str(), 0usize));
    assert!(!borrowed.omit_equal_lengths);
    let mut standalone = FlatMapNode::new(|n: &usize| 0..*n);
    standalone.apply(TupleChange::put(0usize, 0usize));
    let held = standalone.lengths.shared_entries();
    standalone.apply(TupleChange::put(0usize, 0usize));
    let current = standalone.lengths.shared_entries();
    assert!(!std::sync::Arc::ptr_eq(&held[0].1, &current[0].1));
    assert!(!standalone.omit_equal_lengths);
}
