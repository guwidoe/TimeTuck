//! Retractable load balancing with exact aggregate arithmetic.
use crate::{BigInt, Decimal, ReversibleCollector, SnapshotMap};
use std::cmp::Ordering;

/// Owned observation of active keys and their total loads, including initial load.
/// Keys with zero load remain present until their last occurrence is removed.
/// Saved observations do not change when their collector changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadBalance<K> {
    loads: SnapshotMap<K, BigInt>,
    unfairness: Decimal,
}
impl<K> LoadBalance<K> {
    pub fn loads(&self) -> &SnapshotMap<K, BigInt> {
        &self.loads
    }
    /// Square root of the sum of squared deviations from the mean, rounded to
    /// six significant digits (nearest, ties to even), then stripped of trailing
    /// decimal zeros. Perfect balance, including no keys, returns Decimal::ZERO.
    /// This follows the public Java collector's observed significant-digit format,
    /// not the six decimal *places* stated in its public interface documentation.
    /// Native trailing-zero normalization does not preserve Java decimal scale.
    /// Exact integer moments avoid Java's overflow and floating cancellation.
    pub fn unfairness(&self) -> &Decimal {
        &self.unfairness
    }
}

/// Collect captured `(key, load, initial_load)` occurrences. Initial load is added
/// once per active key, never per occurrence; every occurrence contributes load.
/// All simultaneously active occurrences of a key must have the same initial load.
/// A different initial load panics atomically; remove all occurrences first.
/// This requirement keeps reconstruction and undo independent of insertion order.
/// Java instead retains the first initial load until the key disappears.
///
/// Loads accept signed i64 inputs but totals and squared moments are arbitrary
/// precision. Exact captured triples control removal, including multiplicity.
/// Replace stages removal/insertion atomically. Clone and finish share persistent
/// roots; finish computes an exact rational square root rounded to six significant
/// digits. No binary floating-point arithmetic is used.
///
/// Only keys present in the stream participate. Insert zero-load occurrences for
/// resources that should participate even while idle. An empty global stream
/// emits no aggregate row, while a standalone empty collector finishes to zero.
///
/// ```
/// use timetuck::{LoadBalanceCollector, ReversibleCollector, Decimal};
/// let mut c=LoadBalanceCollector::<&str>::default();
/// c.insert(&("A",2,0)); c.insert(&("B",1,0));
/// assert_eq!(c.finish().unfairness(), &Decimal::new(707107.into(),6));
/// let saved=c.finish();
/// c.insert(&("B",1,0));
/// assert_eq!(c.finish().unfairness(), &Decimal::ZERO);
/// assert_eq!(saved.loads()[&"B"], 1.into());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadBalanceCollector<K> {
    captures: SnapshotMap<(K, i64, i64), usize>,
    initial: SnapshotMap<K, (i64, usize)>,
    loads: SnapshotMap<K, BigInt>,
    sum: BigInt,
    squares: BigInt,
}
impl<K> Default for LoadBalanceCollector<K> {
    fn default() -> Self {
        Self {
            captures: SnapshotMap::new(),
            initial: SnapshotMap::new(),
            loads: SnapshotMap::new(),
            sum: BigInt::ZERO,
            squares: BigInt::ZERO,
        }
    }
}
impl<K: Clone + Ord> LoadBalanceCollector<K> {
    fn set_load(&mut self, key: &K, load: Option<BigInt>) {
        if let Some(old) = self.loads.get(key) {
            self.sum -= old;
            self.squares -= old * old;
        }
        if let Some(new) = load {
            self.sum += &new;
            self.squares += &new * &new;
            self.loads.insert(key.clone(), new);
        } else {
            self.loads.remove(key);
        }
    }
}
impl<K: Clone + Ord> ReversibleCollector<(K, i64, i64)> for LoadBalanceCollector<K> {
    type Output = LoadBalance<K>;
    fn insert(&mut self, value: &(K, i64, i64)) {
        let (key, load, initial) = value;
        let n = if let Some(&(old, n)) = self.initial.get(key) {
            assert_eq!(old, *initial, "conflicting active initial loads");
            n.checked_add(1).expect("load occurrence count overflow")
        } else {
            1
        };
        let next_load = self
            .loads
            .get(key)
            .cloned()
            .unwrap_or_else(|| (*initial).into())
            + *load;
        let mut next = self.clone();
        next.captures.insert(
            value.clone(),
            self.captures
                .get(value)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .expect("load occurrence count overflow"),
        );
        next.initial.insert(key.clone(), (*initial, n));
        next.set_load(key, Some(next_load));
        *self = next;
    }
    fn remove(&mut self, value: &(K, i64, i64)) -> bool {
        let Some(&count) = self.captures.get(value) else {
            return false;
        };
        let (key, load, initial) = value;
        let n = self.initial[key].1;
        let mut next = self.clone();
        if count == 1 {
            next.captures.remove(value);
        } else {
            next.captures.insert(value.clone(), count - 1);
        }
        if n == 1 {
            next.initial.remove(key);
            next.set_load(key, None);
        } else {
            next.initial.insert(key.clone(), (*initial, n - 1));
            next.set_load(key, Some(&self.loads[key] - *load));
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        let n = BigInt::from(self.loads.len());
        let numerator = &n * &self.squares - &self.sum * &self.sum;
        let unfairness = if numerator == BigInt::ZERO {
            Decimal::ZERO
        } else {
            rounded_root(&numerator, &n)
        };
        LoadBalance {
            loads: self.loads.clone(),
            unfairness,
        }
    }
}

// Round sqrt(numerator/denominator) directly, without approximating the rational.
// The digit lengths locate its decimal exponent; a squared midpoint decides
// rounding. Integer square root returns the floor, even after rational division.
pub(crate) fn rounded_root(numerator: &BigInt, denominator: &BigInt) -> Decimal {
    let ten = BigInt::from(10);
    let mut exponent =
        numerator.to_str_radix(10).len() as i64 - denominator.to_str_radix(10).len() as i64;
    let below = if exponent >= 0 {
        numerator
            < &(denominator
                * ten.pow(u32::try_from(exponent).expect("unfairness exponent overflow")))
    } else {
        &(numerator * ten.pow(u32::try_from(-exponent).expect("unfairness exponent overflow")))
            < denominator
    };
    if below {
        exponent -= 1;
    }
    let mut scale = i32::try_from(5 - exponent.div_euclid(2)).expect("unfairness scale overflow");
    let (a, b) = if scale >= 0 {
        (
            numerator
                * ten.pow(
                    scale
                        .unsigned_abs()
                        .checked_mul(2)
                        .expect("unfairness scale overflow"),
                ),
            denominator.clone(),
        )
    } else {
        (
            numerator.clone(),
            denominator
                * ten.pow(
                    scale
                        .unsigned_abs()
                        .checked_mul(2)
                        .expect("unfairness scale overflow"),
                ),
        )
    };
    let mut q = (&a / &b).sqrt();
    let mid = &q * 2u8 + 1u8;
    match (&a * 4u8).cmp(&(&b * &mid * &mid)) {
        Ordering::Greater => q += 1u8,
        Ordering::Equal if &q % 2u8 != BigInt::ZERO => q += 1u8,
        _ => {}
    }
    while &q % 10u8 == BigInt::ZERO {
        q /= 10u8;
        scale -= 1;
    }
    Decimal::new(q, scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_roots_round_ties_even_and_cross_decimal_exponents() {
        for (root, expected) in [(1234565, 123456), (1234575, 123458), (9999995, 1000000)] {
            let n = BigInt::from(root).pow(2);
            let actual = rounded_root(&n, &BigInt::from(100));
            assert_eq!(
                actual.numeric_cmp(&Decimal::from(expected)),
                Ordering::Equal
            );
        }
        for exponent in [-80i32, -3, 0, 3, 80] {
            let (n, d) = if exponent >= 0 {
                (
                    BigInt::from(9) * BigInt::from(10).pow(2 * exponent as u32),
                    BigInt::from(1),
                )
            } else {
                (
                    BigInt::from(9),
                    BigInt::from(10).pow(2 * exponent.unsigned_abs()),
                )
            };
            assert_eq!(rounded_root(&n, &d), Decimal::new(3.into(), -exponent));
        }
    }
}
