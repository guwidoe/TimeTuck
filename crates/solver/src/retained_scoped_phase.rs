//! Retained scoped director phase adapters.
pub(crate) struct ScopedSnapshot<A, S> {
    pub(crate) assignment: A,
    pub(crate) contributions: Vec<S>,
    pub(crate) score: S,
}
/// Exact traversals may move through many non-best candidates before the next
/// improvement. Keep the union since the last snapshot, bounded by cache size.
pub(crate) struct ScopedChanges {
    marked: Vec<bool>,
    indices: Vec<usize>,
    all: bool,
}
impl ScopedChanges {
    pub(crate) fn new(size: usize) -> Self {
        Self {
            marked: vec![false; size],
            indices: Vec::with_capacity(size),
            all: false,
        }
    }
    pub(crate) fn record_batch(&mut self, indices: impl ExactSizeIterator<Item = usize>) {
        // Dense changes make sparse bookkeeping more expensive than a Vec clone.
        // A full-dirty marker also dominates any earlier accumulated sparse changes.
        if indices.len() >= self.marked.len().div_ceil(2) {
            self.all = true;
        } else if !self.all {
            for index in indices {
                self.record(index);
            }
        }
    }
    fn record(&mut self, index: usize) {
        if !self.marked[index] {
            self.marked[index] = true;
            self.indices.push(index);
        }
    }
    fn clear(&mut self) {
        for index in self.indices.drain(..) {
            self.marked[index] = false;
        }
        self.all = false;
    }
}
impl<A: Clone, S: Clone> ScopedSnapshot<A, S> {
    pub(crate) fn refresh(
        &mut self,
        assignment: &A,
        contributions: &[S],
        score: &S,
        changes: &mut ScopedChanges,
    ) {
        // Stage every user Clone before changing either the snapshot or journal.
        // A panic must preserve the prior coherent best for the caller's unwind.
        let assignment = assignment.clone();
        let score = score.clone();
        // Dense transitions retain Vec's efficient full-clone path instead of
        // allocating indexed pairs only to replace every contribution anyway.
        if changes.all || changes.indices.len() == contributions.len() {
            let contributions = contributions.to_vec();
            self.assignment = assignment;
            self.score = score;
            self.contributions = contributions;
            changes.clear();
            return;
        }
        let updates: Vec<_> = changes
            .indices
            .iter()
            .map(|&i| (i, contributions[i].clone()))
            .collect();
        self.assignment = assignment;
        self.score = score;
        for (i, value) in updates {
            self.contributions[i] = value;
        }
        changes.clear();
    }
}
macro_rules! retained_scoped {
    ($director:ident, [$($v:ident),+], $assignment:ty, $problem:ty, $movement:ty,
     $error:ty, $local_error:ty, $sequence:ident, $solver:ident,
     $construction:ident, $construction_config:ty, $construction_result:ty,
     $search:ident, $search_config:ty, $search_result:ty, $provider_constructor:ident,
     $initial:expr, [$($clock_field:ident)?]) => {
impl<'p, 'n, $($v: Eq + Clone,)+ S: crate::AdditiveScore> $director<'p, 'n, $($v,)+ S>
{
    /// Run a plan on this initialized scoped contribution cache. Earlier setup is
    /// outside this call's work. Cached phase entry uses one logical evaluation
    /// and no scoring callbacks; best snapshots restore without rescoring.
    ///
    /// Constraints must depend only on their declared scopes and immutable facts.
    /// Ordinary handoffs preserve the scoring payloads of the working solution.
    /// Sparse exact best snapshots clone the union of contributions changed since the
    /// previous best; assignment and total-score snapshots remain independent.
    /// Tracking uses memory bounded by the number of registered constraints.
    /// Every exact phase fully refreshes its first candidate before caching deltas
    /// among that traversal's fixed value representatives. Equality may therefore
    /// describe planning identity without treating external payloads as score-equal.
    /// Repair rebuild boundaries also refresh all contributions: rejected repairs
    /// may change payload representatives even when their planning IDs compare equal.
    /// Successful calls retain the best solution. Errors/panics leave the last
    /// committed state; an unsuccessful scoring transition never installs a cache.
    pub fn run_solver<'c>(&mut self, config: impl Into<crate::$sequence<'c, S>>)
        -> Result<crate::SolverResult<$assignment, S>, crate::SolverError<$local_error>>
    where S: 'c,
    {
        self.run_solver_with_extensions(config, Default::default())
    }
    /// Retained plans with providers, commands, lifecycle/events, metrics and
    /// independent score diagnostics, using the same controls as ordinary solves.
    pub fn run_solver_with_extensions<'c>(
        &mut self,
        config: impl Into<crate::$sequence<'c, S>>,
        extensions: crate::SolverExtensions<'_, $problem, $assignment, $movement, S, $error>,
    ) -> Result<crate::SolverResult<$assignment, S>, crate::SolverError<$local_error>>
    where S: 'c,
    {
        crate::solver::scoped_retained::$solver(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn capture_retained(&self) -> crate::retained_scoped_phase::ScopedSnapshot<$assignment, S> {
        crate::retained_scoped_phase::ScopedSnapshot {
            assignment: self.assignment.clone(), contributions: self.contributions.clone(), score: self.score.clone(),
        }
    }
    pub(crate) fn retained_changes(&self) -> crate::retained_scoped_phase::ScopedChanges {
        crate::retained_scoped_phase::ScopedChanges::new(self.contributions.len())
    }
    pub(crate) fn refresh_retained(
        &self,
        snapshot: &mut crate::retained_scoped_phase::ScopedSnapshot<$assignment, S>,
        changes: &mut crate::retained_scoped_phase::ScopedChanges,
    ) {
        snapshot.refresh(&self.assignment, &self.contributions, &self.score, changes);
    }
    pub(crate) fn install_retained_state(&mut self, snapshot: crate::retained_scoped_phase::ScopedSnapshot<$assignment, S>) {
        self.assignment = snapshot.assignment;
        self.contributions = snapshot.contributions;
        self.score = snapshot.score;
    }
    pub(crate) fn prepare_retained_phase(&mut self, initial: $assignment) -> Result<u128, $error> {
        // Only engine-produced handoffs from this same session reach this method.
        // Unlike the first exact candidate, they retain its payload representatives.
        if self.assignment == initial { return Ok(0); }
        self.replace_retained_assignment(initial, false)
    }
    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, S>,
        config: $construction_config,
    ) -> Result<$construction_result, $error> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment().clone());
        let observed = observer.is_some();
        let mut notify = |director: &Self, complete| {
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(),
                    score: director.score.clone(), complete,
                });
            }
        };
        crate::$construction::construction_impl_monitored(shared, problem, initial, config, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: self, initialization_work, on_best: observed.then_some(&mut notify),
            })
        })
    }
    pub(crate) fn run_local_search_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, S>,
        config: $search_config,
        provider: Option<&mut (dyn crate::MoveProvider<$problem, $assignment, $movement> + '_)>,
    ) -> Result<$search_result, $local_error> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment().clone());
        let mut best = None;
        let mut save = |director: &Self, complete| {
            best = Some(director.capture_retained());
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(), score: director.score.clone(), complete,
                });
            }
        };
        let clock = crate::provider_search::clock(&config$(.$clock_field)?);
        let result = crate::$search::search_impl_with_provider(shared, problem, ($initial)(initial), config, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: &mut *self, initialization_work, on_best: Some(&mut save),
            })
        }, clock, provider.map(|p| p as &mut dyn crate::MoveProvider<$problem, $assignment, $movement>))?;
        if let Some(best) = best { self.install_retained_state(best); }
        Ok(result)
    }
    pub(crate) fn run_provider_construction_monitored(
        &mut self, initial: $assignment, config: crate::ProviderConstructionConfig<'_>,
        context: crate::provider_search::ProviderContext<'_, $problem, $assignment, $movement, S>,
    ) -> Result<crate::ProviderConstructionResult<$assignment, S>, $error> {
        crate::provider_construction::$provider_constructor(self.problem, initial, config, context, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: self, initialization_work, on_best: None,
            })
        })
    }
}
    };
}
pub(crate) use retained_scoped;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    #[derive(Debug)]
    struct Bomb(i64, Rc<Cell<usize>>);
    impl Clone for Bomb {
        fn clone(&self) -> Self {
            let remaining = self.1.get();
            self.1.set(remaining.saturating_sub(1));
            assert_ne!(remaining, 1, "snapshot clone failure");
            Self(self.0, self.1.clone())
        }
    }
    #[test]
    fn sparse_snapshot_clone_panics_preserve_all_best_fields_and_dirty_union() {
        for indices in [vec![0, 2], vec![0, 1, 2]] {
            for at in 1..=2 + indices.len() {
                let fail = Rc::new(Cell::new(0));
                let b = |n| Bomb(n, fail.clone());
                let mut snapshot = ScopedSnapshot {
                    assignment: b(1),
                    contributions: vec![b(2), b(3), b(4)],
                    score: b(9),
                };
                let mut changes = ScopedChanges::new(3);
                for &i in &indices {
                    changes.record(i);
                    changes.record(i);
                }
                fail.set(at);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| snapshot.refresh(
                        &b(5),
                        &[b(6), b(3), b(7)],
                        &b(16),
                        &mut changes
                    )))
                    .is_err()
                );
                assert_eq!(snapshot.assignment.0, 1);
                assert_eq!(snapshot.score.0, 9);
                assert_eq!(
                    snapshot
                        .contributions
                        .iter()
                        .map(|s| s.0)
                        .collect::<Vec<_>>(),
                    [2, 3, 4]
                );
                assert_eq!(changes.indices, indices);
                assert_eq!(changes.marked, [true, indices.len() == 3, true]);
                fail.set(0);
                snapshot.refresh(&b(5), &[b(6), b(3), b(7)], &b(16), &mut changes);
                assert_eq!(snapshot.assignment.0, 5);
                assert_eq!(snapshot.score.0, 16);
                assert_eq!(
                    snapshot
                        .contributions
                        .iter()
                        .map(|s| s.0)
                        .collect::<Vec<_>>(),
                    [6, 3, 7]
                );
                assert!(changes.indices.is_empty());
                assert_eq!(changes.marked, [false; 3]);
            }
        }
    }
    #[test]
    fn dense_dirty_marker_preserves_sparse_history_until_snapshot_commit() {
        let fail = Rc::new(Cell::new(0));
        let b = |n| Bomb(n, fail.clone());
        let mut changes = ScopedChanges::new(3);
        changes.record_batch([0].into_iter());
        changes.record_batch([1, 2].into_iter());
        assert!(changes.all);
        assert_eq!(changes.indices, [0]);
        let mut snapshot = ScopedSnapshot {
            assignment: b(1),
            contributions: vec![b(2), b(3), b(4)],
            score: b(9),
        };
        fail.set(3);
        assert!(
            catch_unwind(AssertUnwindSafe(|| snapshot.refresh(
                &b(5),
                &[b(6), b(7), b(8)],
                &b(21),
                &mut changes
            )))
            .is_err()
        );
        assert!(changes.all);
        assert_eq!(changes.indices, [0]);
        assert_eq!(snapshot.assignment.0, 1);
        assert_eq!(snapshot.score.0, 9);
        assert_eq!(
            snapshot
                .contributions
                .iter()
                .map(|s| s.0)
                .collect::<Vec<_>>(),
            [2, 3, 4]
        );
        fail.set(0);
        snapshot.refresh(&b(5), &[b(6), b(7), b(8)], &b(21), &mut changes);
        assert!(!changes.all);
        assert_eq!(changes.marked, [false; 3]);
        assert_eq!(
            snapshot
                .contributions
                .iter()
                .map(|s| s.0)
                .collect::<Vec<_>>(),
            [6, 7, 8]
        );
        changes.record_batch([0].into_iter());
        assert_eq!(changes.indices, [0]);
    }
}
