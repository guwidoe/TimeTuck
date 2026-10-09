//! Compiled immutable relationships between stable input identities.
use super::fixed_slots::{SlotSnapshot, Slots};
use super::*;
use std::collections::BTreeSet;
type LinkId<E, I, J> = ((E, I), J);
/// Output identity contains relationship ID and both endpoint IDs.
pub type FixedLinkedStreamPort<E, I, J, U> = StreamPort<LinkId<E, I, J>, U>;
type Predicate<'a, E, L, R> = dyn Fn(&E, &L, &R) -> bool + 'a;
type Projection<'a, E, L, R, U> = dyn Fn(&E, &L, &R) -> U + 'a;
struct FixedLinks<'a, E, I, J, L, R, U> {
    links: Vec<LinkId<E, I, J>>,
    endpoints: Vec<(usize, usize)>,
    by_left: BTreeMap<I, usize>,
    by_right: BTreeMap<J, usize>,
    left_links: Vec<Vec<usize>>,
    right_links: Vec<Vec<usize>>,
    left: Slots<L>,
    right: Slots<R>,
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
fn after<'a, T>(
    changes: &[(usize, Option<&'a T>)],
    slot: usize,
    old: Option<&'a T>,
) -> Option<&'a T> {
    changes
        .binary_search_by_key(&slot, |c| c.0)
        .map_or(old, |i| changes[i].1)
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
        Box::new((self.left.snapshot(), self.right.snapshot()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        Box::new(LinkCheckpoint {
            left: self.left.begin(),
            right: self.right.begin(),
        })
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<LinkCheckpoint>()
            .expect("fixed-link checkpoint");
        self.left.commit(saved.left);
        self.right.commit(saved.right);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<LinkCheckpoint>() {
            Ok(saved) => {
                self.left.rollback(saved.left);
                self.right.rollback(saved.right);
            }
            Err(saved) => {
                let (left, right) = *saved
                    .downcast::<(SlotSnapshot<L>, SlotSnapshot<R>)>()
                    .expect("fixed-link snapshot");
                self.left.install(left);
                self.right.install(right);
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
        let left: Vec<_> = latest::<I, L>(inputs, false)
            .into_iter()
            .filter_map(|c| self.by_left.get(&c.id).map(|&i| (i, c.value.as_ref())))
            .collect();
        let right: Vec<_> = latest::<J, R>(inputs, true)
            .into_iter()
            .filter_map(|c| self.by_right.get(&c.id).map(|&i| (i, c.value.as_ref())))
            .collect();
        let mut affected = Vec::new();
        for &(i, _) in &left {
            affected.extend_from_slice(&self.left_links[i]);
        }
        for &(j, _) in &right {
            affected.extend_from_slice(&self.right_links[j]);
        }
        affected.sort_unstable();
        affected.dedup();
        let mut out = Vec::with_capacity(affected.len());
        for index in affected {
            let edge = &self.links[index].0.0;
            let (i, j) = self.endpoints[index];
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
        for (index, value) in left {
            self.left.set(index, value.cloned());
        }
        for (index, value) in right {
            self.right.set(index, value.cloned());
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
    /// Endpoints use compiled dense slots with move-based undo; saved candidates
    /// copy O(linked endpoints) shared row pointers without cloning payloads.
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
        // Slots follow key order so coalesced source changes remain sorted by slot.
        let left_keys: BTreeSet<_> = links.iter().map(|((_, i), _)| i.clone()).collect();
        let right_keys: BTreeSet<_> = links.iter().map(|(_, j)| j.clone()).collect();
        let by_left: BTreeMap<_, _> = left_keys
            .into_iter()
            .enumerate()
            .map(|(n, i)| (i, n))
            .collect();
        let by_right: BTreeMap<_, _> = right_keys
            .into_iter()
            .enumerate()
            .map(|(n, j)| (j, n))
            .collect();
        let mut left_links = vec![Vec::new(); by_left.len()];
        let mut right_links = vec![Vec::new(); by_right.len()];
        let endpoints: Vec<_> = links
            .iter()
            .enumerate()
            .map(|(n, ((_, i), j))| {
                let (i, j) = (by_left[i], by_right[j]);
                left_links[i].push(n);
                right_links[j].push(n);
                (i, j)
            })
            .collect();
        let out = self.push(FixedLinks {
            left: Slots::new(by_left.len()),
            right: Slots::new(by_right.len()),
            links,
            endpoints,
            by_left,
            by_right,
            left_links,
            right_links,
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
            b.score_fixed("sum", &p, [((0, 0), 1), ((1, 1), 1)], |&v| {
                SimpleScore::new(v)
            })
            .unwrap();
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
