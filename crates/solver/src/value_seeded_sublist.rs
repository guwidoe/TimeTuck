//! Value-seeded, snapshot-bound sublist sampling.
use crate::{
    ListAssignment, ListError, ListProblem, ListRange, SubListSelection, SubListSize,
    random::SplitMix64,
};
use std::{collections::BTreeMap, iter::FusedIterator};

/// Resolve selected values to movable owners, then draw uniformly among that
/// owner's admissible spans. The seed need not occur inside the drawn span.
/// Repeated seeds retain their weighting; entities are not weighted by span count.
///
/// The index borrows the assignment's values and cannot outlive that snapshot.
/// After model validation, construction takes O(n log n) time and O(n) borrowed
/// entries; lookup takes O(log n), followed by the span index's rank lookup.
/// Values are never cloned. Rebuild after moving values or changing the model.
/// Unknown, unassigned, pinned-prefix, fully pinned and too-short seeds produce
/// no span and consume no randomness. Excluding pinned seeds is a native policy.
#[derive(Debug)]
pub struct ValueSeededSubListSelection<'a, V> {
    owners: BTreeMap<&'a V, usize>,
    spans: SubListSelection,
}
impl<'a, V: Ord> ValueSeededSubListSelection<'a, V> {
    pub fn new(
        problem: &ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        size: SubListSize,
    ) -> Result<Self, ListError>
    where
        V: Clone,
    {
        let spans = SubListSelection::new(problem, assignment, size)?;
        let mut owners = BTreeMap::new();
        for (entity, meta) in problem.entities().iter().enumerate() {
            if spans.entity_len(entity) != 0 {
                owners.extend(
                    assignment.lists()[entity][meta.pinned_prefix()..]
                        .iter()
                        .map(|value| (value, entity)),
                );
            }
        }
        Ok(Self { owners, spans })
    }
    pub(crate) fn retain_entities(&mut self, entities: &std::collections::BTreeSet<usize>) -> u128 {
        self.owners.retain(|_, entity| entities.contains(entity));
        entities
            .iter()
            .map(|entity| self.spans.entity_len(*entity))
            .sum()
    }
    pub fn seed_count(&self) -> usize {
        self.owners.len()
    }
    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }
    /// The callback receives a positive exclusive span-rank bound. An invalid
    /// rank panics; an ineligible seed does not call the callback.
    pub fn sample_for_with(&self, value: &V, draw: impl FnMut(u128) -> u128) -> Option<ListRange> {
        let entity = *self.owners.get(value)?;
        self.spans
            .random_iter_for_entities_with([entity], 1, draw)
            .next()
    }
    /// Bounded seed consumption, including rejected seeds. Opening is lazy;
    /// exhausted input stays exhausted even for a non-fused source. An empty
    /// index reads no seeds. Repeated values may yield repeated spans.
    pub fn random_iter_with<'s>(
        &'s self,
        seeds: impl IntoIterator<Item = &'s V> + 's,
        maximum_seed_reads: usize,
        mut draw: impl FnMut(u128) -> u128 + 's,
    ) -> impl FusedIterator<Item = ListRange> + 's {
        let mut seeds = seeds.into_iter().take(if self.is_empty() {
            0
        } else {
            maximum_seed_reads
        });
        std::iter::from_fn(move || {
            seeds
                .by_ref()
                .find_map(|value| self.sample_for_with(value, &mut draw))
        })
        .fuse()
    }
    /// Native reproducible rank draws; no Java PRNG trace equivalence.
    pub fn random_iter<'s>(
        &'s self,
        seeds: impl IntoIterator<Item = &'s V> + 's,
        maximum_seed_reads: usize,
        seed: u64,
    ) -> impl FusedIterator<Item = ListRange> + 's {
        let mut random = SplitMix64::new(seed);
        self.random_iter_with(seeds, maximum_seed_reads, move |bound| {
            random.below_wide(bound)
        })
    }
}
