//! Bounded search over a coupled basic/list objective.
use crate::branch_and_bound::{Retained, search_tree};
use crate::branch_tree::MixedTree;
use crate::*;

/// Bounded-search counters and the best retained joint assignment.
pub type MixedBranchAndBoundResult<B, L, S> = BoundedSearchResult<ScoredMixedAssignment<B, L, S>>;
impl<B, L, S> Retained<MixedAssignment<B, L>, S> for ScoredMixedAssignment<B, L, S> {
    fn new(assignment: MixedAssignment<B, L>, score: S) -> Self {
        Self { assignment, score }
    }
    fn score(&self) -> &S {
        &self.score
    }
}

/// Maximize a coupled objective over lists and basic variables with supplied bounds.
///
/// The tree decides remaining list values first, then movable basic variables.
/// Pins, ranges, initial-value clearing and optional choices follow each component's
/// bounded-search contract. Callback arguments are `(assignment, basic_remaining,
/// list_remaining)`; indices refer to the basic variables and list value registry.
/// A leaf has BOTH slices empty; model completeness alone cannot identify a leaf.
/// An impossible component makes the whole space empty without scoring.
///
/// Each bound must dominate every eligible completion of BOTH components together.
/// Independently optimizing components is insufficient for coupled objectives.
/// The shared [`branch_and_bound`] contract governs ordering, limits, eligibility,
/// counters, observations and bound validation. Queue entries own full joint states.
/// No incremental scoring, automatic bounds or Java traversal parity is implied.
pub fn branch_and_bound_mixed<B: Eq + Clone, L: Eq + Clone, S: Score>(
    problem: &MixedProblem<B, L>,
    evaluate: impl FnMut(&MixedAssignment<B, L>, &[usize], &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
) -> Result<MixedBranchAndBoundResult<B, L, S>, InvalidOptimisticBound<S>> {
    branch_and_bound_mixed_with_observer(problem, evaluate, config, |_| {})
}
/// Notify synchronously after each strict improvement, as for basic bounded search.
pub fn branch_and_bound_mixed_with_observer<B: Eq + Clone, L: Eq + Clone, S: Score>(
    problem: &MixedProblem<B, L>,
    mut evaluate: impl FnMut(&MixedAssignment<B, L>, &[usize], &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
    observer: impl FnMut(&ScoredMixedAssignment<B, L, S>),
) -> Result<MixedBranchAndBoundResult<B, L, S>, InvalidOptimisticBound<S>> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    let time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let (tree, root) = MixedTree::new(problem);
    search_tree(
        &tree,
        root,
        |a, depth| {
            let (basic, lists) = tree.remaining(depth);
            evaluate(a, basic, lists)
        },
        config,
        observer,
        time,
    )
}
