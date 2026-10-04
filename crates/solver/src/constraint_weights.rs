//! Named per-graph weight overrides. Weights are fixed before source initialization.
use crate::AdditiveScore;
use std::collections::BTreeMap;

/// An absent entry keeps the declared default; an explicit zero disables a
/// weighted constraint. Iteration follows constraint-name order, independent of
/// insertion order. Build a new graph to change weights between solve revisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstraintWeightOverrides<S>(BTreeMap<String, S>);
impl<S> Default for ConstraintWeightOverrides<S> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}
impl<S> ConstraintWeightOverrides<S> {
    pub fn insert(&mut self, name: impl Into<String>, weight: S) -> Option<S> {
        self.0.insert(name.into(), weight)
    }
    pub fn get(&self, name: &str) -> Option<&S> {
        self.0.get(name)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &S)> {
        self.0.iter().map(|(name, weight)| (name.as_str(), weight))
    }
}
impl<S> FromIterator<(String, S)> for ConstraintWeightOverrides<S> {
    fn from_iter<T: IntoIterator<Item = (String, S)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

// Exact additive integer scaling for all native and user-defined additive scores.
// Unsigned magnitude includes i64::MIN; doubling follows the score's own overflow
// semantics and never converts numeric levels through floating point.
pub(crate) fn scale<S: AdditiveScore>(weight: &S, factor: i64) -> S {
    match factor {
        0 => return S::ZERO,
        1 => return weight.clone(),
        -1 => return S::ZERO - weight.clone(),
        _ => {}
    }
    let mut count = factor.unsigned_abs();
    let mut term = weight.clone();
    let mut result = S::ZERO;
    while count != 0 {
        if count & 1 != 0 {
            result = result + term.clone();
        }
        count >>= 1;
        if count != 0 {
            term = term.clone() + term;
        }
    }
    if factor < 0 { S::ZERO - result } else { result }
}

/// Score families supporting exact decimal match impacts. This operation clears
/// structural state and preserves the full product precision. It differs from
/// decimal score `multiply`, which floors at the weight's original scale.
/// User-defined additive scores can implement this trait for the same graph API.
pub trait DecimalWeightedScore: AdditiveScore {
    fn multiply_exact(
        &self,
        factor: &crate::Decimal,
    ) -> Result<Self, crate::DecimalArithmeticError>;
}
