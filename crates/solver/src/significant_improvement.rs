//! Exact score-difference thresholds for cooperative stagnation termination.
use crate::{Decimal, Score, ScoreLevel};
use std::{cmp::Ordering, error::Error, fmt, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignificantImprovementError {
    FlawedThreshold,
    NoScoreLevels,
    UnsupportedArithmetic,
    InvalidIgnoredLevels,
}
impl fmt::Display for SignificantImprovementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FlawedThreshold => "improvement threshold must have zero structural state",
            Self::NoScoreLevels => "improvement threshold requires ordinary score levels",
            Self::UnsupportedArithmetic => {
                "improvement threshold requires exact decimal arithmetic"
            }
            Self::InvalidIgnoredLevels => {
                "at least one score level must remain in the improvement threshold"
            }
        })
    }
}
impl Error for SignificantImprovementError {}

/// Require an accumulated retained-best improvement within an elapsed-time window.
/// The difference is lexicographic and exact, without wrapping score subtraction.
/// A qualifying improvement resets both the score anchor and its time. Ties never
/// reset, even with a zero or negative minimum. Expiry is strictly *after* the
/// deadline; an improvement at the deadline may still qualify. Score ordering
/// must be structural-first, then lexicographic ordinary numeric levels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignificantImprovementConfig<S> {
    window: Duration,
    minimum: S,
    levels: usize,
}
impl<S: Score> SignificantImprovementConfig<S> {
    pub fn new(window: Duration, minimum: S) -> Result<Self, SignificantImprovementError> {
        if minimum.structural() != 0 {
            return Err(SignificantImprovementError::FlawedThreshold);
        }
        if S::Level::zero().exact_decimal().is_none() {
            return Err(SignificantImprovementError::UnsupportedArithmetic);
        }
        let levels = minimum.ordinary_levels().count();
        if levels == 0 {
            return Err(SignificantImprovementError::NoScoreLevels);
        }
        Ok(Self {
            window,
            minimum,
            levels,
        })
    }
    /// Ignore a suffix of the score, using a true prefix comparison rather than
    /// substituting a finite minimum value for a wildcard. At least one ordinary
    /// level must remain. The count is relative to the original score width.
    pub fn ignoring_trailing_levels(
        mut self,
        count: usize,
    ) -> Result<Self, SignificantImprovementError> {
        let width = self.minimum.ordinary_levels().count();
        if count >= width {
            return Err(SignificantImprovementError::InvalidIgnoredLevels);
        }
        self.levels = width - count;
        Ok(self)
    }
    pub fn window(&self) -> Duration {
        self.window
    }
    pub fn minimum(&self) -> &S {
        &self.minimum
    }
    pub fn compared_levels(&self) -> usize {
        self.levels
    }
    pub fn is_met(&self, before: &S, after: &S) -> bool {
        if after <= before {
            return false;
        }
        match after.structural().cmp(&before.structural()) {
            Ordering::Greater => return true,
            Ordering::Less => return false,
            Ordering::Equal => (),
        }
        let exact = |v: S::Level| v.exact_decimal().expect("validated exact score level");
        let order = after
            .ordinary_levels()
            .zip(before.ordinary_levels())
            .zip(self.minimum.ordinary_levels())
            .take(self.levels)
            .map(|((a, b), target)| {
                let difference: Decimal = exact(a).subtract(&exact(b));
                difference.numeric_cmp(&exact(target))
            })
            .find(|o| !o.is_eq())
            .unwrap_or(Ordering::Equal);
        !order.is_lt()
    }
}

/// Deterministic elapsed-time policy. Call start after initialization, then
/// observe retained best scores. A late improvement is retained by the caller
/// but cannot revive an expired policy. Backward time samples clamp monotonically.
/// Large decimal scale gaps retain Decimal's normal arithmetic allocation cost.
///
/// ```
/// use timetuck::{SimpleScore, SignificantImprovement, SignificantImprovementConfig};
/// use std::time::Duration;
/// let ms=Duration::from_millis;
/// let mut limit=SignificantImprovement::new(SignificantImprovementConfig::new(ms(1000),SimpleScore::new(7))?);
/// limit.start(ms(0),SimpleScore::ZERO);
/// assert!(!limit.is_terminated(ms(500),&SimpleScore::new(5)));
/// assert!(!limit.is_terminated(ms(1000),&SimpleScore::new(10)));
/// assert_eq!(limit.progress(ms(1500)),0.5);
/// assert!(limit.is_terminated(ms(2001),&SimpleScore::new(30)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct SignificantImprovement<S> {
    config: SignificantImprovementConfig<S>,
    anchor: Option<(Duration, S)>,
    last: Duration,
    terminated: bool,
}
impl<S: Score> SignificantImprovement<S> {
    pub fn new(config: SignificantImprovementConfig<S>) -> Self {
        Self {
            config,
            anchor: None,
            last: Duration::ZERO,
            terminated: false,
        }
    }
    pub fn start(&mut self, now: Duration, best: S) {
        self.anchor = Some((now, best));
        self.last = now;
        self.terminated = false;
    }
    pub fn is_started(&self) -> bool {
        self.anchor.is_some()
    }
    fn sample(&mut self, now: Duration) {
        self.last = self.last.max(now);
        if let Some((time, _)) = &self.anchor {
            self.terminated |= self.last.saturating_sub(*time) > self.config.window;
        }
    }
    pub(crate) fn expired(&mut self, now: Duration) -> bool {
        self.sample(now);
        self.terminated
    }
    pub fn is_terminated(&mut self, now: Duration, best: &S) -> bool {
        self.sample(now);
        if !self.terminated
            && let Some((_, previous)) = &self.anchor
            && self.config.is_met(previous, best)
        {
            // Clone before replacing the old anchor.
            let value = best.clone();
            self.anchor = Some((self.last, value));
        }
        self.terminated
    }
    /// Capped floating-point estimate; a value of one at the exact deadline
    /// does not imply expiry. Sampling progress also latches a passed deadline.
    pub fn progress(&mut self, now: Duration) -> f64 {
        self.sample(now);
        let Some((start, _)) = &self.anchor else {
            return 0.0;
        };
        crate::ElapsedTimeLimit::new(self.config.window).progress(self.last.saturating_sub(*start))
    }
}
