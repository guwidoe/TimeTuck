//! Score-based stopping criteria and weighted progress estimates.
use crate::Score;
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoreProgressError {
    FlawedStart,
    FlawedTarget,
    WeightCount { expected: usize, actual: usize },
    InvalidWeight { index: usize },
    HardLevelCount,
}
impl fmt::Display for ScoreProgressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FlawedStart => {
                f.write_str("progress requires a structurally sound starting score")
            }
            Self::FlawedTarget => {
                f.write_str("progress requires a structurally sound target score")
            }
            Self::WeightCount { expected, actual } => {
                write!(f, "expected {expected} level split weights, got {actual}")
            }
            Self::InvalidWeight { index } => {
                write!(f, "level split weight {index} must be finite and in [0, 1]")
            }
            Self::HardLevelCount => f.write_str("hard level count exceeds ordinary level count"),
        }
    }
}
impl Error for ScoreProgressError {}

fn weights(levels: usize, splits: &[f64]) -> Result<(), ScoreProgressError> {
    let expected = levels.saturating_sub(1);
    if splits.len() != expected {
        return Err(ScoreProgressError::WeightCount {
            expected,
            actual: splits.len(),
        });
    }
    if let Some(index) = splits
        .iter()
        .position(|w| !w.is_finite() || !(0.0..=1.0).contains(w))
    {
        return Err(ScoreProgressError::InvalidWeight { index });
    }
    Ok(())
}

// Integer differences are widened before conversion; subtracting two rounded
// f64 scores could lose even a small, exactly representable score improvement.
fn level_fraction(start: i64, target: i64, current: i64) -> f64 {
    if current >= target {
        1.0
    } else if current <= start || target <= start {
        0.0
    } else {
        (i128::from(current) - i128::from(start)) as f64
            / (i128::from(target) - i128::from(start)) as f64
    }
}
fn weighted(
    start: impl Iterator<Item = i64>,
    target: impl Iterator<Item = i64>,
    current: impl Iterator<Item = i64>,
    splits: &[f64],
) -> f64 {
    let mut remaining = 1.0;
    let mut progress = 0.0;
    for (i, ((start, target), current)) in start.zip(target).zip(current).enumerate() {
        let split = splits.get(i).copied().unwrap_or(1.0);
        progress += level_fraction(start, target, current) * remaining * split;
        remaining *= 1.0 - split;
    }
    progress.clamp(0.0, 1.0)
}

/// A fixed starting score, target, and weighted estimate of progress toward it.
/// For N ordinary levels, provide N-1 split weights. Each weight allocates that
/// fraction of the remaining share to its level; the final level gets the rest.
/// Use `is_reached` for termination: a rounded or zero-weight progress estimate
/// can be 1 before the exact target is reached. Negative structural states report
/// zero progress and never reach the target. The start and target must be sound.
///
/// ```
/// use timetuck::{HardSoftScore, ScoreTargetProgress};
/// let p = ScoreTargetProgress::new(HardSoftScore::new(-20,-400),
///     HardSoftScore::new(-10,-300), vec![0.75]).unwrap();
/// assert_eq!(p.progress(HardSoftScore::new(-14,-340)), 0.6);
/// assert!(!p.is_reached(HardSoftScore::new(-14,-340)));
/// ```
#[derive(Clone, Debug)]
pub struct ScoreTargetProgress<S> {
    start: S,
    target: S,
    splits: Vec<f64>,
}
impl<S: Score> ScoreTargetProgress<S> {
    pub fn new(start: S, target: S, splits: Vec<f64>) -> Result<Self, ScoreProgressError> {
        if start.structural() < 0 {
            return Err(ScoreProgressError::FlawedStart);
        }
        if target.structural() < 0 {
            return Err(ScoreProgressError::FlawedTarget);
        }
        weights(start.ordinary_levels().count(), &splits)?;
        Ok(Self {
            start,
            target,
            splits,
        })
    }
    pub fn is_reached(&self, best: S) -> bool {
        best.structural() >= 0 && best >= self.target
    }
    /// The exact target check wins, then lexicographic regression below the start
    /// gives zero. Other scores use capped per-level fractions. For a flat or
    /// reversed level interval, its share is earned iff that level meets target.
    pub fn progress(&self, best: S) -> f64 {
        if best.structural() < 0 {
            return 0.0;
        }
        if best >= self.target {
            return 1.0;
        }
        if best < self.start {
            return 0.0;
        }
        weighted(
            self.start.ordinary_levels(),
            self.target.ordinary_levels(),
            best.ordinary_levels(),
            &self.splits,
        )
    }
}

/// Progress toward zero on every hard level, ignoring soft levels. Split weights
/// are relative to the hard levels only. Positive hard values do not compensate
/// for a negative hard level. No-hard-level scores are feasible when structurally
/// sound. Use `is_reached` for the exact feasibility decision, not the estimate.
#[derive(Clone, Debug)]
pub struct FeasibilityProgress<S> {
    start: S,
    splits: Vec<f64>,
}
impl<S: Score> FeasibilityProgress<S> {
    pub fn new(start: S, splits: Vec<f64>) -> Result<Self, ScoreProgressError> {
        if start.structural() < 0 {
            return Err(ScoreProgressError::FlawedStart);
        }
        if S::HARD_LEVEL_COUNT > start.ordinary_levels().count() {
            return Err(ScoreProgressError::HardLevelCount);
        }
        weights(S::HARD_LEVEL_COUNT, &splits)?;
        Ok(Self { start, splits })
    }
    pub fn is_reached(&self, best: S) -> bool {
        best.is_feasible()
    }
    pub fn progress(&self, best: S) -> f64 {
        if best.structural() < 0 {
            return 0.0;
        }
        if best.is_feasible() {
            return 1.0;
        }
        weighted(
            self.start.ordinary_levels().take(S::HARD_LEVEL_COUNT),
            std::iter::repeat(0),
            best.ordinary_levels().take(S::HARD_LEVEL_COUNT),
            &self.splits,
        )
    }
}
