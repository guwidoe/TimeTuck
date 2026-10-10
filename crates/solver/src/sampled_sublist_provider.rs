//! Bounded sublist sampling in complete list and mixed solver plans.
use crate::{
    ListAssignment, ListError, ListMove, ListProblem, MixedAssignment, MixedMove, MixedProblem,
    Move, MoveProvider, SubListChangeConfig, SubListSwapConfig, sampled_sublist_change_moves,
    sampled_sublist_swap_moves,
};

/// Sampling policy. Changes relocate/reverse/unassign contiguous spans; swaps
/// exchange two disjoint spans, optionally reversing them. Existing configuration
/// bounds and sampling distributions apply, including asymmetric swap lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampledSubListNeighborhood {
    Change(SubListChangeConfig),
    Swap(SubListSwapConfig),
}

/// A reproducible, bounded provider for list and mixed local search.
///
/// Each opening uses `seed.wrapping_add(completed_steps)` and rebuilds span indexes
/// from the current assignment. Reopening the same snapshot and step replays the
/// same sequence, regardless of how much of an earlier iterator was consumed.
/// Different phases with the same step counter replay the same seed; use separate
/// seeds to give phases distinct sampling streams. Native PRNG traces do not
/// reproduce Java traces.
///
/// At most `attempts_per_step` proposals are drawn. Rejected ranges, overlaps,
/// dependencies and unchanged assignments consume attempts; they never reach the
/// scoring engine. Duplicates are retained. An empty sample can stop local search
/// without proving a local optimum. This provider never assigns missing values;
/// run construction first when required membership is incomplete.
///
/// Opening validates inputs and builds only span indexes, not a move catalog.
/// Candidates are drawn lazily and checked against a borrowed immutable snapshot.
/// No planning values are cloned and no candidate is applied merely to filter it.
/// Dependency-free moves use direct geometry/pin/range checks; dependency-enabled
/// models use borrowed draft rows and validate the final predecessor graph.
/// Mixed moves preserve all basic variables. Standard engine budgets, acceptance,
/// undo and score assertions continue to apply.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])?;
/// let a = p.initial_assignment();
/// let provider = SampledSubListProvider::new(
///     SampledSubListNeighborhood::Swap(SubListSwapConfig::default()), 42, 100);
/// assert!(provider.select(&p, &a, 0)?.count() <= 100);
/// # Ok::<(), ListError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampledSubListProvider {
    pub neighborhood: SampledSubListNeighborhood,
    pub seed: u64,
    pub attempts_per_step: u64,
}
impl SampledSubListProvider {
    pub fn new(
        neighborhood: SampledSubListNeighborhood,
        seed: u64,
        attempts_per_step: u64,
    ) -> Self {
        Self {
            neighborhood,
            seed,
            attempts_per_step,
        }
    }

    /// Fallible opening for direct callers. Solver trait bindings require valid
    /// assignments/configuration and panic if this opening fails.
    pub fn select<'a, V: Clone + Eq>(
        &self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        completed_steps: u64,
    ) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
        let seed = self.seed.wrapping_add(completed_steps);
        let candidates: Box<dyn Iterator<Item = ListMove<V>> + 'a> = match self.neighborhood {
            SampledSubListNeighborhood::Change(config) => Box::new(sampled_sublist_change_moves(
                problem,
                assignment,
                config,
                seed,
                self.attempts_per_step,
            )?),
            SampledSubListNeighborhood::Swap(config) => Box::new(sampled_sublist_swap_moves(
                problem,
                assignment,
                config,
                seed,
                self.attempts_per_step,
            )?),
        };
        let checker = problem.move_checker(assignment)?;
        Ok(candidates.filter(move |movement| checker.is_doable(movement) == Ok(true)))
    }
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for SampledSubListProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            self.select(p, a, step)
                .expect("sublist provider requires valid assignment and configuration"),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for SampledSubListProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        p.validate(a)
            .expect("sublist provider requires valid mixed assignment");
        Box::new(
            self.select(p.lists(), a.lists(), step)
                .expect("sublist provider requires valid configuration")
                .map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                }),
        )
    }
}
