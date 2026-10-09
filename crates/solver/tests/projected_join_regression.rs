//! Original fused join/projection semantics and failure-recovery regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

struct Row(i32, Rc<Cell<usize>>);
impl Clone for Row {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
#[test]
fn projection_borrows_accepted_pairs_without_cloning_opposite_rows() {
    let clones = Rc::new(Cell::new(0));
    let projections = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, i32>();
    let right = b.source::<usize, Row>();
    let joined = b
        .join_compare_map_batched(
            left.port(),
            right.port(),
            JoinComparison::Equal,
            |_| (),
            |_| (),
            |l, r| *l >= r.0,
            |l, r| {
                projections.set(projections.get() + 1);
                i64::from(*l + r.0)
            },
        )
        .unwrap();
    let data = b.dataset(&joined).unwrap();
    b.score("sum", &joined, |&x| SimpleScore::new(x)).unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&left, TupleChange::put(0, 32)).unwrap();
    for i in 0..32 {
        batch
            .push(&right, TupleChange::put(i, Row(i as i32, clones.clone())))
            .unwrap();
    }
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(projections.get(), 32);
    clones.set(0);
    projections.set(0);
    {
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 7)).unwrap();
        assert_eq!(projections.get(), 8); // Old accepted pairs do not invoke the mapper.
        assert_eq!(clones.get(), 0);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(84));
        assert_eq!(tx.graph().unwrap().dataset_rows(&data).unwrap().len(), 8);
    }
    assert_eq!(g.dataset_rows(&data).unwrap().len(), 32);
    assert_eq!(clones.get(), 0);
    projections.set(0);
    {
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::retract(0)).unwrap();
        assert_eq!(projections.get(), 0);
        assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
        tx.commit().unwrap();
    }
    g.assert_consistent().unwrap();
}

#[test]
fn mapper_failure_rolls_back_prepared_pairs_and_allows_retry() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let fail = Cell::new(false);
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let left = b.source::<usize, i32>();
        let right = b.source::<usize, i32>();
        let joined = b
            .join_compare_map_batched(
                left.port(),
                right.port(),
                comparison,
                |_| 0,
                |_| match comparison {
                    JoinComparison::LessThan => 1,
                    JoinComparison::GreaterThan => -1,
                    _ => 0,
                },
                |_, _| true,
                |l, r| {
                    calls.set(calls.get() + 1);
                    assert!(!fail.get() || calls.get() != 2, "mapper failure");
                    l + r
                },
            )
            .unwrap();
        let data = b.dataset(&joined).unwrap();
        b.score("sum", &joined, |&x| SimpleScore::new(i64::from(x)))
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&left, TupleChange::put(0, 1)).unwrap();
            for i in 0..3 {
                tx.apply(&right, TupleChange::put(i, i as i32)).unwrap();
            }
            tx.commit().unwrap();
        }
        let before = g.dataset_rows(&data).unwrap().clone();
        calls.set(0);
        fail.set(true);
        {
            let mut tx = g.transaction();
            assert!(
                catch_unwind(AssertUnwindSafe(|| tx.apply(&left, TupleChange::put(0, 10))))
                    .is_err()
            );
        }
        fail.set(false);
        assert_eq!(g.dataset_rows(&data).unwrap(), &before);
        assert_eq!(g.score(), SimpleScore::new(6));
        {
            let mut tx = g.transaction();
            tx.apply(&left, TupleChange::put(0, 10)).unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(33));
            tx.commit().unwrap();
        }
        g.assert_consistent().unwrap();
    }
}

#[test]
fn self_join_projects_one_final_pair_and_rejects_foreign_ports() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let rows = b.source::<usize, i32>();
    let joined = b
        .join_compare_map_batched(
            rows.port(),
            rows.port(),
            JoinComparison::Equal,
            |_| (),
            |_| (),
            |l, r| l < r,
            |l, r| {
                calls.set(calls.get() + 1);
                l + r
            },
        )
        .unwrap();
    let data = b.dataset(&joined).unwrap();
    b.score("sum", &joined, |&v| SimpleScore::new(i64::from(v)))
        .unwrap();
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let other = foreign.source::<usize, i32>();
    assert!(
        b.join_compare_map_batched(
            rows.port(),
            other.port(),
            JoinComparison::Equal,
            |_| (),
            |_| (),
            |_, _| true,
            |_, _| 0
        )
        .is_err()
    );
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&rows, TupleChange::put(0, 0)).unwrap();
    batch.push(&rows, TupleChange::put(1, 1)).unwrap();
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(calls.get(), 1);
    calls.set(0);
    let mut batch = StreamBatch::new();
    batch.push(&rows, TupleChange::put(0, 3)).unwrap();
    batch.push(&rows, TupleChange::put(1, 2)).unwrap();
    {
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(5));
        assert_eq!(
            tx.graph()
                .unwrap()
                .dataset_rows(&data)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![(1, 0)]
        );
    }
    assert_eq!(
        g.dataset_rows(&data)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![(0, 1)]
    );
    g.assert_consistent().unwrap();
}
