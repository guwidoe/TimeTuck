//! Readable terminal datasets using the graph's ordinary transaction journal.
use super::*;
use crate::NeighborhoodDataset;

/// One captured output port. Repeated builder.dataset calls on that same port
/// share a node. Handles cannot write rows and reject use with a foreign graph.
pub struct StreamDataset<I, T>(StreamPort<I, T>);
impl<I, T> Clone for StreamDataset<I, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<I, T> PartialEq for StreamDataset<I, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.node == other.0.node && Rc::ptr_eq(&self.0.owner, &other.0.owner)
    }
}
impl<I, T> Eq for StreamDataset<I, T> {}
impl<I, T> fmt::Debug for StreamDataset<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamDataset")
            .field("node", &self.0.node)
            .finish_non_exhaustive()
    }
}
struct DatasetNode<I, T>(SnapshotMap<I, T>);
impl<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static, S: AdditiveScore> Node<S>
    for DatasetNode<I, T>
{
    fn clear(&mut self) {
        self.0.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.0.clone())
    }
    fn restore(&mut self, c: Box<dyn Any>) {
        self.0 = *c.downcast().expect("dataset checkpoint type");
    }
    fn dataset_rows(&self) -> Option<&dyn Any> {
        Some(&self.0)
    }
    fn scores_match(&self, checkpoint: &dyn Any) -> bool {
        checkpoint
            .downcast_ref::<SnapshotMap<I, T>>()
            .is_some_and(|old| old == &self.0)
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        if let Some(value) = &change.value {
            self.0.insert(change.id.clone(), value.clone());
        } else {
            self.0.remove(&change.id);
        }
        Ok(Vec::new())
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Capture any typed output without adding a scoring constraint. Dataset
    /// branches stay live even if all adjacent weighted constraints are disabled.
    /// Tuple identity preserves duplicates. Reusing the same port shares capture;
    /// equivalent independently built streams remain distinct. PartialEq compares
    /// payloads during diagnostic rebuilds and must be stable/reflexive.
    ///
    /// ```
    /// use timetuck::*;
    /// let mut builder=StreamGraphBuilder::<SimpleScore>::new();
    /// let source=builder.source::<usize,i32>();
    /// let positive=builder.filter(source.port(),|v|*v>0)?;
    /// let dataset=builder.dataset(&positive)?;
    /// assert_eq!(dataset,builder.dataset(&positive)?);
    /// let mut graph=builder.build();
    /// let mut trial=graph.transaction();
    /// trial.apply(&source,TupleChange::put(0,3))?;
    /// let held=trial.graph()?.dataset_snapshot(&dataset)?;
    /// drop(trial);
    /// assert!(graph.dataset_rows(&dataset)?.is_empty());
    /// assert_eq!(held.iter().copied().collect::<Vec<_>>(),vec![3]);
    /// # Ok::<(),GraphError>(())
    /// ```
    pub fn dataset<I: Ord + Clone + 'static, T: Clone + PartialEq + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
    ) -> Result<StreamDataset<I, T>, GraphError> {
        self.validate(input)?;
        if let Some(&node) = self.datasets.get(&input.node) {
            return Ok(StreamDataset(StreamPort {
                owner: self.owner.clone(),
                node,
                marker: PhantomData,
            }));
        }
        let out = self.push(DatasetNode::<I, T>(SnapshotMap::new()));
        self.connect(input.node, out.node, false);
        self.datasets.insert(input.node, out.node);
        Ok(StreamDataset(out))
    }
}
impl<S: AdditiveScore> StreamGraph<'_, S> {
    /// Borrow current captured rows. Clone the persistent map for an inexpensive
    /// immutable snapshot; later updates copy only modified tree paths.
    pub fn dataset_rows<I: Ord + 'static, T: 'static>(
        &self,
        dataset: &StreamDataset<I, T>,
    ) -> Result<&SnapshotMap<I, T>, GraphError> {
        if !Rc::ptr_eq(&self.owner, &dataset.0.owner) {
            return Err(GraphError::ForeignPort);
        }
        Ok(self.nodes[dataset.0.node]
            .dataset_rows()
            .expect("dataset handle")
            .downcast_ref()
            .expect("dataset row type"))
    }
    /// Materialize current row values for random access and neighborhood sampling.
    /// This clones values once; the returned dataset is independent of later graph
    /// changes. For inspection without materialization, prefer dataset_rows.
    pub fn dataset_snapshot<I: Ord + 'static, T: Clone + 'static>(
        &self,
        dataset: &StreamDataset<I, T>,
    ) -> Result<NeighborhoodDataset<T>, GraphError> {
        Ok(NeighborhoodDataset::new(
            self.dataset_rows(dataset)?.values().cloned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimpleScore, TupleChange};
    #[test]
    fn saved_candidate_installation_restores_datasets_without_replaying_callbacks() {
        let calls = std::cell::Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, i32>();
        let mapped = b
            .map(s.port(), |v| {
                calls.set(calls.get() + 1);
                v * 2
            })
            .unwrap();
        let d = b.dataset(&mapped).unwrap();
        let mut g = b.build();
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, 3)).unwrap();
        let saved = t.capture().unwrap();
        drop(t);
        assert!(g.dataset_rows(&d).unwrap().is_empty());
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, 8)).unwrap();
        t.commit().unwrap();
        let count = calls.get();
        g.install(saved);
        assert_eq!(calls.get(), count);
        assert_eq!(g.dataset_rows(&d).unwrap()[&0], 6);
        assert_eq!(g.source_rows(&s).unwrap()[&0], 3);
        g.assert_consistent().unwrap();
    }
}
