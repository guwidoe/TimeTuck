//! Arbitrary-precision decimal scores with compile-time hard/soft dimensions.
use crate::{
    AdditiveScore, BigInt, Decimal, DecimalArithmeticError, ParseDecimalScoreError, Score,
};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops,
    str::FromStr,
};

/// Structural state, then H hard and S soft numeric levels. Every hard level
/// must be nonnegative for feasibility. Unlike the fixed decimal families,
/// construction/arithmetic preserve zero scales and negation preserves all scales.
/// Dimensions are part of the type; groups may be empty.
///
/// ```
/// use timetuck::{BendableDecimalScore, Decimal};
/// let score: BendableDecimalScore<2,1> = "[0.00/-0.01]hard/[1E+2]soft".parse().unwrap();
/// assert!(!score.is_feasible());
/// assert_eq!((-score).to_string(), "[0.00/0.01]hard/[-1E+2]soft");
/// let empty = BendableDecimalScore::<0,0>::ZERO;
/// assert_eq!(empty.divide(&Decimal::ZERO).unwrap(), empty);
/// ```
///
/// ```compile_fail
/// use timetuck::{BendableDecimalScore, Decimal};
/// let a = BendableDecimalScore::new([Decimal::ZERO], []);
/// let b = BendableDecimalScore::new([], [Decimal::ZERO]);
/// let incompatible = a + b;
/// ```
#[derive(Clone, Debug)]
pub struct BendableDecimalScore<const H: usize, const S: usize> {
    structural: i64,
    hard: [Decimal; H],
    soft: [Decimal; S],
}
impl<const H: usize, const S: usize> BendableDecimalScore<H, S> {
    pub const ZERO: Self = Self {
        structural: 0,
        hard: [const { Decimal::ZERO }; H],
        soft: [const { Decimal::ZERO }; S],
    };
    /// Retain every input scale, including all-zero shapes.
    pub fn new(hard: [Decimal; H], soft: [Decimal; S]) -> Self {
        Self::from_parts(0, hard, soft)
    }
    pub fn from_parts(structural: i64, hard: [Decimal; H], soft: [Decimal; S]) -> Self {
        Self {
            structural,
            hard,
            soft,
        }
    }
    pub fn with_structural(mut self, structural: i64) -> Self {
        self.structural = structural;
        self
    }
    pub fn structural(&self) -> i64 {
        self.structural
    }
    pub fn hard_levels(&self) -> &[Decimal; H] {
        &self.hard
    }
    pub fn soft_levels(&self) -> &[Decimal; S] {
        &self.soft
    }
    /// Borrowed ordinary levels, hard first, in priority order.
    pub fn levels(&self) -> impl Iterator<Item = &Decimal> {
        self.hard.iter().chain(&self.soft)
    }
    pub fn level(&self, index: usize) -> Option<&Decimal> {
        if index < H {
            self.hard.get(index)
        } else {
            self.soft.get(index - H)
        }
    }
    pub fn of_hard(index: usize, value: Decimal) -> Option<Self> {
        let mut score = Self::ZERO;
        *score.hard.get_mut(index)? = value;
        Some(score)
    }
    pub fn of_soft(index: usize, value: Decimal) -> Option<Self> {
        let mut score = Self::ZERO;
        *score.soft.get_mut(index)? = value;
        Some(score)
    }
    pub fn is_feasible(&self) -> bool {
        self.structural >= 0
            && self
                .hard
                .iter()
                .all(|v| !v.numeric_cmp(&Decimal::ZERO).is_lt())
    }
    pub fn is_zero(&self) -> bool {
        self == &Self::ZERO
    }
    fn map(&self, f: impl Fn(&Decimal) -> Decimal) -> Self {
        Self::new(
            std::array::from_fn(|i| f(&self.hard[i])),
            std::array::from_fn(|i| f(&self.soft[i])),
        )
    }
    fn zip(&self, other: &Self, f: impl Fn(&Decimal, &Decimal) -> Decimal) -> Self {
        Self::new(
            std::array::from_fn(|i| f(&self.hard[i], &other.hard[i])),
            std::array::from_fn(|i| f(&self.soft[i], &other.soft[i])),
        )
    }
    fn try_map(
        &self,
        f: impl Fn(&Decimal) -> Result<Decimal, DecimalArithmeticError>,
    ) -> Result<Self, DecimalArithmeticError> {
        let mut result = Self::ZERO;
        for (out, value) in result
            .hard
            .iter_mut()
            .chain(result.soft.iter_mut())
            .zip(self.levels())
        {
            *out = f(value)?;
        }
        Ok(result)
    }
    /// Exact decimal multiplication, floored at max(original level scale, 0).
    /// All arithmetic clears structural state. Large coefficients/scale gaps may
    /// require proportional memory/time; there is no fixed resource bound.
    pub fn multiply(&self, factor: &Decimal) -> Result<Self, DecimalArithmeticError> {
        self.try_map(|v| v.multiply_floor(factor))
    }
    /// Zero divisor fails if there is at least one level. The empty shape has no
    /// divisions to perform and returns its structurally sound zero.
    pub fn divide(&self, divisor: &Decimal) -> Result<Self, DecimalArithmeticError> {
        self.try_map(|v| v.divide_floor(divisor))
    }
    pub fn power(&self, exponent: u32) -> Result<Self, DecimalArithmeticError> {
        self.try_map(|v| v.power_floor(exponent))
    }
    pub fn abs(&self) -> Self {
        self.map(|v| Decimal::new(BigInt::from(v.coefficient().magnitude().clone()), v.scale()))
    }
    /// Omit wholly zero groups, retaining zero slots/scales within other groups.
    /// Short strings are not generally parseable back to a full fixed shape.
    pub fn to_short_string(&self) -> String {
        let mut parts = Vec::new();
        if self.structural != 0 {
            parts.push(format!("{}structural", self.structural));
        }
        for (levels, label) in [(&self.hard[..], "hard"), (&self.soft[..], "soft")] {
            if levels.iter().any(|v| v.coefficient() != &BigInt::ZERO) {
                parts.push(group_string(levels, label));
            }
        }
        if parts.is_empty() {
            "0".into()
        } else {
            parts.join("/")
        }
    }
}
fn group_string(levels: &[Decimal], label: &str) -> String {
    format!(
        "[{}]{label}",
        levels
            .iter()
            .map(Decimal::to_standard_string)
            .collect::<Vec<_>>()
            .join("/")
    )
}
fn parse_group<const N: usize>(text: &str) -> Result<[Decimal; N], ParseDecimalScoreError> {
    let body = text
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .ok_or(ParseDecimalScoreError)?;
    let mut levels = [const { Decimal::ZERO }; N];
    if N == 0 {
        return if body.is_empty() {
            Ok(levels)
        } else {
            Err(ParseDecimalScoreError)
        };
    }
    let mut parts = body.split('/');
    for v in &mut levels {
        *v = parts
            .next()
            .ok_or(ParseDecimalScoreError)?
            .parse()
            .map_err(|_| ParseDecimalScoreError)?;
    }
    if parts.next().is_some() {
        Err(ParseDecimalScoreError)
    } else {
        Ok(levels)
    }
}
impl<const H: usize, const S: usize> FromStr for BendableDecimalScore<H, S> {
    type Err = ParseDecimalScoreError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (structural, body) = match text.split_once("structural/") {
            Some((s, b)) => (
                crate::score::number(s).map_err(|_| ParseDecimalScoreError)?,
                b,
            ),
            None => (0, text),
        };
        let (hard, soft) = body.split_once("hard/").ok_or(ParseDecimalScoreError)?;
        let soft = soft.strip_suffix("soft").ok_or(ParseDecimalScoreError)?;
        Ok(Self::from_parts(
            structural,
            parse_group(hard)?,
            parse_group(soft)?,
        ))
    }
}
impl<const H: usize, const S: usize> fmt::Display for BendableDecimalScore<H, S> {
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
impl<const H: usize, const S: usize> Default for BendableDecimalScore<H, S> {
    fn default() -> Self {
        Self::ZERO
    }
}
impl<const H: usize, const S: usize> PartialEq for BendableDecimalScore<H, S> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o).is_eq()
    }
}
impl<const H: usize, const S: usize> Eq for BendableDecimalScore<H, S> {}
impl<const H: usize, const S: usize> PartialOrd for BendableDecimalScore<H, S> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<const H: usize, const S: usize> Ord for BendableDecimalScore<H, S> {
    fn cmp(&self, o: &Self) -> Ordering {
        self.structural.cmp(&o.structural).then_with(|| {
            self.levels()
                .zip(o.levels())
                .map(|(a, b)| a.numeric_cmp(b))
                .find(|o| !o.is_eq())
                .unwrap_or(Ordering::Equal)
        })
    }
}
impl<const H: usize, const S: usize> Hash for BendableDecimalScore<H, S> {
    fn hash<T: Hasher>(&self, state: &mut T) {
        H.hash(state);
        S.hash(state);
        self.structural.hash(state);
        for v in self.levels() {
            v.numeric_hash(state);
        }
    }
}
impl<const H: usize, const S: usize> Score for BendableDecimalScore<H, S> {
    type Level = Decimal;
    const HARD_LEVEL_COUNT: usize = H;
    fn ordinary_levels(&self) -> impl Iterator<Item = Decimal> {
        self.levels().cloned()
    }
    fn structural(&self) -> i64 {
        self.structural
    }
    fn hard_cmp(&self, o: &Self) -> Ordering {
        self.hard
            .iter()
            .zip(&o.hard)
            .map(|(a, b)| a.numeric_cmp(b))
            .find(|o| !o.is_eq())
            .unwrap_or(Ordering::Equal)
    }
    fn is_feasible(&self) -> bool {
        self.is_feasible()
    }
}
impl<const H: usize, const S: usize> AdditiveScore for BendableDecimalScore<H, S> {
    const ZERO: Self = Self::ZERO;
    fn with_structural(self, s: i64) -> Self {
        self.with_structural(s)
    }
}
impl<const H: usize, const S: usize> ops::Add for BendableDecimalScore<H, S> {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        self.zip(&o, Decimal::add)
    }
}
impl<const H: usize, const S: usize> ops::Sub for BendableDecimalScore<H, S> {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        self.zip(&o, Decimal::subtract)
    }
}
impl<const H: usize, const S: usize> ops::Neg for BendableDecimalScore<H, S> {
    type Output = Self;
    fn neg(self) -> Self {
        self.map(|v| Decimal::new(-v.coefficient(), v.scale()))
    }
}

impl<const H: usize, const S: usize> crate::DecimalWeightedScore for BendableDecimalScore<H, S> {
    fn multiply_exact(&self, factor: &Decimal) -> Result<Self, DecimalArithmeticError> {
        self.try_map(|value| value.multiply_exact(factor))
    }
}
