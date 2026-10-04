//! Bounded on-demand proposals without an exhaustive candidate window.
use crate::list_k_opt::{cut_paths, edge, movement};
use crate::{ListAssignment, ListError, ListKOptConfig, ListMove, ListProblem};

/// Sampling over one immutable snapshot. Each attempt chooses an eligible entity
/// uniformly, a feasible k uniformly within its clipped bounds, then distinct cut
/// edges, path order and path directions at random. Proposals that retain a cut
/// edge are discarded, so every yielded move changes exactly the selected k edges.
///
/// `attempts` limits proposals, not yielded moves. Zero disables proposals. Repeats
/// between attempts are allowed; there is no guarantee of neighborhood coverage,
/// a uniform distribution over final tours, or Java PRNG/distribution parity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListKOptSampling {
    sizes: ListKOptConfig,
    seed: u64,
    attempts: usize,
}
impl ListKOptSampling {
    pub const fn new(sizes: ListKOptConfig, seed: u64, attempts: usize) -> Self {
        Self {
            sizes,
            seed,
            attempts,
        }
    }
    pub fn sizes(self) -> ListKOptConfig {
        self.sizes
    }
    pub fn seed(self) -> u64 {
        self.seed
    }
    pub fn attempts(self) -> usize {
        self.attempts
    }
    /// Derive the seed used for a zero-based local-search step. Step zero preserves
    /// the supplied seed. Each phase restarts its step counter. All list/mixed
    /// scoring backends use this derivation, independently of move-order shuffling.
    pub fn for_step(self, step: u64) -> Self {
        Self {
            seed: self
                .seed
                .wrapping_add(step.wrapping_mul(0xd1b54a32d192ed03)),
            ..self
        }
    }
}
// C4 has only two alternative undirected cycles, both differing in two edges.
// C5 has edge differences 2, 3 or 5; a four-edge difference cannot form one cycle.
// For n >= 6, all k from 2 through n are feasible. Filtering the small exceptions
// avoids walking a huge attempt budget for an impossible neighborhood.
fn feasible(n: usize, k: usize) -> bool {
    n >= 4 && k >= 2 && k <= n && (n != 4 || k == 2) && !(n == 5 && k == 4)
}
/// Lazy, bounded cyclic reconnections within individual movable list suffixes.
/// Anchors, pins, entity ranges, payloads, missing identities and other lists are
/// preserved exactly as by `list_k_opt_moves`. No complete move window is built.
/// Source validation still runs at zero attempts; no eligible entity returns empty
/// immediately. Each proposal uses linear scratch space and at most O(n*k) edge
/// checking plus O(n²) relocation construction, independent of neighborhood size.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new((0..8).collect(),vec![ListEntity::new((0..8).collect())])?;
/// let a=p.initial_assignment();
/// let sampling=ListKOptSampling::new(ListKOptConfig::new(2,4)?,19,100);
/// let first:Vec<_>=sample_list_k_opt_moves(&p,&a,sampling)?.collect();
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
            let sizes: Vec<_> = (config.sizes.minimum()..=config.sizes.maximum().min(n))
                .filter(|&k| feasible(n, k))
                .collect();
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
            let k = sizes[random.below(sizes.len() as u64) as usize];
            let mut cuts: Vec<_> = (0..*n).collect();
            for i in 0..k {
                let j = i + random.below((n - i) as u64) as usize;
                cuts.swap(i, j);
            }
            cuts.truncate(k);
            cuts.sort_unstable();
            let paths = cut_paths(*n, &cuts);
            let mut order: Vec<_> = (0..k).collect();
            random.shuffle(&mut order);
            let mut tour = Vec::with_capacity(*n);
            for i in order {
                if random.below(2) == 0 {
                    tour.extend(paths[i].iter().copied());
                } else {
                    tour.extend(paths[i].iter().rev().copied());
                }
            }
            let removed: Vec<_> = cuts.iter().map(|&i| edge(i, (i + 1) % n)).collect();
            if (0..*n).any(|i| removed.contains(&edge(tour[i], tour[(i + 1) % n]))) {
                return None;
            }
            let anchor = tour.iter().position(|&i| i == 0).expect("anchor");
            tour.rotate_left(anchor);
            let target: Vec<_> = (0..*start)
                .chain(tour.into_iter().map(|i| i + start))
                .collect();
            Some(movement(a, *entity, &target))
        })
}
