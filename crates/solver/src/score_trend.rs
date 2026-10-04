//! Declared per-level initialization trends and exact lexicographic bounds.
use crate::{AdditiveScore, Score, ScoreLevel};
use std::{cmp::Ordering, error::Error, fmt};

/// Caller-supplied behavior of an ordinary score level during initialization.
/// This must describe every completion, including optional omissions and later
/// list insertions. Integer overflow and constraints becoming unmatched matter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InitializingScoreTrendLevel {
    #[default]
    Any,
    OnlyUp,
    OnlyDown,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoreTrendError {
    UnknownLevel { index: usize, text: String },
    LevelCount { expected: usize, actual: usize },
}
impl fmt::Display for ScoreTrendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLevel { index, text } => {
                write!(f, "invalid score trend level {index}: {text:?}")
            }
            Self::LevelCount { expected, actual } => {
                write!(f, "score trend has {actual} levels; expected {expected}")
            }
        }
    }
}
impl Error for ScoreTrendError {}

/// A declaration, never inferred from constraint names or weight signs.
/// A single parsed token applies to every ordinary level; slash-separated tokens
/// must match the level count. Structural state is not a trend level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializingScoreTrend(Vec<InitializingScoreTrendLevel>);
impl InitializingScoreTrend {
    pub fn new(levels: Vec<InitializingScoreTrendLevel>) -> Self {
        Self(levels)
    }
    pub fn parse(text: &str, level_count: usize) -> Result<Self, ScoreTrendError> {
        let levels = text
            .split('/')
            .enumerate()
            .map(|(index, text)| {
                Ok(match text {
                    "ANY" => InitializingScoreTrendLevel::Any,
                    "ONLY_UP" => InitializingScoreTrendLevel::OnlyUp,
                    "ONLY_DOWN" => InitializingScoreTrendLevel::OnlyDown,
                    _ => {
                        return Err(ScoreTrendError::UnknownLevel {
                            index,
                            text: text.into(),
                        });
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if levels.len() == 1 {
            Ok(Self(vec![levels[0]; level_count]))
        } else if levels.len() == level_count {
            Ok(Self(levels))
        } else {
            Err(ScoreTrendError::LevelCount {
                expected: level_count,
                actual: levels.len(),
            })
        }
    }
    pub fn levels(&self) -> &[InitializingScoreTrendLevel] {
        &self.0
    }
    pub fn is_only_up(&self) -> bool {
        self.0
            .iter()
            .all(|t| *t == InitializingScoreTrendLevel::OnlyUp)
    }
    pub fn is_only_down(&self) -> bool {
        self.0
            .iter()
            .all(|t| *t == InitializingScoreTrendLevel::OnlyDown)
    }
    pub(crate) fn validate<S: Score>(&self, score: &S) -> Result<(), ScoreTrendError> {
        let expected = score.ordinary_levels().count();
        if self.0.len() == expected {
            Ok(())
        } else {
            Err(ScoreTrendError::LevelCount {
                expected,
                actual: self.0.len(),
            })
        }
    }
    /// Derive an upper bound. ONLY_DOWN levels retain their partial value; other
    /// levels become positive infinity. No arbitrary finite maximum is substituted.
    pub fn optimistic_bound<S: AdditiveScore>(
        &self,
        partial: S,
    ) -> Result<InitializingScoreBound<S>, ScoreTrendError> {
        self.bound(
            partial,
            InitializingScoreTrendLevel::OnlyDown,
            Ordering::Greater,
        )
    }
    /// Derive a lower bound. ONLY_UP levels retain their partial value; others
    /// become negative infinity. This is not a scored feasible assignment and
    /// cannot be used as a solver incumbent without a witness solution.
    pub fn pessimistic_bound<S: AdditiveScore>(
        &self,
        partial: S,
    ) -> Result<InitializingScoreBound<S>, ScoreTrendError> {
        self.bound(partial, InitializingScoreTrendLevel::OnlyUp, Ordering::Less)
    }
    fn bound<S: AdditiveScore>(
        &self,
        partial: S,
        finite: InitializingScoreTrendLevel,
        infinity: Ordering,
    ) -> Result<InitializingScoreBound<S>, ScoreTrendError> {
        self.validate(&partial)?;
        let prefix = self.0.iter().take_while(|&&t| t == finite).count();
        Ok(InitializingScoreBound {
            score: partial.with_structural(0),
            prefix,
            tail: (prefix < self.0.len()).then_some(infinity),
        })
    }
}

/// A lexicographic bound with exact finite leading levels and an optional
/// infinite next level. Levels after the first infinity cannot affect ordering.
/// Derived bounds target structurally complete (zero structural state) scores.
/// Score ordering must be structural-first and then lexicographic numeric levels.
/// No floating-point conversion or finite numeric sentinel is used.
///
/// ```
/// use timetuck::{InitializingScoreTrend, HardSoftScore};
/// let trend = InitializingScoreTrend::parse("ONLY_DOWN/ANY", 2)?;
/// let bound = trend.optimistic_bound(HardSoftScore::new(-2, 7).with_structural(-3))?;
/// assert!(bound.compare_score(&HardSoftScore::new(-1, i64::MIN)).is_lt());
/// assert!(bound.compare_score(&HardSoftScore::new(-2, i64::MAX)).is_gt());
/// assert_eq!(bound.finite_prefix_len(), 1);
/// assert!(!bound.is_finite());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct InitializingScoreBound<S> {
    score: S,
    prefix: usize,
    // None: finite score. Less/Greater: negative/positive infinity respectively.
    tail: Option<Ordering>,
}
impl<S: Score> InitializingScoreBound<S> {
    pub(crate) fn finite(score: S) -> Self {
        Self {
            prefix: score.ordinary_levels().count(),
            score,
            tail: None,
        }
    }
    pub(crate) fn into_finite(self) -> Option<S> {
        self.tail.is_none().then_some(self.score)
    }
    pub fn is_finite(&self) -> bool {
        self.tail.is_none()
    }
    pub fn finite_prefix_len(&self) -> usize {
        self.prefix
    }
    /// The retained partial score (structural state cleared for derived bounds).
    /// Values beyond finite_prefix_len do not represent finite bound levels.
    pub fn partial_score(&self) -> &S {
        &self.score
    }
    pub fn compare_score(&self, score: &S) -> Ordering {
        if self.tail.is_none() {
            return self.score.cmp(score);
        }
        self.score
            .structural()
            .cmp(&score.structural())
            .then_with(|| {
                self.score
                    .ordinary_levels()
                    .zip(score.ordinary_levels())
                    .take(self.prefix)
                    .map(|(a, b)| a.numeric_cmp(&b))
                    .find(|o| !o.is_eq())
                    .unwrap_or(self.tail.unwrap())
            })
    }
    pub(crate) fn ordinary_cmp(&self, other: &Self) -> Ordering {
        let prefix_cmp = self
            .score
            .ordinary_levels()
            .zip(other.score.ordinary_levels())
            .take(self.prefix.min(other.prefix))
            .map(|(a, b)| a.numeric_cmp(&b))
            .find(|o| !o.is_eq())
            .unwrap_or(Ordering::Equal);
        prefix_cmp.then_with(|| match self.prefix.cmp(&other.prefix) {
            Ordering::Less => self.tail.expect("shorter prefix ends at infinity"),
            Ordering::Greater => other
                .tail
                .expect("shorter prefix ends at infinity")
                .reverse(),
            Ordering::Equal => self
                .tail
                .unwrap_or(Ordering::Equal)
                .cmp(&other.tail.unwrap_or(Ordering::Equal)),
        })
    }
}
impl<S: Score> Ord for InitializingScoreBound<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.tail.is_none() && other.tail.is_none() {
            return self.score.cmp(&other.score);
        }
        self.score
            .structural()
            .cmp(&other.score.structural())
            .then_with(|| self.ordinary_cmp(other))
    }
}
impl<S: Score> PartialOrd for InitializingScoreBound<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<S: Score> PartialEq for InitializingScoreBound<S> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl<S: Score> Eq for InitializingScoreBound<S> {}
