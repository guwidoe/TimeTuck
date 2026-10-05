//! Immutable, bounded candidate snapshots with fractional selection weights.
use crate::random::SplitMix64;
use std::{error::Error, fmt, iter::FusedIterator, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightedSelectionError {
    InvalidWeight { index: usize },
    PrecisionLoss { index: usize },
    CandidateLimit { maximum: usize },
    InvalidDraw,
}
impl fmt::Display for WeightedSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWeight { index } => {
                write!(f, "weight at index {index} must be finite and nonnegative")
            }
            Self::PrecisionLoss { index } => write!(
                f,
                "positive weight at index {index} is lost at this floating-point scale"
            ),
            Self::CandidateLimit { maximum } => {
                write!(f, "candidate source exceeds limit {maximum}")
            }
            Self::InvalidDraw => f.write_str("weighted draw must be finite and in [0, 1)"),
        }
    }
}
impl Error for WeightedSelectionError {}

type Cut = (f64, usize);
fn distribution(weights: &[f64]) -> Result<(Vec<Cut>, f64), WeightedSelectionError> {
    let mut maximum = 0.0_f64;
    for (index, &weight) in weights.iter().enumerate() {
        if !weight.is_finite() || weight < 0.0 {
            return Err(WeightedSelectionError::InvalidWeight { index });
        }
        maximum = maximum.max(weight);
    }
    if maximum == 0.0 {
        return Ok((vec![], 0.0));
    }
    // Scale by an exact power of two. This avoids overflow of large totals and
    // quantization of subnormal totals without changing ordinary CDF boundaries.
    // Subnormal maxima use the minimum normal power, preserving their mantissas.
    let exponent = ((maximum.to_bits() >> 52) & 0x7ff).max(1);
    let scale = f64::from_bits(exponent << 52);
    let mut total = 0.0;
    let mut cuts = vec![];
    for (index, &weight) in weights.iter().enumerate() {
        if weight == 0.0 {
            continue;
        }
        let next = total + weight / scale;
        if next <= total {
            return Err(WeightedSelectionError::PrecisionLoss { index });
        }
        cuts.push((next, index));
        total = next;
    }
    Ok((cuts, total))
}

/// Cached occurrence-weighted sampling with replacement. Duplicate items retain
/// separate weights. Zero-weight entries stay in items()/len() but cannot be drawn;
/// empty or all-zero snapshots yield no samples. Positive snapshots can repeat
/// indefinitely, so search providers must impose their ordinary candidate budget.
///
/// Weights must be finite and nonnegative. Construction rescales by a power of two
/// and rejects positive weights lost to underflow or cumulative rounding, instead
/// of silently disabling them. This is f64 probability arithmetic, not exact real
/// arithmetic; seeded draws use the native 53-bit uniform grid, not Java traces.
/// Construct or reweight at the assignment/step boundary to capture current facts.
///
/// ```
/// use timetuck::*;
/// let choices = WeightedSelection::new(vec![("ordinary", 1.0), ("promising", 3.0)])?;
/// assert_eq!(choices.choose(0.0)?, Some(&"ordinary"));
/// assert_eq!(choices.choose(0.25)?, Some(&"promising"));
/// let candidates = choices.into_samples(7).take(20).collect::<Vec<_>>();
/// assert_eq!(candidates.len(), 20);
/// # Ok::<(), WeightedSelectionError>(())
/// ```
pub struct WeightedSelection<T> {
    items: Arc<[T]>,
    cuts: Arc<[Cut]>,
    total: f64,
}
impl<T> Clone for WeightedSelection<T> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            cuts: self.cuts.clone(),
            total: self.total,
        }
    }
}
impl<T> WeightedSelection<T> {
    pub fn new(entries: Vec<(T, f64)>) -> Result<Self, WeightedSelectionError> {
        let (items, weights): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        let (cuts, total) = distribution(&weights)?;
        Ok(Self {
            items: items.into(),
            cuts: cuts.into(),
            total,
        })
    }
    /// Collect at most maximum candidates. To detect excess, polls one additional
    /// item; source/weight side effects cannot be undone. Infinite productive
    /// sources terminate with CandidateLimit. Source next() remains cooperative.
    pub fn from_bounded_iter(
        entries: impl IntoIterator<Item = (T, f64)>,
        maximum: usize,
    ) -> Result<Self, WeightedSelectionError> {
        let mut collected = vec![];
        for entry in entries {
            if collected.len() == maximum {
                return Err(WeightedSelectionError::CandidateLimit { maximum });
            }
            collected.push(entry);
        }
        Self::new(collected)
    }
    pub fn items(&self) -> &[T] {
        &self.items
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn active_len(&self) -> usize {
        self.cuts.len()
    }
    /// Capture new weights once per occurrence, sharing items without cloning T.
    /// Old distributions/iterators stay unchanged on success, error or callback panic.
    /// User callback side effects cannot be undone.
    pub fn reweighted(
        &self,
        mut weight: impl FnMut(usize, &T) -> f64,
    ) -> Result<Self, WeightedSelectionError> {
        let weights = self
            .items
            .iter()
            .enumerate()
            .map(|(i, v)| weight(i, v))
            .collect::<Vec<_>>();
        let (cuts, total) = distribution(&weights)?;
        Ok(Self {
            items: self.items.clone(),
            cuts: cuts.into(),
            total,
        })
    }
    /// Draw by unit-interval position. An exact cumulative boundary selects the
    /// following positive-weight occurrence; endpoint rounding selects the last.
    pub fn choose(&self, unit: f64) -> Result<Option<&T>, WeightedSelectionError> {
        if !(0.0..1.0).contains(&unit) {
            return Err(WeightedSelectionError::InvalidDraw);
        }
        if self.cuts.is_empty() {
            return Ok(None);
        }
        let target = unit * self.total;
        let index = self
            .cuts
            .partition_point(|&(end, _)| end <= target)
            .min(self.cuts.len() - 1);
        Ok(Some(&self.items[self.cuts[index].1]))
    }
    pub fn samples(&self, seed: u64) -> impl FusedIterator<Item = &T> {
        let mut random = SplitMix64::new(seed);
        self.samples_with(move || random.unit())
    }
    /// Inject unit draws in [0,1). Invalid draws panic; empty/disabled snapshots
    /// never invoke the draw callback. Useful for scripted selection tests.
    pub fn samples_with(&self, mut draw: impl FnMut() -> f64) -> impl FusedIterator<Item = &T> {
        std::iter::from_fn(move || {
            if self.cuts.is_empty() {
                return None;
            }
            self.choose(draw())
                .expect("weighted sampler requires draws in [0,1)")
        })
        .fuse()
    }
    /// Own a captured snapshot and clone only selected occurrences. This can be
    /// returned directly from a move provider, with take() to bound its proposals.
    pub fn into_samples(self, seed: u64) -> impl FusedIterator<Item = T>
    where
        T: Clone,
    {
        let mut random = SplitMix64::new(seed);
        std::iter::from_fn(move || {
            self.choose(random.unit())
                .expect("native unit draw")
                .cloned()
        })
        .fuse()
    }
}
