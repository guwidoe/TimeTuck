//! Numeric decimal keys for ordered and hashed indexes.
use crate::{BigInt, Decimal, ParseDecimalError};
use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    str::FromStr,
};

/// An exact decimal whose equality, ordering and hashing ignore scale.
///
/// Use this as a grouping, join or candidate-index key when `1.0` and `1.00`
/// should match. The original coefficient and scale remain available through
/// [`Self::as_decimal`]; construction does not normalize or round them. Numeric
/// comparison and hashing do not allocate padding proportional to scale gaps.
///
/// This deliberately does not implement `Borrow<Decimal>`: `Decimal` has
/// representation-sensitive equality and hashing. Convert lookup keys too.
/// A collection may retain either representation of an equal key; its scale
/// must not be used as a stable group attribute.
///
/// ```
/// use std::collections::HashSet;
/// use timetuck::NumericDecimal;
/// let keys: HashSet<NumericDecimal> = ["1.0", "1.00", "2"]
///     .map(|text| text.parse().unwrap()).into_iter().collect();
/// assert_eq!(keys.len(), 2);
/// assert!(keys.contains(&"1".parse().unwrap()));
/// ```
#[derive(Clone, Debug, Default)]
pub struct NumericDecimal(Decimal);

impl NumericDecimal {
    pub const ZERO: Self = Self(Decimal::ZERO);

    pub const fn new(value: Decimal) -> Self {
        Self(value)
    }
    pub const fn as_decimal(&self) -> &Decimal {
        &self.0
    }
    pub fn into_decimal(self) -> Decimal {
        self.0
    }
    pub fn is_zero(&self) -> bool {
        self.0.coefficient() == &BigInt::ZERO
    }
    pub fn is_negative(&self) -> bool {
        self.0.coefficient().sign() == num_bigint::Sign::Minus
    }
}
impl From<Decimal> for NumericDecimal {
    fn from(value: Decimal) -> Self {
        Self(value)
    }
}
impl From<i64> for NumericDecimal {
    fn from(value: i64) -> Self {
        Self(value.into())
    }
}
impl AsRef<Decimal> for NumericDecimal {
    fn as_ref(&self) -> &Decimal {
        self.as_decimal()
    }
}
impl FromStr for NumericDecimal {
    type Err = ParseDecimalError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse().map(Self)
    }
}
impl PartialEq for NumericDecimal {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for NumericDecimal {}
impl PartialOrd for NumericDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for NumericDecimal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.numeric_cmp(&other.0)
    }
}
impl Hash for NumericDecimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.numeric_hash(state);
    }
}
impl fmt::Display for NumericDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
