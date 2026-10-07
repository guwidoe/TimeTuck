//! Typed score shapes, level-wise normalization and finite integer extrema.
use crate::{
    AdditiveScore, BendableDecimalScore, BendableScore, Decimal, DecimalArithmeticError,
    HardMediumSoftDecimalScore, HardMediumSoftScore, HardSoftDecimalScore, HardSoftScore,
    InitializingScoreTrend, InitializingScoreTrendLevel, Score, ScoreTrendError,
    SimpleDecimalScore, SimpleScore,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoreDefinitionError {
    LevelCount { expected: usize, actual: usize },
    NoLevels,
    IntegerOverflow { level: usize },
    Decimal(DecimalArithmeticError),
    Trend(ScoreTrendError),
}
impl fmt::Display for ScoreDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "score definition: {self:?}")
    }
}
impl Error for ScoreDefinitionError {}

/// Construction and metadata for a fixed score shape, including const-generic
/// bendable shapes. All constructors and arithmetic here clear structural state.
/// Existing score values and their exact ordering remain unchanged.
///
/// ```
/// use timetuck::*;
/// let score = HardSoftScore::from_levels(vec![-7, 10])?;
/// let divisor = HardSoftScore::from_levels(vec![2, 0])?;
/// assert_eq!(score.divide_by_sanitized_divisor(&divisor)?, HardSoftScore::new(-4, 10));
/// assert_eq!(HardSoftScore::level_labels(), ["hard score", "soft score"]);
/// # Ok::<(), ScoreDefinitionError>(())
/// ```
pub trait ScoreDefinition: AdditiveScore {
    fn level_labels() -> Vec<String>;
    fn from_levels(levels: Vec<Self::Level>) -> Result<Self, ScoreDefinitionError>;
    fn one_softest_score() -> Result<Self, ScoreDefinitionError>;
    fn zero_score() -> Self {
        Self::ZERO
    }
    fn level_count() -> usize {
        Self::ZERO.ordinary_levels().count()
    }
    fn feasible_level_count() -> usize {
        Self::HARD_LEVEL_COUNT
    }
    /// Divide corresponding levels, replacing numeric zero divisors with one.
    /// Integer quotients floor exactly and reject MIN/-1 overflow. Decimal
    /// quotients floor at max(dividend scale, 0), using no floating conversion.
    fn divide_by_sanitized_divisor(&self, divisor: &Self) -> Result<Self, ScoreDefinitionError>;
    /// Represent an optimistic trend bound using the integer type's finite MAX.
    /// ONLY_DOWN levels retain their current value; other levels use MAX.
    /// This is distinct from the solver's infinity-based InitializingScoreBound.
    /// Trend declarations must hold for every completion, including overflow.
    fn finite_optimistic_bound(
        &self,
        trend: &InitializingScoreTrend,
    ) -> Result<Self, ScoreDefinitionError>
    where
        Self: Score<Level = i64>,
    {
        finite_integer_bound(self, trend, InitializingScoreTrendLevel::OnlyDown, i64::MAX)
    }
    /// ONLY_UP levels retain their value; other levels use the integer MIN.
    /// A bound is not a feasible incumbent without a witness assignment.
    fn finite_pessimistic_bound(
        &self,
        trend: &InitializingScoreTrend,
    ) -> Result<Self, ScoreDefinitionError>
    where
        Self: Score<Level = i64>,
    {
        finite_integer_bound(self, trend, InitializingScoreTrendLevel::OnlyUp, i64::MIN)
    }
}
fn finite_integer_bound<S: ScoreDefinition<Level = i64>>(
    score: &S,
    trend: &InitializingScoreTrend,
    retained: InitializingScoreTrendLevel,
    extreme: i64,
) -> Result<S, ScoreDefinitionError> {
    trend.validate(score).map_err(ScoreDefinitionError::Trend)?;
    S::from_levels(
        score
            .ordinary_levels()
            .zip(trend.levels())
            .map(|(value, &level)| if level == retained { value } else { extreme })
            .collect(),
    )
}
fn shape_check(actual: usize, expected: usize) -> Result<(), ScoreDefinitionError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ScoreDefinitionError::LevelCount { expected, actual })
    }
}
fn softest_unit<S: ScoreDefinition>(one: S::Level) -> Result<S, ScoreDefinitionError> {
    let mut levels: Vec<_> = S::ZERO.ordinary_levels().collect();
    *levels.last_mut().ok_or(ScoreDefinitionError::NoLevels)? = one;
    S::from_levels(levels)
}
fn divide_integer(level: usize, dividend: i64, divisor: i64) -> Result<i64, ScoreDefinitionError> {
    let a = i128::from(dividend);
    let b = i128::from(if divisor == 0 { 1 } else { divisor });
    let quotient = a / b;
    let floor = quotient - i128::from(a % b != 0 && (a < 0) != (b < 0));
    i64::try_from(floor).map_err(|_| ScoreDefinitionError::IntegerOverflow { level })
}
fn divide_decimal(
    _: usize,
    dividend: Decimal,
    divisor: Decimal,
) -> Result<Decimal, ScoreDefinitionError> {
    let divisor = if divisor.coefficient() == &crate::BigInt::ZERO {
        Decimal::from(1)
    } else {
        divisor
    };
    dividend
        .divide_floor(&divisor)
        .map_err(ScoreDefinitionError::Decimal)
}
fn divided<S: ScoreDefinition>(
    a: &S,
    b: &S,
    divide: impl Fn(usize, S::Level, S::Level) -> Result<S::Level, ScoreDefinitionError>,
) -> Result<S, ScoreDefinitionError> {
    S::from_levels(
        a.ordinary_levels()
            .zip(b.ordinary_levels())
            .enumerate()
            .map(|(i, (a, b))| divide(i, a, b))
            .collect::<Result<_, _>>()?,
    )
}
macro_rules! fixed_definition {
    ($name:ty, $level:ty, $one:expr, $divide:ident, $n:expr, [$($index:tt => $label:literal),+]) => {
        impl ScoreDefinition for $name {
            fn level_labels() -> Vec<String> { vec![$($label.into()),+] }
            fn from_levels(levels: Vec<$level>) -> Result<Self, ScoreDefinitionError> {
                shape_check(levels.len(), $n)?;
                let mut values = levels.into_iter();
                Ok(Self::new($({ let _ = $index; values.next().expect("validated score shape") }),+))
            }
            fn one_softest_score() -> Result<Self, ScoreDefinitionError> { softest_unit::<Self>($one) }
            fn divide_by_sanitized_divisor(&self, divisor: &Self) -> Result<Self, ScoreDefinitionError> { divided(self, divisor, $divide) }
        }
    };
}
fixed_definition!(SimpleScore, i64, 1, divide_integer, 1, [0 => "score"]);
fixed_definition!(HardSoftScore, i64, 1, divide_integer, 2, [0 => "hard score", 1 => "soft score"]);
fixed_definition!(HardMediumSoftScore, i64, 1, divide_integer, 3, [0 => "hard score", 1 => "medium score", 2 => "soft score"]);
fixed_definition!(SimpleDecimalScore, Decimal, Decimal::from(1), divide_decimal, 1, [0 => "score"]);
fixed_definition!(HardSoftDecimalScore, Decimal, Decimal::from(1), divide_decimal, 2, [0 => "hard score", 1 => "soft score"]);
fixed_definition!(HardMediumSoftDecimalScore, Decimal, Decimal::from(1), divide_decimal, 3, [0 => "hard score", 1 => "medium score", 2 => "soft score"]);
macro_rules! bendable_definition {
    ($name:ident, $level:ty, $one:expr, $divide:ident) => {
        impl<const H: usize, const S: usize> ScoreDefinition for $name<H, S> {
            fn level_labels() -> Vec<String> {
                (0..H)
                    .map(|i| format!("hard {i} score"))
                    .chain((0..S).map(|i| format!("soft {i} score")))
                    .collect()
            }
            fn from_levels(levels: Vec<$level>) -> Result<Self, ScoreDefinitionError> {
                shape_check(levels.len(), Self::level_count())?;
                let mut values = levels.into_iter();
                Ok(Self::new(
                    std::array::from_fn(|_| values.next().expect("validated hard levels")),
                    std::array::from_fn(|_| values.next().expect("validated soft levels")),
                ))
            }
            fn one_softest_score() -> Result<Self, ScoreDefinitionError> {
                softest_unit::<Self>($one)
            }
            fn divide_by_sanitized_divisor(
                &self,
                divisor: &Self,
            ) -> Result<Self, ScoreDefinitionError> {
                divided(self, divisor, $divide)
            }
        }
    };
}
bendable_definition!(BendableScore, i64, 1, divide_integer);
bendable_definition!(
    BendableDecimalScore,
    Decimal,
    Decimal::from(1),
    divide_decimal
);
