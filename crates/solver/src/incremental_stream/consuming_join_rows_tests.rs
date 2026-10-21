//! Original consumed-graph join row controls; no upstream source-method credit.
use super::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
type Row = (usize, usize, usize);
type Key = (usize, usize);
type Node<'a> = JoinNode<'a, usize, usize, Row, Row, Key>;
type Trace = Vec<(char, Row)>;
type State = Vec<((usize, usize), (Row, Row))>;
fn state(n: &Node<'_>) -> State {
    n.matches()
        .map(|((i, j), (a, b))| ((*i, *j), (*a, *b)))
        .collect()
}
fn node<'a>(
    copy: bool,
    comparison: JoinComparison,
    count: usize,
    both: bool,
    trace: &'a RefCell<Trace>,
) -> Node<'a> {
    let mut n = JoinNode::comparing(
        comparison,
        move |r: &Row| {
            trace.borrow_mut().push(('L', *r));
            (r.1, r.2)
        },
        move |r: &Row| {
            trace.borrow_mut().push(('R', *r));
            (r.1, r.2)
        },
    );
    assert!(n.bind_primitive_graph_journals());
    if copy {
        assert!(n.enable_consuming_rows());
    }
    for id in 0..count {
        n.apply_left(TupleChange::put(id, (id, id % 3, id % 5)));
        n.apply_right(TupleChange::put(id, (id, (id + 1) % 3, id % 5)));
    }
    let entries = n.left.rows.shared_entries();
    let key = entries[0].0.clone();
    let old_value = Arc::downgrade(&entries[0].1);
    drop(entries);
    assert!(n.bind_fixed_input(false, count));
    assert_eq!(old_value.upgrade().is_none(), copy);
    assert!(std::ptr::eq(
        n.left.row_iter().next().unwrap().0,
        key.as_ref()
    ));
    if both {
        assert!(n.bind_fixed_input(true, count));
    }
    trace.borrow_mut().clear();
    n
}
const COMPARISONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
#[test]
fn consuming_join_rows_all_comparisons_left_right_callback_output_order_and_max_keys_match_shared()
{
    for comparison in COMPARISONS {
        for both in [false, true] {
            let trace = RefCell::new(Vec::new());
            let old_trace = RefCell::new(Vec::new());
            let mut n = node(true, comparison, 131, both, &trace);
            let mut old = node(false, comparison, 131, both, &old_trace);
            for step in 0usize..30 {
                let id = [0, 63, 64, 65, 130][step % 5];
                let value = (step % 7 != 0).then_some((
                    id,
                    step % 3,
                    if step % 11 == 0 { usize::MAX } else { step % 5 },
                ));
                let change = TupleChange { id, value };
                let (actual, expected) = if step % 2 == 0 {
                    (n.apply_left(change.clone()), old.apply_left(change))
                } else {
                    (n.apply_right(change.clone()), old.apply_right(change))
                };
                assert_eq!(actual, expected, "{comparison:?} step{step}");
                assert_eq!(state(&n), state(&old));
            }
            assert_eq!(*trace.borrow(), *old_trace.borrow());
        }
    }
}
#[test]
fn consuming_join_rows_full_current_nested_mixed_bounds_and_outer_restore_match_shared() {
    for both in [false, true] {
        let trace = RefCell::new(Vec::new());
        let old_trace = RefCell::new(Vec::new());
        let mut n = node(true, JoinComparison::Equal, 8, both, &trace);
        let mut old = node(false, JoinComparison::Equal, 8, both, &old_trace);
        let base = state(&n);
        let full = n.checkpoint();
        let old_full = old.checkpoint();
        let outer = n.begin_bound_checkpoint().unwrap();
        let old_outer = old.begin_bound_checkpoint().unwrap();
        assert_eq!(
            n.apply_left(TupleChange::put(0, (0, 0, usize::MAX))),
            old.apply_left(TupleChange::put(0, (0, 0, usize::MAX)))
        );
        let child = n.begin_bound_checkpoint().unwrap();
        let old_child = old.begin_bound_checkpoint().unwrap();
        assert_eq!(
            n.apply_right(TupleChange::put(1, (1, usize::MAX, usize::MAX))),
            old.apply_right(TupleChange::put(1, (1, usize::MAX, usize::MAX)))
        );
        let capture = n.capture_checkpoint(outer.as_ref());
        let old_capture = old.capture_checkpoint(old_outer.as_ref());
        let wanted = state(&n);
        n.commit_bound_checkpoint(child);
        old.commit_bound_checkpoint(old_child);
        n.restore_bound_checkpoint(outer);
        old.restore_bound_checkpoint(old_outer);
        assert_eq!(state(&n), base);
        let outer = n.begin_bound_checkpoint().unwrap();
        let old_outer = old.begin_bound_checkpoint().unwrap();
        n.restore_bound_checkpoint(capture);
        old.restore_bound_checkpoint(old_capture);
        assert_eq!(state(&n), wanted);
        assert_eq!(state(&n), state(&old));
        n.clear();
        old.clear();
        n.restore_bound_checkpoint(full);
        old.restore_bound_checkpoint(old_full);
        assert_eq!(state(&n), base);
        n.restore_bound_checkpoint(outer);
        old.restore_bound_checkpoint(old_outer);
        assert_eq!(state(&n), state(&old));
        let outer = n.begin_bound_checkpoint().unwrap();
        let old_outer = old.begin_bound_checkpoint().unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply_left(TupleChange::put(usize::MAX, (0, 0, 0)))
            ))
            .is_err()
        );
        assert!(
            catch_unwind(AssertUnwindSafe(
                || old.apply_left(TupleChange::put(usize::MAX, (0, 0, 0)))
            ))
            .is_err()
        );
        assert!(n.is_poisoned() && old.is_poisoned());
        n.restore_bound_checkpoint(outer);
        old.restore_bound_checkpoint(old_outer);
        assert_eq!(state(&n), base);
        assert_eq!(state(&n), state(&old));
        assert_eq!(
            n.apply_left(TupleChange::retract(usize::MAX)),
            old.apply_left(TupleChange::retract(usize::MAX))
        );
    }
}
thread_local! {static TRACE:RefCell<Vec<usize>>=const{RefCell::new(Vec::new())};static FAIL:Cell<bool>=const{Cell::new(false)};}
struct Hooked(usize, usize);
impl Clone for Hooked {
    fn clone(&self) -> Self {
        TRACE.with(|t| t.borrow_mut().push(self.1));
        assert!(!FAIL.with(Cell::get), "row Clone panic");
        Self(self.0, self.1)
    }
}
#[test]
fn consuming_join_rows_custom_nodrop_clone_panic_and_default_standalone_keep_shared_owners() {
    fn run(attempt: bool) -> Vec<usize> {
        TRACE.with(|t| t.borrow_mut().clear());
        let mut n = JoinNode::new(|r: &Hooked| r.0, |r: &Hooked| r.0);
        assert!(n.bind_primitive_graph_journals());
        if attempt {
            assert!(!n.enable_consuming_rows());
        }
        for id in 0usize..2 {
            n.apply_left(TupleChange::put(id, Hooked(id, id)));
            n.apply_right(TupleChange::put(id, Hooked(id, id)));
        }
        let held = n.left.rows.shared_entries();
        let weak = Arc::downgrade(&held[0].1);
        drop(held);
        assert!(n.bind_fixed_input(false, 2));
        assert!(weak.upgrade().is_some());
        FAIL.with(|f| f.set(true));
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply_left(TupleChange::put(0, Hooked(0, 9)))
            ))
            .is_err()
        );
        FAIL.with(|f| f.set(false));
        assert!(!n.is_poisoned());
        assert_eq!(n.left.row(&0).unwrap().1.1, 0);
        TRACE.with(|t| t.take())
    }
    assert_eq!(run(false), run(true));
    let mut standalone =
        JoinNode::<usize, usize, Row, Row, Key>::new(|r| (r.1, r.2), |r| (r.1, r.2));
    assert!(!standalone.enable_consuming_rows());
    assert!(standalone.left.consuming_copy.is_none());
    let mut pairs = JoinNode::<(usize, usize), (usize, usize), Row, Row, Key>::new(
        |r| (r.1, r.2),
        |r| (r.1, r.2),
    );
    assert!(pairs.bind_primitive_graph_journals());
    assert!(pairs.enable_consuming_rows());
    assert!(pairs.left.rows.is_copied() && pairs.right.rows.is_copied());
    pairs.apply_left(TupleChange::put((usize::MAX, usize::MAX), (0, 0, 0)));
    assert!(pairs.left.fixed_rows.is_none());
    assert_eq!(
        pairs.left.row(&(usize::MAX, usize::MAX)),
        Some(&((0, 0), (0, 0, 0)))
    );
}
#[test]
fn consuming_join_rows_projection_and_batch_preflight_panics_keep_node_healthy() {
    for copy in [false, true] {
        let fail = Cell::new(false);
        let mut n = JoinNode::<usize, usize, Row, Row, Key>::new(
            |r| {
                assert!(!fail.get(), "projection panic");
                (r.1, r.2)
            },
            |r| (r.1, r.2),
        );
        n.bind_primitive_graph_journals();
        if copy {
            assert!(n.enable_consuming_rows());
        }
        for id in 0usize..4 {
            n.apply_left(TupleChange::put(id, (id, 0, 0)));
            n.apply_right(TupleChange::put(id, (id, 0, 0)));
        }
        assert!(n.bind_fixed_input(false, 4));
        assert!(n.bind_fixed_input(true, 4));
        let before = state(&n);
        fail.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply_left(TupleChange::put(0, (0, 1, 1)))
            ))
            .is_err()
        );
        assert_eq!(state(&n), before);
        assert!(!n.is_poisoned());
        assert!(
            catch_unwind(AssertUnwindSafe(
                || n.apply_batch([TupleChange::put(0, (0, 1, 1))], [])
            ))
            .is_err()
        );
        assert_eq!(state(&n), before);
        assert!(!n.is_poisoned());
    }
}

mod pair_payload_tests {
    use super::*;
    type Id = (usize, usize);
    type Value = (usize, usize, usize, usize);
    type PairNode<'a> = JoinNode<'a, Id, Id, Value, Value, Key>;
    type PairState = Vec<((Id, Id), (Value, Value))>;
    const IDS: [Id; 6] = [
        (0, 0),
        (0, 15),
        (0, 16),
        (4095, 1023),
        (4096, 0),
        (usize::MAX, usize::MAX),
    ];
    fn blank<'a>(
        copy: bool,
        comparison: JoinComparison,
        trace: &'a RefCell<Vec<(char, Value)>>,
        phase: &'a Cell<usize>,
    ) -> PairNode<'a> {
        let mut node = JoinNode::comparing(
            comparison,
            move |v: &Value| {
                trace.borrow_mut().push(('L', *v));
                assert_ne!(phase.get(), 1, "left projection");
                (v.1, v.2)
            },
            move |v: &Value| {
                trace.borrow_mut().push(('R', *v));
                assert_ne!(phase.get(), 2, "right projection");
                (v.1, v.2)
            },
        );
        assert!(node.bind_primitive_graph_journals());
        if copy {
            assert!(node.enable_consuming_rows());
        }
        assert_eq!(node.left.rows.is_copied(), copy);
        assert_eq!(node.right.rows.is_copied(), copy);
        node
    }
    fn contents(n: &PairNode<'_>) -> PairState {
        n.matches()
            .map(|((a, b), (l, r))| ((*a, *b), (*l, *r)))
            .collect()
    }
    fn populate(n: &mut PairNode<'_>) {
        for (i, id) in IDS.into_iter().enumerate() {
            n.apply_left(TupleChange::put(id, (i, i % 3, i % 2, 17)));
            n.apply_right(TupleChange::put(id, (i, i % 3, i % 2, 19)));
        }
    }
    #[test]
    fn copied_pair_join_payloads_all_relations_full_ids_output_order_and_projection_calls_match_shared()
     {
        for comparison in COMPARISONS {
            let trace = RefCell::new(Vec::new());
            let old_trace = RefCell::new(Vec::new());
            let phase = Cell::new(0);
            let old_phase = Cell::new(0);
            let mut n = blank(true, comparison, &trace, &phase);
            let mut old = blank(false, comparison, &old_trace, &old_phase);
            populate(&mut n);
            populate(&mut old);
            for step in 0..40 {
                let id = IDS[step % IDS.len()];
                let value = (step % 7 != 0).then_some((
                    step,
                    step % 3,
                    if step % 11 == 0 { usize::MAX } else { step % 2 },
                    usize::MAX,
                ));
                let change = TupleChange { id, value };
                let (actual, expected) = if step % 2 == 0 {
                    (n.apply_left(change.clone()), old.apply_left(change))
                } else {
                    (n.apply_right(change.clone()), old.apply_right(change))
                };
                assert_eq!(actual, expected);
                assert_eq!(contents(&n), contents(&old));
                assert_eq!(*trace.borrow(), *old_trace.borrow());
            }
        }
    }
    #[test]
    fn copied_pair_join_payloads_nested_current_full_empty_snapshots_batch_and_outer_restore_match_shared()
     {
        let trace = RefCell::new(Vec::new());
        let old_trace = RefCell::new(Vec::new());
        let phase = Cell::new(0);
        let old_phase = Cell::new(0);
        let mut n = blank(true, JoinComparison::Equal, &trace, &phase);
        let mut old = blank(false, JoinComparison::Equal, &old_trace, &old_phase);
        populate(&mut n);
        populate(&mut old);
        let base = contents(&n);
        let full = n.checkpoint();
        let old_full = old.checkpoint();
        let outer = n.begin_bound_checkpoint().unwrap();
        let old_outer = old.begin_bound_checkpoint().unwrap();
        for id in IDS {
            assert_eq!(
                n.apply_left(TupleChange::retract(id)),
                old.apply_left(TupleChange::retract(id))
            );
        }
        let child = n.begin_bound_checkpoint().unwrap();
        let old_child = old.begin_bound_checkpoint().unwrap();
        for id in IDS {
            assert_eq!(
                n.apply_left(TupleChange::put(id, (7, 0, 1, usize::MAX))),
                old.apply_left(TupleChange::put(id, (7, 0, 1, usize::MAX)))
            );
        }
        n.commit_bound_checkpoint(child);
        old.commit_bound_checkpoint(old_child);
        let saved = n.capture_checkpoint(outer.as_ref());
        let old_saved = old.capture_checkpoint(old_outer.as_ref());
        let current = contents(&n);
        n.restore_bound_checkpoint(outer);
        old.restore_bound_checkpoint(old_outer);
        assert_eq!(contents(&n), base);
        assert_eq!(contents(&n), contents(&old));
        let parent = n.begin_bound_checkpoint().unwrap();
        let old_parent = old.begin_bound_checkpoint().unwrap();
        n.restore_bound_checkpoint(saved);
        old.restore_bound_checkpoint(old_saved);
        assert_eq!(contents(&n), current);
        assert_eq!(contents(&n), contents(&old));
        n.clear();
        old.clear();
        n.restore(full);
        old.restore(old_full);
        assert_eq!(contents(&n), base);
        assert_eq!(contents(&n), contents(&old));
        n.restore_bound_checkpoint(parent);
        old.restore_bound_checkpoint(old_parent);
        assert_eq!(contents(&n), contents(&old));
        let left = vec![
            TupleChange::put(IDS[0], (9, 1, 0, usize::MAX)),
            TupleChange::retract(IDS[1]),
        ];
        let right = vec![TupleChange::put(IDS[2], (7, 1, 0, usize::MAX))];
        assert_eq!(
            n.apply_batch(left.clone(), right.clone()),
            old.apply_batch(left, right)
        );
        assert_eq!(contents(&n), contents(&old));
        assert_eq!(*trace.borrow(), *old_trace.borrow());
        // A full snapshot predating activation retains the copied working mode.
        let trace2 = RefCell::new(Vec::new());
        let phase2 = Cell::new(0);
        let mut empty = blank(false, JoinComparison::Equal, &trace2, &phase2);
        let prior = empty.checkpoint();
        assert!(empty.enable_consuming_rows());
        populate(&mut empty);
        empty.restore(prior);
        assert!(empty.matches().next().is_none());
        assert!(empty.left.rows.is_copied());
    }
    #[test]
    fn copied_pair_join_payloads_projection_panics_nonempty_activation_and_custom_nodrop_fallback_keep_contract()
     {
        let trace = RefCell::new(Vec::new());
        let old_trace = RefCell::new(Vec::new());
        let phase = Cell::new(0);
        let old_phase = Cell::new(0);
        let mut n = blank(true, JoinComparison::Equal, &trace, &phase);
        let mut old = blank(false, JoinComparison::Equal, &old_trace, &old_phase);
        populate(&mut n);
        populate(&mut old);
        let base = contents(&n);
        for failing in [1, 2] {
            phase.set(failing);
            old_phase.set(failing);
            let c = TupleChange::put(IDS[0], (9, 0, 0, 9));
            assert!(
                catch_unwind(AssertUnwindSafe(|| if failing == 1 {
                    n.apply_left(c.clone())
                } else {
                    n.apply_right(c.clone())
                }))
                .is_err()
            );
            assert!(
                catch_unwind(AssertUnwindSafe(|| if failing == 1 {
                    old.apply_left(c.clone())
                } else {
                    old.apply_right(c.clone())
                }))
                .is_err()
            );
            assert!(!n.is_poisoned());
            assert_eq!(contents(&n), base);
            assert_eq!(contents(&n), contents(&old));
            phase.set(0);
            old_phase.set(0);
        }
        assert_eq!(*trace.borrow(), *old_trace.borrow());
        assert!(!old.enable_consuming_rows());
        assert!(!old.left.rows.is_copied());
        #[derive(Clone)]
        struct Custom(Value);
        assert!(!std::mem::needs_drop::<Custom>());
        let mut unsupported: JoinNode<'_, Id, Id, Value, Custom, Key> =
            JoinNode::new(|v: &Value| (v.1, v.2), |v: &Custom| (v.0.1, v.0.2));
        assert!(unsupported.bind_primitive_graph_journals());
        assert!(!unsupported.enable_consuming_rows());
        assert!(!unsupported.left.rows.is_copied());
        assert!(!unsupported.right.rows.is_copied());
    }
}
