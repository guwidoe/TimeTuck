// Adapted Timefold Community TEST methods, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::cell::Cell;
use timetuck::*;
#[test]
fn without_preload_walks_each_pair_twice() {
    let count = Cell::new(0);
    let mut node = BatchedJoinNode::new(
        JoinEvaluation::DeferredByInputRole,
        |_: &String| (),
        |_: &String| (),
        |_, _| {
            count.set(count.get() + 1);
            true
        },
    );
    for i in 0..2 {
        node.enqueue_left(TupleChange::put(i, format!("L{i}")));
    }
    for i in 0..3 {
        node.enqueue_right(TupleChange::put(i, format!("R{i}")));
    }
    assert_eq!(count.get(), 0);
    let events = node.propagate();
    assert_eq!(count.get(), 2 * 2 * 3);
    // Original publication control: actual pair identities deduplicate output.
    assert_eq!(events.len(), 6);
    assert_eq!(node.rows().count(), 6);
}
#[test]
fn array_mode_is_always_ascending_regardless_of_reversed() {
    let mut map = ScalingMap::new();
    map.get_or_insert_with(3, || "three");
    map.get_or_insert_with(1, || "one");
    map.get_or_insert_with(2, || "two");
    assert!(map.is_array_based());
    assert_eq!(
        map.storage_traversal(false)
            .map(|(k, _)| *k)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        map.storage_traversal(true)
            .map(|(k, _)| *k)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
}
