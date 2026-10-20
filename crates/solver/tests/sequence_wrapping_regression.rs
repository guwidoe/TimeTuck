//! Original explicit wrapping-distance controls; no upstream-method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::{
    CollectNode, IntegerSequenceMetric, ReversibleCollector, SequenceChain, SequenceCollector,
    TupleChange, WrappingIntegerSequenceMetric,
};

type Wrapped<const STEP: i32 = 1> =
    SequenceCollector<usize, i32, &'static str, WrappingIntegerSequenceMetric<STEP>>;
type WrappedChain = SequenceChain<usize, i32, &'static str, i32>;

#[test]
fn exact_i32_default_and_explicit_wrapping_keep_distinct_extreme_break_lengths() {
    let mut exact = SequenceCollector::<usize, i32, &'static str>::default();
    let mut wrapped = Wrapped::<1>::default();
    // Distinct identities at one index retain ascending logical-ID order.
    for row in [
        (9, i32::MAX, "right"),
        (7, i32::MIN, "second"),
        (2, i32::MIN, "first"),
    ] {
        exact.insert(&row);
        wrapped.insert(&row);
    }
    let exact = exact.finish();
    let wrapped = wrapped.finish();
    assert_eq!(exact.sequences().len(), 2);
    assert_eq!(wrapped.sequences().len(), 2);
    assert_eq!(*exact.first_break().unwrap().length(), 4_294_967_295i128);
    assert_eq!(*wrapped.first_break().unwrap().length(), -1);
    let first = wrapped.first_sequence().unwrap();
    assert_eq!(
        first.items().iter().map(|item| item.id).collect::<Vec<_>>(),
        [2, 7]
    );
    assert_eq!(first.count(), 2);
    assert_eq!(*first.length(), 1);
    assert!(first.is_first());
    assert!(!first.is_last());
    assert_eq!(first.previous_break_index(), None);
    assert_eq!(first.next_break_index(), Some(0));
    let last = wrapped.last_sequence().unwrap();
    assert_eq!(last.first_item().id, 9);
    assert_eq!(*last.length(), 1);
    assert!(!last.is_first());
    assert!(last.is_last());
    assert_eq!(last.previous_break_index(), Some(0));
    assert_eq!(last.next_break_index(), None);
    let gap = wrapped.first_break().unwrap();
    assert!(gap.is_first());
    assert!(gap.is_last());
    assert_eq!(gap.previous_sequence_end().id, 7);
    assert_eq!(gap.next_sequence_start().id, 9);
    assert_eq!(gap.previous_sequence_end().value, "second");
    assert_eq!(gap.next_sequence_start().value, "right");
    assert_eq!(wrapped.last_break(), wrapped.first_break());
}

#[test]
fn large_positive_step_wraps_endpoint_distance_and_length_while_exact_stays_widened() {
    let mut wrapped = Wrapped::<1_500_000_000>::default();
    let mut exact = SequenceCollector::<
        usize,
        i32,
        &'static str,
        IntegerSequenceMetric<1_500_000_000>,
    >::default();
    for row in [
        (0, -1_000_000_000, "left"),
        (1, 0, "middle"),
        (2, 1_000_000_000, "right"),
    ] {
        wrapped.insert(&row);
        exact.insert(&row);
    }
    let chain = wrapped.finish();
    assert_eq!(chain.sequences().len(), 1);
    assert_eq!(chain.first_sequence().unwrap().count(), 3);
    assert_eq!(*chain.first_sequence().unwrap().length(), -794_967_296);
    assert!(chain.breaks().is_empty());
    assert_eq!(
        *exact.finish().first_sequence().unwrap().length(),
        3_500_000_000i128
    );
    let mut endpoint_wrapped = Wrapped::<1_500_000_000>::default();
    for row in [
        (0, -1_500_000_000, "left"),
        (1, 0, "middle"),
        (2, 1_500_000_000, "right"),
    ] {
        endpoint_wrapped.insert(&row);
    }
    let chain = endpoint_wrapped.finish();
    assert_eq!(chain.sequences().len(), 1);
    assert_eq!(chain.first_sequence().unwrap().count(), 3);
    assert_eq!(*chain.first_sequence().unwrap().length(), 205_032_704);
    assert!(chain.breaks().is_empty());
    let mut widest = Wrapped::<{ i32::MAX }>::default();
    widest.insert(&(0, 0, "left"));
    assert_eq!(
        *widest.finish().first_sequence().unwrap().length(),
        i32::MAX
    );
    widest.insert(&(1, i32::MAX, "right"));
    assert_eq!(*widest.finish().first_sequence().unwrap().length(), -2);
}

#[test]
fn nonpositive_wrapping_steps_are_rejected_by_the_existing_constructor() {
    assert!(catch_unwind(Wrapped::<0>::default).is_err());
    assert!(catch_unwind(Wrapped::<-1>::default).is_err());
    assert!(catch_unwind(Wrapped::<{ i32::MIN }>::default).is_err());
    assert!(
        catch_unwind(SequenceCollector::<usize, i32, (), IntegerSequenceMetric<0>>::default)
            .is_err()
    );
    assert!(Wrapped::<1>::default().finish().sequences().is_empty());
}

#[test]
fn wrapping_collect_node_retains_snapshots_capture_multiplicities_and_failed_edit_atomicity() {
    type Node = CollectNode<usize, (), (usize, i32, &'static str), Wrapped>;
    fn output(node: &Node) -> WrappedChain {
        node.groups()
            .next()
            .map_or_else(|| Wrapped::<1>::default().finish(), |(_, chain)| chain)
    }
    let mut node = Node::new();
    for (slot, row) in [
        (0, (10, i32::MIN, "left")),
        (1, (10, i32::MIN, "left")),
        (2, (20, i32::MAX, "right")),
        (3, (5, i32::MIN, "earlier")),
    ] {
        node.apply(TupleChange::put(slot, ((), row)));
    }
    let saved = output(&node);
    assert_eq!(
        saved
            .first_sequence()
            .unwrap()
            .items()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [5, 10]
    );
    assert_eq!(*saved.first_break().unwrap().length(), -1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            // One other occurrence of identity 10 remains: changing its captured
            // index/value is invalid and must not publish the staged removal.
            node.apply(TupleChange::put(0, ((), (10, i32::MIN + 1, "changed"))));
        }))
        .is_err()
    );
    assert_eq!(output(&node), saved);
    assert!(node.apply(TupleChange::retract(99)).is_empty());
    assert_eq!(output(&node), saved);
    node.apply(TupleChange::retract(1));
    assert_eq!(output(&node), saved);
    node.apply(TupleChange::put(0, ((), (10, i32::MIN + 1, "left"))));
    let changed = output(&node);
    assert_eq!(changed.first_sequence().unwrap().count(), 2);
    assert_eq!(*changed.first_sequence().unwrap().length(), 2);
    assert_eq!(*changed.first_break().unwrap().length(), -2);
    assert_eq!(*saved.first_sequence().unwrap().length(), 1);
    assert_eq!(*saved.first_break().unwrap().length(), -1);
    let mut collector = Wrapped::<1>::default();
    collector.insert(&(10, i32::MIN, "left"));
    let before_missing = collector.finish();
    assert!(!collector.replace(&(99, 0, "missing"), &(10, i32::MAX, "changed")));
    assert_eq!(collector.finish(), before_missing);
    // Removal resolves the captured logical ID, ignoring the supplied fields.
    assert!(collector.remove(&(10, i32::MAX, "ignored")));
    assert!(collector.finish().sequences().is_empty());
    for slot in [0, 3, 2] {
        node.apply(TupleChange::retract(slot));
    }
    assert_eq!(node.groups().count(), 0);
    assert_eq!(saved.sequences().len(), 2);
}
