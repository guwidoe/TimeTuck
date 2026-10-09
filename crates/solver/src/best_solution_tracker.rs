//! Initialization-aware best snapshots for candidate and solver-update consumers.
use std::cmp::Ordering;

/// Owned solution and its quality. `required_unassigned` counts missing required
/// values only; optional unassigned values must not contribute to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedSolution<A, S> {
    pub assignment: A,
    pub score: S,
    pub required_unassigned: usize,
}

/// Retains the first solution at the best quality seen so far. Fewer required
/// unassigned values take priority over a higher score. With equal counts, normal
/// score ordering applies, including any structural and feasibility levels.
///
/// Supply scores and counts from the same candidate snapshot. This tracker does
/// not score, validate or mutate the working solution. Use it from construction,
/// custom search, or a solver observer; use a fresh tracker after problem changes.
/// Snapshots are constructed only for strict improvements, before replacing the
/// retained state, so a failed snapshot construction leaves the old best intact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BestSolutionTracker<A, S> {
    best: RankedSolution<A, S>,
}
impl<A, S: Ord> BestSolutionTracker<A, S> {
    pub fn new(assignment: A, score: S, required_unassigned: usize) -> Self {
        Self {
            best: RankedSolution {
                assignment,
                score,
                required_unassigned,
            },
        }
    }
    pub fn best(&self) -> &RankedSolution<A, S> {
        &self.best
    }
    pub fn into_best(self) -> RankedSolution<A, S> {
        self.best
    }
    /// Compare without scoring or constructing a snapshot. Equal quality retains
    /// the earlier solution, making tie behavior deterministic.
    pub fn would_improve(&self, score: &S, required_unassigned: usize) -> bool {
        match required_unassigned.cmp(&self.best.required_unassigned) {
            Ordering::Less => true,
            Ordering::Equal => score > &self.best.score,
            Ordering::Greater => false,
        }
    }
    /// Consider an evaluated move or completed step. The snapshot factory is
    /// invoked exactly once for an improvement and never for a rejected candidate.
    /// The assignment need not implement Clone (for example, a saved transaction).
    pub fn consider(
        &mut self,
        score: S,
        required_unassigned: usize,
        snapshot: impl FnOnce() -> A,
    ) -> bool {
        if !self.would_improve(&score, required_unassigned) {
            return false;
        }
        let replacement = RankedSolution {
            assignment: snapshot(),
            score,
            required_unassigned,
        };
        self.best = replacement;
        true
    }
}
impl<A: Clone, S: Clone + Ord> BestSolutionTracker<A, S> {
    /// Borrowed alternative for observer callbacks. Neither the assignment nor
    /// its score is cloned when quality is unchanged or worse.
    pub fn consider_ref(&mut self, assignment: &A, score: &S, required_unassigned: usize) -> bool {
        if !self.would_improve(score, required_unassigned) {
            return false;
        }
        let replacement = RankedSolution {
            assignment: assignment.clone(),
            score: score.clone(),
            required_unassigned,
        };
        self.best = replacement;
        true
    }
}
