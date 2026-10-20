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
    assert!(!pairs.enable_consuming_rows());
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
