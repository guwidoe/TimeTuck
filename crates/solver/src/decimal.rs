//! Exact decimal values with explicit representation and numeric comparison.
use num_bigint::{BigInt, Sign};
use std::{cmp::Ordering, fmt};

/// Exact `coefficient * 10^(-scale)` with arbitrary-precision integer storage.
/// Equality and hashing include scale. Ordering compares numeric value first,
/// then scale, keeping Ord consistent with Eq (1.0 and 1.00 are distinct keys).
/// Use numeric_cmp when scale should not break ties. Arithmetic retains scale;
/// it never converts through binary floating point.
///
/// Scale spans i32, as in the public BigDecimal value model. Aligning very distant
/// nonzero scales requires storage proportional to that distance; arithmetic has
/// no fixed memory or time bound. Construction/comparison need no such padding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Decimal {
    coefficient: BigInt,
    scale: i32,
}
impl Decimal {
    pub fn new(coefficient: BigInt, scale: i32) -> Self {
        Self { coefficient, scale }
    }
    pub fn coefficient(&self) -> &BigInt {
        &self.coefficient
    }
    pub const fn scale(&self) -> i32 {
        self.scale
    }
    fn at_scale(&self, scale: i32) -> BigInt {
        if self.coefficient == BigInt::ZERO {
            return BigInt::ZERO;
        }
        debug_assert!(scale >= self.scale);
        &self.coefficient * BigInt::from(10).pow((scale as i64 - self.scale as i64) as u32)
    }
    pub fn add(&self, other: &Self) -> Self {
        let scale = self.scale.max(other.scale);
        Self::new(self.at_scale(scale) + other.at_scale(scale), scale)
    }
    pub fn subtract(&self, other: &Self) -> Self {
        let scale = self.scale.max(other.scale);
        Self::new(self.at_scale(scale) - other.at_scale(scale), scale)
    }
    /// Divide at the current scale, rounding to nearest with ties to even.
    /// Zero divisor returns None. The output retains this value's scale.
    pub fn divide_count(&self, count: usize) -> Option<Self> {
        if count == 0 {
            return None;
        }
        Some(Self::new(
            round_half_even(&self.coefficient, count),
            self.scale,
        ))
    }
    pub fn numeric_cmp(&self, other: &Self) -> Ordering {
        let a = self.coefficient.sign();
        let b = other.coefficient.sign();
        let rank = |s| match s {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        };
        if a != b {
            return rank(a).cmp(&rank(b));
        }
        if a == Sign::NoSign {
            return Ordering::Equal;
        }
        let ad = self.coefficient.magnitude().to_str_radix(10);
        let bd = other.coefficient.magnitude().to_str_radix(10);
        let ae = ad.len() as i128 - self.scale as i128;
        let be = bd.len() as i128 - other.scale as i128;
        let mut order = ae.cmp(&be);
        if order == Ordering::Equal {
            let len = ad.len().max(bd.len());
            order = ad
                .bytes()
                .chain(std::iter::repeat(b'0'))
                .take(len)
                .cmp(bd.bytes().chain(std::iter::repeat(b'0')).take(len));
        }
        if a == Sign::Minus {
            order.reverse()
        } else {
            order
        }
    }
}
impl From<BigInt> for Decimal {
    fn from(value: BigInt) -> Self {
        Self::new(value, 0)
    }
}
impl From<i64> for Decimal {
    fn from(value: i64) -> Self {
        Self::from(BigInt::from(value))
    }
}
impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Decimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numeric_cmp(other)
            .then_with(|| self.scale.cmp(&other.scale))
    }
}
impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}e{}", self.coefficient, -(self.scale as i64))
    }
}
pub(crate) fn round_half_even(value: &BigInt, count: usize) -> BigInt {
    debug_assert!(count != 0);
    let divisor = BigInt::from(count);
    let mut quotient = value / &divisor;
    let remainder = value % &divisor;
    let twice = remainder.magnitude() * 2u8;
    let comparison = twice.cmp(divisor.magnitude());
    if comparison == Ordering::Greater
        || (comparison == Ordering::Equal && &quotient % 2u8 != BigInt::ZERO)
    {
        if value.sign() == Sign::Minus {
            quotient -= 1u8;
        } else {
            quotient += 1u8;
        }
    }
    quotient
}
