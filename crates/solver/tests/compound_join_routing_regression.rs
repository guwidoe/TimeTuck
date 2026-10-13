// Original native routing/lifecycle controls; no upstream method credit.
use std::cell::Cell;
use timetuck::*;

#[test]
fn routing_query_does_not_read_rows_and_normalizes_trailing_equalities() {
    let calls = Cell::new(0);
    let key = JoinProjection::new(|value: &i32| {
        calls.set(calls.get() + 1);
        *value
    });
    let empty = JoinConditions::<i32, i32>::new([]);
    assert!(!empty.uses_equality_partitions());
    let ordered = JoinConditions::new([JoinCondition::new(JoinComparison::LessThan, &key, &key)]);
    assert!(!ordered.uses_equality_partitions());
    let trailing = ordered.and(JoinCondition::new(JoinComparison::Equal, &key, &key));
    assert!(trailing.uses_equality_partitions());
    assert!(trailing.equals_first().uses_equality_partitions());
    assert_eq!(calls.get(), 0);
}

#[test]
fn empty_and_conflicting_conditions_keep_cartesian_and_conjunction_semantics() {
    for conflicting in [false, true] {
        let age = JoinProjection::new(|value: &i32| *value);
        let conditions = if conflicting {
            JoinConditions::new([
                JoinCondition::new(JoinComparison::LessThan, &age, &age),
                JoinCondition::new(JoinComparison::GreaterThan, &age, &age),
            ])
        } else {
            JoinConditions::new([])
        };
        assert!(!conditions.uses_equality_partitions());
        for pruning in [false, true] {
            let mut builder = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                builder = builder.with_activity_pruning();
            }
            let left = builder.source::<usize, i32>();
            let right = builder.source::<usize, i32>();
            let joined = conditions
                .apply(&mut builder, left.port(), right.port())
                .unwrap();
            let shared = conditions
                .apply(&mut builder, left.port(), right.port())
                .unwrap();
            assert!(joined.same_node(&shared));
            let count = builder
                .score("pairs", &joined, |_| SimpleScore::new(1))
                .unwrap();
            let mut graph = builder.build();
            let mut batch = StreamBatch::new();
            batch.push(&left, TupleChange::put(0, 10)).unwrap();
            batch.push(&left, TupleChange::put(1, 20)).unwrap();
            batch.push(&right, TupleChange::put(0, 30)).unwrap();
            batch.push(&right, TupleChange::put(1, 40)).unwrap();
            let mut tx = graph.transaction();
            tx.apply_batch(batch).unwrap();
            tx.commit().unwrap();
            let expected = if conflicting { 0 } else { 4 };
            assert_eq!(graph.score(), SimpleScore::new(expected));
            assert_eq!(
                graph.constraint_matches(&count).unwrap().len(),
                expected as usize
            );
            {
                let mut tx = graph.transaction();
                tx.apply(&left, TupleChange::retract(0)).unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::new(expected / 2));
            }
            assert_eq!(graph.score(), SimpleScore::new(expected));
            graph.assert_consistent().unwrap();
        }
    }
}
