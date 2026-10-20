//! Original explicit integer-policy checks; no upstream method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;

type Wrapped = ConnectedRangeCollector<usize, i32, usize, WrappingIntegerRangeMetric>;
type Exact = ConnectedRangeCollector<usize, i32, usize>;

#[test]
fn selected_distances_preserve_signed_endpoint_order_and_connectivity() {
    let mut exact = Exact::default();
    let mut wrapped = Wrapped::default();
    let ends = [
        (0, i32::MIN, i32::MIN + 1, 0),
        (1, i32::MAX - 1, i32::MAX, 1),
    ];
    for row in ends {
        exact.insert(&row);
        wrapped.insert(&row);
    }
    let saved = wrapped.finish();
    assert_eq!(saved.ranges().len(), 2);
    assert_eq!(*saved.gaps()[0].length(), -3);
    assert_eq!(*exact.finish().gaps()[0].length(), 4_294_967_293);
    assert_eq!(*saved.gaps()[0].previous_range_end(), i32::MIN + 1);
    assert_eq!(*saved.gaps()[0].next_range_start(), i32::MAX - 1);
    let bridge = (2, i32::MIN + 1, i32::MAX - 1, 2);
    wrapped.insert(&bridge);
    exact.insert(&bridge);
    let joined = wrapped.finish();
    assert_eq!(joined.ranges().len(), 1);
    assert!(joined.gaps().is_empty());
    let range = &joined.ranges()[0];
    assert_eq!(
        (*range.start(), *range.end(), *range.length(), range.count()),
        (i32::MIN, i32::MAX, -1, 3)
    );
    assert_eq!(
        (
            range.minimum_overlap(),
            range.maximum_overlap(),
            range.has_overlap()
        ),
        (Some(1), Some(1), false)
    );
    assert_eq!(*exact.finish().ranges()[0].length(), 4_294_967_295);
    assert_eq!(saved.ranges().len(), 2);
    assert!(wrapped.remove(&bridge));
    assert_eq!(wrapped.finish(), saved);
}

#[test]
fn wrapped_observations_retain_captures_duplicates_and_failed_node_edits() {
    let mut node = CollectNode::<usize, (), (usize, i32, i32, usize), Wrapped>::new();
    for slot in 0..2 {
        node.apply(TupleChange {
            id: slot,
            value: Some(((), (0, i32::MIN, i32::MAX, 7))),
        });
    }
    let saved = node.groups().next().unwrap().1;
    assert_eq!(saved.ranges()[0].count(), 2);
    assert_eq!(saved.ranges()[0].maximum_overlap(), Some(2));
    assert_eq!(*saved.ranges()[0].length(), -1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| node.apply(TupleChange {
            id: 0,
            value: Some(((), (0, 3, 2, 7)))
        })))
        .is_err()
    );
    assert_eq!(node.groups().next().unwrap().1, saved);
    node.apply(TupleChange { id: 9, value: None });
    assert_eq!(node.groups().next().unwrap().1, saved);
    node.apply(TupleChange { id: 0, value: None });
    assert_eq!(node.groups().next().unwrap().1.ranges()[0].count(), 1);
    assert_eq!(saved.ranges()[0].count(), 2);
    node.apply(TupleChange { id: 1, value: None });
    assert_eq!(node.groups().count(), 0);
}

#[test]
fn wrapped_lengths_reach_real_scores_and_rollback_failed_transactions() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (usize, i32, i32, usize)>();
    let ranges = builder
        .group_collect_with::<_, _, _, _, Wrapped>(source.port(), |row| ((), *row))
        .unwrap();
    let handle = builder
        .score("lengths", &ranges, |(_, chain)| {
            SimpleScore::new(chain.ranges().iter().map(|r| i64::from(*r.length())).sum())
        })
        .unwrap();
    let mut graph = builder.build();
    let full = (0, i32::MIN, i32::MAX, 0);
    let mut transaction = graph.transaction();
    transaction
        .apply(
            &source,
            TupleChange {
                id: 0,
                value: Some(full),
            },
        )
        .unwrap();
    assert_eq!(transaction.score().unwrap(), SimpleScore::new(-1));
    transaction.commit().unwrap();
    let saved = graph.constraint_matches(&handle).unwrap().clone();
    {
        let mut transaction = graph.transaction();
        transaction
            .apply(
                &source,
                TupleChange {
                    id: 0,
                    value: Some((0, -3, 4, 0)),
                },
            )
            .unwrap();
        assert_eq!(transaction.score().unwrap(), SimpleScore::new(7));
    }
    assert_eq!(graph.constraint_matches(&handle).unwrap(), &saved);
    let mut transaction = graph.transaction();
    // Ordinary collector panics propagate; the transaction stays poisoned until
    // it is dropped, just as with the existing exact-metric collector.
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            transaction.apply(
                &source,
                TupleChange {
                    id: 0,
                    value: Some((0, 4, 3, 0)),
                },
            )
        }))
        .is_err()
    );
    assert!(transaction.score().is_err());
    drop(transaction);
    assert_eq!(graph.constraint_matches(&handle).unwrap(), &saved);
    graph.assert_consistent().unwrap();
}
