//! Original graph-private row-cache controls; no upstream method credit.
use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

type Id = (usize, usize);
type Node = JoinNode<'static, Id, Id, Row, Row, usize>;
thread_local! {
    static TRACE:RefCell<Vec<(char,i64)>>=const {RefCell::new(Vec::new())};
    static FAIL:Cell<Option<i64>>=const {Cell::new(None)};
    static FAIL_PROJECTION: Cell<bool> = const { Cell::new(false) };
}
#[derive(Debug, PartialEq, Eq)]
struct Row {
    key: usize,
    value: i64,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|t| t.borrow_mut().push(('c', self.value)));
        assert!(
            !FAIL.with(|f| f.get() == Some(self.value)),
            "row clone failure"
        );
        Self {
            key: self.key,
            value: self.value,
        }
    }
}
fn projection(row: &Row, side: char) -> usize {
    assert!(!FAIL_PROJECTION.with(Cell::get), "projection failure");
    TRACE.with(|t| t.borrow_mut().push((side, row.value)));
    row.key
}
fn node(comparison: JoinComparison, primitive: bool) -> Node {
    let mut n = JoinNode::comparing(
        comparison,
        |r: &Row| projection(r, 'l'),
        |r: &Row| projection(r, 'r'),
    );
    if primitive {
        assert!(n.bind_primitive_graph_journals());
    }
    assert_eq!(n.left.rows.is_primitive(), primitive);
    for (index, id) in [
        (0, 0),
        (0, 15),
        (1, 16),
        (4096, 1024),
        (usize::MAX, usize::MAX),
    ]
    .into_iter()
    .enumerate()
    {
        n.apply_left(TupleChange::put(
            id,
            Row {
                key: index % 3,
                value: index as i64,
            },
        ));
        n.apply_right(TupleChange::put(
            id,
            Row {
                key: index % 3,
                value: 10 + index as i64,
            },
        ));
    }
    n
}
fn begin(n: &mut Node) -> Box<dyn Any> {
    n.begin_bound_checkpoint().unwrap_or_else(|| n.checkpoint())
}
fn matches(n: &Node) -> Vec<(Id, Id, i64, i64)> {
    n.matches()
        .map(|((i, j), (l, r))| (*i, *j, l.value, r.value))
        .collect()
}
fn update(
    n: &mut Node,
    right: bool,
    id: Id,
    value: Option<(usize, i64)>,
) -> JoinChanges<Id, Id, Row, Row> {
    let change = TupleChange {
        id,
        value: value.map(|(key, value)| Row { key, value }),
    };
    if right {
        n.apply_right(change)
    } else {
        n.apply_left(change)
    }
}
fn trace() -> Vec<(char, i64)> {
    TRACE.with(|t| t.take())
}
const COMPARISONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
#[test]
fn primitive_join_rows_all_comparisons_left_right_order_and_clone_trace_match_tree() {
    for comparison in COMPARISONS {
        let mut fast = node(comparison, true);
        let mut original = node(comparison, false);
        trace();
        for (right, id, value) in [
            (false, (0, 0), Some((0, 50))), // surviving pair replacement
            (true, (0, 15), Some((2, 51))), // bucket change
            (false, (usize::MAX, usize::MAX), None),
            (true, (usize::MAX, usize::MAX), Some((1, 52))),
            (false, (4096, usize::MAX), Some((2, 53))),
            (false, (99, 99), None),
            (true, (0, 15), Some((2, 51))),
        ] {
            let actual = update(&mut fast, right, id, value);
            let actual_trace = trace();
            let expected = update(&mut original, right, id, value);
            assert_eq!(actual, expected);
            assert_eq!(actual_trace, trace());
            assert_eq!(matches(&fast), matches(&original));
        }
        let before = matches(&fast);
        let outer = begin(&mut fast);
        update(&mut fast, false, (0, 0), Some((2, 60)));
        let child = begin(&mut fast);
        update(&mut fast, true, (usize::MAX, usize::MAX), Some((2, 61)));
        fast.commit_bound_checkpoint(child);
        fast.restore_bound_checkpoint(outer);
        assert_eq!(matches(&fast), before);
        assert!(!fast.is_poisoned());
    }
}
#[test]
fn primitive_join_rows_capture_is_stable_across_trials_and_installs_inside_outer_journal() {
    let mut fast = node(JoinComparison::Equal, true);
    let mut original = node(JoinComparison::Equal, false);
    let base = matches(&fast);
    let a = begin(&mut fast);
    let b = begin(&mut original);
    update(&mut fast, false, (0, 0), Some((2, 70)));
    update(&mut original, false, (0, 0), Some((2, 70)));
    let child = begin(&mut fast);
    update(&mut fast, true, (usize::MAX, usize::MAX), Some((2, 71)));
    update(&mut original, true, (usize::MAX, usize::MAX), Some((2, 71)));
    fast.commit_bound_checkpoint(child);
    let saved_fast = fast.capture_checkpoint(a.as_ref());
    let saved_original = original.capture_checkpoint(b.as_ref());
    let captured = matches(&fast);
    fast.restore_bound_checkpoint(a);
    original.restore_bound_checkpoint(b);
    assert_eq!(matches(&fast), base);
    for n in [&mut fast, &mut original] {
        let trial = begin(n);
        update(n, false, (0, 0), None);
        update(n, true, (7, usize::MAX), Some((2, 72)));
        n.restore_bound_checkpoint(trial);
    }
    let outer_fast = begin(&mut fast);
    let outer_original = begin(&mut original);
    fast.restore_bound_checkpoint(saved_fast);
    original.restore_bound_checkpoint(saved_original);
    assert_eq!(matches(&fast), captured);
    assert_eq!(matches(&fast), matches(&original));
    let full = fast.checkpoint();
    update(&mut fast, false, (0, 0), None);
    fast.restore_bound_checkpoint(full);
    assert_eq!(matches(&fast), captured);
    fast.restore_bound_checkpoint(outer_fast);
    original.restore_bound_checkpoint(outer_original);
    assert_eq!(matches(&fast), base);
    assert_eq!(matches(&fast), matches(&original));
}
#[test]
fn primitive_join_rows_weak_snapshot_and_captured_payloads_keep_original_owners() {
    let mut n = node(JoinComparison::Equal, true);
    let rows = n.left.rows.shared_entries();
    let (key, value) = rows.iter().find(|(id, _)| **id == (0, 0)).unwrap();
    let key_ptr = Arc::as_ptr(key);
    let payload_ptr = Arc::as_ptr(value);
    let key_weak = Arc::downgrade(key);
    let payload_weak = Arc::downgrade(value);
    let snapshot = n.left.rows.materialized().clone();
    drop(rows);
    let outer = begin(&mut n);
    update(&mut n, false, (0, 0), Some((0, 80)));
    n.commit_bound_checkpoint(outer);
    let current = n.left.rows.shared_entries();
    assert_eq!(
        Arc::as_ptr(&current.iter().find(|(id, _)| **id == (0, 0)).unwrap().0),
        key_ptr
    );
    assert_eq!(snapshot.get(&(0, 0)).unwrap() as *const _, payload_ptr);
    assert!(payload_weak.upgrade().is_some());
    drop(current);
    drop(snapshot);
    assert!(
        payload_weak.upgrade().is_none(),
        "Weak observes original retirement, not pooling"
    );
    let outer = begin(&mut n);
    update(&mut n, false, (0, 0), Some((0, 81)));
    let captured = n.capture_checkpoint(outer.as_ref());
    let rows = n.left.rows.shared_entries();
    let ptr = Arc::as_ptr(&rows.iter().find(|(id, _)| **id == (0, 0)).unwrap().1);
    drop(rows);
    n.restore_bound_checkpoint(outer);
    n.restore_bound_checkpoint(captured);
    assert_eq!(n.left.rows.get(&(0, 0)).unwrap() as *const _, ptr);
    update(&mut n, false, (0, 0), None);
    assert!(key_weak.upgrade().is_none());
}
#[test]
fn primitive_join_rows_projection_and_output_clone_panics_preserve_original_preflight() {
    for right in [false, true] {
        for comparison in COMPARISONS {
            for projection_panic in [false, true] {
                let mut expected_trace = None;
                for primitive in [false, true] {
                    let mut n = node(comparison, primitive);
                    let before = matches(&n);
                    let token = begin(&mut n);
                    trace();
                    FAIL.with(|f| f.set(Some(90)));
                    FAIL_PROJECTION.with(|f| f.set(projection_panic));
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        update(&mut n, right, (0, 0), Some((1, 90)))
                    }));
                    FAIL.with(|f| f.set(None));
                    FAIL_PROJECTION.with(|f| f.set(false));
                    let observed = trace();
                    if let Some(expected) = &expected_trace {
                        assert_eq!(&observed, expected);
                    } else {
                        expected_trace = Some(observed);
                    }
                    assert!(result.is_err());
                    assert!(!n.is_poisoned());
                    assert_eq!(matches(&n), before);
                    n.restore_bound_checkpoint(token);
                    assert_eq!(matches(&n), before);
                }
            }
        }
    }
}
#[test]
fn primitive_join_rows_fixed_conversion_retains_owners_and_ordinary_side_snapshots() {
    let mut n = JoinNode::<usize, Id, Row, Row, usize>::new(|r| r.key, |r| r.key);
    assert!(n.bind_primitive_graph_journals());
    for id in 0..3 {
        n.apply_left(TupleChange::put(
            id,
            Row {
                key: 0,
                value: id as i64,
            },
        ));
    }
    n.apply_right(TupleChange::put(
        (usize::MAX, usize::MAX),
        Row { key: 0, value: 4 },
    ));
    let owners = n.left.rows.shared_entries();
    assert!(n.bind_fixed_input(false, 3));
    for (id, owner) in &owners {
        assert_eq!(n.left.row(id).unwrap() as *const _, Arc::as_ptr(owner));
    }
    let before: Vec<_> = n
        .matches()
        .map(|((i, j), (l, r))| (*i, *j, l.value, r.value))
        .collect();
    let outer = n.begin_bound_checkpoint().unwrap();
    n.apply_left(TupleChange::put(0, Row { key: 0, value: 8 }));
    n.apply_right(TupleChange::put(
        (usize::MAX, usize::MAX),
        Row { key: 1, value: 9 },
    ));
    let saved = n.capture_checkpoint(outer.as_ref());
    n.restore_bound_checkpoint(outer);
    assert_eq!(
        n.matches()
            .map(|((i, j), (l, r))| (*i, *j, l.value, r.value))
            .collect::<Vec<_>>(),
        before
    );
    n.restore_bound_checkpoint(saved);
    assert_eq!(n.matches().count(), 0);
}

#[test]
fn primitive_join_rows_custom_owned_and_borrowed_paths_keep_tree_storage() {
    let mut owned = JoinNode::<usize, usize, String, String, usize>::new(String::len, String::len);
    assert!(!owned.bind_primitive_graph_journals());
    assert!(!owned.left.rows.is_primitive());
    owned.apply_left(TupleChange::put(0, "left".to_owned()));
    owned.apply_right(TupleChange::put(0, "pair".to_owned()));
    assert_eq!(owned.matches().count(), 1);

    let id = "borrowed".to_owned();
    let value = "payload".to_owned();
    let mut borrowed = JoinNode::<&str, &str, &str, &str, usize>::new(|r| r.len(), |r| r.len());
    borrowed.apply_left(TupleChange::put(id.as_str(), value.as_str()));
    borrowed.apply_right(TupleChange::put(id.as_str(), value.as_str()));
    assert_eq!(borrowed.matches().count(), 1);
    assert!(!borrowed.left.rows.is_primitive());
}

#[test]
fn primitive_join_rows_atomic_batches_preserve_clone_phase_and_outer_rollback() {
    for comparison in COMPARISONS {
        let mut fast = node(comparison, true);
        let mut original = node(comparison, false);
        let before = matches(&fast);
        let outer = begin(&mut fast);
        let old_outer = begin(&mut original);
        for structural in [false, true] {
            fn changes(structural: bool) -> Vec<TupleChange<Id, Row>> {
                vec![
                    TupleChange::put(
                        (0, 0),
                        Row {
                            key: usize::from(structural),
                            value: 100,
                        },
                    ),
                    TupleChange::put(
                        (0, 0),
                        Row {
                            key: usize::from(structural),
                            value: 101,
                        },
                    ),
                    TupleChange::put((usize::MAX, usize::MAX), Row { key: 2, value: 102 }),
                ]
            }
            trace();
            let actual = fast.apply_batch(changes(structural), changes(structural));
            let actual_trace = trace();
            let expected = original.apply_batch(changes(structural), changes(structural));
            assert_eq!(actual, expected);
            assert_eq!(actual_trace, trace());
            assert_eq!(matches(&fast), matches(&original));
        }
        fast.restore_bound_checkpoint(outer);
        original.restore_bound_checkpoint(old_outer);
        assert_eq!(matches(&fast), before);
        assert_eq!(matches(&original), before);
    }
}
