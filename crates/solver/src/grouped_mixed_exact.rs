//! Unbounded mixed exact search with one lazy Cartesian basic-variable branch.
use crate::branch_and_bound::{BranchContext, TreeEvaluation, TreeSearchError, run_tree};
use crate::branch_tree::{BasicTree, BranchTree, ListTree};
use crate::*;
use std::{error::Error, fmt, time::Instant};
#[derive(Debug)]
pub enum GroupedMixedExactError {
    CartesianProductOverflow,
    Scoring(MixedScoringError),
}
impl fmt::Display for GroupedMixedExactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CartesianProductOverflow => {
                f.write_str("basic Cartesian branch exceeds usize capacity")
            }
            Self::Scoring(e) => e.fmt(f),
        }
    }
}
impl Error for GroupedMixedExactError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) => Some(e),
            _ => None,
        }
    }
}
struct GroupedTree<'a, B, L> {
    basic: BasicTree<'a, B>,
    lists: ListTree<'a, L>,
    radices: Vec<usize>,
    product: usize,
}
impl<'a, B: Eq + Clone, L: Eq + Clone> GroupedTree<'a, B, L> {
    fn new(
        problem: &'a MixedProblem<B, L>,
    ) -> Result<(Self, Option<MixedAssignment<B, L>>), GroupedMixedExactError> {
        let (basic, b) = BasicTree::new(problem.basic());
        let (lists, l) = ListTree::new(problem.lists());
        let root = b.zip(l).map(|(b, l)| MixedAssignment::new(b, l));
        let radices: Vec<_> = basic
            .remaining()
            .iter()
            .map(|&i| {
                problem.basic().variables()[i].domain().len()
                    + usize::from(problem.basic().variables()[i].allows_unassigned())
            })
            .collect();
        // Empty component spaces are exhausted without scoring, not overflows.
        let product = if root.is_none() {
            0
        } else {
            radices
                .iter()
                .try_fold(1usize, |n, &r| n.checked_mul(r))
                .ok_or(GroupedMixedExactError::CartesianProductOverflow)?
        };
        Ok((
            Self {
                basic,
                lists,
                radices,
                product,
            },
            root,
        ))
    }
}
enum Choice {
    Basic(usize),
    List(Option<ListPosition>),
}
enum Choices<I> {
    Basic(std::ops::Range<usize>),
    List(I),
}
impl<I: ExactSizeIterator<Item = Option<ListPosition>>> Iterator for Choices<I> {
    type Item = Choice;
    fn next(&mut self) -> Option<Choice> {
        match self {
            Self::Basic(i) => i.next().map(Choice::Basic),
            Self::List(i) => i.next().map(Choice::List),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}
impl<I: ExactSizeIterator<Item = Option<ListPosition>>> ExactSizeIterator for Choices<I> {
    fn len(&self) -> usize {
        match self {
            Self::Basic(i) => i.len(),
            Self::List(i) => i.len(),
        }
    }
}
impl<B: Eq + Clone, L: Eq + Clone> BranchTree for GroupedTree<'_, B, L> {
    type Assignment = MixedAssignment<B, L>;
    type Choice = Choice;
    fn depth(&self) -> usize {
        self.lists.depth() + usize::from(!self.radices.is_empty())
    }
    fn remaining_components(&self, depth: usize) -> (&[usize], &[usize]) {
        if depth < self.lists.depth() {
            (self.basic.remaining(), &self.lists.remaining()[depth..])
        } else if depth < self.depth() {
            (self.basic.remaining(), &[])
        } else {
            (&[], &[])
        }
    }
    fn choices<'b>(
        &'b self,
        a: &'b Self::Assignment,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Choice> + 'b {
        if depth < self.lists.depth() {
            Choices::List(self.lists.choices(a.lists(), depth))
        } else {
            Choices::Basic(0..self.product)
        }
    }
    fn child(&self, a: &Self::Assignment, depth: usize, choice: Choice) -> Self::Assignment {
        self.child_owned(a.clone(), depth, choice)
    }
    fn child_owned(&self, a: Self::Assignment, depth: usize, choice: Choice) -> Self::Assignment {
        let (mut basic, lists) = a.into_parts();
        match choice {
            Choice::List(c) => MixedAssignment::new(basic, self.lists.child_owned(lists, depth, c)),
            Choice::Basic(mut index) => {
                for (d, &radix) in self.radices.iter().enumerate().rev() {
                    basic = self.basic.child_owned(basic, d, index % radix);
                    index /= radix;
                }
                MixedAssignment::new(basic, lists)
            }
        }
    }
}
struct Context<'a, 'n, 'l, B, L, S, F> {
    problem: &'a MixedProblem<B, L>,
    network: &'n MixedConstraintNetwork<'n, B, L, S>,
    config: BranchAndBoundConfig<'a>,
    started: Instant,
    lifecycle: crate::solver_lifecycle::Lifecycle<'l>,
    leaf_depth: usize,
    leaf_observer: F,
}
impl<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore, F: FnMut(&MixedAssignment<B, L>, &S)>
    BranchContext<MixedAssignment<B, L>, S, ScoredMixedAssignment<B, L, S>>
    for Context<'_, '_, '_, B, L, S, F>
{
    type Error = MixedScoringError;
    fn evaluate(
        &mut self,
        a: &MixedAssignment<B, L>,
        depth: usize,
    ) -> Result<TreeEvaluation<S>, Self::Error> {
        let score = self.network.calculate(self.problem, a)?;
        if depth == self.leaf_depth && score.structural() >= 0 {
            (self.leaf_observer)(a, &score);
        }
        Ok(TreeEvaluation {
            score,
            optimistic_bound: None,
        })
    }
    fn stop(&mut self, evaluations: u64, _: Option<&S>) -> Option<BranchTermination> {
        if evaluations >= self.config.max_evaluations {
            Some(BranchTermination::EvaluationLimit)
        } else if self
            .config
            .cancellation
            .is_some_and(CancellationToken::is_cancelled)
        {
            Some(BranchTermination::Cancelled)
        } else if self
            .config
            .max_time
            .is_some_and(|limit| self.started.elapsed() >= limit)
        {
            Some(BranchTermination::TimeLimit)
        } else {
            None
        }
    }
    fn improved(&mut self, _: &ScoredMixedAssignment<B, L, S>) {}
    fn expansion_started(&mut self) {
        self.lifecycle.start_step();
    }
    fn expansion_ended(&mut self, _: &S, completed: bool) {
        self.lifecycle.end_step(completed);
    }
}
/// Explicit alternative to per-variable mixed branching. List choices retain the
/// existing eligible tree; all movable basic variables form one mixed-radix
/// Cartesian branch. Complete assignments are scored once, with no pruning bound.
/// Each actually expanded partial node is one lifecycle step. Leaf scoring work
/// remains visible in evaluations/complete_evaluations. Large products are lazy,
/// but products exceeding usize capacity are rejected before lifecycle callbacks.
/// Limits, pins, ranges, optional choices and dependency filtering remain active.
/// This standalone one-phase solver does not change default solve sequences or
/// establish Java traversal/count equality. Exhaustion is required for optimality.
#[derive(Clone, Copy, Debug, Default)]
pub struct GroupedMixedExactSolver<'a> {
    pub config: BranchAndBoundConfig<'a>,
}
impl<'a> GroupedMixedExactSolver<'a> {
    pub fn solve<'l, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
        &self,
        problem: &'a MixedProblem<B, L>,
        network: &MixedConstraintNetwork<'_, B, L, S>,
        lifecycle: Option<&'l mut SolverLifecycleCallback<'l>>,
    ) -> Result<MixedBranchAndBoundResult<B, L, S>, GroupedMixedExactError> {
        self.solve_with_leaf_observer(problem, network, lifecycle, |_, _| {})
    }
    /// Notify once for each scored eligible complete leaf, including ties and
    /// worse scores. No partial or unscored state is sent. The callback is inline
    /// and its cost belongs to the budget; panics retain normal unwind behavior.
    pub fn solve_with_leaf_observer<'l, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
        &self,
        problem: &'a MixedProblem<B, L>,
        network: &MixedConstraintNetwork<'_, B, L, S>,
        lifecycle: Option<&'l mut SolverLifecycleCallback<'l>>,
        leaf_observer: impl FnMut(&MixedAssignment<B, L>, &S),
    ) -> Result<MixedBranchAndBoundResult<B, L, S>, GroupedMixedExactError> {
        let started = Instant::now();
        let expected = [
            network.variable_count(),
            network.entity_count(),
            network.value_count(),
        ];
        let actual = [
            problem.basic().variables().len(),
            problem.lists().entities().len(),
            problem.lists().values().len(),
        ];
        if expected != actual {
            return Err(GroupedMixedExactError::Scoring(
                MixedScoringError::ModelShape { expected, actual },
            ));
        }
        problem
            .validate(&problem.initial_assignment())
            .map_err(|e| GroupedMixedExactError::Scoring(MixedScoringError::Model(e)))?;
        let (tree, root) = GroupedTree::new(problem)?;
        let mut context = Context {
            problem,
            network,
            config: self.config,
            started,
            lifecycle: crate::solver_lifecycle::Lifecycle::new(lifecycle),
            leaf_depth: tree.depth(),
            leaf_observer,
        };
        context.lifecycle.start();
        context.lifecycle.start_phase(0, SolverPhaseKind::Exact);
        let result = run_tree(
            &tree,
            root,
            self.config.exploration,
            self.config.max_frontier,
            None,
            &mut context,
        );
        context.lifecycle.end_phase();
        match result {
            Ok(r) => Ok(r),
            Err(TreeSearchError::Evaluation(e)) => Err(GroupedMixedExactError::Scoring(e)),
            Err(TreeSearchError::Bound { .. } | TreeSearchError::Trend(_)) => {
                unreachable!("unbounded grouped search supplies no bounds/trends")
            }
        }
    }
}
