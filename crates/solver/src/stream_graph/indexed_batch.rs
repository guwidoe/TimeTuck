//! Batch-only publication using the compact indexed join state.
use super::*;
struct IndexedBatchJoin<'a, I, J, L, R, K>(JoinNode<'a, I, J, L, R, K>);
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore,
> Node<S> for IndexedBatchJoin<'_, I, J, L, R, K>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.0.checkpoint()
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.0.begin_journal_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.0.commit_journal_checkpoint(saved);
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.0.restore_checkpoint(saved);
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(if right {
            self.0.apply_right(typed::<J, R>(event).clone())
        } else {
            self.0.apply_left(typed::<I, L>(event).clone())
        }))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        Ok(erase(
            self.0.apply_batch(
                inputs
                    .iter()
                    .filter(|(r, _)| !r)
                    .map(|(_, e)| typed::<I, L>(e).clone()),
                inputs
                    .iter()
                    .filter(|(r, _)| *r)
                    .map(|(_, e)| typed::<J, R>(e).clone()),
            ),
        ))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Equality join publishing one final change per affected pair in a graph
    /// batch. Input rows/indexes are retained, but matched pairs are not duplicated
    /// in a join-owned cache. Observing this node enables batched score-director
    /// updates; direct transaction `apply` retains immediate event semantics.
    /// Use `join_batched` when the join itself needs a residual pair predicate.
    pub fn join_equal_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.join_compare_batched(left, right, JoinComparison::Equal, left_key, right_key)
    }
    /// Ordered-key join with final batch publication and compact input/index
    /// storage. Relation orientation is left versus right. Range traversal skips
    /// nonmatching keys; affected pair/output buffers can be quadratic for dense
    /// rounds. Intermediate input payloads are coalesced before projection.
    /// Graph checkpoint, activity rebuild, diagnostic replay and rollback apply
    /// normally. Pure/stable projections and keys are required.
    pub fn join_compare_batched<
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Result<JoinedStreamPort<I, J, L, R>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let out = self.push(IndexedBatchJoin(JoinNode::<I, J, L, R, K>::comparing(
            comparison, left_key, right_key,
        )));
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}

#[cfg(test)]
mod journal_tests {
    use super::*;
    use crate::StreamBatch;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    #[test]
    fn value_trials_saved_candidates_rebuilds_and_structural_changes_interoperate() {
        for pruning in [false, true] {
            let fail = Cell::new(false);
            let mut builder = StreamGraphBuilder::<crate::SimpleScore>::new();
            if pruning {
                builder = builder.with_activity_pruning();
            }
            let source = builder.source::<usize, (usize, i64)>();
            let joined = builder
                .join_equal_batched(source.port(), source.port(), |r| r.0, |r| r.0)
                .unwrap();
            let matches = builder.dataset(&joined).unwrap();
            builder
                .score("sum", &joined, |(a, b)| {
                    assert!(!fail.get(), "sink failure");
                    crate::SimpleScore::new(a.1 + b.1)
                })
                .unwrap();
            let mut graph = builder.build();
            let batch = |values: Vec<(usize, (usize, i64))>| {
                let mut b = StreamBatch::new();
                for (i, value) in values {
                    b.push(&source, TupleChange::put(i, value)).unwrap();
                }
                b
            };
            {
                let mut tx = graph.transaction();
                tx.apply_batch(batch((0..6).map(|i| (i, (i % 2, i as i64 + 1))).collect()))
                    .unwrap();
                tx.commit().unwrap();
            }
            assert_eq!(graph.score(), crate::SimpleScore::new(126));
            let held = graph.dataset_rows(&matches).unwrap().clone();
            let saved;
            let expected;
            {
                let mut tx = graph.transaction();
                tx.apply_batch(batch(vec![(0, (0, 10)), (1, (1, 11))]))
                    .unwrap();
                assert_eq!(tx.score().unwrap(), crate::SimpleScore::new(234));
                tx.assert_consistent().unwrap(); // Nested clear/rebuild under an outer value journal.
                tx.apply_batch(batch(vec![(2, (1, 20)), (0, (0, 12))]))
                    .unwrap();
                tx.assert_consistent().unwrap();
                expected = tx.score().unwrap();
                saved = tx.capture().unwrap();
            }
            assert_eq!(graph.score(), crate::SimpleScore::new(126));
            assert_eq!(graph.dataset_rows(&matches).unwrap(), &held);
            graph.install(saved);
            assert_eq!(graph.score(), expected);
            graph.assert_consistent().unwrap();
            let installed = graph.dataset_rows(&matches).unwrap().clone();
            fail.set(true);
            {
                let mut tx = graph.transaction();
                assert!(
                    catch_unwind(AssertUnwindSafe(
                        || tx.apply_batch(batch(vec![(0, (0, 88)), (1, (1, 99))]))
                    ))
                    .is_err()
                );
                assert!(tx.score().is_err());
            }
            fail.set(false);
            assert_eq!(graph.score(), expected);
            assert_eq!(graph.dataset_rows(&matches).unwrap(), &installed);
            graph.assert_consistent().unwrap();
            for round in 0..12 {
                let before = graph.score();
                let mut tx = graph.transaction();
                tx.apply_batch(batch(vec![(0, (0, round)), (1, (1, round + 1))]))
                    .unwrap();
                if round % 2 == 0 {
                    tx.commit().unwrap();
                } else {
                    drop(tx);
                    assert_eq!(graph.score(), before);
                }
            }
            graph.assert_consistent().unwrap();
        }
    }
}
