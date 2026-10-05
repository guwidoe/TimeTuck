//! Bounded probing and seeded random sampling with explicit retirement.
use crate::SampleRandom;
use std::{error::Error, fmt, iter::FusedIterator, num::NonZeroUsize};

/// Unlike an iterator's `None`, running out of probe budget is retryable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilterProbe<T> {
    Found(T),
    BudgetExhausted,
    SourceExhausted,
}
impl<T> FilterProbe<T> {
    /// Extract a match when the caller deliberately treats this probe's two
    /// failure modes alike (for example, a bounded retiring-pair attempt).
    pub fn into_match(self) -> Option<T> {
        match self {
            Self::Found(value) => Some(value),
            Self::BudgetExhausted | Self::SourceExhausted => None,
        }
    }
}

/// A filter whose individual probes inspect at most `budget` source items.
///
/// A successful probe consumes its match, with no lookahead. A later probe gets
/// a fresh budget after a miss. The first source `None` permanently exhausts this
/// wrapper, even for a non-fused source. No `Iterator` implementation is provided:
/// a budget miss must not silently truncate a normal iterator consumer.
pub struct BoundedFilter<I, F> {
    source: I,
    predicate: F,
    budget: NonZeroUsize,
    exhausted: bool,
}
impl<I: Iterator, F: FnMut(&I::Item) -> bool> BoundedFilter<I, F> {
    pub fn new(source: I, budget: NonZeroUsize, predicate: F) -> Self {
        Self {
            source,
            predicate,
            budget,
            exhausted: false,
        }
    }
    pub fn probe(&mut self) -> FilterProbe<I::Item> {
        if self.exhausted {
            return FilterProbe::SourceExhausted;
        }
        for _ in 0..self.budget.get() {
            let Some(value) = self.source.next() else {
                self.exhausted = true;
                return FilterProbe::SourceExhausted;
            };
            if (self.predicate)(&value) {
                return FilterProbe::Found(value);
            }
        }
        FilterProbe::BudgetExhausted
    }
}

/// Retirement requires a successful draw since the previous retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoDrawToRetire;
impl fmt::Display for NoDrawToRetire {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("retirement requires an unretired successful draw")
    }
}
impl Error for NoDrawToRetire {}

/// Uniform draws with replacement over live row occurrences. Retiring the last
/// drawn occurrence removes it permanently in constant time. Duplicate payloads
/// remain separate occurrences. Construction consumes a finite source once;
/// draws/retirement never reorder or mutate any external source.
///
/// `draw` borrows a value; the Iterator implementation clones only yielded values.
/// A nonempty pool is infinite unless the caller retires rows or bounds iteration.
pub struct RetiringRandom<T> {
    slots: Vec<Option<T>>,
    active: Vec<usize>,
    last: Option<usize>,
    random: SampleRandom,
}
impl<T> RetiringRandom<T> {
    pub fn new(source: impl IntoIterator<Item = T>, seed: u64) -> Self {
        Self::from_slots(source.into_iter().map(Some).collect(), seed)
    }
    /// None slots are absent, not null payloads. Use Some(None) to store a null
    /// payload when T itself is Option. Physical holes are never drawn.
    pub fn from_slots(slots: Vec<Option<T>>, seed: u64) -> Self {
        let active = slots
            .iter()
            .enumerate()
            .filter_map(|(i, value)| value.as_ref().map(|_| i))
            .collect();
        Self {
            slots,
            active,
            last: None,
            random: SampleRandom::new(seed),
        }
    }
    pub fn remaining(&self) -> usize {
        self.active.len()
    }
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
    pub fn draw(&mut self) -> Option<&T> {
        let Some(bound) = NonZeroUsize::new(self.active.len()) else {
            self.last = None;
            return None;
        };
        let index = self.random.index(bound);
        self.last = Some(index);
        self.slots[self.active[index]].as_ref()
    }
    pub fn retire(&mut self) -> Result<(), NoDrawToRetire> {
        let index = self.last.take().ok_or(NoDrawToRetire)?;
        let slot = self.active.swap_remove(index);
        self.slots[slot] = None;
        Ok(())
    }
}
impl<T: Clone> Iterator for RetiringRandom<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.draw().cloned()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_empty() {
            (0, Some(0))
        } else {
            (usize::MAX, None)
        }
    }
}
impl<T: Clone> FusedIterator for RetiringRandom<T> {}

/// Select a live left occurrence uniformly, then try to obtain one right value.
/// Each failed left gets exactly `attempts` probes before permanent retirement;
/// successful lefts stay available. A failure is local to an attempt: the next
/// call may reopen a source or run a new random probe. No pairs are precomputed.
///
/// One next call makes at most `remaining_lefts * attempts` callback invocations.
/// Callbacks must themselves finish promptly; BoundedFilter can bound filtering
/// over infinite right sources. Bounded random probes can miss rare matches and
/// retire productive rows. Use exhaustive right scans when that is unacceptable.
/// Left-row fairness is not uniform sampling over all matching pairs. Recreate
/// the sampler from a new immutable snapshot after changing the solution.
///
/// ```
/// use timetuck::*;
/// use std::num::NonZeroUsize;
/// let pairs = RetiringPairs::new(0..3, NonZeroUsize::new(4).unwrap(), 9,
///     |left: &i32, _: &mut SampleRandom| (*left == 2).then_some(7));
/// assert!(pairs.take(20).all(|pair| pair == (2, 7)));
/// ```
pub struct RetiringPairs<A, F> {
    left: RetiringRandom<A>,
    attempts: NonZeroUsize,
    pick_right: F,
    random: SampleRandom,
}
impl<A, F> RetiringPairs<A, F> {
    pub fn new(
        source: impl IntoIterator<Item = A>,
        attempts: NonZeroUsize,
        seed: u64,
        pick_right: F,
    ) -> Self {
        Self {
            left: RetiringRandom::new(source, seed),
            attempts,
            pick_right,
            random: SampleRandom::new(seed ^ 0x9e3779b97f4a7c15),
        }
    }
    pub fn remaining_lefts(&self) -> usize {
        self.left.remaining()
    }
}
impl<A: Clone, B, F: FnMut(&A, &mut SampleRandom) -> Option<B>> Iterator for RetiringPairs<A, F> {
    type Item = (A, B);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let left = self.left.draw()?;
            for _ in 0..self.attempts.get() {
                if let Some(right) = (self.pick_right)(left, &mut self.random) {
                    return Some((left.clone(), right));
                }
            }
            self.left.retire().expect("left was drawn");
        }
    }
}
impl<A: Clone, B, F: FnMut(&A, &mut SampleRandom) -> Option<B>> FusedIterator
    for RetiringPairs<A, F>
{
}
