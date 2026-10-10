//! Checked integer counts and logarithmic assignment-space estimates.
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombinatoricsError {
    NegativeFactorial(i32),
    FactorialOverflow(i32),
    InvalidBinomial { n: i32, k: i32 },
    BinomialOverflow { n: i32, k: i32 },
    InvalidLogBase(u64),
}
impl fmt::Display for CombinatoricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeFactorial(n) => write!(f, "Factorial of {n} is undefined."),
            Self::FactorialOverflow(n) => {
                write!(f, "Factorial of {n} is too large to fit in a long.")
            }
            Self::InvalidBinomial { n, k } => write!(f, "invalid binomial arguments: n={n}, k={k}"),
            Self::BinomialOverflow { n, k } => {
                write!(f, "binomial coefficient ({n}, {k}) exceeds i64")
            }
            Self::InvalidLogBase(base) => write!(f, "logarithm base must exceed one: {base}"),
        }
    }
}
impl Error for CombinatoricsError {}

/// Exact factorial in signed 64-bit storage; reject negative and overflowing inputs.
pub fn factorial(n: i32) -> Result<i64, CombinatoricsError> {
    if n < 0 {
        return Err(CombinatoricsError::NegativeFactorial(n));
    }
    if n > 20 {
        return Err(CombinatoricsError::FactorialOverflow(n));
    }
    Ok((2..=i64::from(n)).product())
}
/// Exact binomial coefficient, with widened intermediate products and no floats.
pub fn binomial_coefficient(n: i32, k: i32) -> Result<i64, CombinatoricsError> {
    if n < 0 || k < 0 || k > n {
        return Err(CombinatoricsError::InvalidBinomial { n, k });
    }
    let draws = k.min(n - k);
    let mut count = 1u128;
    for i in 1..=draws {
        count = count * (n - draws + i) as u128 / i as u128;
        if count > i64::MAX as u128 {
            return Err(CombinatoricsError::BinomialOverflow { n, k });
        }
    }
    Ok(count as i64)
}
/// Base must exceed one. Zero values have logarithm negative infinity.
pub fn log_in_base(base: u64, value: u64) -> Result<f64, CombinatoricsError> {
    if base < 2 {
        return Err(CombinatoricsError::InvalidLogBase(base));
    }
    Ok((value as f64).ln() / (base as f64).ln())
}
/// Round to the nearest fixed-point unit and saturate to signed 64-bit bounds.
/// A precision of zero returns zero, including for an empty assignment space.
pub fn scaled_log(precision: u64, base: u64, value: u64) -> Result<i64, CombinatoricsError> {
    let log = log_in_base(base, value)?;
    Ok(if precision == 0 {
        0
    } else {
        (precision as f64 * log).round() as i64
    })
}
/// Ordered distribution of distinct values among labelled lists (empty lists
/// allowed). Sums logarithms without constructing factorials or large integers.
/// Zero values have one arrangement; nonempty values with no owners have none.
pub fn list_arrangements_scaled_log(
    precision: u64,
    base: u64,
    values: usize,
    owners: usize,
) -> Result<i64, CombinatoricsError> {
    if base < 2 {
        return Err(CombinatoricsError::InvalidLogBase(base));
    }
    let log = list_arrangements_ln(values, owners, false) / (base as f64).ln();
    Ok(if precision == 0 {
        0
    } else {
        (precision as f64 * log).round() as i64
    })
}
// Compensated summation keeps long products from accumulating avoidable rounding.
#[derive(Default)]
pub(crate) struct LogSum {
    sum: f64,
    correction: f64,
}
impl LogSum {
    pub(crate) fn add(&mut self, term: f64) {
        let corrected = term - self.correction;
        let next = self.sum + corrected;
        self.correction = (next - self.sum) - corrected;
        self.sum = next;
    }
    pub(crate) fn value(&self) -> f64 {
        self.sum
    }
}
pub(crate) fn list_arrangements_ln(values: usize, owners: usize, optional: bool) -> f64 {
    if values == 0 {
        return 0.0;
    }
    if owners == 0 {
        return if optional { 0.0 } else { f64::NEG_INFINITY };
    }
    let mut term = LogSum::default();
    let mut total: f64 = 0.0;
    for k in 1..=values {
        term.add((owners as f64 + (k - 1) as f64).ln());
        if optional {
            // Increment C(values,k) times the ordered arrangements for k values.
            term.add(((values - k + 1) as f64).ln());
            term.add(-(k as f64).ln());
            let high = total.max(term.value());
            total = high + (total.min(term.value()) - high).exp().ln_1p();
        }
    }
    if optional { total } else { term.value() }
}
