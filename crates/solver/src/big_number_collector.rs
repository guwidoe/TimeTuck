//! Persistent arbitrary-precision integer and scale-preserving decimal collectors.
use crate::decimal::round_half_even;
use crate::{BigInt, Decimal, ReversibleCollector, SnapshotMap};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct IntegerState {
    values: SnapshotMap<BigInt, usize>,
    sum: BigInt,
    count: usize,
}
impl IntegerState {
    fn insert(&mut self, value: &BigInt) {
        let count = self.count.checked_add(1).expect("collector count overflow");
        let sum = &self.sum + value;
        self.values.insert(
            value.clone(),
            self.values.get(value).copied().unwrap_or(0) + 1,
        );
        self.sum = sum;
        self.count = count;
    }
    fn remove(&mut self, value: &BigInt) -> bool {
        let Some(&n) = self.values.get(value) else {
            return false;
        };
        let sum = &self.sum - value;
        if n == 1 {
            self.values.remove(value);
        } else {
            self.values.insert(value.clone(), n - 1);
        }
        self.sum = sum;
        self.count -= 1;
        true
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DecimalState {
    values: SnapshotMap<Decimal, usize>,
    sum: Decimal,
    count: usize,
}
impl DecimalState {
    fn insert(&mut self, value: &Decimal) {
        let count = self.count.checked_add(1).expect("collector count overflow");
        let sum = self.sum.add(value);
        self.values.insert(
            value.clone(),
            self.values.get(value).copied().unwrap_or(0) + 1,
        );
        self.sum = sum;
        self.count = count;
    }
    fn remove(&mut self, value: &Decimal) -> bool {
        let Some(&n) = self.values.get(value) else {
            return false;
        };
        let sum = self.sum.subtract(value);
        if n == 1 {
            self.values.remove(value);
        } else {
            self.values.insert(value.clone(), n - 1);
        }
        self.sum = sum;
        self.count -= 1;
        true
    }
}
/// Exact arbitrary-precision integer sum; zero for an empty collector.
/// Captured multiplicities distinguish missing removals. Snapshots share map roots;
/// cloning the cached sum costs space proportional to its digit count.
/// ```
/// use timetuck::{BigInt, BigIntSumCollector, ReversibleCollector};
/// let large=BigInt::from(10).pow(100);
/// let mut sum=BigIntSumCollector::default();
/// sum.insert(&large);sum.insert(&large);
/// assert_eq!(sum.finish(), &large * 2);
/// assert!(sum.remove(&large));
/// assert_eq!(sum.finish(), large);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BigIntSumCollector(IntegerState);
/// Integer average rounded to nearest with ties to even, at decimal scale zero.
/// Empty finish is None. No fixed-width integer or floating-point conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BigIntAverageCollector(IntegerState);
/// Exact decimal sum. The cached scale starts at zero and never decreases,
/// including after removal of the last value. Empty finish can be scaled zero.
/// Inputs with numerically equal values but different scales are distinct captures.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecimalSumCollector(DecimalState);
/// Decimal average rounded half-even at the cached sum's scale; empty is None.
/// Scale persists after removals, so different histories may produce different
/// rounding. Normalize input scales for history-independent solver objectives
/// and full-rebuild diagnostics. A graph discards a group when it becomes empty;
/// a standalone collector retains its scale until replaced with Default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecimalAverageCollector(DecimalState);
macro_rules! collector {
    ($name:ident,$input:ty,$output:ty,$finish:expr) => {
        impl ReversibleCollector<$input> for $name {
            type Output = $output;
            fn insert(&mut self, value: &$input) {
                self.0.insert(value)
            }
            fn remove(&mut self, value: &$input) -> bool {
                self.0.remove(value)
            }
            fn finish(&self) -> Self::Output {
                ($finish)(self)
            }
        }
    };
}
collector!(
    BigIntSumCollector,
    BigInt,
    BigInt,
    |c: &BigIntSumCollector| c.0.sum.clone()
);
collector!(
    BigIntAverageCollector,
    BigInt,
    Option<Decimal>,
    |c: &BigIntAverageCollector| (c.0.count != 0)
        .then(|| Decimal::from(round_half_even(&c.0.sum, c.0.count)))
);
collector!(
    DecimalSumCollector,
    Decimal,
    Decimal,
    |c: &DecimalSumCollector| c.0.sum.clone()
);
collector!(
    DecimalAverageCollector,
    Decimal,
    Option<Decimal>,
    |c: &DecimalAverageCollector| c.0.sum.divide_count(c.0.count)
);
