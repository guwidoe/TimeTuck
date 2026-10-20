//! Phase-local, opt-in memoization of pure scoped candidate contributions.

struct Contribution<S> {
    generation: u64,
    score: S,
}

struct RowMemo<S> {
    base: S,
    result: S,
}

struct Row<S> {
    slots: Vec<Option<Contribution<S>>>,
    // Allocate score payloads only for rows actually remembering a result.
    // Generic cached scores otherwise pay just one optional pointer per row.
    memo: Option<Box<RowMemo<S>>>,
}

struct VariableRows {
    offset: usize,
    cached_occurrences: usize,
    active: Vec<(usize, usize)>,
}

/// Rows are indexed by exact variable/domain occurrence, not value equality.
/// Each admitted row reserves one slot per dependent constraint, including
/// disabled contributions. Index ranges are planned in variable order within the
/// slot cap; occurrences outside those ranges use ordinary evaluation.
/// Empty rows are never admitted, bounding row metadata
/// by the contribution-slot cap as well. Score payload sizes remain user-defined.
pub(crate) struct PooledContributionCache<S> {
    max_contributions: usize,
    admitted: usize,
    generations: Vec<u64>,
    variables: Vec<VariableRows>,
    rows: Vec<Option<Row<S>>>,
}

impl<S> PooledContributionCache<S> {
    pub(crate) fn new<'a>(
        max_contributions: usize,
        constraint_count: usize,
        variables: impl IntoIterator<Item = (usize, &'a [usize])>,
        mut is_active: impl FnMut(usize) -> bool,
    ) -> Self {
        let mut planned_slots = 0;
        let mut rows = Vec::new();
        let variables = variables
            .into_iter()
            .map(|(domain_len, dependents)| {
                let dependent_count = dependents.len();
                let cached_occurrences = if dependent_count == 0 {
                    0
                } else {
                    domain_len.min((max_contributions - planned_slots) / dependent_count)
                };
                let offset = rows.len();
                // Both the index table and planned contribution slots are bounded
                // by the cap; no total-domain product is allocated or computed.
                planned_slots += cached_occurrences * dependent_count;
                rows.resize_with(offset + cached_occurrences, || None);
                // Every indexed variable reserves at least one whole row, so
                // total immutable list entries cannot exceed planned_slots.
                let active = if cached_occurrences == 0 {
                    Vec::new()
                } else {
                    dependents
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|&(_, constraint)| is_active(constraint))
                        .collect()
                };
                VariableRows {
                    offset,
                    cached_occurrences,
                    active,
                }
            })
            .collect();
        Self {
            max_contributions,
            admitted: 0,
            generations: vec![0; constraint_count],
            variables,
            rows,
        }
    }

    pub(crate) fn constraint_count(&self) -> usize {
        self.generations.len()
    }

    pub(crate) fn clear(&mut self) {
        for row in &mut self.rows {
            *row = None;
        }
        self.admitted = 0;
        self.generations.fill(0);
    }

    pub(crate) fn is_indexed(&self, variable: usize, occurrence: usize) -> bool {
        self.variables
            .get(variable)
            .is_some_and(|range| occurrence < range.cached_occurrences)
    }

    pub(crate) fn candidate(
        &mut self,
        variable: usize,
        occurrence: usize,
        dependent_count: usize,
    ) -> CandidateContributions<'_, S> {
        let row = match self.variables.get(variable) {
            Some(range) if occurrence < range.cached_occurrences => {
                self.rows.get_mut(range.offset + occurrence)
            }
            _ => None,
        };
        let row = row.and_then(|row| {
            if dependent_count == 0 {
                return None;
            }
            if row.is_none() {
                if dependent_count > self.max_contributions - self.admitted {
                    return None;
                }
                let mut slots = Vec::with_capacity(dependent_count);
                slots.resize_with(dependent_count, || None);
                self.admitted += dependent_count;
                *row = Some(Row { slots, memo: None });
            }
            row.as_mut()
                .filter(|row| row.slots.len() == dependent_count)
        });
        let (slots, memo) = match row {
            Some(row) => (Some(row.slots.as_mut_slice()), Some(&mut row.memo)),
            None => (None, None),
        };
        CandidateContributions {
            generations: &self.generations,
            slots,
            memo,
            active: self
                .variables
                .get(variable)
                .map_or(&[], |range| range.active.as_slice()),
        }
    }

    /// Invalidate inputs, not just changed scores. An edge's base penalty may
    /// stay zero when one endpoint becomes assigned, while a future trial changes.
    pub(crate) fn invalidate(&mut self, constraints: &[usize]) {
        for &index in constraints {
            let Some(next) = self.generations[index].checked_add(1) else {
                // Entries are discarded before tags can repeat (no ABA).
                self.clear();
                return;
            };
            self.generations[index] = next;
        }
    }

    pub(crate) fn pass(&mut self) -> CachePass<'_, S> {
        CachePass {
            cache: self,
            successful: false,
        }
    }
}

pub(crate) struct CandidateContributions<'a, S> {
    pub(crate) active: &'a [(usize, usize)],
    generations: &'a [u64],
    slots: Option<&'a mut [Option<Contribution<S>>]>,
    memo: Option<&'a mut Option<Box<RowMemo<S>>>>,
}

impl<S> CandidateContributions<'_, S> {
    fn active_is_current(&self) -> bool {
        self.slots.as_ref().is_some_and(|slots| {
            self.active.iter().all(|&(slot, constraint)| {
                slots
                    .get(slot)
                    .and_then(Option::as_ref)
                    .is_some_and(|entry| entry.generation == self.generations[constraint])
            })
        })
    }
}

impl<S: Clone> CandidateContributions<'_, S> {
    pub(crate) fn get(&self, slot: usize, constraint: usize) -> Option<S> {
        self.slots
            .as_ref()?
            .get(slot)?
            .as_ref()
            .filter(|entry| entry.generation == self.generations[constraint])
            .map(|entry| entry.score.clone())
    }

    pub(crate) fn put(&mut self, slot: usize, constraint: usize, score: &S) {
        // A refresh can make every generation current again. It must not revive
        // a result computed before the invalidation, even if the base is equal.
        // Clear before cloning, including when that Clone subsequently panics.
        if let Some(memo) = &mut self.memo {
            **memo = None;
        }
        if let Some(slots) = &mut self.slots {
            slots[slot] = Some(Contribution {
                generation: self.generations[constraint],
                score: score.clone(),
            });
        }
    }

    /// Own every active contribution in its original index order, without
    /// evaluating callbacks. Only a completely current row can be materialized.
    pub(crate) fn materialize(&self) -> Vec<(usize, S)> {
        assert!(self.active_is_current(), "current complete pooled row");
        self.active
            .iter()
            .map(|&(slot, constraint)| {
                (
                    constraint,
                    self.get(slot, constraint)
                        .expect("current pooled contribution"),
                )
            })
            .collect()
    }
}

impl<S: crate::AdditiveScore> CandidateContributions<'_, S> {
    /// The caller supplies a normalized ordinary base and has already applied
    /// the real assignment transaction. Fresh structural state is not memoized.
    pub(crate) fn memo_score(&self, base: &S) -> Option<S> {
        if !S::POOLED_SAME_CONTEXT_MEMO {
            return None;
        }
        let memo = self.memo.as_ref()?.as_deref()?;
        if !self.active_is_current() || memo.base != *base {
            return None;
        }
        Some(memo.result.clone())
    }

    /// Remember only successful ordinary adjustment after the complete row has
    /// settled. Errors/unwinding remain covered by the enclosing CachePass.
    pub(crate) fn remember_score(&mut self, base: S, result: S) {
        if !S::POOLED_SAME_CONTEXT_MEMO || !self.active_is_current() {
            return;
        }
        if let Some(memo) = &mut self.memo {
            let saved = RowMemo {
                base: base.with_structural(0),
                result: result.with_structural(0),
            };
            if let Some(existing) = memo.as_deref_mut() {
                *existing = saved;
            } else {
                **memo = Some(Box::new(saved));
            }
        }
    }
}

/// Covers evaluation, diagnostics, acceptance, foraging and winner installation.
/// Partial refreshes are not trusted after a typed error or ordinary unwind.
pub(crate) struct CachePass<'a, S> {
    cache: &'a mut PooledContributionCache<S>,
    successful: bool,
}
impl<S> CachePass<'_, S> {
    pub(crate) fn cache(&mut self) -> &mut PooledContributionCache<S> {
        self.cache
    }
    pub(crate) fn finish(mut self) {
        self.successful = true;
    }
}
impl<S> Drop for CachePass<'_, S> {
    fn drop(&mut self) {
        if !self.successful {
            self.cache.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdditiveScore, HardSoftScore};

    #[test]
    fn ordinary_memos_require_exact_base_and_complete_active_rows() {
        let mut cache = PooledContributionCache::new(6, 3, [(2, &[0, 1, 2][..])], |i| i != 1);
        let base = HardSoftScore::new(5, -8);
        let result = HardSoftScore::new(7, -9);
        {
            let mut row = cache.candidate(0, 0, 3);
            row.put(0, 0, &HardSoftScore::ZERO);
            row.remember_score(base, result);
            assert_eq!(row.memo_score(&base), None, "partial rows have no memo");
            row.put(2, 2, &HardSoftScore::new(2, -1));
            row.remember_score(base.with_structural(-3), result.with_structural(-2));
            assert_eq!(row.memo_score(&base), Some(result));
            assert_eq!(row.memo_score(&HardSoftScore::new(5, -7)), None);
            assert_eq!(
                row.memo_score(&base.with_structural(-1)),
                None,
                "lookup key must be normalized"
            );
            assert_eq!(
                row.materialize(),
                [(0, HardSoftScore::ZERO), (2, HardSoftScore::new(2, -1))]
            );
        }
        // Equal domain payloads do not merge distinct occurrence rows.
        assert_eq!(cache.candidate(0, 1, 3).memo_score(&base), None);
        // Disabled constraints do not invalidate the active-row certificate.
        cache.invalidate(&[1]);
        assert_eq!(cache.candidate(0, 0, 3).memo_score(&base), Some(result));
    }

    #[test]
    fn refreshed_generations_and_equal_value_puts_cannot_revive_old_memos() {
        let mut cache = PooledContributionCache::new(2, 2, [(1, &[0, 1][..])], |_| true);
        let base = HardSoftScore::ZERO;
        let old = HardSoftScore::new(1, 2);
        {
            let mut row = cache.candidate(0, 0, 2);
            row.put(0, 0, &old);
            row.put(1, 1, &HardSoftScore::ZERO);
            row.remember_score(base, old);
        }
        cache.invalidate(&[0, 1]);
        {
            let mut row = cache.candidate(0, 0, 2);
            assert_eq!(row.memo_score(&base), None);
            row.put(0, 0, &old);
            assert_eq!(row.memo_score(&base), None, "second entry remains dirty");
            row.put(1, 1, &HardSoftScore::ZERO);
            assert_eq!(row.materialize(), [(0, old), (1, HardSoftScore::ZERO)]);
            assert_eq!(
                row.memo_score(&base),
                None,
                "all tags are current but old memo was discarded"
            );
            row.remember_score(base, old);
            assert_eq!(row.memo_score(&base), Some(old));
            // Even a current, equal-valued put explicitly discards its old result.
            row.put(1, 1, &HardSoftScore::ZERO);
            assert_eq!(row.memo_score(&base), None);
        }
    }

    #[test]
    fn clear_generation_wrap_and_unfinished_pass_discard_memos_before_tag_reuse() {
        let mut cache = PooledContributionCache::new(1, 1, [(1, &[0][..])], |_| true);
        let score = HardSoftScore::new(1, 0);
        for unwind in [false, true] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut pass = cache.pass();
                let mut row = pass.cache().candidate(0, 0, 1);
                row.put(0, 0, &score);
                row.remember_score(HardSoftScore::ZERO, score);
                assert_eq!(row.memo_score(&HardSoftScore::ZERO), Some(score));
                assert!(!unwind, "injected failure after successful memo creation");
                // A typed-error path also drops this unfinished pass.
            }));
            assert_eq!(result.is_err(), unwind);
            let mut row = cache.candidate(0, 0, 1);
            row.put(0, 0, &score);
            assert_eq!(row.memo_score(&HardSoftScore::ZERO), None);
        }
        cache.generations[0] = u64::MAX;
        {
            let mut row = cache.candidate(0, 0, 1);
            row.put(0, 0, &score);
            row.remember_score(HardSoftScore::ZERO, score);
        }
        cache.invalidate(&[0]);
        assert_eq!(cache.generations, [0]);
        assert_eq!(cache.admitted, 0);
        assert!(cache.rows.iter().all(Option::is_none));
        let mut pass = cache.pass();
        {
            let mut row = pass.cache().candidate(0, 0, 1);
            row.put(0, 0, &score);
            assert_eq!(row.memo_score(&HardSoftScore::ZERO), None);
            row.remember_score(HardSoftScore::ZERO, score);
        }
        pass.finish();
        assert_eq!(
            cache.candidate(0, 0, 1).memo_score(&HardSoftScore::ZERO),
            Some(score)
        );
        cache.clear();
        let mut row = cache.candidate(0, 0, 1);
        row.put(0, 0, &score);
        assert_eq!(row.memo_score(&HardSoftScore::ZERO), None);
    }

    #[test]
    fn unadmitted_and_incomplete_materializations_fail_without_publishing_partial_updates() {
        let mut cache = PooledContributionCache::new(1, 1, [(2, &[0][..])], |_| true);
        let base = HardSoftScore::ZERO;
        {
            let mut row = cache.candidate(0, 1, 1);
            row.remember_score(base, base);
            assert_eq!(row.memo_score(&base), None);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| row.materialize()))
                    .is_err()
            );
        }
        let mut row = cache.candidate(0, 0, 1);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| row.materialize())).is_err()
        );
        row.put(0, 0, &base);
        assert_eq!(row.materialize(), [(0, base)]);
    }

    #[test]
    fn custom_score_default_preserves_normalization_and_adjustment_hooks() {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
        struct Custom(i64);
        impl crate::Score for Custom {
            type Level = i64;
            const HARD_LEVEL_COUNT: usize = 0;
            fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
                std::iter::once(self.0)
            }
            fn structural(&self) -> i64 {
                0
            }
            fn hard_cmp(&self, _: &Self) -> std::cmp::Ordering {
                std::cmp::Ordering::Equal
            }
            fn is_feasible(&self) -> bool {
                true
            }
        }
        impl std::ops::Add for Custom {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }
        impl std::ops::Sub for Custom {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }
        impl AdditiveScore for Custom {
            const ZERO: Self = Self(0);
            fn with_structural(self, _: i64) -> Self {
                panic!("memo must not normalize default-false scores")
            }
            fn checked_adjust<'a>(
                &self,
                _: impl IntoIterator<Item = (&'a Self, &'a Self)>,
            ) -> Option<Self>
            where
                Self: 'a,
            {
                panic!("memo helper must not call custom adjustment")
            }
        }
        let mut cache = PooledContributionCache::new(1, 1, [(1, &[0][..])], |_| true);
        let mut row = cache.candidate(0, 0, 1);
        row.put(0, 0, &Custom(1));
        row.remember_score(Custom(0), Custom(1));
        assert_eq!(row.memo_score(&Custom(0)), None);
        assert_eq!(row.materialize(), [(0, Custom(1))]);
        assert!(row.memo.as_ref().unwrap().is_none());
    }

    #[test]
    fn slots_are_bounded_and_input_generations_cannot_wrap_into_old_entries() {
        let mut cache = PooledContributionCache::new(2, 3, [(2, &[0][..])], |_| true);
        {
            let mut row = cache.candidate(0, 0, 2);
            row.put(0, 0, &10);
            row.put(1, 1, &20);
        }
        assert_eq!(cache.candidate(0, 1, 1).get(0, 0), None);
        assert_eq!(cache.admitted, 2);
        cache.invalidate(&[1]);
        let row = cache.candidate(0, 0, 2);
        assert_eq!(row.get(0, 0), Some(10));
        assert_eq!(row.get(1, 1), None);
        cache.generations[0] = u64::MAX;
        cache.invalidate(&[0]);
        assert_eq!(cache.admitted, 0);
        assert!(cache.rows.iter().all(Option::is_none));
        assert_eq!(cache.generations, vec![0; 3]);
    }

    #[test]
    fn occurrence_table_skips_oversized_and_empty_dependencies_with_bounded_indices() {
        let mut cache = PooledContributionCache::new(
            3,
            5,
            [(usize::MAX, &[0, 1, 2, 3][..]), (9, &[][..]), (9, &[0][..])],
            |_| true,
        );
        assert_eq!(cache.rows.len(), 3);
        assert_eq!(cache.variables[0].cached_occurrences, 0);
        assert_eq!(cache.variables[1].cached_occurrences, 0);
        assert!(cache.candidate(0, 0, 4).slots.is_none());
        assert!(cache.candidate(1, 0, 0).slots.is_none());
        assert!(cache.candidate(2, 3, 1).slots.is_none());
        assert!(cache.candidate(3, 0, 1).slots.is_none());
        for occurrence in 0..3 {
            cache.candidate(2, occurrence, 1).put(0, 0, &occurrence);
        }
        assert_eq!(cache.admitted, 3);
        cache.clear();
        assert_eq!(cache.rows.len(), 3);
        assert!(cache.rows.iter().all(Option::is_none));
        cache.candidate(2, 2, 1).put(0, 0, &7);
        assert_eq!(cache.candidate(2, 2, 1).get(0, 0), Some(7));
    }

    #[test]
    fn active_plan_is_bounded_filters_once_and_survives_row_clear() {
        let mut compared = Vec::new();
        let mut cache = PooledContributionCache::<i64>::new(
            6,
            5,
            [(2, &[0, 1, 2][..]), (10, &[3, 4][..]), (100, &[][..])],
            |constraint| {
                compared.push(constraint);
                constraint != 1
            },
        );
        assert_eq!(compared, [0, 1, 2]);
        assert_eq!(
            cache
                .variables
                .iter()
                .map(|v| v.active.len())
                .sum::<usize>(),
            2
        );
        assert!(cache.is_indexed(0, 1));
        assert!(!cache.is_indexed(1, 0));
        assert_eq!(cache.candidate(0, 0, 3).active, &[(0, 0), (2, 2)]);
        cache.candidate(0, 0, 3).put(0, 0, &7);
        cache.clear();
        let row = cache.candidate(0, 1, 3);
        assert_eq!(row.active, &[(0, 0), (2, 2)]);
        assert_eq!(row.get(0, 0), None);
        assert_eq!(compared, [0, 1, 2]);
    }

    #[test]
    fn failed_or_unwound_pass_discards_all_partially_refreshed_entries() {
        let mut cache = PooledContributionCache::new(4, 1, [(1, &[0][..])], |_| true);
        for panic in [false, true] {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut pass = cache.pass();
                pass.cache().candidate(0, 0, 1).put(0, 0, &1);
                assert!(!panic, "injected later failure");
                // A typed-error return also drops an unfinished pass.
            }));
            assert_eq!(result.is_err(), panic);
            assert_eq!(cache.admitted, 0);
            assert!(cache.rows.iter().all(Option::is_none));
        }
        let mut pass = cache.pass();
        pass.cache().candidate(0, 0, 1).put(0, 0, &2);
        pass.finish();
        assert_eq!(cache.candidate(0, 0, 1).get(0, 0), Some(2));
    }

    #[test]
    fn score_clone_panic_invalidates_admitted_rows_without_trusting_pending_entries() {
        use std::{cell::Cell, rc::Rc};
        struct Score(Rc<Cell<bool>>);
        impl Clone for Score {
            fn clone(&self) -> Self {
                assert!(!self.0.get(), "injected score clone panic");
                Self(self.0.clone())
            }
        }
        let panic = Rc::new(Cell::new(false));
        let score = Score(panic.clone());
        let mut cache = PooledContributionCache::new(4, 1, [(1, &[0][..])], |_| true);
        {
            let mut pass = cache.pass();
            pass.cache().candidate(0, 0, 1).put(0, 0, &score);
            pass.finish();
        }
        panic.set(true);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut pass = cache.pass();
                let _ = pass.cache().candidate(0, 0, 1).get(0, 0);
                pass.finish();
            }))
            .is_err()
        );
        assert_eq!(cache.admitted, 0);
        assert!(cache.rows.iter().all(Option::is_none));
        panic.set(false);
        let mut pass = cache.pass();
        pass.cache().candidate(0, 0, 1).put(0, 0, &score);
        pass.finish();
        assert!(cache.candidate(0, 0, 1).get(0, 0).is_some());
    }
}
