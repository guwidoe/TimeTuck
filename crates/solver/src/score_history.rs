//! A nonempty circular history with lazy reset and borrowed observations.
use crate::AcceptanceError;
use std::sync::OnceLock;

/// Scores written since the latest reset override a shared baseline. Reset
/// preserves the cursor; update writes the current slot and advances it.
///
/// Ordinary reset/update/current/get operations clone no scores. A full borrowed
/// slice is materialized only by `as_slice` and cached until the next write/reset.
/// Invalidating that optional snapshot drops its contents. Generation exhaustion
/// clears all stored slots once before reusing the generation counter. Stale
/// values can remain allocated until overwritten, rollover or history drop.
/// Cloning a history makes an independent logical snapshot and may clone scores.
///
/// ```
/// use timetuck::{ScoreHistory, SimpleScore};
/// let mut h = ScoreHistory::new(3, SimpleScore::new(-10))?;
/// h.update(SimpleScore::new(-5));
/// assert_eq!(h.current(), &SimpleScore::new(-10));
/// h.reset(SimpleScore::ZERO);
/// assert_eq!(h.cursor(), 1);
/// assert_eq!(h.as_slice(), &[SimpleScore::ZERO; 3]);
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct ScoreHistory<S> {
    baseline: S,
    slots: Vec<Option<(u64, S)>>,
    generation: u64,
    cursor: usize,
    written: bool,
    snapshot: OnceLock<Vec<S>>,
}
impl<S: Clone + Eq> ScoreHistory<S> {
    pub fn new(size: usize, initial: S) -> Result<Self, AcceptanceError> {
        if size == 0 {
            return Err(AcceptanceError::EmptyHistory);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(size)
            .map_err(|_| AcceptanceError::HistoryAllocation)?;
        slots.resize_with(size, || None);
        Ok(Self {
            baseline: initial,
            slots,
            generation: 0,
            cursor: 0,
            written: false,
            snapshot: OnceLock::new(),
        })
    }
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn is_empty(&self) -> bool {
        false
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    fn resolve<'a>(&'a self, slot: &'a Option<(u64, S)>) -> &'a S {
        match slot {
            Some((generation, value)) if *generation == self.generation => value,
            _ => &self.baseline,
        }
    }
    pub fn get(&self, index: usize) -> Option<&S> {
        self.slots.get(index).map(|slot| self.resolve(slot))
    }
    pub fn current(&self) -> &S {
        self.resolve(&self.slots[self.cursor])
    }
    /// Iterate in slot-index order, without cloning or materializing a snapshot.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &S> + ExactSizeIterator {
        self.slots.iter().map(|slot| self.resolve(slot))
    }
    /// Preserve borrowed contiguous observations, paying for score clones only
    /// on first observation after mutation. A clone panic leaves the cache empty
    /// and the logical history unchanged; the next observation may retry.
    pub fn as_slice(&self) -> &[S] {
        self.snapshot
            .get_or_init(|| self.iter().cloned().collect())
            .as_slice()
    }
    /// Write even when equal to the previous value; every update advances once.
    pub fn update(&mut self, score: S) {
        self.snapshot.take();
        self.slots[self.cursor] = Some((self.generation, score));
        self.written = true;
        self.cursor = if self.cursor + 1 == self.len() {
            0
        } else {
            self.cursor + 1
        };
    }
    /// Replace every logical slot without changing the cursor. Return false for
    /// an identical baseline with no intervening writes. Equality runs before any
    /// mutation; a comparison panic leaves history and cached observations intact.
    pub fn reset(&mut self, score: S) -> bool {
        if !self.written && self.baseline == score {
            return false;
        }
        self.snapshot.take();
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
        } else {
            for slot in &mut self.slots {
                *slot = None;
            }
            self.generation = 0;
        }
        self.baseline = score;
        self.written = false;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_rollover_discards_every_old_stamp_and_preserves_cursor() {
        let mut h = ScoreHistory::new(3, -10).unwrap();
        h.update(-9); // Generation zero must not reappear after rollover.
        h.generation = u64::MAX;
        h.update(-8); // Both zero and MAX stamps now exist.
        assert_eq!(h.as_slice(), &[-10, -8, -10]);
        assert!(h.reset(7));
        assert_eq!(h.cursor(), 2);
        assert_eq!(h.as_slice(), &[7, 7, 7]);
        h.update(8);
        assert_eq!(h.as_slice(), &[7, 7, 8]);
        assert_eq!(h.current(), &7);
        assert!(h.reset(7));
        assert!(!h.reset(7));
        assert_eq!(h.as_slice(), &[7, 7, 7]);
    }
}
