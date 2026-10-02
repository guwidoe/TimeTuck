//! Signed nanosecond values and reversible duration aggregation.
use crate::{ReversibleCollector, SnapshotMap};
use std::{error::Error, fmt};
const BILLION: i128 = 1_000_000_000;

/// A duration value or an intermediate duration calculation exceeded its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurationOverflow;
impl fmt::Display for DurationOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("duration arithmetic overflow")
    }
}
impl Error for DurationOverflow {}

/// Signed nanoseconds with a normalized signed-64-bit seconds component.
/// Unlike std::time::Duration, negative values are supported. Seconds use floor
/// division, so -1 nanosecond has seconds -1 and subsecond nanoseconds 999999999.
/// Arithmetic checks the seconds intermediate before normalizing nanoseconds,
/// matching the collector's public Java Duration arithmetic boundaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SignedDuration(i128);
impl SignedDuration {
    pub const ZERO: Self = Self(0);
    pub const MIN: Self = Self(i64::MIN as i128 * BILLION);
    pub const MAX: Self = Self(i64::MAX as i128 * BILLION + BILLION - 1);
    pub const fn from_seconds(seconds: i64) -> Self {
        Self(seconds as i128 * BILLION)
    }
    pub fn from_nanos(nanos: i128) -> Result<Self, DurationOverflow> {
        if (Self::MIN.0..=Self::MAX.0).contains(&nanos) {
            Ok(Self(nanos))
        } else {
            Err(DurationOverflow)
        }
    }
    pub fn seconds(self) -> i64 {
        self.0.div_euclid(BILLION) as i64
    }
    pub fn subsec_nanos(self) -> u32 {
        self.0.rem_euclid(BILLION) as u32
    }
    pub const fn as_nanos(self) -> i128 {
        self.0
    }
    fn add_parts(self, seconds: i64, nanos: i64) -> Result<Self, DurationOverflow> {
        let seconds = self
            .seconds()
            .checked_add(seconds)
            .ok_or(DurationOverflow)?;
        Self::from_nanos(seconds as i128 * BILLION + self.subsec_nanos() as i128 + nanos as i128)
    }
    pub fn checked_add(self, other: Self) -> Result<Self, DurationOverflow> {
        self.add_parts(other.seconds(), other.subsec_nanos() as i64)
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, DurationOverflow> {
        let nanos = -(other.subsec_nanos() as i64);
        if other.seconds() == i64::MIN {
            self.add_parts(i64::MAX, nanos)?.add_parts(1, 0)
        } else {
            self.add_parts(-other.seconds(), nanos)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DurationState {
    values: SnapshotMap<SignedDuration, usize>,
    count: usize,
    sum: SignedDuration,
}
impl DurationState {
    fn insert(&mut self, value: SignedDuration) -> Result<(), DurationOverflow> {
        let sum = self.sum.checked_add(value)?;
        let count = self.count.checked_add(1).ok_or(DurationOverflow)?;
        self.values
            .insert(value, self.values.get(&value).copied().unwrap_or(0) + 1);
        self.count = count;
        self.sum = sum;
        Ok(())
    }
    fn remove(&mut self, value: SignedDuration) -> Result<bool, DurationOverflow> {
        let Some(&multiplicity) = self.values.get(&value) else {
            return Ok(false);
        };
        let sum = self.sum.checked_sub(value)?;
        if multiplicity == 1 {
            self.values.remove(&value);
        } else {
            self.values.insert(value, multiplicity - 1);
        }
        self.count -= 1;
        self.sum = sum;
        Ok(true)
    }
    fn replace(
        &mut self,
        old: SignedDuration,
        new: SignedDuration,
    ) -> Result<bool, DurationOverflow> {
        let mut next = self.clone();
        if !next.remove(old)? {
            return Ok(false);
        }
        next.insert(new)?;
        *self = next;
        Ok(true)
    }
}

/// Retractable signed duration sum; empty finish is ZERO.
/// Updates check normalized duration arithmetic. Replacement subtracts the old
/// value before adding the new one. Try methods preserve state on error.
/// Trait methods panic on overflow, participating in graph transaction rollback.
/// Cloning shares persistent occurrence roots; missing removal returns false.
///
/// ```
/// use timetuck::{DurationSumCollector, SignedDuration, ReversibleCollector};
/// let mut sum = DurationSumCollector::default();
/// sum.try_insert(SignedDuration::from_seconds(-2))?;
/// sum.try_insert(SignedDuration::from_nanos(500_000_000)?)?;
/// assert_eq!(sum.finish().as_nanos(), -1_500_000_000);
/// # Ok::<(), timetuck::DurationOverflow>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DurationSumCollector(DurationState);

/// Retractable duration average; empty finish is None.
/// Pinned Community arithmetic first converts the total to signed-64-bit
/// nanoseconds, then divides by the count, truncating toward zero. Consequently,
/// finishing can overflow even if each input and the final average would fit.
/// try_finish exposes this without panicking; finish panics and graph transactions
/// roll back. Updates have the same checked arithmetic as DurationSumCollector.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DurationAverageCollector(DurationState);
impl DurationAverageCollector {
    pub fn try_finish(&self) -> Result<Option<SignedDuration>, DurationOverflow> {
        if self.0.count == 0 {
            return Ok(None);
        }
        let nanos = i64::try_from(self.0.sum.as_nanos()).map_err(|_| DurationOverflow)?;
        Ok(Some(SignedDuration(nanos as i128 / self.0.count as i128)))
    }
}
macro_rules! duration_updates {
    ($name:ident, $output:ty, $finish:expr) => {
        impl $name {
            pub fn try_insert(&mut self, value: SignedDuration) -> Result<(), DurationOverflow> {
                self.0.insert(value)
            }
            pub fn try_remove(&mut self, value: SignedDuration) -> Result<bool, DurationOverflow> {
                self.0.remove(value)
            }
            pub fn try_replace(
                &mut self,
                old: SignedDuration,
                new: SignedDuration,
            ) -> Result<bool, DurationOverflow> {
                self.0.replace(old, new)
            }
        }
        impl ReversibleCollector<SignedDuration> for $name {
            type Output = $output;
            fn insert(&mut self, value: &SignedDuration) {
                self.try_insert(*value)
                    .expect("duration insertion overflow");
            }
            fn remove(&mut self, value: &SignedDuration) -> bool {
                self.try_remove(*value).expect("duration removal overflow")
            }
            fn replace(&mut self, old: &SignedDuration, new: &SignedDuration) -> bool {
                self.try_replace(*old, *new)
                    .expect("duration replacement overflow")
            }
            fn finish(&self) -> Self::Output {
                ($finish)(self)
            }
        }
    };
}
duration_updates!(
    DurationSumCollector,
    SignedDuration,
    |c: &DurationSumCollector| c.0.sum
);
duration_updates!(
    DurationAverageCollector,
    Option<SignedDuration>,
    |c: &DurationAverageCollector| c
        .try_finish()
        .expect("duration average total exceeds signed nanoseconds")
);
