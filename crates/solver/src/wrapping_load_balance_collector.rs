//! Signed-64-bit wrapping load totals and moments with explicit root arithmetic.
use crate::load_balance_collector::rounded_root;
use crate::{BigInt, Decimal, ReversibleCollector, SnapshotMap};
use std::{error::Error, fmt};

/// No real square root exists for the selected policy's radicand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrappingLoadBalanceError {
    NegativeRadicand,
}
impl fmt::Display for WrappingLoadBalanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("wrapping load-balance moments produce a negative radicand")
    }
}
impl Error for WrappingLoadBalanceError {}

/// Explicit root evaluation on already-wrapped i64 moments.
/// This is a native numerical policy, not a claim about Java implementation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WrappingLoadBalanceRootPolicy {
    /// Existing widened exact-rational evaluation and six-digit half-even root.
    #[default]
    Exact,
    /// Convert Q, wrapping S*S and key count separately to binary64; divide,
    /// subtract and sqrt in binary64. Then round that binary64 root's exact
    /// numeric value to six significant decimal digits, half-even.
    Binary64,
}

/// Immutable wrapped per-key totals and six-significant-digit unfairness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrappingLoadBalance<K> {
    loads: SnapshotMap<K, i64>,
    unfairness: Decimal,
}
impl<K> WrappingLoadBalance<K> {
    pub fn loads(&self) -> &SnapshotMap<K, i64> {
        &self.loads
    }
    pub fn unfairness(&self) -> &Decimal {
        &self.unfairness
    }
}

/// Opt-in modular arithmetic; the ordinary LoadBalanceCollector remains exact.
/// Per-key totals, their sum S, and squared moment Q use wrapping i64 arithmetic.
/// By default, unfairness rounds sqrt((n*Q - S.wrapping_mul(S))/n), widening n*Q and
/// subtraction to BigInt, to six significant digits with half-even ties.
/// Negative radicands return a typed error; empty input returns zero. Exact is
/// the default. The explicitly selected Binary64 root policy permits precision
/// loss and cancellation; it makes no Java implementation/decimal-scale claim.
///
/// Captured triples, multiplicities, active initial-load consistency and immutable
/// snapshots have the same contracts as the exact collector. Membership counts
/// remain checked usize. Modular updates are reversible and independent of order.
/// An error at finish does not erase captures or prevent later removal/recovery.
/// Keys require stable, lawful ordering and identity-preserving clones. Panicking
/// destructors and allocation aborts are unsupported.
///
/// ```
/// use timetuck::{ReversibleCollector, WrappingLoadBalanceCollector, WrappingLoadBalanceError};
/// let mut loads = WrappingLoadBalanceCollector::default();
/// loads.insert(&("A", i64::MAX, 0));
/// loads.insert(&("A", i64::MAX, 0));
/// assert_eq!(loads.try_finish()?.loads()[&"A"], -2);
/// # Ok::<(), WrappingLoadBalanceError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrappingLoadBalanceCollector<K> {
    captures: SnapshotMap<(K, i64, i64), usize>,
    initial: SnapshotMap<K, (i64, usize)>,
    loads: SnapshotMap<K, i64>,
    sum: i64,
    squares: i64,
    root_policy: WrappingLoadBalanceRootPolicy,
}
impl<K> Default for WrappingLoadBalanceCollector<K> {
    fn default() -> Self {
        Self {
            captures: SnapshotMap::new(),
            initial: SnapshotMap::new(),
            loads: SnapshotMap::new(),
            sum: 0,
            squares: 0,
            root_policy: WrappingLoadBalanceRootPolicy::Exact,
        }
    }
}
impl<K: Clone + Ord> WrappingLoadBalanceCollector<K> {
    /// Select observation arithmetic. Existing captures/moments stay unchanged;
    /// Clone retains the selection and prior returned snapshots stay immutable.
    pub fn with_root_policy(mut self, policy: WrappingLoadBalanceRootPolicy) -> Self {
        self.root_policy = policy;
        self
    }
    pub fn root_policy(&self) -> WrappingLoadBalanceRootPolicy {
        self.root_policy
    }
    /// Observe without mutation. A negative radicand is an arithmetic result,
    /// not a missing capture, panic, NaN, or implicit clamp to zero.
    pub fn try_finish(&self) -> Result<WrappingLoadBalance<K>, WrappingLoadBalanceError> {
        let unfairness = if self.loads.is_empty() {
            Decimal::ZERO
        } else if self.root_policy == WrappingLoadBalanceRootPolicy::Binary64 {
            binary64_unfairness(self.sum, self.squares, self.loads.len())?
        } else {
            let n = BigInt::from(self.loads.len());
            let numerator =
                &n * BigInt::from(self.squares) - BigInt::from(self.sum.wrapping_mul(self.sum));
            if numerator < BigInt::ZERO {
                return Err(WrappingLoadBalanceError::NegativeRadicand);
            }
            if numerator == BigInt::ZERO {
                Decimal::ZERO
            } else {
                rounded_root(&numerator, &n)
            }
        };
        Ok(WrappingLoadBalance {
            loads: self.loads.clone(),
            unfairness,
        })
    }
    fn set_load(&mut self, key: &K, load: Option<i64>) {
        if let Some(old) = self.loads.get(key) {
            self.sum = self.sum.wrapping_sub(*old);
            self.squares = self.squares.wrapping_sub(old.wrapping_mul(*old));
        }
        if let Some(new) = load {
            self.sum = self.sum.wrapping_add(new);
            self.squares = self.squares.wrapping_add(new.wrapping_mul(new));
            self.loads.insert(key.clone(), new);
        } else {
            self.loads.remove(key);
        }
    }
}
impl<K: Clone + Ord> ReversibleCollector<(K, i64, i64)> for WrappingLoadBalanceCollector<K> {
    type Output = Result<WrappingLoadBalance<K>, WrappingLoadBalanceError>;
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
            .copied()
            .unwrap_or(*initial)
            .wrapping_add(*load);
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
            next.set_load(key, Some(self.loads[key].wrapping_sub(*load)));
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        self.try_finish()
    }
}

// Evaluate only the observation. Captured totals and modular moments are shared
// with the exact mode, including its existing transactional mutation paths.
fn binary64_unfairness(
    sum: i64,
    squares: i64,
    count: usize,
) -> Result<Decimal, WrappingLoadBalanceError> {
    if count == 0 {
        return Ok(Decimal::ZERO);
    }
    // Do not cast count through i64: usize::MAX is a valid unsigned count bound.
    // Counts above 2^53 are rounded by binary64 conversion as part of this policy.
    let q = squares as f64;
    let t = sum.wrapping_mul(sum) as f64;
    let n = count as f64;
    let quotient = t / n;
    let radicand = q - quotient;
    // For supported usize widths <=64, finite i64 moments/count imply
    // |radicand| <= 2^64. No nonfinite public result can arise from valid state.
    assert!(
        n.is_finite() && radicand.is_finite(),
        "finite bounded moments"
    );
    if radicand < 0.0 {
        return Err(WrappingLoadBalanceError::NegativeRadicand);
    }
    rounded_binary64_root(radicand.sqrt())
}
fn rounded_binary64_root(root: f64) -> Result<Decimal, WrappingLoadBalanceError> {
    // Private helper precondition: public evaluation above supplies a finite root.
    assert!(root.is_finite(), "finite bounded root");
    if root < 0.0 {
        return Err(WrappingLoadBalanceError::NegativeRadicand);
    }
    if root == 0.0 {
        return Ok(Decimal::ZERO);
    } // Normalize either zero sign.
    // Decimal has no float conversion API. Decode the finite binary number
    // exactly, avoiding a shortest-string conversion or binary decimal scaling.
    let bits = root.to_bits();
    let field = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1_u64 << 52), field - 1023 - 52)
    };
    let (numerator, denominator) = if exponent >= 0 {
        (BigInt::from(mantissa) << exponent as usize, BigInt::from(1))
    } else {
        (
            BigInt::from(mantissa),
            BigInt::from(1) << exponent.unsigned_abs() as usize,
        )
    };
    // rounded_root(squared exact binary root) rounds the already-evaluated
    // binary64 value, not the original exact-rational moment expression.
    Ok(rounded_root(&numerator.pow(2), &denominator.pow(2)))
}

#[cfg(test)]
mod binary64_policy_tests {
    use super::*;
    #[test]
    fn finite_zero_negative_and_unsigned_count_bound_have_explicit_results() {
        assert_eq!(rounded_binary64_root(-0.0), Ok(Decimal::ZERO));
        assert!(std::panic::catch_unwind(|| rounded_binary64_root(f64::NAN)).is_err());
        assert!(std::panic::catch_unwind(|| rounded_binary64_root(f64::INFINITY)).is_err());
        assert_eq!(
            rounded_binary64_root(-1.0),
            Err(WrappingLoadBalanceError::NegativeRadicand)
        );
        assert_eq!(binary64_unfairness(0, 0, usize::MAX), Ok(Decimal::ZERO));
        assert!(binary64_unfairness(1, 1, usize::MAX).is_ok());
        // Exact six-significant half-even at representable integer root ties.
        for (root, coefficient, scale) in [
            (1234565.0, 123456, -1),
            (1234575.0, 123458, -1),
            (9999995.0, 1, -7),
        ] {
            assert_eq!(
                rounded_binary64_root(root).unwrap(),
                Decimal::new(coefficient.into(), scale)
            );
        }
    }
}
