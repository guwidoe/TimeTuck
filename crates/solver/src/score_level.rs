//! Numeric score levels used by progress estimates and simulated annealing.
use crate::Decimal;
use std::cmp::Ordering;

/// Arithmetic needed by search for an ordinary score level. Numeric comparison
/// must ignore representation differences (for example decimal trailing zeroes).
/// Implementations must preserve small differences before converting to f64.
/// Estimates may round; exact ordering and termination belong to the Score.
/// All methods must leave the supplied values unchanged.
///
/// Decimal estimates preserve cancellation before conversion, using a bounded
/// significand and a separate exponent. They are floating-point approximations,
/// not correctly-rounded decimal arithmetic or exact termination predicates.
///
/// ```
/// use timetuck::{Decimal, ScoreLevel};
/// let start = Decimal::new(10.into(), 1000);
/// let target = Decimal::new(20.into(), 1000);
/// let current = Decimal::new(16.into(), 1000);
/// assert!((start.progress_fraction(&target, &current) - 0.6).abs() < 1e-15);
/// ```
pub trait ScoreLevel: Clone {
    /// Optional exact arithmetic representation for rising score thresholds.
    /// Return Some for every value or None for the entire level type. The scale
    /// specifies the level's arithmetic precision (integer levels use zero).
    /// Unsupported levels remain usable by all other existing search policies.
    fn exact_decimal(&self) -> Option<Decimal> {
        None
    }
    fn zero() -> Self;
    fn numeric_cmp(&self, other: &Self) -> Ordering;
    /// Capped progress from self to target. A flat/reversed interval earns its
    /// share iff current meets target. The result must be finite and in [0, 1].
    fn progress_fraction(&self, target: &Self, current: &Self) -> f64;
    /// Nonnegative loss from self to candidate divided by the temperature cooled
    /// by gradient. Gradient is finite in [0, 1]; temperature is nonnegative.
    /// Non-declining levels return zero. A decline at zero temperature returns
    /// positive infinity. The result must never be NaN.
    fn cooling_loss(&self, candidate: &Self, temperature: &Self, gradient: f64) -> f64;
}
impl ScoreLevel for i64 {
    fn exact_decimal(&self) -> Option<Decimal> {
        Some(Decimal::from(*self))
    }
    fn zero() -> Self {
        0
    }
    fn numeric_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
    fn progress_fraction(&self, target: &Self, current: &Self) -> f64 {
        if current >= target {
            1.0
        } else if current <= self || target <= self {
            0.0
        } else {
            (i128::from(*current) - i128::from(*self)) as f64
                / (i128::from(*target) - i128::from(*self)) as f64
        }
    }
    fn cooling_loss(&self, candidate: &Self, temperature: &Self, gradient: f64) -> f64 {
        let loss = i128::from(*self) - i128::from(*candidate);
        if loss <= 0 {
            return 0.0;
        }
        let temperature = *temperature as f64 * (1.0 - gradient);
        if temperature == 0.0 {
            f64::INFINITY
        } else {
            loss as f64 / temperature
        }
    }
}

// A positive magnitude, with bounded significand and a separate decimal exponent.
// Keeping the exponent separate prevents inf/inf and 0/0 for finite decimals.
struct Magnitude {
    significand: f64,
    exponent: i64,
}
impl Magnitude {
    fn of(value: &Decimal) -> Self {
        let digits = value.coefficient().magnitude().to_str_radix(10);
        let n = digits.len().min(18);
        Self {
            significand: digits[..n].parse::<f64>().expect("integer digits")
                / 10f64.powi(n as i32 - 1),
            exponent: digits.len() as i64 - 1 - i64::from(value.scale()),
        }
    }
    // high > low numerically. Subtract exactly wherever cancellation can matter.
    // Otherwise the scale gap proves the smaller operand is below 10^-32 of
    // the larger. Its omission is far below the precision of the final estimate.
    fn difference(high: &Decimal, low: &Decimal) -> Self {
        if high.coefficient() == &crate::BigInt::ZERO {
            return Self::of(low);
        }
        if low.coefficient() == &crate::BigInt::ZERO {
            return Self::of(high);
        }
        let digits = high
            .coefficient()
            .magnitude()
            .to_str_radix(10)
            .len()
            .max(low.coefficient().magnitude().to_str_radix(10).len());
        let gap = (i64::from(high.scale()) - i64::from(low.scale())).unsigned_abs();
        if gap <= digits as u64 + 32 {
            Self::of(&high.subtract(low))
        } else if high.scale() < low.scale() {
            Self::of(high)
        } else {
            Self::of(low)
        }
    }
    fn ratio(&self, denominator: &Self, cooling: f64) -> f64 {
        let ratio = self.significand / (denominator.significand * cooling);
        let exponent = self.exponent - denominator.exponent;
        // Fold a negative decimal exponent into the denominator when finite.
        // This avoids the extra rounding in (6 / 1) * 0.1 versus 6 / 10.
        if (-308..0).contains(&exponent) {
            let scaled = denominator.significand * cooling * 10f64.powi(-exponent as i32);
            if scaled.is_finite() {
                return self.significand / scaled;
            }
        }
        // Split the power near floating-point extremes to avoid prematurely
        // rounding a subnormal ratio to zero or an otherwise finite one to inf.
        if exponent > 400 {
            f64::INFINITY
        } else if exponent < -400 {
            0.0
        } else if exponent > 308 {
            (ratio * 10f64.powi(exponent as i32 - 308)) * 1e308
        } else if exponent < -308 {
            (ratio * 10f64.powi(exponent as i32 + 308)) * 1e-308
        } else {
            ratio * 10f64.powi(exponent as i32)
        }
    }
}
impl ScoreLevel for Decimal {
    fn exact_decimal(&self) -> Option<Decimal> {
        Some(self.clone())
    }
    fn zero() -> Self {
        Self::from(0)
    }
    fn numeric_cmp(&self, other: &Self) -> Ordering {
        Decimal::numeric_cmp(self, other)
    }
    fn progress_fraction(&self, target: &Self, current: &Self) -> f64 {
        if current.numeric_cmp(target) != Ordering::Less {
            1.0
        } else if current.numeric_cmp(self) != Ordering::Greater
            || target.numeric_cmp(self) != Ordering::Greater
        {
            0.0
        } else {
            Magnitude::difference(current, self)
                .ratio(&Magnitude::difference(target, self), 1.0)
                .clamp(0.0, 1.0)
        }
    }
    fn cooling_loss(&self, candidate: &Self, temperature: &Self, gradient: f64) -> f64 {
        if self.numeric_cmp(candidate) != Ordering::Greater {
            return 0.0;
        }
        if temperature.coefficient() == &crate::BigInt::ZERO || gradient == 1.0 {
            return f64::INFINITY;
        }
        Magnitude::difference(self, candidate).ratio(&Magnitude::of(temperature), 1.0 - gradient)
    }
}
