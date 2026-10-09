//! Ordered, distinct samples for custom neighborhoods. Sampling visits the supplied
//! source order; it does not shuffle members or promise Java random traces.
use std::{
    error::Error,
    fmt,
    hash::{Hash, Hasher},
};

/// An owned set retaining the first occurrence of each member. Equality and hashing
/// ignore encounter order; iteration and the representative preserve it.
#[derive(Clone, Debug)]
pub struct NeighborhoodSample<T> {
    members: Vec<T>,
}
impl<T: Eq> NeighborhoodSample<T> {
    pub fn new(source: impl IntoIterator<Item = T>) -> Self {
        let mut members = Vec::new();
        for item in source {
            if !members.contains(&item) {
                members.push(item);
            }
        }
        Self { members }
    }
    pub fn contains(&self, value: &T) -> bool {
        self.members.contains(value)
    }
}
impl<T> NeighborhoodSample<T> {
    pub fn len(&self) -> usize {
        self.members.len()
    }
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
    pub fn representative(&self) -> Option<&T> {
        self.members.first()
    }
    pub fn members(&self) -> &[T] {
        &self.members
    }
}
impl<T: Eq> PartialEq for NeighborhoodSample<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.members.iter().all(|v| other.contains(v))
    }
}
impl<T: Eq> Eq for NeighborhoodSample<T> {}
impl<T: Hash> Hash for NeighborhoodSample<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let sum = self.members.iter().fold(0u64, |sum, member| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            member.hash(&mut h);
            sum.wrapping_add(h.finish())
        });
        self.len().hash(state);
        sum.hash(state);
    }
}
impl<T> IntoIterator for NeighborhoodSample<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.members.into_iter()
    }
}
impl<'a, T> IntoIterator for &'a NeighborhoodSample<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.members.iter()
    }
}

/// Reproducible randomness owned by the caller and advanced across assemblies.
#[derive(Clone, Debug)]
pub struct SampleRandom(crate::random::SplitMix64);
impl SampleRandom {
    pub fn new(seed: u64) -> Self {
        Self(crate::random::SplitMix64::new(seed))
    }
    /// Draw uniformly from 0..bound. The nonzero type prevents empty ranges.
    pub fn index(&mut self, bound: std::num::NonZeroUsize) -> usize {
        self.0.below(bound.get() as u64) as usize
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleDecision {
    Accept,
    Reject,
    AcceptAndStop,
    /// Stop without accepting the current candidate.
    Stop,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SampleError {
    InvalidSizes { minimum: usize, maximum: usize },
    InvalidExpectedSize,
    InvalidMinimum,
    TargetBelowMinimum { target: usize, minimum: usize },
    StoppedBelowMinimum { actual: usize, minimum: usize },
}
impl fmt::Display for SampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSizes { minimum, maximum } => write!(
                f,
                "sample sizes require 1 <= minimum <= maximum, got {minimum}..={maximum}"
            ),
            Self::InvalidExpectedSize => write!(f, "expected size (0) must be positive"),
            Self::InvalidMinimum => write!(f, "minimum size must be positive"),
            Self::TargetBelowMinimum { target, minimum } => write!(
                f,
                "target size ({target}) must be at least the minimum size ({minimum})"
            ),
            Self::StoppedBelowMinimum { actual, minimum } => write!(
                f,
                "sampler stopped at {actual} members below its minimum size ({minimum})"
            ),
        }
    }
}
impl Error for SampleError {}

/// Stateful acceptance rule. reset runs exactly once before size validation and
/// candidate evaluation. The first distinct candidate sees accepted == 0.
/// target_size is a hint/contract, not an assembler-enforced stopping point;
/// the rule itself decides when to stop. minimum_size is the accepted-member floor.
pub trait Sampler<T> {
    /// Stable traversal hint: true when this rule intends to drain its source.
    /// Assemblers preserve decisions regardless of the hint; datasets use it to
    /// choose eager ordering instead of optimizing a small prefix. Custom rules
    /// default to prefix sampling. Keep the hint fixed during repeated sampling.
    fn exhausts_source(&self) -> bool {
        false
    }
    fn reset(&mut self, _random: &mut SampleRandom) {}
    fn minimum_size(&self) -> usize {
        1
    }
    fn target_size(&self) -> usize {
        self.minimum_size()
    }
    fn evaluate(&mut self, accepted: usize, candidate: &T) -> SampleDecision;
}
impl<T, F: FnMut(usize, &T) -> SampleDecision> Sampler<T> for F {
    fn evaluate(&mut self, accepted: usize, candidate: &T) -> SampleDecision {
        self(accepted, candidate)
    }
}
/// A rule whose reset can depend on the current group key. Candidate membership
/// in that group is the source's responsibility.
pub trait KeyedSampler<K, T>: Sampler<T> {
    fn reset_for_key(&mut self, random: &mut SampleRandom, key: &K);
}
/// Adapt any ordinary rule to keyed assembly without changing its decisions.
#[derive(Clone, Debug)]
pub struct PillarSampler<S>(pub S);
impl<T, S: Sampler<T>> Sampler<T> for PillarSampler<S> {
    fn exhausts_source(&self) -> bool {
        self.0.exhausts_source()
    }
    fn reset(&mut self, r: &mut SampleRandom) {
        self.0.reset(r);
    }
    fn minimum_size(&self) -> usize {
        self.0.minimum_size()
    }
    fn target_size(&self) -> usize {
        self.0.target_size()
    }
    fn evaluate(&mut self, n: usize, v: &T) -> SampleDecision {
        self.0.evaluate(n, v)
    }
}
impl<K, T, S: Sampler<T>> KeyedSampler<K, T> for PillarSampler<S> {
    fn reset_for_key(&mut self, r: &mut SampleRandom, _: &K) {
        self.0.reset(r);
    }
}

/// Fixed-size, random bounded-size or source-draining rules. Random bounds are
/// inclusive; the declared minimum stays fixed while reset draws a new target.
#[derive(Clone, Debug)]
pub struct SizeSampler {
    minimum: usize,
    maximum: usize,
    target: usize,
    drain: bool,
}
impl SizeSampler {
    pub fn exactly(size: usize) -> Result<Self, SampleError> {
        Self::between(size, size)
    }
    pub fn up_to(maximum: usize) -> Result<Self, SampleError> {
        Self::between(1, maximum)
    }
    pub fn between(minimum: usize, maximum: usize) -> Result<Self, SampleError> {
        if minimum == 0 || minimum > maximum {
            return Err(SampleError::InvalidSizes { minimum, maximum });
        }
        Ok(Self {
            minimum,
            maximum,
            target: minimum,
            drain: false,
        })
    }
    pub fn all() -> Self {
        Self {
            minimum: 1,
            maximum: usize::MAX,
            target: 1,
            drain: true,
        }
    }
    pub fn all_with_expected_size(expected: usize) -> Result<Self, SampleError> {
        if expected == 0 {
            return Err(SampleError::InvalidExpectedSize);
        }
        Ok(Self {
            target: expected,
            ..Self::all()
        })
    }
    pub fn minimum_size(&self) -> usize {
        self.minimum
    }
    pub fn target_size(&self) -> usize {
        self.target
    }
    pub fn reset(&mut self, random: &mut SampleRandom) {
        if !self.drain && self.minimum != self.maximum {
            let width = std::num::NonZeroUsize::new(self.maximum - self.minimum + 1)
                .expect("positive bounds");
            self.target = self.minimum + random.index(width);
        }
    }
}
impl<T> Sampler<T> for SizeSampler {
    fn exhausts_source(&self) -> bool {
        self.drain
    }
    fn reset(&mut self, r: &mut SampleRandom) {
        self.reset(r);
    }
    fn minimum_size(&self) -> usize {
        self.minimum
    }
    fn target_size(&self) -> usize {
        self.target
    }
    fn evaluate(&mut self, n: usize, _: &T) -> SampleDecision {
        if !self.drain && n >= self.target - 1 {
            SampleDecision::AcceptAndStop
        } else {
            SampleDecision::Accept
        }
    }
}

/// Assemble distinct members in source order, stopping without an extra pull.
/// Exhaustion below the minimum returns None; an explicit premature stop is an
/// error. Duplicate accepted members are skipped before calling the rule again.
/// An endless source with a rejecting/draining rule requires a caller-supplied
/// bound (for example `source.take(limit)`). This function does not score moves.
///
/// ```
/// use timetuck::{assemble_sample, SizeSampler, SampleRandom};
/// let mut rule=SizeSampler::exactly(2)?;
/// let mut rng=SampleRandom::new(7);
/// let sample=assemble_sample([3,3,5,7], &mut rule, &mut rng)?.unwrap();
/// assert_eq!(sample.members(), &[3,5]);
/// # Ok::<(), timetuck::SampleError>(())
/// ```
pub fn assemble_sample<T: Eq>(
    source: impl IntoIterator<Item = T>,
    sampler: &mut (impl Sampler<T> + ?Sized),
    random: &mut SampleRandom,
) -> Result<Option<NeighborhoodSample<T>>, SampleError> {
    sampler.reset(random);
    assemble_reset(source, sampler)
}
pub fn assemble_keyed_sample<K, T: Eq>(
    source: impl IntoIterator<Item = T>,
    key: &K,
    sampler: &mut (impl KeyedSampler<K, T> + ?Sized),
    random: &mut SampleRandom,
) -> Result<Option<NeighborhoodSample<T>>, SampleError> {
    sampler.reset_for_key(random, key);
    assemble_reset(source, sampler)
}
fn assemble_reset<T: Eq>(
    source: impl IntoIterator<Item = T>,
    sampler: &mut (impl Sampler<T> + ?Sized),
) -> Result<Option<NeighborhoodSample<T>>, SampleError> {
    let minimum = sampler.minimum_size();
    let target = sampler.target_size();
    if minimum == 0 {
        return Err(SampleError::InvalidMinimum);
    }
    if target < minimum {
        return Err(SampleError::TargetBelowMinimum { target, minimum });
    }
    // The hint may be enormous. Allocate only as accepted members arrive.
    let mut members = Vec::new();
    for candidate in source {
        if members.contains(&candidate) {
            continue;
        }
        match sampler.evaluate(members.len(), &candidate) {
            SampleDecision::Stop => {
                if members.len() < minimum {
                    return Err(SampleError::StoppedBelowMinimum {
                        actual: members.len(),
                        minimum,
                    });
                }
                return Ok(Some(NeighborhoodSample { members }));
            }
            SampleDecision::Reject => {}
            SampleDecision::Accept => members.push(candidate),
            SampleDecision::AcceptAndStop => {
                members.push(candidate);
                if members.len() < minimum {
                    return Err(SampleError::StoppedBelowMinimum {
                        actual: members.len(),
                        minimum,
                    });
                }
                return Ok(Some(NeighborhoodSample { members }));
            }
        }
    }
    Ok((members.len() >= minimum).then_some(NeighborhoodSample { members }))
}
