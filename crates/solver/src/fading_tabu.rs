//! Tabu memory with a hard interval followed by progressively weaker rejection.
use crate::{AcceptanceError, Score, random::SplitMix64};

/// Keys age only on completed steps. Repeated keys refresh one retained entry.
/// A just-selected key has age 1; ages through `hard_tenure` reject, then age
/// `hard_tenure + k` accepts with probability `(k + 1) / (fading_tenure + 1)`.
/// At the combined horizon the probability is one. With no fading interval this
/// behaves like [`crate::TabuMemory`]. The most recently touched candidate key
/// determines the probability. This multiple-key policy is native; the adapted
/// Community fading case covers one queried key at a time.
///
/// Optional aspiration accepts a strict global-best improvement. Randomness is
/// consumed only for probabilities strictly between zero and one. The native
/// seeded generator does not reproduce Java random traces.
///
/// ```
/// use timetuck::{FadingTabuMemory, SimpleScore};
/// let mut memory = FadingTabuMemory::new(2, 4, false, 19)?;
/// memory.step_ended(&[7]);
/// assert_eq!(memory.probability(&[7], SimpleScore::ZERO, SimpleScore::ZERO), 0.0);
/// memory.step_ended(&[]);
/// memory.step_ended(&[]);
/// assert_eq!(memory.probability(&[7], SimpleScore::ZERO, SimpleScore::ZERO), 0.4);
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct FadingTabuMemory<K> {
    hard_tenure: usize,
    fading_tenure: usize,
    horizon: usize,
    aspiration: bool,
    ages: Vec<(K, usize)>,
    random: SplitMix64,
}
impl<K: Eq + Clone> FadingTabuMemory<K> {
    pub(crate) fn validate(hard: usize, fading: usize) -> Result<usize, AcceptanceError> {
        let horizon = hard
            .checked_add(fading)
            .ok_or(AcceptanceError::TabuTenureOverflow)?;
        if horizon == 0 {
            return Err(AcceptanceError::EmptyTabu);
        }
        Ok(horizon)
    }
    pub fn new(
        hard_tenure: usize,
        fading_tenure: usize,
        aspiration: bool,
        seed: u64,
    ) -> Result<Self, AcceptanceError> {
        Ok(Self {
            horizon: Self::validate(hard_tenure, fading_tenure)?,
            hard_tenure,
            fading_tenure,
            aspiration,
            ages: Vec::new(),
            random: SplitMix64::new(seed),
        })
    }
    /// Inspect acceptance without drawing or changing memory. Empty/unretained
    /// key sets always accept. Candidate and best use the score's full ordering.
    pub fn probability<S: Score>(&self, keys: &[K], candidate: S, best: S) -> f64 {
        if self.aspiration && candidate > best {
            return 1.0;
        }
        let youngest = self
            .ages
            .iter()
            .filter(|(k, _)| keys.contains(k))
            .map(|(_, age)| *age)
            .min();
        let Some(age) = youngest else {
            return 1.0;
        };
        if age <= self.hard_tenure {
            return 0.0;
        }
        if age >= self.horizon {
            return 1.0;
        }
        ((age - self.hard_tenure) as f64 + 1.0) / (self.fading_tenure as f64 + 1.0)
    }
    /// Replay a lazy uniform draw in [0,1). The callback is never called for
    /// certain acceptance/rejection, including aspiration. Invalid draws return
    /// an error without changing memory or this instance's seeded generator.
    pub fn accepts_with_draw<S: Score>(
        &self,
        keys: &[K],
        candidate: S,
        best: S,
        draw: impl FnOnce() -> f64,
    ) -> Result<bool, AcceptanceError> {
        let p = self.probability(keys, candidate, best);
        if p <= 0.0 {
            return Ok(false);
        }
        if p >= 1.0 {
            return Ok(true);
        }
        let value = draw();
        if !value.is_finite() || !(0.0..1.0).contains(&value) {
            return Err(AcceptanceError::InvalidRandomDraw);
        }
        Ok(value < p)
    }
    pub fn accepts<S: Score>(&mut self, keys: &[K], candidate: S, best: S) -> bool {
        let p = self.probability(keys, candidate, best);
        p >= 1.0 || (p > 0.0 && self.random.unit() < p)
    }
    /// Age retained keys and refresh all selected keys. Empty steps age memory;
    /// trial checks must not call this. No global step counter can overflow.
    pub fn step_ended(&mut self, keys: &[K]) {
        self.ages.retain_mut(|(_, age)| {
            if *age >= self.horizon {
                return false;
            }
            *age += 1;
            true
        });
        for key in keys {
            if let Some((_, age)) = self.ages.iter_mut().find(|(k, _)| k == key) {
                *age = 1;
            } else {
                self.ages.push((key.clone(), 1));
            }
        }
    }
}
