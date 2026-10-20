//! Explicit signed-64-bit wrapping load totals and moments; exact rational rounding.
use crate::load_balance_collector::rounded_root;
use crate::{BigInt, Decimal, ReversibleCollector, SnapshotMap};
use std::{error::Error, fmt};

/// No real square root exists for the policy's widened rational radicand.
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
/// Unfairness rounds sqrt((n*Q - S.wrapping_mul(S))/n), widening n*Q and
/// subtraction to BigInt, to six significant digits with half-even ties.
/// Negative radicands return a typed error; empty input returns zero. No binary
/// floating arithmetic, cancellation emulation, or Java decimal-scale claim.
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
}
impl<K> Default for WrappingLoadBalanceCollector<K> {
    fn default() -> Self {
        Self {
            captures: SnapshotMap::new(),
            initial: SnapshotMap::new(),
            loads: SnapshotMap::new(),
            sum: 0,
            squares: 0,
        }
    }
}
impl<K: Clone + Ord> WrappingLoadBalanceCollector<K> {
    /// Observe without mutation. A negative radicand is an arithmetic result,
    /// not a missing capture, panic, NaN, or implicit clamp to zero.
    pub fn try_finish(&self) -> Result<WrappingLoadBalance<K>, WrappingLoadBalanceError> {
        let unfairness = if self.loads.is_empty() {
            Decimal::ZERO
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
