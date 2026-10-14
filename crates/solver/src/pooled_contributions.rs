//! Phase-local, opt-in memoization of pure scoped candidate contributions.

struct Contribution<S> {
    generation: u64,
    score: S,
}

type Row<S> = Vec<Option<Contribution<S>>>;

struct VariableRows {
    offset: usize,
    cached_occurrences: usize,
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
    pub(crate) fn new(
        max_contributions: usize,
        constraint_count: usize,
        variables: impl IntoIterator<Item = (usize, usize)>,
    ) -> Self {
        let mut planned_slots = 0;
        let mut rows = Vec::new();
        let variables = variables
            .into_iter()
            .map(|(domain_len, dependent_count)| {
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
                VariableRows {
                    offset,
                    cached_occurrences,
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
        let slots = row.and_then(|row| {
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
                *row = Some(slots);
            }
            row.as_mut()
                .filter(|slots| slots.len() == dependent_count)
                .map(Vec::as_mut_slice)
        });
        CandidateContributions {
            generations: &self.generations,
            slots,
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
    generations: &'a [u64],
    slots: Option<&'a mut [Option<Contribution<S>>]>,
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
        if let Some(slots) = &mut self.slots {
            slots[slot] = Some(Contribution {
                generation: self.generations[constraint],
                score: score.clone(),
            });
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

    #[test]
    fn slots_are_bounded_and_input_generations_cannot_wrap_into_old_entries() {
        let mut cache = PooledContributionCache::new(2, 3, [(2, 1)]);
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
        let mut cache = PooledContributionCache::new(3, 5, [(usize::MAX, 4), (9, 0), (9, 1)]);
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
    fn failed_or_unwound_pass_discards_all_partially_refreshed_entries() {
        let mut cache = PooledContributionCache::new(4, 1, [(1, 1)]);
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
        let mut cache = PooledContributionCache::new(4, 1, [(1, 1)]);
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
