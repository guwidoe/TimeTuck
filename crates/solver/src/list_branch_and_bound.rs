//! Bounded search over ordered list assignments.
use crate::branch_and_bound::{Retained, search_tree};
use crate::branch_tree::ListTree;
use crate::*;

/// Bounded-search counters and the best retained ordered-list assignment.
pub type ListBranchAndBoundResult<V, S> = BoundedSearchResult<ScoredListAssignment<V, S>>;
impl<V, S> Retained<ListAssignment<V>, S> for ScoredListAssignment<V, S> {
    fn new(assignment: ListAssignment<V>, score: S) -> Self {
        Self { assignment, score }
    }
    fn score(&self) -> &S {
        &self.score
    }
}

/// Search ordered lists with admissible, caller-supplied bounds.
///
/// Frozen lists and pinned prefixes stay fixed; other initial values are cleared.
/// Each remaining registry value branches over permitted entity/index insertions,
/// followed by leaving it unassigned when optional. Every complete assignment has
/// exactly one path. Exploration chooses the next partial parent, not the order of
/// its children. The callback's remaining indices refer to `problem.values()`;
/// decided optional omissions are excluded. Partial list positions and neighbors
/// can still change when later values are inserted. Bounds must account for this.
///
/// The shared [`branch_and_bound`] contract governs eligibility, bounds, ties,
/// counters, limits and panic behavior. Owned partial assignments are queued;
/// memory is bounded by the frontier limit, not by depth. Empty/impossible spaces
/// have the same meaning as [`exhaustive_list_search`]. There is no inferred bound.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![10, 20, 30], vec![ListEntity::new(vec![])])?;
/// let r = branch_and_bound_lists(&p, |a, remaining| {
///     let score = SimpleScore::new(-(a.lists()[0].len() as i64));
///     BranchEvaluation::bounded(score, SimpleScore::new(-3))
/// }, BranchAndBoundConfig::default())?;
/// assert!(r.is_optimal());
/// assert_eq!(r.best.unwrap().score, SimpleScore::new(-3));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn branch_and_bound_lists<V: Eq + Clone, S: Score>(
    problem: &ListProblem<V>,
    evaluate: impl FnMut(&ListAssignment<V>, &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
) -> Result<ListBranchAndBoundResult<V, S>, InvalidOptimisticBound<S>> {
    branch_and_bound_lists_with_observer(problem, evaluate, config, |_| {})
}
/// Notify synchronously after each strict improvement, as for basic bounded search.
pub fn branch_and_bound_lists_with_observer<V: Eq + Clone, S: Score>(
    problem: &ListProblem<V>,
    mut evaluate: impl FnMut(&ListAssignment<V>, &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
    observer: impl FnMut(&ScoredListAssignment<V, S>),
) -> Result<ListBranchAndBoundResult<V, S>, InvalidOptimisticBound<S>> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    let time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let (tree, root) = ListTree::new(problem);
    search_tree(
        &tree,
        root,
        |a, depth| evaluate(a, &tree.remaining()[depth..]),
        config,
        observer,
        time,
    )
}
