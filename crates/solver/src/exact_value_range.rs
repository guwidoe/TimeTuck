//! Arbitrary-precision arithmetic progressions for indexed planning ranges.
use crate::{BigInt, Decimal, ValueRange};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactRangeError {
    DescendingBounds,
    NonPositiveStep,
    CountOverflow,
    ScaleMismatch { expected: i32, actual: i32 },
}
impl fmt::Display for ExactRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescendingBounds => f.write_str("range start must not exceed its end"),
            Self::NonPositiveStep => f.write_str("range step must be positive"),
            Self::CountOverflow => f.write_str("range occurrence count exceeds u128"),
            Self::ScaleMismatch { expected, actual } => write!(
                f,
                "range scale {actual} differs from required scale {expected}"
            ),
        }
    }
}
impl Error for ExactRangeError {}

/// Half-open progression with unbounded integer endpoints/steps and a checked
/// u128 occurrence count. A nondividing step ends at the last member below end.
/// Construction stores only endpoints/counts, not the sequence's values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BigIntegerValueRange {
    start: BigInt,
    end: BigInt,
    step: BigInt,
    count: u128,
}
impl BigIntegerValueRange {
    pub fn new(start: BigInt, end: BigInt) -> Result<Self, ExactRangeError> {
        Self::with_step(start, end, BigInt::from(1))
    }
    pub fn with_step(start: BigInt, end: BigInt, step: BigInt) -> Result<Self, ExactRangeError> {
        if start > end {
            return Err(ExactRangeError::DescendingBounds);
        }
        if step <= BigInt::ZERO {
            return Err(ExactRangeError::NonPositiveStep);
        }
        let width = &end - &start;
        let count = u128::try_from((&width + &step - BigInt::from(1)) / &step)
            .map_err(|_| ExactRangeError::CountOverflow)?;
        Ok(Self {
            start,
            end,
            step,
            count,
        })
    }
}
impl ValueRange for BigIntegerValueRange {
    type Value = BigInt;
    fn len(&self) -> u128 {
        self.count
    }
    fn get(&self, index: u128) -> Option<BigInt> {
        (index < self.count).then(|| &self.start + &self.step * BigInt::from(index))
    }
    fn contains(&self, value: &BigInt) -> bool {
        value >= &self.start
            && value < &self.end
            && (value - &self.start) % &self.step == BigInt::ZERO
    }
}

/// Exact decimal progression at one explicit scale. Endpoints and an explicit step
/// must all have that scale. The default step is one unit at the endpoints' scale:
/// scale 2 means 0.01; scale -2 means 100. No binary floating point is used.
///
/// Membership is representation-sensitive, matching Decimal's Eq: 1.0 and 1.00
/// belong to different scale domains. Convert deliberately before combining them.
/// Rejecting mismatched scales avoids silently changing precision or allocating
/// padding proportional to arbitrary scale differences. Counts must fit u128.
///
/// ```
/// use timetuck::*;
/// let r = DecimalValueRange::new("1.00".parse()?, "1.04".parse()?)?;
/// assert_eq!(r.len(), 4);
/// assert_eq!(r.get(2), Some("1.02".parse()?));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecimalValueRange {
    coefficients: BigIntegerValueRange,
    scale: i32,
}
impl DecimalValueRange {
    pub fn new(start: Decimal, end: Decimal) -> Result<Self, ExactRangeError> {
        let step = Decimal::new(BigInt::from(1), start.scale());
        Self::with_step(start, end, step)
    }
    pub fn with_step(start: Decimal, end: Decimal, step: Decimal) -> Result<Self, ExactRangeError> {
        let scale = start.scale();
        for actual in [end.scale(), step.scale()] {
            if actual != scale {
                return Err(ExactRangeError::ScaleMismatch {
                    expected: scale,
                    actual,
                });
            }
        }
        let coefficients = BigIntegerValueRange::with_step(
            start.coefficient().clone(),
            end.coefficient().clone(),
            step.coefficient().clone(),
        )?;
        Ok(Self {
            coefficients,
            scale,
        })
    }
    pub fn scale(&self) -> i32 {
        self.scale
    }
}
impl ValueRange for DecimalValueRange {
    type Value = Decimal;
    fn len(&self) -> u128 {
        self.coefficients.len()
    }
    fn get(&self, index: u128) -> Option<Decimal> {
        self.coefficients
            .get(index)
            .map(|v| Decimal::new(v, self.scale))
    }
    fn contains(&self, value: &Decimal) -> bool {
        value.scale() == self.scale && self.coefficients.contains(value.coefficient())
    }
}
