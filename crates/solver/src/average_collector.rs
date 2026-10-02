//! Integer averages matching the pinned Community collector's arithmetic boundaries.
use crate::{IntCollector, ReversibleCollector};
use std::{error::Error, fmt};

/// A checked average insertion/removal sum or replacement difference overflowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AverageOverflow;
impl fmt::Display for AverageOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("average arithmetic overflow")
    }
}
impl Error for AverageOverflow {}

/// Retractable signed-64-bit average, returning None for an empty collection.
/// The sum and count are converted to f64 before division. This is not an exact
/// rational, decimal or duration average.
///
/// Community 2.7.0 inserts/removals check signed sum overflow. Replacement checks
/// `new - old`, then adds that difference with wrapping arithmetic. Consequently,
/// a replacement can yield a sum that rebuilding via insertions would reject.
/// Keep all intermediate sums/differences representable when requiring rebuild
/// diagnostics to succeed. These boundaries are checked by the public API oracle.
///
/// The try methods preserve state on errors. ReversibleCollector methods panic
/// on overflow; graph transactions invalidate and restore their checkpoint on drop.
/// Missing captured values return false without mutation. Clone shares roots.
///
/// ```
/// use timetuck::{AverageCollector, ReversibleCollector};
/// let mut average = AverageCollector::default();
/// average.try_insert(4)?;
/// average.try_insert(2)?;
/// assert_eq!(average.finish(), Some(3.0));
/// assert!(average.try_replace(4, 6)?);
/// assert_eq!(average.finish(), Some(4.0));
/// # Ok::<(), timetuck::AverageOverflow>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AverageCollector(IntCollector);
impl AverageCollector {
    pub fn try_insert(&mut self, value: i64) -> Result<(), AverageOverflow> {
        self.0
            .summary()
            .sum
            .checked_add(value)
            .ok_or(AverageOverflow)?;
        self.0.insert(value);
        Ok(())
    }
    pub fn try_remove(&mut self, value: i64) -> Result<bool, AverageOverflow> {
        let mut next = self.0.clone();
        if !next.remove(value) {
            return Ok(false);
        }
        self.0
            .summary()
            .sum
            .checked_sub(value)
            .ok_or(AverageOverflow)?;
        self.0 = next;
        Ok(true)
    }
    pub fn try_replace(&mut self, old: i64, new: i64) -> Result<bool, AverageOverflow> {
        let mut next = self.0.clone();
        if !next.remove(old) {
            return Ok(false);
        }
        new.checked_sub(old).ok_or(AverageOverflow)?;
        next.insert(new);
        self.0 = next;
        Ok(true)
    }
}
impl ReversibleCollector<i64> for AverageCollector {
    type Output = Option<f64>;
    fn insert(&mut self, value: &i64) {
        self.try_insert(*value).expect("average insertion overflow");
    }
    fn remove(&mut self, value: &i64) -> bool {
        self.try_remove(*value).expect("average removal overflow")
    }
    fn replace(&mut self, old: &i64, new: &i64) -> bool {
        self.try_replace(*old, *new)
            .expect("average replacement difference overflow")
    }
    fn finish(&self) -> Self::Output {
        let summary = self.0.summary();
        (summary.count != 0).then(|| summary.sum as f64 / summary.count as f64)
    }
}
