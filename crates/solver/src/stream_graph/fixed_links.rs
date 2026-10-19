//! Compiled immutable relationships between stable input identities.
mod shared_inputs;
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
    direct_left: bool,
    direct_right: bool,
    left_links: Vec<Vec<usize>>,
    right_links: Vec<Vec<usize>>,
    left: Slots<L>,
    right: Slots<R>,
    checkpoint_box: CheckpointBox<LinkCheckpoint>,
    // Batch scratch holds only input ordinals and immutable relationship slots.
    // Retain capacities across trials; snapshots need only logical row state.
    left_changes: Vec<(usize, usize)>,
    right_changes: Vec<(usize, usize)>,
    affected: Vec<usize>,
    // Current input positions by immutable endpoint slot. Clear only the slots
    // recorded by the preceding successful indexing pass before reusing changes.
    left_inputs: Vec<usize>,
    right_inputs: Vec<usize>,
    inputs_indexed: bool,
    predicate: Box<Predicate<'a, E, L, R>>,
    projection: Box<Projection<'a, E, L, R, U>>,
}
struct LinkCheckpoint {
    left: usize,
    right: usize,
}
// Only primitive integer identities can use their value as an ordinal. Keep
// user-defined key comparison and ownership operations on the original path.
fn contiguous_slots<I: 'static>(slots: &BTreeMap<I, usize>) -> bool {
    std::any::TypeId::of::<I>() == std::any::TypeId::of::<usize>()
        && !slots.is_empty()
        && slots.iter().enumerate().all(|(ordinal, (id, &slot))| {
            (id as &dyn Any).downcast_ref::<usize>() == Some(&ordinal) && slot == ordinal
        })
}
fn latest_slots<I: Ord + 'static, T: 'static>(
    inputs: &[(bool, Event)],
    side: bool,
    slots: &BTreeMap<I, usize>,
    direct: bool,
    changes: &mut Vec<(usize, usize)>,
) {
    changes.clear();
    if std::any::TypeId::of::<I>() == std::any::TypeId::of::<usize>() {
        // Cache primitive IDs once instead of downcasting events in sort/dedup
        // comparisons. Input ordinals break ties and preserve the last update.
        changes.extend(
            inputs
                .iter()
                .enumerate()
                .filter(|(_, (s, _))| *s == side)
                .map(|(index, (_, event))| {
                    let id = (&typed::<I, T>(event).id as &dyn Any)
                        .downcast_ref::<usize>()
                        .expect("primitive endpoint ID");
                    (*id, index)
                }),
        );
        changes.sort_unstable();
        changes.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                std::mem::swap(later, earlier);
                true
            } else {
                false
            }
        });
        if direct {
            changes.retain(|(slot, _)| *slot < slots.len());
            return;
        }
    } else {
        changes.extend(
            inputs
                .iter()
                .enumerate()
                .filter_map(|(index, (s, _))| (*s == side).then_some((0, index))),
        );
        changes.sort_by(|a, b| {
            typed::<I, T>(&inputs[a.1].1)
                .id
                .cmp(&typed::<I, T>(&inputs[b.1].1).id)
        });
        changes.dedup_by(|later, earlier| {
            if typed::<I, T>(&inputs[later.1].1).id == typed::<I, T>(&inputs[earlier.1].1).id {
                std::mem::swap(later, earlier);
                true
            } else {
                false
            }
        });
    }
    changes.retain_mut(|(slot, index)| {
        if let Some(&found) = slots.get(&typed::<I, T>(&inputs[*index].1).id) {
            *slot = found;
            true
        } else {
            false
        }
    });
}
fn after<'a, I: 'static, T: 'static>(
    input: usize,
    inputs: &'a [(bool, Event)],
    old: Option<&'a T>,
) -> Option<&'a T> {
    if input == usize::MAX {
        old
    } else {
        typed::<I, T>(&inputs[input].1).value.as_ref()
    }
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
        let checkpoint = LinkCheckpoint {
            left: self.left.begin(),
            right: self.right.begin(),
        };
        self.checkpoint_box.begin(checkpoint)
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (saved, empty) =
            CheckpointBox::<LinkCheckpoint>::unpack(saved).expect("fixed-link checkpoint");
        self.left.commit(saved.left);
        self.right.commit(saved.right);
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<LinkCheckpoint>::unpack(saved) {
            Ok((saved, empty)) => {
                self.left.rollback(saved.left);
                self.right.rollback(saved.right);
                self.checkpoint_box.recycle(empty);
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
            .map(Outputs::from)
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        // Invalidate positions before latest_slots can clear/rewrite its scratch
        // or panic in user ordering. A failed indexing pass leaves both arrays
        // empty; callback/input-clone failures retain valid cleanup slot lists.
        if self.inputs_indexed {
            for &(slot, _) in &self.left_changes {
                self.left_inputs[slot] = usize::MAX;
            }
            for &(slot, _) in &self.right_changes {
                self.right_inputs[slot] = usize::MAX;
            }
            self.inputs_indexed = false;
        }
        latest_slots::<I, L>(
            inputs,
            false,
            &self.by_left,
            self.direct_left,
            &mut self.left_changes,
        );
        latest_slots::<J, R>(
            inputs,
            true,
            &self.by_right,
            self.direct_right,
            &mut self.right_changes,
        );
        // Both changed-slot lists are now valid cleanup lists, including if
        // affected-link scratch growth fails before every position is filled.
        self.inputs_indexed = true;
        self.affected.clear();
        for &(i, input) in &self.left_changes {
            self.left_inputs[i] = input;
            self.affected.extend_from_slice(&self.left_links[i]);
        }
        for &(j, input) in &self.right_changes {
            self.right_inputs[j] = input;
            self.affected.extend_from_slice(&self.right_links[j]);
        }
        self.affected.sort_unstable();
        self.affected.dedup();
        let mut out = Vec::with_capacity(self.affected.len());
        for &index in &self.affected {
            let edge = &self.links[index].0.0;
            let (i, j) = self.endpoints[index];
            let old_l = self.left.get(i);
            let old_r = self.right.get(j);
            let value = after::<I, L>(self.left_inputs[i], inputs, old_l)
                .zip(after::<J, R>(self.right_inputs[j], inputs, old_r))
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
        for &(index, input) in &self.left_changes {
            self.left
                .set(index, typed::<I, L>(&inputs[input].1).value.clone());
        }
        for &(index, input) in &self.right_changes {
            self.right
                .set(index, typed::<J, R>(&inputs[input].1).value.clone());
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
            checkpoint_box: CheckpointBox::new(),
            left_changes: Vec::new(),
            right_changes: Vec::new(),
            affected: Vec::new(),
            left_inputs: vec![usize::MAX; by_left.len()],
            right_inputs: vec![usize::MAX; by_right.len()],
            inputs_indexed: false,
            links,
            endpoints,
            direct_left: contiguous_slots(&by_left),
            direct_right: contiguous_slots(&by_right),
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

#[cfg(test)]
#[path = "fixed_link_scratch_tests.rs"]
mod scratch_tests;

#[cfg(test)]
mod index_tests;
