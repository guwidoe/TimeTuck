//! Live, value-weighted span selection from a bounded entity catalog.
use crate::{
    ListAssignment, ListProblem, ListRange, ListSelectionContext, ListSelectionView,
    SelectionLifecycleEvent, SelectionSource, SubListCardinality, SubListSize,
    ValueSeededSubListSelection, random::SplitMix64,
};
use std::{collections::BTreeSet, num::NonZeroUsize, sync::Arc};

/// Entity reads bound catalog construction per opening or cardinality query.
/// Seed reads bound each requested span, including its successful seed. Duplicate
/// catalog entities are deduplicated; duplicate seeds retain their weighting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueSubListSourceLimits {
    pub entity_reads: usize,
    pub seed_reads_per_item: NonZeroUsize,
}
type Draw = Box<dyn FnMut(u128) -> u128>;

/// Resolve live selected values to owners, then draw a span in that owner.
/// The chosen span need not contain the seed. A finite entity catalog restricts
/// eligible owners; whole pins, prefix seeds, missing values and short suffixes
/// are excluded. No list payload is cloned by the index or sampler.
///
/// Every opening rebuilds borrowed owner and span indexes from the assignment.
/// Catalog reads are bounded, even for a repeating entity child. An empty model
/// span catalog opens neither child; an empty selected catalog opens no seeds.
/// Cardinality queries reopen the entity child and count unique eligible spans,
/// not seed occurrences or eventual outputs. Output size is unknown unless empty.
///
/// Productive repeating seeds can yield without an output ceiling; consumers
/// must impose one. Exhausted or persistently ineligible seeds fuse the opening.
/// A caught seed/draw panic also fuses it; reopening retries the children's state.
/// Random state continues across openings; native seeded traces are reproducible,
/// without implying Java PRNG equivalence. Events forward to entities, then seeds,
/// once each, including empty selections. Composing two sources forwards shared
/// children twice. Owned contexts can be captured by ConfiguredMoveProvider.
pub struct ValueSubListSource<E, S, D = Draw> {
    entities: E,
    seeds: S,
    bounds: SubListSize,
    limits: ValueSubListSourceLimits,
    draw: D,
}
impl<E, S> ValueSubListSource<E, S> {
    pub fn new(
        entities: E,
        seeds: S,
        bounds: SubListSize,
        limits: ValueSubListSourceLimits,
        seed: u64,
    ) -> Self {
        let mut random = SplitMix64::new(seed);
        Self::with_draw(
            entities,
            seeds,
            bounds,
            limits,
            Box::new(move |n| random.below_wide(n)),
        )
    }
}
impl<E, S, D> ValueSubListSource<E, S, D> {
    pub fn with_draw(
        entities: E,
        seeds: S,
        bounds: SubListSize,
        limits: ValueSubListSourceLimits,
        draw: D,
    ) -> Self {
        Self {
            entities,
            seeds,
            bounds,
            limits,
            draw,
        }
    }
    pub fn entities(&self) -> &E {
        &self.entities
    }
    pub fn seeds(&self) -> &S {
        &self.seeds
    }
    pub fn entities_mut(&mut self) -> &mut E {
        &mut self.entities
    }
    pub fn seeds_mut(&mut self) -> &mut S {
        &mut self.seeds
    }

    fn index<'a, V: Ord + Clone, C: ListSelectionView<V>>(
        &mut self,
        context: &'a C,
    ) -> (ValueSeededSubListSelection<'a, V>, u128)
    where
        E: SelectionSource<C, usize>,
    {
        let mut index =
            ValueSeededSubListSelection::new(context.problem(), context.assignment(), self.bounds)
                .expect("validated list context");
        let entities = if index.is_empty() || self.limits.entity_reads == 0 {
            BTreeSet::new()
        } else {
            self.entities
                .open(context)
                .take(self.limits.entity_reads)
                .collect()
        };
        let count = index.retain_entities(&entities);
        (index, count)
    }
    fn open_view<'a, V: Ord + Clone + 'a, C: ListSelectionView<V>>(
        &'a mut self,
        context: &'a C,
    ) -> Box<dyn Iterator<Item = ListRange> + 'a>
    where
        E: SelectionSource<C, usize>,
        S: SelectionSource<C, V>,
        D: FnMut(u128) -> u128,
    {
        let (index, _) = self.index(context);
        if index.is_empty() {
            return Box::new(std::iter::empty());
        }
        let mut seeds = self.seeds.open(context);
        let limit = self.limits.seed_reads_per_item.get();
        let draw = &mut self.draw;
        let mut ended = false;
        Box::new(
            std::iter::from_fn(move || {
                if ended {
                    return None;
                }
                ended = true;
                let span = seeds
                    .by_ref()
                    .take(limit)
                    .find_map(|seed| index.sample_for_with(&seed, &mut *draw));
                if span.is_some() {
                    ended = false;
                }
                span
            })
            .fuse(),
        )
    }
}
macro_rules! source_for_context {
    ([$($lifetime:lifetime)?] $context:ty) => {
        impl<$($lifetime,)? V: Ord + Clone, E, S, D> SelectionSource<$context, ListRange>
            for ValueSubListSource<E, S, D>
        where E: SelectionSource<$context, usize>, S: SelectionSource<$context, V>, D: FnMut(u128) -> u128 {
            fn event(&mut self, event: SelectionLifecycleEvent, context: &$context) {
                self.entities.event(event, context);
                self.seeds.event(event, context);
            }
            fn is_never_ending(&self) -> bool { self.seeds.is_never_ending() }
            fn size(&mut self, context: &$context) -> Option<usize> {
                if self.index(context).0.is_empty() { Some(0) } else { None }
            }
            fn open<'a>(&'a mut self, context: &'a $context) -> Box<dyn Iterator<Item = ListRange> + 'a>
            where ListRange: 'a { self.open_view(context) }
        }
        impl<$($lifetime,)? V: Ord + Clone, E, S, D> SubListCardinality<$context>
            for ValueSubListSource<E, S, D>
        where E: SelectionSource<$context, usize>, S: SelectionSource<$context, V>, D: FnMut(u128) -> u128 {
            fn span_count(&mut self, context: &$context) -> u128 { self.index(context).1 }
        }
    };
}
source_for_context!(['p] ListSelectionContext<'p, V>);
source_for_context!([] (Arc<ListProblem<V>>, ListAssignment<V>));
