//! Checked signed integer summation from captured occurrences.
use crate::{ReversibleCollector, int_collector_counts::IntCollectorCounts};
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SumOverflow;
impl fmt::Display for SumOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("integer sum overflow")
    }
}
impl Error for SumOverflow {}
/// Retractable checked signed-64-bit sum; empty finish is zero.
///
/// Unlike the wrapping sum in IntSummary, insert/removal checks the new total.
/// Replacement checks `new - old`, then addition of that difference to the total.
/// Both intermediates must fit i64, even if removing then inserting would fit.
/// Equal-value replacements have zero difference. These rules match observations
/// of the pinned public Community integer-sum collector.
///
/// Missing captures return false; overflow leaves state unchanged. Trait methods
/// panic on overflow for graph transaction rollback. Captures retain duplicate and
/// zero occurrences. Clone shares persistent roots; finish does not scan inputs.
///
/// ```
/// use timetuck::{SumCollector,ReversibleCollector,SumOverflow};
/// let mut c=SumCollector::default();
/// c.try_insert(i64::MAX)?;
/// assert_eq!(c.try_insert(1),Err(SumOverflow));
/// assert_eq!(c.finish(),i64::MAX);
/// assert_eq!(c.try_replace(i64::MAX,0),Ok(true));
/// # Ok::<(),SumOverflow>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SumCollector {
    values: IntCollectorCounts,
    count: usize,
    sum: i64,
}
impl SumCollector {
    pub fn try_insert(&mut self, value: i64) -> Result<(), SumOverflow> {
        let sum = self.sum.checked_add(value).ok_or(SumOverflow)?;
        let count = self.count.checked_add(1).expect("collector count overflow");
        // All recoverable failures precede mutation. Counts have primitive keys
        // and values, so no user comparison, clone or destructor can panic here.
        self.values.increment(value);
        self.count = count;
        self.sum = sum;
        Ok(())
    }
    pub fn try_remove(&mut self, value: i64) -> Result<bool, SumOverflow> {
        let Some(sum) = self.sum.checked_sub(value) else {
            return if self.values.contains(value) {
                Err(SumOverflow)
            } else {
                Ok(false)
            };
        };
        if !self.values.remove(value) {
            return Ok(false);
        }
        self.count -= 1;
        self.sum = sum;
        Ok(true)
    }
    pub fn try_replace(&mut self, old: i64, new: i64) -> Result<bool, SumOverflow> {
        if old == new {
            return Ok(self.values.contains(old));
        }
        let Some(sum) = new
            .checked_sub(old)
            .and_then(|delta| self.sum.checked_add(delta))
        else {
            return if self.values.contains(old) {
                Err(SumOverflow)
            } else {
                Ok(false)
            };
        };
        if !self.values.replace(old, new) {
            return Ok(false);
        }
        self.sum = sum;
        Ok(true)
    }
}

impl ReversibleCollector<i64> for SumCollector {
    type Output = i64;
    fn insert(&mut self, v: &i64) {
        self.try_insert(*v).expect("sum insertion overflow");
    }
    fn remove(&mut self, v: &i64) -> bool {
        self.try_remove(*v).expect("sum removal overflow")
    }
    fn replace(&mut self, old: &i64, new: &i64) -> bool {
        self.try_replace(*old, *new)
            .expect("sum replacement overflow")
    }
    fn finish(&self) -> i64 {
        self.sum
    }
}
