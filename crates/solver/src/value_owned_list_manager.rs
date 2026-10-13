//! Publish existing list-manager builtin shadows into actual value-owned fields.
use crate::{
    AdditiveScore, ListAssignment, ListBuiltinValues, ListConstraintNetwork, ListProblem,
    ListScoringError, ListSolutionManager, ListSolutionShadows, ManagedSolution,
    SolutionRefreshError, SolutionUpdatePolicy, list_solution_manager,
};
use std::{error::Error, fmt};
#[derive(Debug)]
pub enum ValueOwnedListUpdateError<V> {
    OwnerCount { expected: usize, actual: usize },
    Refresh(SolutionRefreshError<ListScoringError, V>),
}
impl<V: fmt::Debug> fmt::Display for ValueOwnedListUpdateError<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value-owned list update: {self:?}")
    }
}
impl<V: fmt::Debug> Error for ValueOwnedListUpdateError<V> {}
type Publish<'a, V, E> = dyn FnMut(&V, &ListBuiltinValues<usize, V>, Option<&E>) + 'a;
/// Bind explicit real owner objects to the existing list solution manager.
/// All model/score-policy validation occurs in that manager before publication.
/// ShadowsOnly preserves saved score and never scores. ScoreOnly does not call
/// setters. Actual owner references come from the supplied row-ordered catalog,
/// preserving object identity when updating null/stale value-owned inverse fields.
/// The callback can publish all four builtin fields; native lists remain explicit.
/// Publication is sequential: external setter effects are not rolled back on panic.
pub struct ValueOwnedListSolutionManager<'a, V, E, S> {
    manager: ListSolutionManager<'a, V, S>,
    problem: &'a ListProblem<V>,
    owners: &'a [E],
    publish: Box<Publish<'a, V, E>>,
}
impl<'a, V: Eq + Clone + 'a, E, S: AdditiveScore + 'a> ValueOwnedListSolutionManager<'a, V, E, S> {
    pub fn new(
        problem: &'a ListProblem<V>,
        network: &'a ListConstraintNetwork<'_, V, S>,
        owners: &'a [E],
        publish: impl FnMut(&V, &ListBuiltinValues<usize, V>, Option<&E>) + 'a,
    ) -> Result<Self, ValueOwnedListUpdateError<V>> {
        if owners.len() != problem.entities().len() {
            return Err(ValueOwnedListUpdateError::OwnerCount {
                expected: problem.entities().len(),
                actual: owners.len(),
            });
        }
        Ok(Self {
            manager: list_solution_manager(problem, network),
            problem,
            owners,
            publish: Box::new(publish),
        })
    }
    pub fn update<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<ListAssignment<V>, ListSolutionShadows<V>, S>,
        policy: SolutionUpdatePolicy,
    ) -> Result<Option<&'r S>, ValueOwnedListUpdateError<V>> {
        self.manager
            .update_with_policy(solution, policy)
            .map_err(ValueOwnedListUpdateError::Refresh)?;
        if policy != SolutionUpdatePolicy::ScoreOnly {
            for (value, fields) in self.problem.values().iter().zip(&solution.shadows) {
                (self.publish)(value, fields, fields.owner.map(|owner| &self.owners[owner]));
            }
        }
        Ok(solution.score.as_ref())
    }
}
