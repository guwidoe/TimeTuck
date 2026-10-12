//! Shared-stream range-aware list changes with explicit compact/decorated draws.
use crate::*;
use std::{
    cell::RefCell,
    num::{NonZeroU32, NonZeroUsize},
    rc::Rc,
};
type SharedRandom = Rc<RefCell<Lcg48Random>>;
fn seeded_range_index(random: &SharedRandom, n: usize) -> usize {
    random
        .borrow_mut()
        .below(
            NonZeroU32::new(u32::try_from(n).expect("range draw fits u32")).expect("nonempty draw"),
        )
        .expect("LCG48 bound fits i32") as usize
}
fn seeded_range_union_ids<V: Clone + Eq, C: ListSelectionView<V>>(context: &C) -> Vec<usize> {
    let p = context.problem();
    if p.entities().is_empty() {
        return vec![];
    }
    if p.entities().iter().any(|e| e.value_range().is_none()) {
        return (0..p.values().len()).collect();
    }
    let mut present = vec![false; p.values().len()];
    for e in p.entities() {
        for v in e.value_range().unwrap() {
            present[p.value_index(v).unwrap()] = true;
        }
    }
    present
        .iter()
        .enumerate()
        .filter_map(|(i, &yes)| yes.then_some(i))
        .collect()
}
pub trait EntityRangeAnchorFilter<C, V> {
    fn accept(&mut self, context: &C, value: &V) -> bool;
}
impl<C, V, F: FnMut(&C, &V) -> bool> EntityRangeAnchorFilter<C, V> for F {
    fn accept(&mut self, context: &C, value: &V) -> bool {
        self(context, value)
    }
}
pub struct AcceptAllRangeAnchors;
impl<C, V> EntityRangeAnchorFilter<C, V> for AcceptAllRangeAnchors {
    fn accept(&mut self, _: &C, _: &V) -> bool {
        true
    }
}
struct SeededRangeValues {
    random: SharedRandom,
    seed: u64,
    step: u64,
}
impl<C: ListSelectionView<V>, V: Clone + Eq> SelectionSource<C, V> for SeededRangeValues {
    fn event(&mut self, event: SelectionLifecycleEvent, _: &C) {
        match event {
            SelectionLifecycleEvent::SolveStarted => {
                self.step = 0;
                *self.random.borrow_mut() = Lcg48Random::new(self.seed);
            }
            SelectionLifecycleEvent::StepStarted => {
                self.step = self.step.wrapping_add(1);
                *self.random.borrow_mut() = Lcg48Random::new(self.seed.wrapping_add(self.step));
            }
            _ => {}
        }
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        Some(seeded_range_union_ids(c).len())
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = V> + 'a>
    where
        V: 'a,
    {
        let random = self.random.clone();
        ListRangeValueSource::select_with(c, move |n| seeded_range_index(&random, n.get()))
    }
}
struct SeededRangeDestinations<F> {
    random: SharedRandom,
    filter: F,
    decorated: bool,
    rejections: NonZeroUsize,
}
impl<C: ListSelectionView<V>, V: Clone + Eq, F: EntityRangeAnchorFilter<C, V>>
    ListDestinationFactory<C, V> for SeededRangeDestinations<F>
{
    fn open<'a>(
        &'a mut self,
        c: &'a C,
        value: &'a V,
        _: &'a ListDestinationIndex<'_, V>,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a> {
        let p = c.problem();
        let a = c.assignment();
        let ids = seeded_range_union_ids(c);
        let entities: Vec<_> = p
            .entities()
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_pinned())
            .map(|(i, _)| i)
            .collect();
        let receivers: Vec<_> = entities
            .iter()
            .copied()
            .filter(|&e| {
                p.entities()[e]
                    .value_range()
                    .is_none_or(|r| r.contains(value))
            })
            .collect();
        let selected = p.value_index(value).expect("registered selected value");
        let positions: Vec<_> = p.values().iter().map(|v| a.location(v)).collect();
        let active: Vec<_> = p
            .values()
            .iter()
            .enumerate()
            .map(|(id, _)| {
                id != selected
                    && positions[id].is_some_and(|pos| {
                        receivers.contains(&pos.entity)
                            && pos.index >= p.entities()[pos.entity].pinned_prefix()
                    })
            })
            .collect();
        // Compact rejected IDs into a suffix by swapping with the last live ID.
        // A decorator sees its predicate-filtered full scratch order and rejects
        // inactive draws; the compact path samples only the eligible prefix.
        let mut scratch = ids.clone();
        let mut count = scratch.len();
        let mut i = 0;
        while i < count {
            if active[scratch[i]] {
                i += 1;
            } else {
                count -= 1;
                scratch.swap(i, count);
            }
        }
        let pool = if self.decorated {
            scratch
                .into_iter()
                .filter(|&id| self.filter.accept(c, &p.values()[id]))
                .collect::<Vec<_>>()
        } else {
            scratch[..count].to_vec()
        };
        let random = self.random.clone();
        let limit = self.rejections.get();
        let optional = p.allows_unassigned();
        let total = ids.len() + entities.len() + usize::from(optional);
        let mut ended = false;
        Box::new(
            std::iter::from_fn(move || {
                if ended {
                    return None;
                }
                ended = true;
                if total == 0 {
                    return None;
                }
                let category = seeded_range_index(&random, total);
                let destination = if optional && category == 0 {
                    None
                } else {
                    if receivers.is_empty() {
                        return None;
                    }
                    if category - usize::from(optional) < entities.len() {
                        let entity = receivers[seeded_range_index(&random, receivers.len())];
                        Some(ListPosition {
                            entity,
                            index: p.entities()[entity].pinned_prefix(),
                        })
                    } else if count == 0 || pool.is_empty() {
                        let entity = receivers[seeded_range_index(&random, receivers.len())];
                        Some(ListPosition {
                            entity,
                            index: a.lists()[entity].len(),
                        })
                    } else {
                        let mut accepted = None;
                        for _ in 0..limit {
                            let id = pool[seeded_range_index(&random, pool.len())];
                            if active[id] {
                                let mut pos = positions[id].unwrap();
                                pos.index += 1;
                                accepted = Some(pos);
                                break;
                            }
                        }
                        Some(accepted?)
                    }
                };
                ended = false;
                Some(destination)
            })
            .fuse(),
        )
    }
}
/// One shared LCG48 stream drives distinct range-union value draws, global
/// entity/anchor category weights and range-qualified destinations. Opening does
/// not rewind the stream; SolveStarted resets it and StepStarted selects seed+step.
/// Compact anchor eligibility excludes the selected value. Empty eligible anchor
/// branches append to a receiving entity. A decorator retains the full compacted
/// scratch order and performs bounded rejection sampling after its own predicate.
/// This is an explicit policy, not a change to default native sampling. All output
/// goes through the existing correlated move checker for pins, ranges, dependency
/// validity and PRE-to-POST normalization. Reopen after assignment edits.
pub struct SeededEntityRangeListChangeSource<F = AcceptAllRangeAnchors> {
    inner: CorrelatedListValueChangeSource<SeededRangeValues, SeededRangeDestinations<F>>,
}
impl SeededEntityRangeListChangeSource {
    pub fn new(seed: u64, limits: CorrelatedListChangeLimits, include_noops: bool) -> Self {
        Self::create(seed, limits, include_noops, AcceptAllRangeAnchors, false)
    }
}
impl<F> SeededEntityRangeListChangeSource<F> {
    pub fn with_anchor_filter(
        seed: u64,
        limits: CorrelatedListChangeLimits,
        include_noops: bool,
        filter: F,
    ) -> Self {
        Self::create(seed, limits, include_noops, filter, true)
    }
    fn create(
        seed: u64,
        limits: CorrelatedListChangeLimits,
        include_noops: bool,
        filter: F,
        decorated: bool,
    ) -> Self {
        let random = Rc::new(RefCell::new(Lcg48Random::new(seed)));
        Self {
            inner: CorrelatedListValueChangeSource::new(
                SeededRangeValues {
                    random: random.clone(),
                    seed,
                    step: 0,
                },
                SeededRangeDestinations {
                    random,
                    filter,
                    decorated,
                    rejections: limits.destinations_per_value,
                },
                limits,
                include_noops,
            ),
        }
    }
}
impl<V: Clone + Eq, C: ListSelectionView<V>, F: EntityRangeAnchorFilter<C, V>>
    SelectionSource<C, ListValueChange<V>> for SeededEntityRangeListChangeSource<F>
{
    fn event(&mut self, event: SelectionLifecycleEvent, c: &C) {
        self.inner.event(event, c);
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        self.inner.size(c)
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = ListValueChange<V>> + 'a>
    where
        ListValueChange<V>: 'a,
    {
        self.inner.open(c)
    }
}
