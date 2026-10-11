use std::collections::VecDeque;

const ROW_LIMIT: usize = 8;

struct Row {
    left: usize,
    choices: Box<[usize]>,
}
/// Opening-local cache of occurrence IDs, not values or validated moves. A small
/// probation window avoids allocating rows for one-off left choices. Boxed slices
/// have no spare Vec capacity; total retained slice lengths are bounded. Scratch
/// allocation remains independent, reusable, and at most the right pool length.
pub(super) struct EligibilityCache {
    rows: VecDeque<Row>,
    recent: VecDeque<usize>,
    retained: usize,
    maximum: usize,
}
impl EligibilityCache {
    pub(super) fn for_opening(right_len: usize, attempts: u64) -> Option<Self> {
        (right_len >= 64 && attempts >= 4).then(|| Self {
            rows: VecDeque::with_capacity(ROW_LIMIT),
            recent: VecDeque::with_capacity(ROW_LIMIT),
            retained: 0,
            maximum: right_len.saturating_mul(2),
        })
    }
    pub(super) fn get_or_fill<'a>(
        &'a mut self,
        left: usize,
        scratch: &'a mut Vec<usize>,
        fill: impl FnOnce(&mut Vec<usize>),
    ) -> &'a [usize] {
        if let Some(index) = self.rows.iter().position(|r| r.left == left) {
            let row = self.rows.remove(index).expect("located row");
            self.rows.push_back(row);
            return &self.rows.back().expect("hit row").choices;
        }
        scratch.clear();
        fill(scratch);
        let Some(index) = self.recent.iter().position(|id| *id == left) else {
            if self.recent.len() == ROW_LIMIT {
                self.recent.pop_front();
            }
            self.recent.push_back(left);
            return scratch;
        };
        self.recent.remove(index);
        // Fill only contains occurrences from the right pool, so it always fits
        // the budget. Empty rows are retained too, bounded by ROW_LIMIT.
        assert!(scratch.len() <= self.maximum);
        while self.rows.len() == ROW_LIMIT || self.retained > self.maximum - scratch.len() {
            let row = self.rows.pop_front().expect("bounded nonempty cache");
            self.retained -= row.choices.len();
        }
        let choices = scratch.clone().into_boxed_slice();
        self.retained += choices.len();
        self.rows.push_back(Row { left, choices });
        &self.rows.back().expect("inserted row").choices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_rows_and_empty_results_skip_fills_without_expanding_occurrences() {
        let mut cache = EligibilityCache::for_opening(64, 100).unwrap();
        let mut scratch = Vec::with_capacity(64);
        let mut fills = 0;
        for _ in 0..100 {
            assert_eq!(
                cache.get_or_fill(2, &mut scratch, |s| {
                    fills += 1;
                    s.extend([7, 7, 3]);
                }),
                &[7, 7, 3]
            );
        }
        assert_eq!(fills, 2);
        assert_eq!(cache.retained, 3);
        for _ in 0..100 {
            assert!(
                cache
                    .get_or_fill(4, &mut scratch, |_| fills += 1)
                    .is_empty()
            );
        }
        assert_eq!(fills, 4);
        assert_eq!(cache.retained, 3);
        assert_eq!(cache.rows.len(), 2);
    }
    #[test]
    fn one_offs_admit_nothing_and_churn_bounds_boxed_storage_and_row_count() {
        let mut cache = EligibilityCache::for_opening(64, 100).unwrap();
        let mut scratch = Vec::with_capacity(64);
        for left in 0..100 {
            cache.get_or_fill(left, &mut scratch, |s| s.extend(0..64));
        }
        assert_eq!(cache.retained, 0);
        assert!(cache.rows.is_empty());
        for left in 100..200 {
            for _ in 0..2 {
                cache.get_or_fill(left, &mut scratch, |s| s.extend(0..64));
                assert!(cache.retained <= 128);
                assert!(cache.rows.len() <= 8);
                assert!(cache.recent.len() <= 8);
                assert_eq!(
                    cache.retained,
                    cache.rows.iter().map(|r| r.choices.len()).sum::<usize>()
                );
                assert_eq!(scratch.capacity(), 64);
            }
        }
        assert_eq!(cache.rows.len(), 2);
        for left in 200..300 {
            for _ in 0..2 {
                cache.get_or_fill(left, &mut scratch, |_| {});
            }
        }
        assert_eq!(cache.rows.len(), 8);
        assert_eq!(cache.retained, 0);
        assert!(EligibilityCache::for_opening(63, 100).is_none());
        assert!(EligibilityCache::for_opening(100, 3).is_none());
    }
}
