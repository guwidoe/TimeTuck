//! Owned benchmark score comparison and aggregation, independent of a runner.
use crate::{Decimal, Score, ScoreLevel};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkResultError {
    UnsupportedScoreLevel,
    DifferentDimensions { left: usize, right: usize },
}
impl fmt::Display for BenchmarkResultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedScoreLevel => {
                f.write_str("benchmark scores require exact numeric levels")
            }
            Self::DifferentDimensions { left, right } => {
                write!(f, "benchmark score dimensions differ: {left} and {right}")
            }
        }
    }
}
impl Error for BenchmarkResultError {}

/// Exact, owned score suitable for comparing different built-in score families.
/// Missing trailing levels compare as zero. Decimal scale does not break ties.
/// Structural state precedes ordinary levels. Cross-family comparison is numeric;
/// it does not establish that the two models have equivalent objective semantics.
#[derive(Clone, Debug)]
pub struct BenchmarkScore {
    structural: i64,
    levels: Vec<Decimal>,
}
impl BenchmarkScore {
    pub fn from_score<S: Score>(score: &S) -> Result<Self, BenchmarkResultError> {
        let levels = score
            .ordinary_levels()
            .map(|level| {
                level
                    .exact_decimal()
                    .ok_or(BenchmarkResultError::UnsupportedScoreLevel)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            structural: score.structural(),
            levels,
        })
    }
    pub fn levels(&self) -> &[Decimal] {
        &self.levels
    }
    pub fn structural(&self) -> i64 {
        self.structural
    }
}
impl Ord for BenchmarkScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.structural.cmp(&other.structural).then_with(|| {
            (0..self.levels.len().max(other.levels.len()))
                .map(|i| {
                    self.levels
                        .get(i)
                        .unwrap_or(&Decimal::ZERO)
                        .numeric_cmp(other.levels.get(i).unwrap_or(&Decimal::ZERO))
                })
                .find(|order| !order.is_eq())
                .unwrap_or(Ordering::Equal)
        })
    }
}
impl PartialOrd for BenchmarkScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for BenchmarkScore {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for BenchmarkScore {}

/// Fractional improvement by ordinary score level: (candidate - baseline) /
/// abs(baseline). 0.9 means a 90% improvement; negative means deterioration.
/// Structural state is excluded. Equal zeroes yield zero; a nonzero change from
/// zero yields signed infinity. Estimates preserve cancellation before conversion
/// to f64 and support very large/small decimals without inf/inf or 0/0.
/// Arithmetic follows IEEE floating-point rules, including NaN from inf - inf.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreDifferencePercentage(Vec<f64>);
impl ScoreDifferencePercentage {
    pub fn between<A: Score, B: Score>(
        baseline: &A,
        candidate: &B,
    ) -> Result<Self, BenchmarkResultError> {
        let baseline = BenchmarkScore::from_score(baseline)?;
        let candidate = BenchmarkScore::from_score(candidate)?;
        same_dimensions(baseline.levels.len(), candidate.levels.len())?;
        Ok(Self(
            baseline
                .levels
                .iter()
                .zip(&candidate.levels)
                .map(|(a, b)| {
                    let magnitude =
                        Decimal::new(a.coefficient().magnitude().clone().into(), a.scale());
                    match b.numeric_cmp(a) {
                        Ordering::Equal => 0.0,
                        Ordering::Greater => b.cooling_loss(a, &magnitude, 0.0),
                        Ordering::Less => -a.cooling_loss(b, &magnitude, 0.0),
                    }
                })
                .collect(),
        ))
    }
    pub fn levels(&self) -> &[f64] {
        &self.0
    }
    pub fn add(&self, other: &Self) -> Result<Self, BenchmarkResultError> {
        same_dimensions(self.0.len(), other.0.len())?;
        Ok(Self(
            self.0.iter().zip(&other.0).map(|(a, b)| a + b).collect(),
        ))
    }
    pub fn subtract(&self, other: &Self) -> Result<Self, BenchmarkResultError> {
        same_dimensions(self.0.len(), other.0.len())?;
        Ok(Self(
            self.0.iter().zip(&other.0).map(|(a, b)| a - b).collect(),
        ))
    }
    pub fn multiply(&self, factor: f64) -> Self {
        Self(self.0.iter().map(|v| v * factor).collect())
    }
    pub fn divide(&self, divisor: f64) -> Self {
        Self(self.0.iter().map(|v| v / divisor).collect())
    }
}
fn same_dimensions(left: usize, right: usize) -> Result<(), BenchmarkResultError> {
    if left == right {
        Ok(())
    } else {
        Err(BenchmarkResultError::DifferentDimensions { left, right })
    }
}

/// One measured outcome. Initialization is separate from its numeric score.
/// A negative structural component also marks a successful result incomplete.
#[derive(Clone, Debug)]
pub enum BenchmarkOutcome {
    Success {
        score: BenchmarkScore,
        initialized: bool,
    },
    Failure,
}
impl BenchmarkOutcome {
    pub fn success<S: Score>(score: &S, initialized: bool) -> Result<Self, BenchmarkResultError> {
        Ok(Self::Success {
            score: BenchmarkScore::from_score(score)?,
            initialized,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkRanking {
    TotalScore,
    WorstScore,
}

/// Owned aggregate across a configuration's benchmark outcomes. Fewer failures
/// rank first, then fewer incomplete results. TotalScore compares exact sums and
/// breaks ties by the ordered worst-to-best outcomes. WorstScore compares that
/// entire ordered sequence. Initialized outcomes precede incomplete ones in quality.
/// Greater means better. An empty summary ranks below every nonempty summary.
/// Compare matching problem/repetition sets for meaningful configuration rankings;
/// their identities and run scheduling remain the caller's responsibility.
/// Decimal sums have Decimal's ordinary scale-alignment allocation cost.
///
/// ```
/// use timetuck::*;
/// let a = BenchmarkSummary::new([
///     BenchmarkOutcome::success(&SimpleScore::new(-10), true)?,
///     BenchmarkOutcome::success(&SimpleScore::new(-20), true)?,
/// ]);
/// let b = BenchmarkSummary::new([BenchmarkOutcome::Failure, BenchmarkOutcome::Failure]);
/// assert!(a.compare(&b, BenchmarkRanking::TotalScore).is_gt());
/// assert_eq!(a.total_score().unwrap().levels(), &[Decimal::from(-30)]);
/// # Ok::<(), BenchmarkResultError>(())
/// ```
#[derive(Clone, Debug)]
pub struct BenchmarkSummary {
    runs: usize,
    failures: usize,
    incomplete: usize,
    total: Option<BenchmarkScore>,
    ordered: Vec<(bool, BenchmarkScore)>,
}
impl BenchmarkSummary {
    pub fn new(outcomes: impl IntoIterator<Item = BenchmarkOutcome>) -> Self {
        let mut result = Self {
            runs: 0,
            failures: 0,
            incomplete: 0,
            total: None,
            ordered: vec![],
        };
        for outcome in outcomes {
            result.runs += 1;
            match outcome {
                BenchmarkOutcome::Failure => result.failures += 1,
                BenchmarkOutcome::Success { score, initialized } => {
                    let complete = initialized && score.structural >= 0;
                    result.incomplete += usize::from(!complete);
                    let total = result.total.get_or_insert_with(|| BenchmarkScore {
                        structural: 0,
                        levels: vec![],
                    });
                    total
                        .levels
                        .resize(total.levels.len().max(score.levels.len()), Decimal::ZERO);
                    for (sum, value) in total.levels.iter_mut().zip(&score.levels) {
                        *sum = sum.add(value);
                    }
                    result.ordered.push((complete, score));
                }
            }
        }
        result.ordered.sort();
        result
    }
    pub fn run_count(&self) -> usize {
        self.runs
    }
    pub fn failure_count(&self) -> usize {
        self.failures
    }
    pub fn incomplete_count(&self) -> usize {
        self.incomplete
    }
    /// Exact ordinary-level sum of successful outcomes; failures contribute no score.
    /// Structural state is represented by incomplete_count, not added to the sum.
    pub fn total_score(&self) -> Option<&BenchmarkScore> {
        self.total.as_ref()
    }
    pub fn compare(&self, other: &Self, ranking: BenchmarkRanking) -> Ordering {
        (self.runs != 0)
            .cmp(&(other.runs != 0))
            .then_with(|| other.failures.cmp(&self.failures))
            .then_with(|| other.incomplete.cmp(&self.incomplete))
            .then_with(|| match ranking {
                BenchmarkRanking::TotalScore => self
                    .total
                    .cmp(&other.total)
                    .then_with(|| self.ordered.cmp(&other.ordered)),
                BenchmarkRanking::WorstScore => self.ordered.cmp(&other.ordered),
            })
    }
}
