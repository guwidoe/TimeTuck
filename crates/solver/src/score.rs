//! Integer score types. Greater scores are better; comparison is lexicographic.
//!
//! Integer arithmetic explicitly wraps on overflow in all build profiles, matching
//! Java `long`. All arithmetic clears the structural component. Use the checked methods when overflow should be an error. Floating
//! arithmetic floors each ordinary component, then saturates to `i64` (NaN becomes zero).

use std::{error::Error, fmt, ops, str::FromStr};

/// Common behavior required by optimization algorithms.
pub trait Score: Copy + Ord + fmt::Debug {
    /// Ordered ordinary integer levels, excluding structural state. The number
    /// and order of levels must be constant for a score type.
    fn ordinary_levels(&self) -> &[i64];

    /// Negative values identify a structurally flawed candidate.
    fn structural(self) -> i64;

    /// Compare hard levels only, excluding structural and all soft levels.
    fn hard_cmp(self, other: Self) -> std::cmp::Ordering;

    /// Whether the structural and hard constraints are satisfied.
    fn is_feasible(self) -> bool;
}

/// Additive scores used by a scoped constraint network.
/// Ordinary components must form an abelian group under addition/subtraction;
/// arithmetic clears structural state, which the network sets separately.
pub trait AdditiveScore: Score + ops::Add<Output = Self> + ops::Sub<Output = Self> {
    const ZERO: Self;
    fn with_structural(self, structural: i64) -> Self;
}

/// The input does not have the score type's complete, ordered level format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseScoreError;

impl fmt::Display for ParseScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid score: expected complete, ordered integer score levels")
    }
}
impl Error for ParseScoreError {}

fn number(text: &str) -> Result<i64, ParseScoreError> {
    if text == "*" {
        Ok(i64::MIN)
    } else {
        text.parse().map_err(|_| ParseScoreError)
    }
}

fn parse<const N: usize>(
    text: &str,
    labels: [&str; N],
) -> Result<(i64, [i64; N]), ParseScoreError> {
    let (structural, body) = if let Some((prefix, tail)) = text.split_once("structural/") {
        (number(prefix)?, tail)
    } else {
        (0, text)
    };
    let mut parts = body.split('/');
    let mut levels = [0; N];
    for (level, label) in levels.iter_mut().zip(labels) {
        *level = number(
            parts
                .next()
                .ok_or(ParseScoreError)?
                .strip_suffix(label)
                .ok_or(ParseScoreError)?,
        )?;
    }
    if parts.next().is_some() {
        return Err(ParseScoreError);
    }
    Ok((structural, levels))
}

macro_rules! fixed_score {
    ($name:ident, $n:expr, $hard:expr, [$($field:ident : $index:expr => $label:literal),+], $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name {
            structural: i64,
            levels: [i64; $n],
        }

        impl AdditiveScore for $name {
            const ZERO: Self = Self::ZERO;
            fn with_structural(self, structural: i64) -> Self { self.with_structural(structural) }
        }

        impl $name {
            /// A structurally sound score with all levels zero.
            pub const ZERO: Self = Self::new($({ let _ = stringify!($field); 0 }),+);

            /// Construct a score with no structural flaws.
            pub const fn new($($field: i64),+) -> Self {
                Self { structural: 0, levels: [$($field),+] }
            }

            /// Replace the structural component verbatim. Negative values indicate flaws.
            /// This component precedes every ordinary level in comparisons.
            pub const fn with_structural(mut self, structural: i64) -> Self {
                self.structural = structural;
                self
            }

            /// The structural component. Positive values are retained for API conformance.
            pub const fn structural(self) -> i64 { self.structural }

            $(pub const fn $field(self) -> i64 { self.levels[$index] })+

            /// Ordinary score levels in order of priority (structural excluded).
            pub const fn levels(self) -> [i64; $n] { self.levels }

            pub fn is_feasible(self) -> bool {
                self.structural >= 0 && self.levels[..$hard].iter().all(|&x| x >= 0)
            }

            pub fn is_zero(self) -> bool { self == Self::ZERO }

            fn map(self, f: impl Fn(i64) -> i64) -> Self {
                Self { structural: 0, levels: self.levels.map(&f) }
            }

            fn zip(self, rhs: Self, f: impl Fn(i64, i64) -> i64) -> Self {
                Self { structural: 0, levels: std::array::from_fn(|i| f(self.levels[i], rhs.levels[i])) }

            }

            fn checked_zip(self, rhs: Self, f: impl Fn(i64, i64) -> Option<i64>) -> Option<Self> {
                let mut result = Self::ZERO;
                for (i, level) in result.levels.iter_mut().enumerate() {
                    *level = f(self.levels[i], rhs.levels[i])?;
                }
                Some(result)
            }

            /// Component-wise addition; returns `None` on any overflow.
            pub fn checked_add(self, rhs: Self) -> Option<Self> { self.checked_zip(rhs, i64::checked_add) }
            /// Component-wise subtraction; returns `None` on any overflow.
            pub fn checked_sub(self, rhs: Self) -> Option<Self> { self.checked_zip(rhs, i64::checked_sub) }

            /// Exact integer multiplication; returns `None` on any overflow.
            pub fn checked_mul(self, factor: i64) -> Option<Self> {
                let mut result = Self::ZERO;
                for (i, level) in result.levels.iter_mut().enumerate() {
                    *level = self.levels[i].checked_mul(factor)?;
                }
                Some(result)
            }

            /// Multiply by a floating factor, flooring each component.
            /// Conversion to `f64` can lose precision above 2^53.
            pub fn multiply(self, factor: f64) -> Self { self.map(|x| ((x as f64) * factor).floor() as i64) }
            /// Divide by a floating divisor, flooring each component.
            /// Zero, infinity, and NaN follow IEEE-754, then saturating conversion.
            pub fn divide(self, divisor: f64) -> Self { self.map(|x| ((x as f64) / divisor).floor() as i64) }
            /// Raise each component to a power, flooring the result.
            /// Non-integral powers may differ from JVM math by rounding at boundaries.
            pub fn power(self, exponent: f64) -> Self { self.map(|x| (x as f64).powf(exponent).floor() as i64) }
            /// Absolute values with wrapping semantics (`i64::MIN` stays negative).
            pub fn abs(self) -> Self { self.map(i64::wrapping_abs) }

            /// Omit zero levels for display. Short strings are not parseable in general.
            pub fn to_short_string(self) -> String {
                let mut parts = Vec::new();
                if self.structural != 0 { parts.push(format!("{}structural", self.structural)); }
                $(if self.levels[$index] != 0 { parts.push(format!("{}{}", self.levels[$index], $label)); })+
                if parts.is_empty() { "0".to_owned() } else { parts.join("/") }
            }
        }

        impl Score for $name {
            fn ordinary_levels(&self) -> &[i64] { &self.levels }
            fn hard_cmp(self, other: Self) -> std::cmp::Ordering {
                self.levels[..$hard].cmp(&other.levels[..$hard])
            }
            fn structural(self) -> i64 { self.structural() }
            fn is_feasible(self) -> bool { self.is_feasible() }
        }
        impl FromStr for $name {
            type Err = ParseScoreError;
            fn from_str(text: &str) -> Result<Self, Self::Err> {
                let (structural, levels) = parse(text, [$($label),+])?;
                Ok(Self { structural: 0, levels }.with_structural(structural))
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.structural < 0 { write!(f, "{}structural/", self.structural)?; }
                for (i, (level, label)) in self.levels.iter().zip([$($label),+]).enumerate() {
                    if i != 0 { f.write_str("/")?; }
                    write!(f, "{level}{label}")?;
                }
                Ok(())
            }
        }
        impl ops::Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self { self.zip(rhs, i64::wrapping_add) }
        }
        impl ops::Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self { self.zip(rhs, i64::wrapping_sub) }
        }
        impl ops::Neg for $name {
            type Output = Self;
            fn neg(self) -> Self { self.map(i64::wrapping_neg) }
        }
    };
}

fixed_score!(SimpleScore, 1, 0, [score: 0 => ""], "One soft integer level plus a structural component.");
fixed_score!(HardSoftScore, 2, 1, [hard: 0 => "hard", soft: 1 => "soft"], "One hard and one soft integer level plus a structural component.");
fixed_score!(HardMediumSoftScore, 3, 1, [hard: 0 => "hard", medium: 1 => "medium", soft: 2 => "soft"], "One hard, one medium and one soft integer level plus a structural component.");

impl HardSoftScore {
    pub const fn of_hard(hard: i64) -> Self {
        Self::new(hard, 0)
    }
    pub const fn of_soft(soft: i64) -> Self {
        Self::new(0, soft)
    }
}
impl HardMediumSoftScore {
    pub const fn of_hard(hard: i64) -> Self {
        Self::new(hard, 0, 0)
    }
    pub const fn of_medium(medium: i64) -> Self {
        Self::new(0, medium, 0)
    }
    pub const fn of_soft(soft: i64) -> Self {
        Self::new(0, 0, soft)
    }
}
