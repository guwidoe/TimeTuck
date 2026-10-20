//! Original rejected-filter-cache controls; no upstream-method credit.
use super::*;
use std::{
    cell::{Cell, RefCell},
    fmt::Debug,
    rc::Rc,
};

#[derive(Clone, Copy)]
enum Route {
    Primitive,
    Fixed,
    Pair,
}

fn bind_route<I: Ord + Clone + 'static, T: Clone + 'static>(
    node: &mut FilterNode<'_, I, T>,
    route: Route,
) {
    match route {
        Route::Primitive => assert!(node.rows.is_primitive()),
        Route::Fixed => assert!(node.bind_fixed_inputs(3)),
        Route::Pair => assert!(node.bind_fixed_pairs(3, 65)),
    }
}
fn contents<I: Ord + Clone, T: Clone>(node: &FilterNode<'_, I, T>) -> Vec<(I, T)> {
    node.rows()
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect()
}
fn exercise<I: Ord + Copy + Debug + 'static>(ids: [I; 3], route: Route) {
    let calls = Cell::new(0);
    let old_calls = Cell::new(0);
    let mut node = FilterNode::new(|value: &i64| {
        calls.set(calls.get() + 1);
        *value > 0
    });
    let mut original = FilterNode::new(|value: &i64| {
        old_calls.set(old_calls.get() + 1);
        *value > 0
    });
    assert!(node.bind_primitive_inputs());
    // Use the same row storage with the original retained-rejection policy.
    assert!(original.rows.bind_primitive());
    for (id, value) in ids.into_iter().zip([1, 2, 3]) {
        assert_eq!(
            node.apply(TupleChange::put(id, value)),
            original.apply(TupleChange::put(id, value))
        );
    }
    bind_route(&mut node, route);
    bind_route(&mut original, route);
    let base = contents(&node);
    let outer = node.begin_checkpoint();
    let old_outer = original.begin_checkpoint();
    for (slot, value) in [
        (0, Some(-1)),
        (0, Some(-2)),
        (0, Some(5)),
        (1, Some(-3)),
        (1, None),
        (2, None),
        (2, Some(9)),
    ] {
        let change = TupleChange {
            id: ids[slot],
            value,
        };
        assert_eq!(node.apply(change.clone()), original.apply(change));
        assert_eq!(contents(&node), contents(&original));
        assert_eq!(calls.get(), old_calls.get());
        assert_eq!(
            node.row_iter().count(),
            node.rows().count(),
            "only accepted payloads are retained"
        );
    }
    let inner = node.begin_checkpoint();
    let old_inner = original.begin_checkpoint();
    assert_eq!(
        node.apply(TupleChange::put(ids[1], 11)),
        original.apply(TupleChange::put(ids[1], 11))
    );
    assert_eq!(
        node.apply(TupleChange::put(ids[0], -7)),
        original.apply(TupleChange::put(ids[0], -7))
    );
    let saved = node.capture_checkpoint(outer.as_ref());
    let old_saved = original.capture_checkpoint(old_outer.as_ref());
    let wanted = contents(&node);
    node.commit_checkpoint(inner);
    original.commit_checkpoint(old_inner);
    node.restore(outer);
    original.restore(old_outer);
    assert_eq!(contents(&node), base);
    let full = node.checkpoint();
    let old_full = original.checkpoint();
    let other = node.begin_checkpoint();
    let old_other = original.begin_checkpoint();
    assert_eq!(
        node.apply(TupleChange::put(ids[0], -20)),
        original.apply(TupleChange::put(ids[0], -20))
    );
    node.restore(other);
    original.restore(old_other);
    let install = node.begin_checkpoint();
    let old_install = original.begin_checkpoint();
    let before = calls.get();
    node.restore(saved);
    original.restore(old_saved);
    assert_eq!(contents(&node), wanted);
    assert_eq!(contents(&node), contents(&original));
    assert_eq!(node.row_iter().count(), node.rows().count());
    assert_eq!(calls.get(), before, "install never replays predicates");
    assert_eq!(calls.get(), old_calls.get());
    node.restore(install);
    original.restore(old_install);
    assert_eq!(contents(&node), base);
    node.clear();
    original.clear();
    node.restore(full);
    original.restore(old_full);
    assert_eq!(contents(&node), base);
    assert_eq!(contents(&node), contents(&original));
    assert_eq!(calls.get(), old_calls.get());
    assert!(!node.is_poisoned());
}
#[test]
fn rejected_elision_preserves_unbounded_primitive_outputs_nested_journals_and_saved_captures() {
    exercise([usize::MAX, 0, usize::MAX - 64], Route::Primitive);
    let nested: [((usize, usize), (usize, usize)); 3] = [
        ((usize::MAX, 0), (1, 2)),
        ((0, usize::MAX), (3, 4)),
        ((1, 2), (3, usize::MAX)),
    ];
    exercise(nested, Route::Primitive);
}
#[test]
fn rejected_elision_preserves_fixed_and_sparse_pair_captures_and_full_restores() {
    exercise([0usize, 1, 2], Route::Fixed);
    exercise([(0usize, 1usize), (0, 63), (2, 64)], Route::Pair);
}

thread_local! {
    static TRACE: RefCell<Vec<(&'static str, i64)>> = const { RefCell::new(Vec::new()) };
    static FAIL_CLONE: Cell<bool> = const { Cell::new(false) };
    static FAIL_PREDICATE: Cell<bool> = const { Cell::new(false) };
}
#[derive(Debug, PartialEq, Eq)]
struct Counted(i64);
impl Clone for Counted {
    fn clone(&self) -> Self {
        TRACE.with(|trace| trace.borrow_mut().push(("clone", self.0)));
        assert!(
            !FAIL_CLONE.with(|fail| fail.replace(false)),
            "output clone failure"
        );
        Self(self.0)
    }
}
fn predicate(value: &Counted) -> bool {
    TRACE.with(|trace| trace.borrow_mut().push(("predicate", value.0)));
    assert!(
        !FAIL_PREDICATE.with(|fail| fail.replace(false)),
        "predicate failure"
    );
    value.0 > 0
}
type Attempt<I> = (
    Result<Option<TupleChange<I, Counted>>, ()>,
    Vec<(&'static str, i64)>,
);
fn attempt<I: Ord + Clone>(
    node: &mut FilterNode<'_, I, Counted>,
    id: I,
    value: Option<i64>,
    predicate_failure: bool,
    clone_failure: bool,
) -> Attempt<I> {
    TRACE.with(|trace| trace.borrow_mut().clear());
    FAIL_PREDICATE.with(|fail| fail.set(predicate_failure));
    FAIL_CLONE.with(|fail| fail.set(clone_failure));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        node.apply(TupleChange {
            id,
            value: value.map(Counted),
        })
    }))
    .map_err(|_| ());
    FAIL_PREDICATE.with(|fail| fail.set(false));
    FAIL_CLONE.with(|fail| fail.set(false));
    (result, TRACE.with(|trace| trace.borrow().clone()))
}
fn counted_contents<I: Ord + Copy>(node: &FilterNode<'_, I, Counted>) -> Vec<(I, i64)> {
    node.rows().map(|(id, value)| (*id, value.0)).collect()
}
fn exercise_failures<I: Ord + Copy + Debug + 'static>(ids: [I; 3], route: Route) {
    assert!(!std::mem::needs_drop::<Counted>());
    let mut node = FilterNode::new(predicate);
    let mut original = FilterNode::new(predicate);
    assert!(node.bind_primitive_inputs());
    assert!(original.rows.bind_primitive());
    for id in ids {
        assert_eq!(
            attempt(&mut node, id, Some(1), false, false),
            attempt(&mut original, id, Some(1), false, false)
        );
    }
    bind_route(&mut node, route);
    bind_route(&mut original, route);
    for (value, predicate_failure, clone_failure) in [
        (Some(-1), false, false),
        (Some(-2), false, false),
        (Some(2), false, false),
        (Some(3), false, true),
        (Some(-4), true, false),
        (Some(4), false, false),
        (None, false, false),
        (None, false, false),
    ] {
        let before = counted_contents(&node);
        let got = attempt(&mut node, ids[0], value, predicate_failure, clone_failure);
        let expected = attempt(
            &mut original,
            ids[0],
            value,
            predicate_failure,
            clone_failure,
        );
        assert_eq!(got, expected);
        if predicate_failure || clone_failure {
            assert!(got.0.is_err());
            assert_eq!(counted_contents(&node), before);
        }
        assert!(
            !node.is_poisoned(),
            "prepublication failures remain retryable"
        );
        assert_eq!(counted_contents(&node), counted_contents(&original));
        assert_eq!(node.row_iter().count(), node.rows().count());
    }
}
#[test]
fn rejected_elision_keeps_predicate_output_clone_counts_order_and_preflight_failures() {
    exercise_failures([0usize, 1, 2], Route::Primitive);
    exercise_failures([0usize, 1, 2], Route::Fixed);
    exercise_failures([(0usize, 1usize), (0, 63), (2, 64)], Route::Pair);
}

fn invalid_put<I: Ord + Copy + Debug + 'static>(ids: [I; 3], invalid: I, route: Route) {
    let mut node = FilterNode::new(predicate);
    let mut original = FilterNode::new(predicate);
    assert!(node.bind_primitive_inputs());
    assert!(original.rows.bind_primitive());
    for id in ids {
        node.apply(TupleChange::put(id, Counted(1)));
        original.apply(TupleChange::put(id, Counted(1)));
    }
    bind_route(&mut node, route);
    bind_route(&mut original, route);
    let saved = node.checkpoint();
    let old_saved = original.checkpoint();
    let got = attempt(&mut node, invalid, Some(-1), false, false);
    assert_eq!(got, attempt(&mut original, invalid, Some(-1), false, false));
    assert!(
        got.0.is_err(),
        "rejected puts still validate the full universe"
    );
    assert_eq!(got.1, [("predicate", -1)]);
    assert!(node.is_poisoned());
    assert!(original.is_poisoned());
    node.restore(saved);
    original.restore(old_saved);
    assert_eq!(counted_contents(&node), counted_contents(&original));
    assert!(!node.is_poisoned());
    assert_eq!(
        attempt(&mut node, invalid, None, false, false),
        attempt(&mut original, invalid, None, false, false)
    );
    assert!(
        !node.is_poisoned(),
        "out-of-range retractions remain tolerant"
    );
}
#[test]
fn rejected_out_of_universe_puts_keep_original_validation_poison_and_recovery() {
    invalid_put([0usize, 1, 2], 3, Route::Fixed);
    invalid_put([(0usize, 1usize), (0, 63), (2, 64)], (3, 0), Route::Pair);
    invalid_put([(0usize, 1usize), (0, 63), (2, 64)], (0, 65), Route::Pair);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CustomKey(usize);
#[test]
fn standalone_custom_ids_drop_payloads_and_nonempty_binding_keep_retained_rejections() {
    let mut standalone = FilterNode::<usize, i64>::new(|_| false);
    standalone.apply(TupleChange::put(0, -1));
    assert_eq!(standalone.row_iter().count(), 1);
    assert!(
        standalone.bind_primitive_inputs(),
        "nonempty conversion still succeeds"
    );
    assert!(!standalone.elide_rejected);
    standalone.apply(TupleChange::put(1, -2));
    assert_eq!(standalone.row_iter().count(), 2);
    let saved = standalone.checkpoint();
    standalone.clear();
    standalone.restore(saved);
    assert_eq!(standalone.row_iter().count(), 2);
    assert_eq!(standalone.rows().count(), 0);

    let mut generic = FilterNode::<CustomKey, i64>::new(|_| false);
    assert!(!generic.bind_primitive_inputs());
    generic.apply(TupleChange::put(CustomKey(0), -1));
    assert!(!generic.elide_rejected);
    assert_eq!(generic.row_iter().count(), 1);

    let mut owned = FilterNode::<usize, Rc<usize>>::new(|_| false);
    assert!(!owned.bind_primitive_inputs());
    let value = Rc::new(7);
    let weak = Rc::downgrade(&value);
    owned.apply(TupleChange::put(0, value));
    let original = owned.begin_checkpoint();
    let saved = owned.capture_checkpoint(original.as_ref());
    owned.restore(original);
    assert_eq!(owned.rows().count(), 0);
    assert_eq!(owned.row_iter().count(), 1);
    owned.clear();
    assert!(
        weak.upgrade().is_some(),
        "saved rejected custom owner remains alive"
    );
    drop(saved);
    assert!(weak.upgrade().is_none());
}

#[test]
fn accepted_only_sparse_binding_validates_retained_rows_without_relaxing_fixed_contract() {
    let mut fixed = FilterNode::<usize, i64>::new(|v| *v > 0);
    assert!(fixed.bind_primitive_inputs());
    fixed.apply(TupleChange::put(0, 1));
    fixed.apply(TupleChange::put(1, -1));
    assert!(
        !fixed.bind_fixed_inputs(2),
        "fixed binding still requires the complete universe"
    );
    assert_eq!(fixed.row_iter().count(), 1);

    let mut pair = FilterNode::<(usize, usize), i64>::new(|v| *v > 0);
    assert!(pair.bind_primitive_inputs());
    pair.apply(TupleChange::put((0, 1), 1));
    pair.apply(TupleChange::put((2, 64), -1));
    assert!(
        !pair.bind_fixed_pairs(3, 65),
        "discarded rejections prevent proving a narrower universe"
    );
    assert_eq!(pair.row_iter().count(), 1);
    pair.apply(TupleChange::put((2, 64), 2));
    assert_eq!(pair.rows().count(), 2);
    assert_eq!(
        pair.apply(TupleChange::put((0, 1), -2)),
        Some(TupleChange::retract((0, 1)))
    );
    assert_eq!(pair.row_iter().count(), 1);
}

#[test]
fn elided_out_of_domain_rejections_cannot_invent_a_smaller_binding_universe() {
    let mut pair = FilterNode::<(usize, usize), i64>::new(|v| *v > 0);
    assert!(pair.bind_primitive_inputs());
    pair.apply(TupleChange::put((0, 1), 1));
    let saved = pair.begin_checkpoint();
    pair.apply(TupleChange::put((usize::MAX, usize::MAX), -1));
    assert!(!pair.bind_fixed_pairs(3, 65));
    pair.restore(saved);
    // Lost negative-domain evidence is conservatively sticky across rollback.
    assert!(!pair.bind_fixed_pairs(3, 65));
    assert_eq!(
        pair.apply(TupleChange::put((usize::MAX, usize::MAX), 2)),
        Some(TupleChange::put((usize::MAX, usize::MAX), 2))
    );
    let mut fixed = FilterNode::<usize, i64>::new(|v| *v > 0);
    assert!(fixed.bind_primitive_inputs());
    fixed.apply(TupleChange::put(0, 1));
    fixed.apply(TupleChange::put(usize::MAX, -1));
    assert!(!fixed.bind_fixed_inputs(1));
    assert_eq!(
        fixed.apply(TupleChange::put(usize::MAX, 3)),
        Some(TupleChange::put(usize::MAX, 3))
    );
}
