//! Exact tree search with caller-supplied optimistic bounds.
use crate::{
    Assignment, FiniteProblem, InitializingScoreBound, Score, ScoreLevel, ScoredAssignment,
};
use std::{cmp::Ordering, collections::BinaryHeap, error::Error, fmt, time::Duration};

/// Priority of open partial assignments. Greater comparison means explored first.
/// Ordinary score levels order partial scores; structural incompleteness is ignored
/// for scheduling only. Actual pruning and retained-best comparisons use full scores.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub enum NodeExploration {
    /// Prefer greater depth, then score, then optimistic bound.
    #[default]
    DepthFirst,
    /// Prefer smaller depth, then score, then optimistic bound.
    BreadthFirst,
    /// Prefer score, then optimistic bound, then greater depth.
    ScoreFirst,
    /// Prefer optimistic bound, then score, then greater depth.
    OptimisticBoundFirst,
}

/// Observable scheduling metadata. Breadth identifies a node within its parent's
/// domain scan; parent_breadth identifies its parent within the preceding scan.
#[derive(Clone, Debug)]
pub struct SearchNodeKey<S, B = S> {
    pub depth: usize,
    pub score: S,
    /// None is unknown, ranks ahead of finite bounds, and never permits pruning.
    pub optimistic_bound: Option<B>,
    pub parent_breadth: u64,
    pub breadth: u64,
}
fn ordinary_cmp<S: Score>(a: &S, b: &S) -> Ordering {
    a.ordinary_levels()
        .zip(b.ordinary_levels())
        .map(|(a, b)| a.numeric_cmp(&b))
        .find(|o| !o.is_eq())
        .unwrap_or(Ordering::Equal)
}
impl NodeExploration {
    /// Remaining ties prefer larger parent breadth then smaller breadth, except
    /// breadth-first reverses that precedence. Unknown bounds rank above finite
    /// bounds. Exact key ties are equal; the engine then prefers earlier insertion.
    pub fn compare<S: Score>(self, a: &SearchNodeKey<S>, b: &SearchNodeKey<S>) -> Ordering {
        self.compare_with(a, b, ordinary_cmp)
    }
    fn compare_with<S: Score, B>(
        self,
        a: &SearchNodeKey<S, B>,
        b: &SearchNodeKey<S, B>,
        compare_bound: impl Fn(&B, &B) -> Ordering,
    ) -> Ordering {
        let score = || ordinary_cmp(&a.score, &b.score);
        let bound = || match (&a.optimistic_bound, &b.optimistic_bound) {
            (Some(a), Some(b)) => compare_bound(a, b),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (None, None) => Ordering::Equal,
        };
        let depth = || a.depth.cmp(&b.depth);
        let priority = match self {
            Self::DepthFirst => depth().then_with(score).then_with(bound),
            Self::BreadthFirst => depth().reverse().then_with(score).then_with(bound),
            Self::ScoreFirst => score().then_with(bound).then_with(depth),
            Self::OptimisticBoundFirst => bound().then_with(score).then_with(depth),
        };
        if self == Self::BreadthFirst {
            priority
                .then_with(|| b.breadth.cmp(&a.breadth))
                .then_with(|| a.parent_breadth.cmp(&b.parent_breadth))
        } else {
            priority
                .then_with(|| a.parent_breadth.cmp(&b.parent_breadth))
                .then_with(|| b.breadth.cmp(&a.breadth))
        }
    }
}

/// One callback result. On partial nodes score is a scheduling estimate; on leaves
/// it is the actual score. A supplied bound must be at least the score of EVERY
/// structurally eligible completion of this node, under full Score ordering.
/// Unknown bounds are always safe. An underestimated bound can hide the optimum;
/// only visited leaves can be checked automatically.
#[derive(Clone, Debug)]
pub struct BranchEvaluation<S> {
    pub score: S,
    pub optimistic_bound: Option<S>,
}
impl<S> BranchEvaluation<S> {
    pub fn unbounded(score: S) -> Self {
        Self {
            score,
            optimistic_bound: None,
        }
    }
    pub fn bounded(score: S, optimistic_bound: S) -> Self {
        Self {
            score,
            optimistic_bound: Some(optimistic_bound),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchAndBoundConfig<'a> {
    pub exploration: NodeExploration,
    /// Includes the root, partial nodes and complete leaves; one callback per node.
    pub max_evaluations: u64,
    /// Maximum queued partial nodes. A node that cannot fit stops the search;
    /// it is never silently discarded followed by an optimality claim.
    pub max_frontier: usize,
    pub cancellation: Option<&'a crate::CancellationToken>,
    /// Cooperative time including model preparation, callbacks and observations.
    pub max_time: Option<Duration>,
}
impl Default for BranchAndBoundConfig<'_> {
    fn default() -> Self {
        Self {
            exploration: NodeExploration::DepthFirst,
            max_evaluations: 10_000,
            max_frontier: 10_000,
            cancellation: None,
            max_time: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchTermination {
    /// Every branch was evaluated or pruned using supplied bounds.
    Exhausted,
    EvaluationLimit,
    FrontierLimit,
    Cancelled,
    TimeLimit,
    /// Shared or phase-local move budget (ordered solver phases only).
    MoveEvaluationLimit,
    /// Shared solve stagnation controls (ordered solver phases only).
    UnimprovedTimeLimit,
    UnimprovedStepLimit,
    Condition,
    /// Retained-score goals (ordered solver phases only).
    BestScoreLimit,
    FeasibleScore,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedSearchResult<R> {
    pub best: Option<R>,
    pub evaluations: u64,
    pub complete_evaluations: u64,
    pub expanded_nodes: u64,
    /// Disjoint branches cut off, not the number of assignments beneath them.
    pub pruned_branches: u128,
    pub peak_frontier: usize,
    pub termination: BranchTermination,
}
/// Basic-variable bounded-search result.
pub type BranchAndBoundResult<V, S> = BoundedSearchResult<ScoredAssignment<V, S>>;
impl<R> BoundedSearchResult<R> {
    /// Conditional on deterministic scores and admissible bounds from the callback.
    pub fn is_optimal(&self) -> bool {
        self.termination == BranchTermination::Exhausted && self.best.is_some()
    }
}
/// A visited eligible leaf disproved one of its ancestor bounds or its own bound.
/// Unvisited/pruned completions cannot be validated without enumerating them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidOptimisticBound<S> {
    pub score: S,
    pub bound: S,
}
impl<S: fmt::Debug> fmt::Display for InvalidOptimisticBound<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "complete score {:?} exceeds optimistic bound {:?}",
            self.score, self.bound
        )
    }
}
impl<S: fmt::Debug> Error for InvalidOptimisticBound<S> {}

struct Node<A, S> {
    assignment: A,
    key: SearchNodeKey<S, InitializingScoreBound<S>>,
    /// Tightest full-score bound on the path, for pruning and visited-leaf checks.
    path_bound: Option<InitializingScoreBound<S>>,
    serial: u64,
    exploration: NodeExploration,
}
impl<V, S: Score> Ord for Node<V, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.exploration
            .cmp(&other.exploration)
            .then_with(|| {
                self.exploration.compare_with(
                    &self.key,
                    &other.key,
                    InitializingScoreBound::ordinary_cmp,
                )
            })
            .then_with(|| other.serial.cmp(&self.serial))
    }
}
impl<V, S: Score> PartialOrd for Node<V, S> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<V, S: Score> PartialEq for Node<V, S> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o).is_eq()
    }
}
impl<V, S: Score> Eq for Node<V, S> {}
fn dominated<S: Score>(bound: Option<&InitializingScoreBound<S>>, best: Option<&S>) -> bool {
    bound
        .zip(best)
        .is_some_and(|(bound, best)| !bound.compare_score(best).is_gt())
}

/// Maximize over a finite basic-variable tree. Movable variables branch in model
/// order, their domain order followed by optional None. Pins stay fixed; movable
/// initial values do not restrict search. Each callback receives the assignment
/// and the remaining movable indices. A decided optional None is NOT an unresolved
/// variable: use those indices, not assignment completeness, to identify leaves.
///
/// Partial scores guide ordering, never rejection. Negative structural LEAF scores
/// are rejected. Hard-infeasible eligible leaves can be the best result. Ties retain
/// the first evaluated eligible leaf. A parent generates children in domain order;
/// exploration controls which queued partial parent is expanded next.
///
/// Bounds must be admissible under full Score ordering (including structural state).
/// Return None when no safe bound is available. An eligible visited leaf exceeding
/// the tightest bound on its path returns an error; this cannot detect bad bounds
/// on branches pruned before visiting a leaf. No automatic score-trend inference.
///
/// Limits are checked before each callback and parent expansion: evaluation budget,
/// cancellation, then time. Exhaustion after the final callback, or pruning every
/// remaining branch, wins over these controls. A callback is not preempted. Zero
/// evaluation/time budgets invoke no callback for nonempty spaces. Empty spaces
/// report exhaustion without callbacks. A frontier limit retains the best already
/// evaluated leaf but never proves optimality. Owned snapshots keep the model intact
/// on callback/clone/observer failure; panics propagate.
///
/// ```
/// use timetuck::{FiniteProblem, Variable, SimpleScore, BranchEvaluation,
///     BranchAndBoundConfig, branch_and_bound};
/// let p = FiniteProblem::new(vec![Variable::new(vec![2_i64, 1, 0]); 4])?;
/// let r = branch_and_bound(&p, |a, remaining| {
///     let sum = a.values().iter().flatten().sum::<i64>();
///     BranchEvaluation::bounded(SimpleScore::new(sum),
///         SimpleScore::new(sum + 2 * remaining.len() as i64))
/// }, BranchAndBoundConfig::default())?;
/// assert!(r.is_optimal());
/// assert_eq!(r.best.unwrap().score, SimpleScore::new(8));
/// assert!(r.complete_evaluations < 81);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn branch_and_bound<V: Eq + Clone, S: Score>(
    problem: &FiniteProblem<V>,
    evaluate: impl FnMut(&Assignment<V>, &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
) -> Result<BranchAndBoundResult<V, S>, InvalidOptimisticBound<S>> {
    branch_and_bound_with_observer(problem, evaluate, config, |_| {})
}
/// Notify synchronously after every strict best improvement. Observer work counts
/// toward time, not node evaluations. The observer may request cancellation.
pub fn branch_and_bound_with_observer<V: Eq + Clone, S: Score>(
    problem: &FiniteProblem<V>,
    evaluate: impl FnMut(&Assignment<V>, &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
    observer: impl FnMut(&ScoredAssignment<V, S>),
) -> Result<BranchAndBoundResult<V, S>, InvalidOptimisticBound<S>> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    search(problem, evaluate, config, observer, clock)
}
fn search<V: Eq + Clone, S: Score>(
    problem: &FiniteProblem<V>,
    mut evaluate: impl FnMut(&Assignment<V>, &[usize]) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
    mut observer: impl FnMut(&ScoredAssignment<V, S>),
    clock: impl FnMut() -> Duration,
) -> Result<BranchAndBoundResult<V, S>, InvalidOptimisticBound<S>> {
    let time = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let (tree, root) = crate::branch_tree::BasicTree::new(problem);
    search_tree(
        &tree,
        root,
        |a, depth| evaluate(a, &tree.remaining()[depth..]),
        config,
        &mut observer,
        time,
    )
}

pub(crate) trait Retained<A, S> {
    fn new(assignment: A, score: S) -> Self;
    fn score(&self) -> &S;
}
impl<V, S> Retained<Assignment<V>, S> for ScoredAssignment<V, S> {
    fn new(assignment: Assignment<V>, score: S) -> Self {
        Self { assignment, score }
    }
    fn score(&self) -> &S {
        &self.score
    }
}

#[derive(Debug)]
pub(crate) enum TreeSearchError<E, S> {
    Evaluation(E),
    Bound {
        score: S,
        bound: InitializingScoreBound<S>,
    },
    Trend(crate::ScoreTrendError),
}
pub(crate) struct TreeEvaluation<S> {
    pub score: S,
    pub optimistic_bound: Option<InitializingScoreBound<S>>,
}
pub(crate) trait BranchContext<A, S, R> {
    type Error;
    fn evaluate(&mut self, assignment: &A, depth: usize) -> Result<TreeEvaluation<S>, Self::Error>;
    fn stop(&mut self, evaluations: u64, best: Option<&S>) -> Option<BranchTermination>;
    fn improved(&mut self, best: &R);
}
struct StandaloneContext<'a, 'm, F, O, C, S> {
    evaluate: F,
    observer: O,
    config: BranchAndBoundConfig<'a>,
    time: crate::time_limit::TimeBudget<'m, C, S>,
}
impl<A, S: Score, R, F, O, C> BranchContext<A, S, R> for StandaloneContext<'_, '_, F, O, C, S>
where
    F: FnMut(&A, usize) -> BranchEvaluation<S>,
    O: FnMut(&R),
    C: FnMut() -> Duration,
{
    type Error = std::convert::Infallible;
    fn evaluate(&mut self, assignment: &A, depth: usize) -> Result<TreeEvaluation<S>, Self::Error> {
        let e = (self.evaluate)(assignment, depth);
        Ok(TreeEvaluation {
            score: e.score,
            optimistic_bound: e.optimistic_bound.map(InitializingScoreBound::finite),
        })
    }
    fn stop(&mut self, evaluations: u64, _: Option<&S>) -> Option<BranchTermination> {
        stop(self.config, evaluations, &mut self.time)
    }
    fn improved(&mut self, best: &R) {
        (self.observer)(best);
    }
}
pub(crate) fn search_tree<T, S, R>(
    tree: &T,
    root: Option<T::Assignment>,
    evaluate: impl FnMut(&T::Assignment, usize) -> BranchEvaluation<S>,
    config: BranchAndBoundConfig<'_>,
    observer: impl FnMut(&R),
    time: crate::time_limit::TimeBudget<'_, impl FnMut() -> Duration, S>,
) -> Result<BoundedSearchResult<R>, InvalidOptimisticBound<S>>
where
    T: crate::branch_tree::BranchTree,
    S: Score,
    R: Retained<T::Assignment, S>,
{
    let mut context = StandaloneContext {
        evaluate,
        observer,
        config,
        time,
    };
    match run_tree(
        tree,
        root,
        config.exploration,
        config.max_frontier,
        None,
        &mut context,
    ) {
        Ok(r) => Ok(r),
        Err(TreeSearchError::Bound { score, bound }) => Err(InvalidOptimisticBound {
            score,
            bound: bound
                .into_finite()
                .expect("standalone supplies finite bounds"),
        }),
        Err(TreeSearchError::Trend(_)) => unreachable!("standalone has no trend"),
        Err(TreeSearchError::Evaluation(e)) => match e {},
    }
}

pub(crate) fn run_tree<T, S, R, C>(
    tree: &T,
    root: Option<T::Assignment>,
    exploration: NodeExploration,
    max_frontier: usize,
    prior: Option<S>,
    context: &mut C,
) -> Result<BoundedSearchResult<R>, TreeSearchError<C::Error, S>>
where
    T: crate::branch_tree::BranchTree,
    S: Score,
    R: Retained<T::Assignment, S>,
    C: BranchContext<T::Assignment, S, R>,
{
    let mut result = BoundedSearchResult {
        best: None,
        evaluations: 0,
        complete_evaluations: 0,
        expanded_nodes: 0,
        pruned_branches: 0,
        peak_frontier: 0,
        termination: BranchTermination::Exhausted,
    };
    let Some(assignment) = root else {
        return Ok(result);
    };
    let mut frontier = BinaryHeap::new();
    // Each call scores a new node and either saves its leaf or queues its subtree.
    let visit = |assignment: T::Assignment,
                 depth: usize,
                 breadth: u64,
                 parent_breadth: u64,
                 inherited: Option<&InitializingScoreBound<S>>,
                 result: &mut BoundedSearchResult<R>,
                 frontier: &mut BinaryHeap<Node<T::Assignment, S>>,
                 context: &mut C|
     -> Result<bool, TreeSearchError<C::Error, S>> {
        let TreeEvaluation {
            score,
            optimistic_bound,
        } = context
            .evaluate(&assignment, depth)
            .map_err(TreeSearchError::Evaluation)?;
        let serial = result.evaluations;
        result.evaluations += 1;
        let path_bound = match (inherited, optimistic_bound.as_ref()) {
            (Some(a), Some(b)) => Some(a.min(b).clone()),
            (Some(a), None) | (None, Some(a)) => Some(a.clone()),
            (None, None) => None,
        };
        if depth == tree.depth() {
            result.complete_evaluations += 1;
            if score.structural() >= 0 {
                if let Some(bound) = path_bound
                    .as_ref()
                    .filter(|b| b.compare_score(&score).is_lt())
                {
                    return Err(TreeSearchError::Bound {
                        score,
                        bound: bound.clone(),
                    });
                }
                if result
                    .best
                    .as_ref()
                    .map(R::score)
                    .or(prior.as_ref())
                    .is_none_or(|best| score > *best)
                {
                    result.best = Some(R::new(assignment, score));
                    context.improved(result.best.as_ref().unwrap());
                }
            }
        } else if dominated(
            path_bound.as_ref(),
            result.best.as_ref().map(R::score).or(prior.as_ref()),
        ) {
            result.pruned_branches += 1;
        } else {
            if frontier.len() >= max_frontier {
                result.termination = BranchTermination::FrontierLimit;
                return Ok(false);
            }
            frontier.push(Node {
                assignment,
                key: SearchNodeKey {
                    depth,
                    score,
                    optimistic_bound,
                    parent_breadth,
                    breadth,
                },
                path_bound,
                serial,
                exploration,
            });
            result.peak_frontier = result.peak_frontier.max(frontier.len());
        }
        Ok(true)
    };
    if let Some(reason) = context.stop(
        result.evaluations,
        result.best.as_ref().map(R::score).or(prior.as_ref()),
    ) {
        result.termination = reason;
        return Ok(result);
    }
    if !visit(
        assignment,
        0,
        0,
        0,
        None,
        &mut result,
        &mut frontier,
        context,
    )? {
        return Ok(result);
    }
    while let Some(parent) = frontier.pop() {
        if dominated(
            parent.path_bound.as_ref(),
            result.best.as_ref().map(R::score).or(prior.as_ref()),
        ) {
            result.pruned_branches += 1;
            continue;
        }
        if let Some(reason) = context.stop(
            result.evaluations,
            result.best.as_ref().map(R::score).or(prior.as_ref()),
        ) {
            result.termination = reason;
            return Ok(result);
        }
        let mut choices = tree.choices(&parent.assignment, parent.key.depth);
        let choice_count = choices.len();
        result.expanded_nodes += 1;
        for breadth in 0..choice_count {
            if dominated(
                parent.path_bound.as_ref(),
                result.best.as_ref().map(R::score).or(prior.as_ref()),
            ) {
                result.pruned_branches += (choice_count - breadth) as u128;
                break;
            }
            if let Some(reason) = context.stop(
                result.evaluations,
                result.best.as_ref().map(R::score).or(prior.as_ref()),
            ) {
                result.termination = reason;
                return Ok(result);
            }
            let child = tree.child(
                &parent.assignment,
                parent.key.depth,
                choices.next().expect("exact choice count"),
            );
            if !visit(
                child,
                parent.key.depth + 1,
                breadth as u64,
                parent.key.breadth,
                parent.path_bound.as_ref(),
                &mut result,
                &mut frontier,
                context,
            )? {
                return Ok(result);
            }
        }
    }
    Ok(result)
}
fn stop<S>(
    config: BranchAndBoundConfig<'_>,
    evaluations: u64,
    time: &mut crate::time_limit::TimeBudget<'_, impl FnMut() -> Duration, S>,
) -> Option<BranchTermination> {
    if evaluations >= config.max_evaluations {
        Some(BranchTermination::EvaluationLimit)
    } else if config
        .cancellation
        .is_some_and(crate::CancellationToken::is_cancelled)
    {
        Some(BranchTermination::Cancelled)
    } else if time.expired() {
        Some(BranchTermination::TimeLimit)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimpleScore, Variable};
    #[test]
    fn controlled_elapsed_limit_retains_last_finished_leaf() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
        let mut times = [0, 0, 0, 10].into_iter();
        let r = search(
            &p,
            |a, _| BranchEvaluation::unbounded(SimpleScore::new(a.values()[0].unwrap_or(0))),
            BranchAndBoundConfig {
                max_time: Some(Duration::from_millis(10)),
                ..Default::default()
            },
            |_| {},
            || Duration::from_millis(times.next().expect("unexpected clock poll")),
        )
        .unwrap();
        assert_eq!(r.termination, BranchTermination::TimeLimit);
        assert_eq!(r.evaluations, 2);
        assert_eq!(r.complete_evaluations, 1);
        assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
    }
    #[test]
    fn disabled_time_never_reads_clock_and_pruned_frontier_proves_completion() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 0]); 3]).unwrap();
        let mut trace = Vec::new();
        let r = search(
            &p,
            |a, left| {
                trace.push((a.clone(), left.len()));
                let sum = a.values().iter().flatten().sum::<i64>();
                BranchEvaluation::bounded(
                    SimpleScore::new(sum),
                    SimpleScore::new(sum + left.len() as i64),
                )
            },
            BranchAndBoundConfig::default(),
            |_| {},
            || panic!("disabled clock"),
        )
        .unwrap();
        let budget = r.evaluations;
        let again = search(
            &p,
            |a, left| {
                let sum = a.values().iter().flatten().sum::<i64>();
                BranchEvaluation::bounded(
                    SimpleScore::new(sum),
                    SimpleScore::new(sum + left.len() as i64),
                )
            },
            BranchAndBoundConfig {
                max_evaluations: budget,
                ..Default::default()
            },
            |_| {},
            || panic!("disabled clock"),
        )
        .unwrap();
        assert!(again.is_optimal());
        assert_eq!(again, r);
    }
}
