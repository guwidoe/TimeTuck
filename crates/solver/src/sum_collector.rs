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
/// Storage may defer one logical primitive replacement. Checked replacements
/// of its new value compose with it; a checked inverse cancels the pending edit.
/// Other successful mutations first materialize it.
/// Logical equality compares frequencies regardless of deferred representation.
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
#[derive(Clone, Debug, Default)]
pub struct SumCollector {
    // The physical base plus at most one checked logical occurrence replacement.
    values: IntCollectorCounts,
    pending: Option<(i64, i64)>,
    count: usize,
    sum: i64,
}
impl SumCollector {
    fn contains(&self, value: i64) -> bool {
        match self.pending {
            Some((_, new)) if value == new => true,
            Some((old, _)) if value == old => self.values.frequency(old).is_some_and(|n| n > 1),
            _ => self.values.contains(value),
        }
    }
    fn materialize(&mut self) {
        if let Some((old, new)) = self.pending.take() {
            // Preflighted when deferred. Primitive comparison/cloning/drop cannot
            // call user code; this changes storage only, never current sum/count.
            assert!(self.values.replace(old, new), "pending captured occurrence");
        }
    }
    fn logical_values(&self) -> IntCollectorCounts {
        let mut values = self.values.clone();
        if let Some((old, new)) = self.pending {
            assert!(values.replace(old, new), "pending captured occurrence");
        }
        values
    }
    pub fn try_insert(&mut self, value: i64) -> Result<(), SumOverflow> {
        let sum = self.sum.checked_add(value).ok_or(SumOverflow)?;
        let count = self.count.checked_add(1).expect("collector count overflow");
        // Complete recoverable/count preflight before changing pending or roots.
        self.materialize();
        self.values.increment(value);
        self.count = count;
        self.sum = sum;
        Ok(())
    }
    pub fn try_remove(&mut self, value: i64) -> Result<bool, SumOverflow> {
        let Some(sum) = self.sum.checked_sub(value) else {
            return if self.contains(value) {
                Err(SumOverflow)
            } else {
                Ok(false)
            };
        };
        if self.pending.is_some() {
            if !self.contains(value) {
                return Ok(false);
            }
            self.materialize();
            assert!(self.values.remove(value), "preflighted captured occurrence");
        } else if !self.values.remove(value) {
            // Preserve the ordinary direct lookup/removal path without a
            // preliminary membership traversal when no edit is pending.
            return Ok(false);
        }
        self.count -= 1;
        self.sum = sum;
        Ok(true)
    }
    pub fn try_replace(&mut self, old: i64, new: i64) -> Result<bool, SumOverflow> {
        if old == new {
            return Ok(self.contains(old));
        }
        // Even a pending inverse MUST check delta and current total first.
        // E.g. 0->MIN is representable, but MIN->0 has an overflowing delta.
        let Some(sum) = new
            .checked_sub(old)
            .and_then(|delta| self.sum.checked_add(delta))
        else {
            return if self.contains(old) {
                Err(SumOverflow)
            } else {
                Ok(false)
            };
        };
        if !self.contains(old) {
            return Ok(false);
        }
        if let Some((base_old, pending_new)) = self.pending
            && old == pending_new
        {
            // Primitive equal captures are indistinguishable. The frequency
            // deltas compose even if pending_new already existed in the base.
            // Recheck only this operation's delta above, never the composed
            // base_old->new delta, which need not itself fit signed i64.
            self.pending = (base_old != new).then_some((base_old, new));
        } else {
            self.materialize();
            self.pending = Some((old, new));
        }
        self.sum = sum;
        Ok(true)
    }
}
impl PartialEq for SumCollector {
    fn eq(&self, other: &Self) -> bool {
        if self.count != other.count || self.sum != other.sum {
            return false;
        }
        // Identical deferred transforms are injective on captured frequencies:
        // equal physical bases imply equal logical multisets and vice versa.
        // Avoid transient COW materialization for common clone/equal histories.
        if self.pending == other.pending {
            return self.values == other.values;
        }
        self.logical_values() == other.logical_values()
    }
}
impl Eq for SumCollector {}

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
