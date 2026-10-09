//! Compiled immutable relationships between stable input identities.
use super::*;
type LinkId<E, I, J> = ((E, I), J);
/// Output identity contains relationship ID and both endpoint IDs.
pub type FixedLinkedStreamPort<E, I, J, U> = StreamPort<LinkId<E, I, J>, U>;
type Predicate<'a, E, L, R> = dyn Fn(&E, &L, &R) -> bool + 'a;
type Projection<'a, E, L, R, U> = dyn Fn(&E, &L, &R) -> U + 'a;
struct FixedLinks<'a, E, I, J, L, R, U> {
    links: Vec<LinkId<E, I, J>>,
    by_left: BTreeMap<I, Vec<usize>>,
    by_right: BTreeMap<J, Vec<usize>>,
    left: SnapshotMap<I, L>,
    right: SnapshotMap<J, R>,
    predicate: Box<Predicate<'a, E, L, R>>,
    projection: Box<Projection<'a, E, L, R, U>>,
}
struct LinkCheckpoint {
    left: usize,
    right: usize,
}
fn latest<I: Ord + 'static, T: 'static>(
    inputs: &[(bool, Event)],
    side: bool,
) -> Vec<&TupleChange<I, T>> {
    let mut changes: Vec<_> = inputs
        .iter()
        .filter(|(s, _)| *s == side)
        .map(|(_, e)| typed::<I, T>(e))
        .collect();
    changes.sort_by(|a, b| a.id.cmp(&b.id));
    changes.dedup_by(|later, earlier| {
        if later.id == earlier.id {
            std::mem::swap(later, earlier);
            true
        } else {
            false
        }
    });
    changes
}
fn after<'a, I: Ord, T>(
    changes: &[&'a TupleChange<I, T>],
    id: &I,
    old: Option<&'a T>,
) -> Option<&'a T> {
    changes
        .binary_search_by(|c| c.id.cmp(id))
        .map_or(old, |i| changes[i].value.as_ref())
}
impl<
    E: Ord + Clone + 'static,
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    U: 'static,
    S: AdditiveScore,
> Node<S> for FixedLinks<'_, E, I, J, L, R, U>
{
    fn prefers_batch(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new((self.left.clone(), self.right.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        Box::new(LinkCheckpoint {
            left: self.left.begin_journal(),
            right: self.right.begin_journal(),
        })
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<LinkCheckpoint>()
            .expect("fixed-link checkpoint");
        self.left.commit_journal(saved.left);
        self.right.commit_journal(saved.right);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<LinkCheckpoint>() {
            Ok(saved) => {
                self.left.rollback_journal(saved.left);
                self.right.rollback_journal(saved.right);
            }
            Err(saved) => {
                let (left, right) = *saved
                    .downcast::<(SnapshotMap<I, L>, SnapshotMap<J, R>)>()
                    .expect("fixed-link snapshot");
                self.left.install_snapshot(left);
                self.right.install_snapshot(right);
            }
        }
    }
    fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        <Self as Node<S>>::apply_batch(self, &[(right, event.clone())], arithmetic)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let left = latest::<I, L>(inputs, false);
        let right = latest::<J, R>(inputs, true);
        let mut affected = Vec::new();
        for change in &left {
            if let Some(links) = self.by_left.get(&change.id) {
                affected.extend_from_slice(links);
            }
        }
        for change in &right {
            if let Some(links) = self.by_right.get(&change.id) {
                affected.extend_from_slice(links);
            }
        }
        affected.sort_unstable();
        affected.dedup();
        let mut out = Vec::with_capacity(affected.len());
        for index in affected {
            let ((edge, i), j) = &self.links[index];
            let old_l = self.left.get(i);
            let old_r = self.right.get(j);
            let value = after(&left, i, old_l)
                .zip(after(&right, j, old_r))
                .and_then(|(l, r)| {
                    (self.predicate)(edge, l, r).then(|| (self.projection)(edge, l, r))
                });
            let retract = value.is_none()
                && old_l
                    .zip(old_r)
                    .is_some_and(|(l, r)| (self.predicate)(edge, l, r));
            if value.is_some() || retract {
                out.push(TupleChange {
                    id: self.links[index].clone(),
                    value,
                });
            }
        }
        // Callbacks and output-ID cloning finish before installation. The graph's
        // row journals restore partial installation if an input clone panics.
        for change in left {
            if self.by_left.contains_key(&change.id) {
                if let Some(value) = &change.value {
                    self.left.insert(change.id.clone(), value.clone());
                } else {
                    self.left.remove(&change.id);
                }
            }
        }
        for change in right {
            if self.by_right.contains_key(&change.id) {
                if let Some(value) = &change.value {
                    self.right.insert(change.id.clone(), value.clone());
                } else {
                    self.right.remove(&change.id);
                }
            }
        }
        Ok(erase(out))
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Join endpoints connected by immutable `(relationship_id,left_id,right_id)`
    /// links. Relationship IDs must be unique; parallel links and self-links are
    /// allowed. Missing endpoints produce no output. Rebuild the graph to change
    /// links; use ordinary joins when relationships are changing problem facts.
    ///
    /// Each batch publishes one final change per affected link, ordered by link
    /// ID. Predicates run before projection; retractions recompute prior acceptance.
    /// Callbacks/IDs must be pure and stable. No expanded pair cache is retained;
    /// only linked input rows and O(links) immutable adjacency are stored. A row
    /// update visits its incident links, not the entire relationship list.
    /// Graph trials, saved candidates, activity rebuild and rollback apply normally.
    pub fn join_fixed_links_batched<
        E: Ord + Clone + 'static,
        I: Ord + Clone + 'static,
        J: Ord + Clone + 'static,
        L: Clone + 'static,
        R: Clone + 'static,
        U: 'static,
    >(
        &mut self,
        left: &StreamPort<I, L>,
        right: &StreamPort<J, R>,
        links: impl IntoIterator<Item = (E, I, J)>,
        predicate: impl Fn(&E, &L, &R) -> bool + 'a,
        projection: impl Fn(&E, &L, &R) -> U + 'a,
    ) -> Result<FixedLinkedStreamPort<E, I, J, U>, GraphError> {
        self.validate(left)?;
        self.validate(right)?;
        let mut links: Vec<_> = links.into_iter().map(|(e, i, j)| ((e, i), j)).collect();
        links.sort_by(|a, b| a.0.0.cmp(&b.0.0));
        if links.windows(2).any(|w| w[0].0.0 == w[1].0.0) {
            return Err(GraphError::Dependency(
                "duplicate fixed relationship ID".into(),
            ));
        }
        let mut by_left: BTreeMap<I, Vec<usize>> = BTreeMap::new();
        let mut by_right: BTreeMap<J, Vec<usize>> = BTreeMap::new();
        for (n, ((_, i), j)) in links.iter().enumerate() {
            by_left.entry(i.clone()).or_default().push(n);
            by_right.entry(j.clone()).or_default().push(n);
        }
        let out = self.push(FixedLinks {
            links,
            by_left,
            by_right,
            left: SnapshotMap::new(),
            right: SnapshotMap::new(),
            predicate: Box::new(predicate),
            projection: Box::new(projection),
        });
        self.connect(left.node, out.node, false);
        self.connect(right.node, out.node, true);
        Ok(out)
    }
}

#[cfg(test)]
mod cases {
    use super::*;
    use crate::{SimpleScore, StreamBatch};
    #[test]
    fn saved_candidate_and_nested_rebuild_preserve_fixed_relationships() {
        for pruning in [false, true] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let rows = b.source::<usize, i64>();
            let p = b
                .join_fixed_links_batched(
                    rows.port(),
                    rows.port(),
                    [(0, 0, 1), (1, 1, 1)],
                    |_, _, _| true,
                    |_, a, c| a + c,
                )
                .unwrap();
            let data = b.dataset(&p).unwrap();
            b.score("sum", &p, |&v| SimpleScore::new(v)).unwrap();
            let mut g = b.build();
            {
                let mut tx = g.transaction();
                tx.apply(&rows, TupleChange::put(0, 1)).unwrap();
                tx.apply(&rows, TupleChange::put(1, 2)).unwrap();
                tx.commit().unwrap();
            }
            let held = g.dataset_rows(&data).unwrap().clone();
            let saved;
            {
                let mut tx = g.transaction();
                let mut changes = StreamBatch::new();
                changes.push(&rows, TupleChange::put(0, 3)).unwrap();
                changes.push(&rows, TupleChange::put(1, 4)).unwrap();
                tx.apply_batch(changes).unwrap();
                tx.assert_consistent().unwrap();
                saved = tx.capture().unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::new(15));
            }
            assert_eq!(g.score(), SimpleScore::new(7));
            assert_eq!(g.dataset_rows(&data).unwrap(), &held);
            g.install(saved);
            assert_eq!(g.score(), SimpleScore::new(15));
            g.assert_consistent().unwrap();
            {
                let mut tx = g.transaction();
                tx.apply(&rows, TupleChange::retract(1)).unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
                tx.assert_consistent().unwrap();
            }
            assert_eq!(g.score(), SimpleScore::new(15));
        }
    }
}
