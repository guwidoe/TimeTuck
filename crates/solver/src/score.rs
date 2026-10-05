//! Integer score types. Greater scores are better; comparison is lexicographic.
//!
//! Integer arithmetic explicitly wraps on overflow in all build profiles, matching
//! Java `long`. All arithmetic clears the structural component. Use the checked methods when overflow should be an error. Floating
//! arithmetic floors each ordinary component, then saturates to `i64` (NaN becomes zero).

use crate::ScoreLevel;
use std::{error::Error, fmt, ops, str::FromStr};

/// Common behavior required by optimization algorithms. Scores may own storage;
/// cloning must preserve an independent immutable logical value. Clone, comparison
/// and arithmetic must not mutate other retained scores through shared interiors.
/// Existing integer families remain Copy. The associated Level supplies numeric
/// estimates independently of the score representation.
///
/// ```
/// use timetuck::{Score, FiniteProblem, Variable, exhaustive_search};
/// #[derive(Clone,Debug,Eq,PartialEq,Ord,PartialOrd)]
/// struct HeapScore(Vec<i64>);
/// impl Score for HeapScore {
///     type Level = i64;
///     const HARD_LEVEL_COUNT: usize = 0;
///     fn ordinary_levels(&self)->impl Iterator<Item=i64> {self.0.iter().copied()}
///     fn structural(&self)->i64 {0}
///     fn hard_cmp(&self,_:&Self)->std::cmp::Ordering {std::cmp::Ordering::Equal}
///     fn is_feasible(&self)->bool {true}
/// }
/// let p = FiniteProblem::new(vec![Variable::new(vec![1_i64,2])]).unwrap();
/// let r = exhaustive_search(&p, |a| HeapScore(vec![a.values()[0].unwrap()]), 2);
/// assert!(r.is_optimal());
/// assert_eq!(r.best.unwrap().score, HeapScore(vec![2]));
/// ```
pub trait Score: Clone + Ord + fmt::Debug {
    type Level: ScoreLevel;
    /// Number of leading ordinary levels that represent hard constraints.
    /// Must not exceed the number of ordinary levels and is constant per type.
    const HARD_LEVEL_COUNT: usize;

    /// Ordered ordinary numeric levels, excluding structural state. The number
    /// and order of levels must be constant for a score type.
    fn ordinary_levels(&self) -> impl Iterator<Item = Self::Level>;

    /// Negative values identify a structurally flawed candidate.
    fn structural(&self) -> i64;

    /// Compare hard levels only, excluding structural and all soft levels.
    fn hard_cmp(&self, other: &Self) -> std::cmp::Ordering;

    /// Compare the levels whose improvement resets late-acceptance history.
    /// Defaults to hard levels. HardMediumSoft families additionally include
    /// medium; bendable families retain only their declared hard levels.
    /// This policy is distinct from feasibility and excludes structural state.
    ///
    /// ```
    /// use timetuck::{Score, HardMediumSoftScore};
    /// let old = HardMediumSoftScore::new(0, -10, 0);
    /// let new = HardMediumSoftScore::new(0, -9, -100);
    /// assert!(new.late_acceptance_cmp(&old).is_gt());
    /// assert!(new.hard_cmp(&old).is_eq());
    /// assert!(old.is_feasible() && new.is_feasible());
    /// ```
    fn late_acceptance_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hard_cmp(other)
    }

    /// Whether the structural and hard constraints are satisfied.
    fn is_feasible(&self) -> bool;
}

/// Additive scores used by a scoped constraint network.
/// Ordinary components must form an abelian group under addition/subtraction;
/// arithmetic clears structural state, which the network sets separately.
pub trait AdditiveScore: Score + ops::Add<Output = Self> + ops::Sub<Output = Self> {
    const ZERO: Self;
    fn with_structural(self, structural: i64) -> Self;
    /// Checked scoring hook. The default follows this score's additive arithmetic;
    /// bounded custom scores must override it to report unrepresentable impacts.
    fn checked_scale(&self, factor: i64) -> Option<Self> {
        Some(crate::constraint_weights::scale(self, factor))
    }
    /// Sum ordinary levels; built-in integers widen intermediate arithmetic.
    /// Structural state is cleared. Bounded custom scores should override this.
    fn checked_sum<'a>(scores: impl IntoIterator<Item = &'a Self>) -> Option<Self>
    where
        Self: 'a,
    {
        Some(
            scores
                .into_iter()
                .fold(Self::ZERO, |sum, score| sum + score.clone()),
        )
    }
    /// Apply a batch of old/new contributions, checking the final ordinary levels.
    /// Built-in integers permit temporary i64 overflow when the final result fits.
    /// Bounded custom scores should override this; the default uses score operators.
    fn checked_adjust<'a>(
        &self,
        changes: impl IntoIterator<Item = (&'a Self, &'a Self)>,
    ) -> Option<Self>
    where
        Self: 'a,
    {
        Some(changes.into_iter().fold(self.clone(), |sum, (old, new)| {
            sum - old.clone() + new.clone()
        }))
    }
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

pub(crate) fn number(text: &str) -> Result<i64, ParseScoreError> {
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
    ($name:ident, $n:expr, $hard:expr, $late:expr, [$($field:ident : $index:expr => $label:literal),+], $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name {
            structural: i64,
            levels: [i64; $n],
        }

        impl AdditiveScore for $name {
            const ZERO: Self = Self::ZERO;
            fn with_structural(self, structural: i64) -> Self { self.with_structural(structural) }
            fn checked_scale(&self, factor: i64) -> Option<Self> { (*self).checked_mul(factor) }
            fn checked_sum<'a>(scores: impl IntoIterator<Item=&'a Self>) -> Option<Self> where Self:'a {
                let zero=Self::ZERO;
                zero.checked_adjust(scores.into_iter().map(|score| (&zero,score)))
            }
            fn checked_adjust<'a>(&self, changes: impl IntoIterator<Item=(&'a Self,&'a Self)>) -> Option<Self> where Self:'a {
                let mut wide=self.levels.map(i128::from);
                for (old,new) in changes {
                    for ((value,old),new) in wide.iter_mut().zip(old.levels).zip(new.levels) {
                        *value=value.checked_sub(i128::from(old))?.checked_add(i128::from(new))?;
                    }
                }
                let mut out=Self::ZERO;
                for (result,value) in out.levels.iter_mut().zip(wide) { *result=value.try_into().ok()?; }
                Some(out)
            }

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
            type Level = i64;
            const HARD_LEVEL_COUNT: usize = $hard;
            fn ordinary_levels(&self) -> impl Iterator<Item = i64> { self.levels.iter().copied() }
            fn hard_cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.levels[..$hard].cmp(&other.levels[..$hard])
            }
            fn late_acceptance_cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.levels[..$late].cmp(&other.levels[..$late])
            }
            fn structural(&self) -> i64 { (*self).structural() }
            fn is_feasible(&self) -> bool { (*self).is_feasible() }
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

fixed_score!(SimpleScore, 1, 0, 0, [score: 0 => ""], "One soft integer level plus a structural component.");
fixed_score!(HardSoftScore, 2, 1, 1, [hard: 0 => "hard", soft: 1 => "soft"], "One hard and one soft integer level plus a structural component.");
fixed_score!(HardMediumSoftScore, 3, 1, 2, [hard: 0 => "hard", medium: 1 => "medium", soft: 2 => "soft"], "One hard, one medium and one soft integer level plus a structural component.");

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
