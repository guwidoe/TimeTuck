//! Left-uniform filtered pair sampling with exact empty-row retirement.
use crate::{RetiringIndices, random::SplitMix64};
use std::{collections::BTreeMap, iter::FusedIterator};

enum FilteredRowMatches {
    Cached(Box<[usize]>),
    Count(usize),
}
impl FilteredRowMatches {
    fn len(&self) -> usize {
        match self {
            Self::Cached(v) => v.len(),
            Self::Count(n) => *n,
        }
    }
}
/// Repeatedly choose a live left occurrence uniformly, then one of its matching
/// right occurrences uniformly. This weights entities equally, not every pair
/// equally. Equal payloads in separate positions remain separate occurrences.
///
/// Construction does no predicate work. The first visit to a left scans the
/// entire right slice; only a proven empty row is permanently retired. FilteredRowMatches
/// are cached lazily up to `maximum_cached_matches` across all visited rows.
/// Larger rows retain only their count and scan to the selected match on each
/// draw. A zero budget disables match caching. Budget changes do not change the
/// native seeded trace. No payload clones or Cartesian pair catalog are needed.
///
/// Inputs are borrowed snapshots. The predicate must stay stable throughout the
/// traversal; rebuild after a planning-state change. Predicate panics may advance
/// randomness but do not install partial caches or retire the selected left.
/// Empty joins exhaust; nonempty joins repeat indefinitely and need a bound.
/// The first scan costs O(right length); subsequent cached draws use O(log visited
/// lefts) lookup. Retained memory is O(visited lefts + cached matches + retirements).
/// No Java random trace equivalence is claimed.
pub struct FilteredPairSampler<'a, A, B, F> {
    left: &'a [A],
    right: &'a [B],
    predicate: F,
    active: RetiringIndices,
    random: SplitMix64,
    rows: BTreeMap<usize, FilteredRowMatches>,
    cached_matches: usize,
    maximum_cached_matches: usize,
}
impl<'a, A, B, F: Fn(&A, &B) -> bool> FilteredPairSampler<'a, A, B, F> {
    pub fn new(
        left: &'a [A],
        right: &'a [B],
        predicate: F,
        seed: u64,
        maximum_cached_matches: usize,
    ) -> Self {
        Self {
            left,
            right,
            predicate,
            active: RetiringIndices::new(
                if right.is_empty() {
                    0
                } else {
                    left.len() as u128
                },
                seed,
            ),
            random: SplitMix64::new(seed ^ 0xa0761d6478bd642f),
            rows: BTreeMap::new(),
            cached_matches: 0,
            maximum_cached_matches,
        }
    }
    /// Left occurrences not yet proven empty, including unvisited ones.
    pub fn remaining_left(&self) -> usize {
        self.active.remaining() as usize
    }
    pub fn cached_match_count(&self) -> usize {
        self.cached_matches
    }
}
impl<'a, A, B, F: Fn(&A, &B) -> bool> Iterator for FilteredPairSampler<'a, A, B, F> {
    type Item = (&'a A, &'a B);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let left = self.active.draw()? as usize;
            if !self.rows.contains_key(&left) {
                let available = self.maximum_cached_matches - self.cached_matches;
                let mut indices = Some(Vec::new());
                let mut count = 0;
                for (i, right) in self.right.iter().enumerate() {
                    if (self.predicate)(&self.left[left], right) {
                        count += 1;
                        if count > available {
                            indices = None;
                        }
                        if let Some(indices) = &mut indices {
                            indices.push(i);
                        }
                    }
                }
                if count == 0 {
                    self.active.retire().expect("left was drawn");
                    continue;
                }
                let matches = match indices {
                    Some(indices) => {
                        self.cached_matches += count;
                        FilteredRowMatches::Cached(indices.into_boxed_slice())
                    }
                    None => FilteredRowMatches::Count(count),
                };
                self.rows.insert(left, matches);
            }
            let matches = &self.rows[&left];
            let count = matches.len();
            let rank = if count == 1 {
                0
            } else {
                self.random.below(count as u64) as usize
            };
            let right = match matches {
                FilteredRowMatches::Cached(indices) => &self.right[indices[rank]],
                FilteredRowMatches::Count(_) => self
                    .right
                    .iter()
                    .filter(|right| (self.predicate)(&self.left[left], right))
                    .nth(rank)
                    .expect("filter predicate changed during sampling"),
            };
            return Some((&self.left[left], right));
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.active.is_empty() {
            (0, Some(0))
        } else {
            (0, None)
        }
    }
}
impl<A, B, F: Fn(&A, &B) -> bool> FusedIterator for FilteredPairSampler<'_, A, B, F> {}
