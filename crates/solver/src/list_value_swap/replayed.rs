//! Recorded-left partner sampling with bounded rejection and snapshot positions.
use super::ValueSwapSelection;
use crate::{
    Lcg48Random, ListMove, ListSelectionView, SampleRandom, SelectionLifecycleEvent,
    SelectionSource,
};
use std::num::{NonZeroU32, NonZeroUsize};
/// Draw policy over the canonical registered order of co-range partners.
/// Lcg48 uses the documented public Java bounded PRNG; it does not establish
/// arbitrary Timefold selector/RNG call-trace parity. Singletons consume draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListSwapPartnerOrder {
    Native { seed: u64 },
    Lcg48 { seed: u64 },
}
enum Draws {
    Native(SampleRandom),
    Legacy(Lcg48Random),
}
impl Draws {
    fn index(&mut self, n: NonZeroUsize) -> usize {
        match self {
            Self::Native(r) => r.index(n),
            Self::Legacy(r) => r
                .below(
                    NonZeroU32::new(u32::try_from(n.get()).expect("LCG48 partner count fits u32"))
                        .unwrap(),
                )
                .expect("LCG48 partner count fits i32") as usize,
        }
    }
}
impl ListSwapPartnerOrder {
    fn draws(self, step: u64) -> Draws {
        match self {
            Self::Native { seed } => Draws::Native(SampleRandom::new(seed.wrapping_add(step))),
            Self::Lcg48 { seed } => Draws::Legacy(Lcg48Random::new(seed.wrapping_add(step))),
        }
    }
}
/// Per-proposal bounds: left reads include pinned/missing occurrences; partner
/// attempts include rejected range/dependency proposals. Bounds never apply to
/// the total successful output count; callers limit draws for solver use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayedListSwapLimits {
    pub left_reads: NonZeroUsize,
    pub partner_attempts: NonZeroUsize,
}
/// Sample one partner for each recorded movable left occurrence. Build an inverse
/// range index once on the first pull; share membership through entity IDs without
/// requiring Ord or Hash. Co-range candidates exclude the origin, respect pins,
/// and retain canonical registry order. Range/dependency checks run after drawing,
/// so rejected attempts consume randomness. Optional missing right values replace
/// assigned lefts atomically. Origins without positions are skipped; use existing
/// paired/correlated sources when missing values should be recorded origins too.
///
/// Every valid recorded origin yields either a checked swap or an empty composite
/// after the partner attempt budget, including when its co-range set is empty.
/// This fallback means no proposal was found; it does not prove a local optimum.
/// Opening is lazy and captures immutable positions; reopen after assignment edits.
/// Unknown origins/callback panics fuse the cursor when caught. A step event changes
/// the next opening's seed, while reopening one step replays the same draw stream.
/// Events forward once to the left child. Size only certifies an advertised empty
/// left source. No Cartesian move catalog, expected-output script or Java production
/// implementation is used. Storage is O(values + total domain memberships).
/// Repeated large co-range rows reuse the existing bounded eight-row cache;
/// cached IDs occupy at most twice the value count, plus one scratch row.
pub struct ReplayedListValueSwapSource<S> {
    left: S,
    order: ListSwapPartnerOrder,
    limits: ReplayedListSwapLimits,
    step: u64,
}
impl<S> ReplayedListValueSwapSource<S> {
    pub fn new(left: S, order: ListSwapPartnerOrder, limits: ReplayedListSwapLimits) -> Self {
        Self {
            left,
            order,
            limits,
            step: 0,
        }
    }
    pub fn left(&self) -> &S {
        &self.left
    }
    pub fn left_mut(&mut self) -> &mut S {
        &mut self.left
    }
}
impl<V: Clone + Eq, C: ListSelectionView<V>, S: SelectionSource<C, V>>
    SelectionSource<C, ListMove<V>> for ReplayedListValueSwapSource<S>
{
    fn event(&mut self, e: SelectionLifecycleEvent, c: &C) {
        if e == SelectionLifecycleEvent::SolveStarted {
            self.step = 0;
        }
        if e == SelectionLifecycleEvent::StepStarted {
            self.step = self.step.wrapping_add(1);
        }
        self.left.event(e, c);
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        (self.left.size(c) == Some(0)).then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        self.left.is_never_ending()
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        let left = &mut self.left;
        let limits = self.limits;
        let mut draws = self.order.draws(self.step);
        Box::new(
            std::iter::once_with(move || {
                let mut selection = ValueSwapSelection::new(c.problem(), c.assignment())
                    .expect("valid replayed swap assignment");
                let global = selection
                    .problem
                    .entities()
                    .iter()
                    .any(|e| e.value_range().is_none());
                let count = selection.problem.values().len();
                let mut memberships = if global {
                    Vec::new()
                } else {
                    vec![Vec::new(); count]
                };
                if !global {
                    for (entity, meta) in selection.problem.entities().iter().enumerate() {
                        for value in meta.value_range().expect("explicit ranges") {
                            let id = selection
                                .problem
                                .value_index(value)
                                .expect("validated range member");
                            memberships[id].push(entity);
                        }
                    }
                }
                let mut left = left.open(c);
                let mut ended = false;
                let mut scratch = Vec::new();
                let mut cache = super::eligibility_cache::EligibilityCache::for_opening(
                    count,
                    limits.partner_attempts.get() as u64,
                );
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    for _ in 0..limits.left_reads.get() {
                        let value = left.next()?;
                        let l = selection
                            .problem
                            .value_index(&value)
                            .expect("registered recorded swap origin");
                        drop(value);
                        if !selection.selectable[l] || selection.locations[l].is_none() {
                            continue;
                        }
                        let fill = |choices: &mut Vec<usize>| {
                            choices.extend((0..count).filter(|&r| {
                                r != l
                                    && selection.selectable[r]
                                    && (global
                                        || memberships[r]
                                            .iter()
                                            .any(|e| memberships[l].binary_search(e).is_ok()))
                            }));
                        };
                        let choices = if let Some(cache) = &mut cache {
                            cache.get_or_fill(l, &mut scratch, fill)
                        } else {
                            scratch.clear();
                            fill(&mut scratch);
                            &scratch
                        };
                        let result = (|| {
                            let n = NonZeroUsize::new(choices.len())?;
                            for _ in 0..limits.partner_attempts.get() {
                                let r = choices[draws.index(n)];
                                if let Some(m) = selection.movement(l, r, false) {
                                    return Some(m);
                                }
                            }
                            None
                        })()
                        .unwrap_or_else(|| ListMove::Composite(vec![]));
                        ended = false;
                        return Some(result);
                    }
                    None
                })
                .fuse()
            })
            .flatten()
            .fuse(),
        )
    }
}
