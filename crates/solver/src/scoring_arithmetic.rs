//! Overflow policy for solver scoring, separate from score value operators.
use crate::AdditiveScore;
use std::{error::Error, fmt};

/// Checked scoring is the default. Wrapping explicitly retains modular integer
/// behavior. Built-in integer scores check impacts and complete ordinary totals;
/// custom bounded scores must implement the checked AdditiveScore hooks.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::<i64>::new(vec![])?;
/// let mut n = ConstraintNetwork::new(0);
/// n.add_constraint("a", vec![], |_| SimpleScore::new(i64::MAX))?;
/// n.add_constraint("b", vec![], |_| SimpleScore::new(1))?;
/// assert_eq!(n.calculate(&p, &p.initial_assignment()),
///     Err(ScoringError::ScoreOverflow { constraint: None }));
/// let modular = n.with_arithmetic(ScoringArithmetic::Wrapping);
/// assert_eq!(modular.calculate(&p, &p.initial_assignment())?, SimpleScore::new(i64::MIN));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScoringArithmetic {
    #[default]
    Checked,
    Wrapping,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreOverflow;
impl fmt::Display for ScoreOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("integer score arithmetic overflow")
    }
}
impl Error for ScoreOverflow {}
impl ScoringArithmetic {
    pub(crate) fn scale<S: AdditiveScore>(
        self,
        weight: &S,
        factor: i64,
    ) -> Result<S, ScoreOverflow> {
        match self {
            Self::Checked => weight.checked_scale(factor).ok_or(ScoreOverflow),
            Self::Wrapping => Ok(crate::constraint_weights::scale(weight, factor)),
        }
    }
    pub(crate) fn sum<'a, S: AdditiveScore + 'a>(
        self,
        scores: impl IntoIterator<Item = &'a S>,
    ) -> Result<S, ScoreOverflow> {
        match self {
            Self::Checked => S::checked_sum(scores).ok_or(ScoreOverflow),
            Self::Wrapping => Ok(scores.into_iter().fold(S::ZERO, |sum, s| sum + s.clone())),
        }
    }
    pub(crate) fn adjust<'a, S: AdditiveScore + 'a>(
        self,
        total: &S,
        changes: impl IntoIterator<Item = (&'a S, &'a S)>,
    ) -> Result<S, ScoreOverflow> {
        match self {
            Self::Checked => total.checked_adjust(changes).ok_or(ScoreOverflow),
            Self::Wrapping => Ok(changes.into_iter().fold(total.clone(), |sum, (old, new)| {
                sum - old.clone() + new.clone()
            })),
        }
    }
}
pub(crate) enum WeightArithmeticError {
    Decimal(crate::DecimalArithmeticError),
    Overflow,
}
impl From<crate::DecimalArithmeticError> for WeightArithmeticError {
    fn from(e: crate::DecimalArithmeticError) -> Self {
        Self::Decimal(e)
    }
}
impl From<ScoreOverflow> for WeightArithmeticError {
    fn from(_: ScoreOverflow) -> Self {
        Self::Overflow
    }
}
