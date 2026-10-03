//! Integer scores with compile-time hard and soft level counts.
use crate::{AdditiveScore, ParseScoreError, Score, score::number};
use std::{fmt, ops, str::FromStr};

/// Lexicographically ordered structural, hard, then soft levels. Dimensions are
/// part of the Rust type, so arithmetic between incompatible shapes cannot compile.
/// All hard levels must be nonnegative for feasibility; empty groups are allowed.
///
/// ```compile_fail
/// use timetuck::BendableScore;
/// let hard_soft_soft = BendableScore::new([0], [1, 2]);
/// let hard_hard_soft = BendableScore::new([0, 1], [2]);
/// let incompatible = hard_soft_soft + hard_hard_soft;
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BendableScore<const H: usize, const S: usize> {
    structural: i64,
    hard: [i64; H],
    soft: [i64; S],
}

impl<const H: usize, const S: usize> BendableScore<H, S> {
    pub const ZERO: Self = Self::new([0; H], [0; S]);
    pub const fn new(hard: [i64; H], soft: [i64; S]) -> Self {
        Self {
            structural: 0,
            hard,
            soft,
        }
    }
    pub const fn with_structural(mut self, structural: i64) -> Self {
        self.structural = structural;
        self
    }
    pub const fn structural(self) -> i64 {
        self.structural
    }
    pub const fn hard_levels(self) -> [i64; H] {
        self.hard
    }
    pub const fn soft_levels(self) -> [i64; S] {
        self.soft
    }
    /// Ordinary level by combined index, with hard levels preceding soft levels.
    pub fn level(self, index: usize) -> Option<i64> {
        if index < H {
            self.hard.get(index).copied()
        } else {
            self.soft.get(index - H).copied()
        }
    }
    pub fn of_hard(index: usize, value: i64) -> Option<Self> {
        let mut score = Self::ZERO;
        *score.hard.get_mut(index)? = value;
        Some(score)
    }
    pub fn of_soft(index: usize, value: i64) -> Option<Self> {
        let mut score = Self::ZERO;
        *score.soft.get_mut(index)? = value;
        Some(score)
    }
    pub fn is_feasible(self) -> bool {
        self.structural >= 0 && self.hard.iter().all(|&level| level >= 0)
    }
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    fn map(self, f: impl Fn(i64) -> i64) -> Self {
        Self::new(self.hard.map(&f), self.soft.map(&f))
    }
    fn zip(self, rhs: Self, f: impl Fn(i64, i64) -> i64) -> Self {
        Self::new(
            std::array::from_fn(|i| f(self.hard[i], rhs.hard[i])),
            std::array::from_fn(|i| f(self.soft[i], rhs.soft[i])),
        )
    }
    fn checked_zip(self, rhs: Self, f: impl Fn(i64, i64) -> Option<i64>) -> Option<Self> {
        let mut score = Self::ZERO;
        for ((out, a), b) in score
            .hard
            .iter_mut()
            .chain(score.soft.iter_mut())
            .zip(self.ordinary_levels())
            .zip(rhs.ordinary_levels())
        {
            *out = f(a, b)?;
        }
        Some(score)
    }
    /// Arithmetic clears structural state, including the checked operations.
    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.checked_zip(rhs, i64::checked_add)
    }
    pub fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.checked_zip(rhs, i64::checked_sub)
    }
    pub fn checked_mul(self, factor: i64) -> Option<Self> {
        self.checked_zip(Self::new([factor; H], [factor; S]), i64::checked_mul)
    }
    /// Floating operations floor every component and saturate to i64. They may
    /// lose precision above 2^53, just like the fixed integer score families.
    pub fn multiply(self, factor: f64) -> Self {
        self.map(|v| (v as f64 * factor).floor() as i64)
    }
    pub fn divide(self, divisor: f64) -> Self {
        self.map(|v| (v as f64 / divisor).floor() as i64)
    }
    pub fn power(self, exponent: f64) -> Self {
        self.map(|v| (v as f64).powf(exponent).floor() as i64)
    }
    pub fn abs(self) -> Self {
        self.map(i64::wrapping_abs)
    }

    /// Omit entirely zero groups, preserving all positions inside nonzero groups.
    /// Short strings generally cannot be parsed back into the fixed shape.
    pub fn to_short_string(self) -> String {
        let mut parts = Vec::new();
        if self.structural != 0 {
            parts.push(format!("{}structural", self.structural));
        }
        if self.hard.iter().any(|&v| v != 0) {
            parts.push(group_string(&self.hard, "hard"));
        }
        if self.soft.iter().any(|&v| v != 0) {
            parts.push(group_string(&self.soft, "soft"));
        }
        if parts.is_empty() {
            "0".to_owned()
        } else {
            parts.join("/")
        }
    }
}

fn group_string(levels: &[i64], label: &str) -> String {
    format!(
        "[{}]{label}",
        levels
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join("/")
    )
}

fn parse_group<const N: usize>(text: &str) -> Result<[i64; N], ParseScoreError> {
    let body = text
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or(ParseScoreError)?;
    let mut levels = [0; N];
    if N == 0 {
        return if body.is_empty() {
            Ok(levels)
        } else {
            Err(ParseScoreError)
        };
    }
    let mut parts = body.split('/');
    for level in &mut levels {
        *level = number(parts.next().ok_or(ParseScoreError)?)?;
    }
    if parts.next().is_some() {
        return Err(ParseScoreError);
    }
    Ok(levels)
}

impl<const H: usize, const S: usize> FromStr for BendableScore<H, S> {
    type Err = ParseScoreError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (structural, body) = match text.split_once("structural/") {
            Some((prefix, rest)) => (number(prefix)?, rest),
            None => (0, text),
        };
        let (hard, soft) = body.split_once("hard/").ok_or(ParseScoreError)?;
        let soft = soft.strip_suffix("soft").ok_or(ParseScoreError)?;
        Ok(Self::new(parse_group(hard)?, parse_group(soft)?).with_structural(structural))
    }
}
impl<const H: usize, const S: usize> fmt::Display for BendableScore<H, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.structural < 0 {
            write!(f, "{}structural/", self.structural)?;
        }
        write!(
            f,
            "{}/{}",
            group_string(&self.hard, "hard"),
            group_string(&self.soft, "soft")
        )
    }
}
impl<const H: usize, const S: usize> Default for BendableScore<H, S> {
    fn default() -> Self {
        Self::ZERO
    }
}
impl<const H: usize, const S: usize> Score for BendableScore<H, S> {
    const HARD_LEVEL_COUNT: usize = H;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        self.hard.iter().chain(self.soft.iter()).copied()
    }
    fn structural(&self) -> i64 {
        self.structural
    }
    fn hard_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hard.cmp(&other.hard)
    }
    fn is_feasible(&self) -> bool {
        (*self).is_feasible()
    }
}
impl<const H: usize, const S: usize> AdditiveScore for BendableScore<H, S> {
    const ZERO: Self = Self::ZERO;
    fn with_structural(self, structural: i64) -> Self {
        self.with_structural(structural)
    }
}
impl<const H: usize, const S: usize> ops::Add for BendableScore<H, S> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.zip(rhs, i64::wrapping_add)
    }
}
impl<const H: usize, const S: usize> ops::Sub for BendableScore<H, S> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self.zip(rhs, i64::wrapping_sub)
    }
}
impl<const H: usize, const S: usize> ops::Neg for BendableScore<H, S> {
    type Output = Self;
    fn neg(self) -> Self {
        self.map(i64::wrapping_neg)
    }
}
