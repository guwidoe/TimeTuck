//! Bounded on-demand proposals without an exhaustive candidate window.
use crate::list_k_opt::{cut_paths, edge, movement};
use crate::{ListAssignment, ListError, ListKOptConfig, ListMove, ListProblem};
use std::{error::Error, fmt, sync::Arc};

/// Sampling over one immutable snapshot. Each attempt chooses an eligible entity
/// uniformly, a feasible k uniformly (or with configured weights), then distinct cut
/// edges, path order and path directions at random. Proposals that retain a cut
/// edge are discarded, so every yielded move changes exactly the selected k edges.
///
/// `attempts` limits proposals, not yielded moves. Zero disables proposals. Repeats
/// between attempts are allowed; there is no guarantee of neighborhood coverage,
/// a uniform distribution over final tours, or Java PRNG/distribution parity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListKOptSampling {
    sizes: ListKOptConfig,
    seed: u64,
    attempts: usize,
    weights: Option<Arc<[u64]>>,
}
impl ListKOptSampling {
    pub const fn new(sizes: ListKOptConfig, seed: u64, attempts: usize) -> Self {
        Self {
            sizes,
            seed,
            attempts,
            weights: None,
        }
    }
    /// Set exact integer weights for consecutive k values from minimum through
    /// maximum, inclusive. Zero disables a size; some weight must be positive and
    /// the sum must fit u64. Shape validation precedes mass validation.
    ///
    /// Each snapshot drops impossible/oversized sizes and renormalizes the rest.
    /// If all feasible sizes have zero weight, that snapshot produces no moves.
    /// Single-list sampling still chooses eligible entities uniformly before k;
    /// repartition sampling chooses k over the combined movable suffixes. These
    /// are proposal weights, not probabilities of accepted moves: rejection may
    /// depend on k. Unit weights preserve uniform seeded draws.
    ///
    /// Storage is shared when configs are cloned. Distributions are prepared once
    /// per opening, never cloned per candidate. The default allocates no weights.
    pub fn with_weights(mut self, weights: Vec<u64>) -> Result<Self, ListKOptWeightError> {
        let expected = self.sizes.maximum() - self.sizes.minimum() + 1;
        if weights.len() != expected {
            return Err(ListKOptWeightError::Length {
                expected,
                actual: weights.len(),
            });
        }
        let sum = weights
            .iter()
            .try_fold(0u64, |total, &weight| total.checked_add(weight))
            .ok_or(ListKOptWeightError::Overflow)?;
        if sum == 0 {
            return Err(ListKOptWeightError::ZeroMass);
        }
        self.weights = Some(weights.into());
        Ok(self)
    }
    pub fn weights(&self) -> Option<&[u64]> {
        self.weights.as_deref()
    }
    pub fn sizes(&self) -> ListKOptConfig {
        self.sizes
    }
    pub fn seed(&self) -> u64 {
        self.seed
    }
    pub fn attempts(&self) -> usize {
        self.attempts
    }
    /// Derive the seed used for a zero-based local-search step. Step zero preserves
    /// the supplied seed. Each phase restarts its step counter. All list/mixed
    /// scoring backends use this derivation, independently of move-order shuffling.
    pub fn for_step(&self, step: u64) -> Self {
        Self {
            seed: self
                .seed
                .wrapping_add(step.wrapping_mul(0xd1b54a32d192ed03)),
            ..self.clone()
        }
    }
}
/// Invalid integer k-size weights. No sampling state is created on failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListKOptWeightError {
    Length { expected: usize, actual: usize },
    ZeroMass,
    Overflow,
}
impl fmt::Display for ListKOptWeightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { expected, actual } => {
                write!(f, "expected {expected} k-size weights, got {actual}")
            }
            Self::ZeroMass => f.write_str("k-size weights must have positive total mass"),
            Self::Overflow => f.write_str("k-size weight sum exceeds u64"),
        }
    }
}
impl Error for ListKOptWeightError {}

pub(crate) struct SizeDistribution {
    sizes: Vec<usize>,
    cumulative: Option<Vec<u64>>,
}
impl SizeDistribution {
    pub(crate) fn new(config: &ListKOptSampling, n: usize) -> Self {
        let mut sizes = Vec::new();
        let mut cumulative = config.weights.as_ref().map(|_| Vec::new());
        let mut total = 0u64;
        for k in config.sizes.minimum()..=config.sizes.maximum().min(n) {
            if !feasible(n, k) {
                continue;
            }
            if let Some(weights) = config.weights.as_ref() {
                let weight = weights[k - config.sizes.minimum()];
                if weight == 0 {
                    continue;
                }
                total += weight; // Subset of the validated nonnegative total.
                cumulative
                    .as_mut()
                    .expect("weighted distribution")
                    .push(total);
            }
            sizes.push(k);
        }
        Self { sizes, cumulative }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }
    pub(crate) fn draw(&self, random: &mut crate::random::SplitMix64) -> usize {
        let index = match &self.cumulative {
            None => random.below(self.sizes.len() as u64) as usize,
            Some(cumulative) => {
                let draw = random.below(*cumulative.last().expect("positive mass"));
                cumulative.partition_point(|&end| end <= draw)
            }
        };
        self.sizes[index]
    }
}

// C4 has only two alternative undirected cycles, both differing in two edges.
// C5 has edge differences 2, 3 or 5; a four-edge difference cannot form one cycle.
// For n >= 6, all k from 2 through n are feasible. Filtering the small exceptions
// avoids walking a huge attempt budget for an impossible neighborhood.
pub(crate) fn feasible(n: usize, k: usize) -> bool {
    n >= 4 && k >= 2 && k <= n && (n != 4 || k == 2) && !(n == 5 && k == 4)
}
/// Lazy, bounded cyclic reconnections within individual movable list suffixes.
/// Anchors, pins, entity ranges, payloads, missing identities and other lists are
/// preserved exactly as by `list_k_opt_moves`. No complete move window is built.
/// Source validation still runs at zero attempts; no eligible entity returns empty
/// immediately. Each proposal uses linear scratch space and at most O(n*k) edge
/// checking plus O(n log n) relocation construction, independent of neighborhood size.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new((0..8).collect(),vec![ListEntity::new((0..8).collect())])?;
/// let a=p.initial_assignment();
/// let sampling=ListKOptSampling::new(ListKOptConfig::new(2,4)?,19,100);
/// let first:Vec<_>=sample_list_k_opt_moves(&p,&a,sampling.clone())?.collect();
/// assert!(!first.is_empty() && first.len()<=100);
/// assert_eq!(first,sample_list_k_opt_moves(&p,&a,sampling)?.collect::<Vec<_>>());
/// # Ok::<(), ListError>(())
/// ```
pub fn sample_list_k_opt_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptSampling,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(ordered(p, a, config, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptSampling,
    order: impl Iterator<Item = usize>,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    let eligible: Vec<_> = order
        .filter_map(|e| {
            let meta = &p.entities()[e];
            if meta.is_pinned() {
                return None;
            }
            let n = a.lists()[e].len() - meta.pinned_prefix();
            let sizes = SizeDistribution::new(&config, n);
            (!sizes.is_empty()).then_some((e, meta.pinned_prefix(), n, sizes))
        })
        .collect();
    let mut random = crate::random::SplitMix64::new(config.seed);
    (0..if eligible.is_empty() {
        0
    } else {
        config.attempts
    })
        .filter_map(move |_| {
            let (entity, start, n, sizes) = &eligible[random.below(eligible.len() as u64) as usize];
            let k = sizes.draw(&mut random);
            let tour = draw_tour(*n, k, &mut random)?;
            let target: Vec<_> = (0..*start)
                .chain(tour.into_iter().map(|i| i + start))
                .collect();
            Some(movement(a, *entity, &target))
        })
}

pub(crate) fn draw_tour(
    n: usize,
    k: usize,
    random: &mut crate::random::SplitMix64,
) -> Option<Vec<usize>> {
    let mut cuts: Vec<_> = (0..n).collect();
    for i in 0..k {
        let j = i + random.below((n - i) as u64) as usize;
        cuts.swap(i, j);
    }
    cuts.truncate(k);
    cuts.sort_unstable();
    let paths = cut_paths(n, &cuts);
    let mut order: Vec<_> = (0..k).collect();
    random.shuffle(&mut order);
    let mut tour = Vec::with_capacity(n);
    for i in order {
        if random.below(2) == 0 {
            tour.extend(paths[i].iter().copied());
        } else {
            tour.extend(paths[i].iter().rev().copied());
        }
    }
    let removed: Vec<_> = cuts.iter().map(|&i| edge(i, (i + 1) % n)).collect();
    if (0..n).any(|i| removed.contains(&edge(tour[i], tour[(i + 1) % n]))) {
        return None;
    }
    let anchor = tour.iter().position(|&i| i == 0).expect("anchor");
    tour.rotate_left(anchor);
    Some(tour)
}

#[cfg(test)]
mod weighted_tests {
    use super::*;
    #[test]
    fn exact_integer_draws_match_an_independent_expanded_distribution() {
        let config = ListKOptSampling::new(ListKOptConfig::new(2, 6).unwrap(), 0, 1)
            .with_weights(vec![2, 3, 0, 1, 4])
            .unwrap();
        for n in 0..10 {
            let expanded: Vec<_> = (2..=6.min(n))
                .filter(|&k| feasible(n, k))
                .flat_map(|k| std::iter::repeat_n(k, config.weights().unwrap()[k - 2] as usize))
                .collect();
            let distribution = SizeDistribution::new(&config, n);
            assert_eq!(distribution.is_empty(), expanded.is_empty());
            if expanded.is_empty() {
                continue;
            }
            for seed in [0, 9, u64::MAX] {
                let mut actual = crate::random::SplitMix64::new(seed);
                let mut reference = actual.clone();
                for _ in 0..1000 {
                    assert_eq!(
                        distribution.draw(&mut actual),
                        expanded[reference.below(expanded.len() as u64) as usize]
                    );
                }
                assert_eq!(actual.below(u64::MAX), reference.below(u64::MAX));
            }
        }
        let wide = config.with_weights(vec![u64::MAX - 1, 0, 0, 0, 1]).unwrap();
        let distribution = SizeDistribution::new(&wide, 6);
        assert_eq!(distribution.cumulative, Some(vec![u64::MAX - 1, u64::MAX]));
    }
}
