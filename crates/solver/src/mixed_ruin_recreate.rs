//! Joint sampling and reconstruction of mixed basic/list models.
use crate::*;
use std::time::Instant;

/// Shared ruin limits and placement with an explicit component encounter order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedRuinRecreateConfig<'a, S = SimpleScore> {
    pub search: RuinRecreateConfig<'a, S>,
    pub order: MixedConstructionOrder,
}
impl<S> Default for MixedRuinRecreateConfig<'_, S> {
    fn default() -> Self {
        Self {
            search: RuinRecreateConfig::default(),
            order: MixedConstructionOrder::default(),
        }
    }
}
pub type MixedRuinRecreateResult<B, L, S> = RuinRecreateOutcome<ScoredMixedAssignment<B, L, S>>;

impl From<MixedLocalSearchError> for RuinRecreateError {
    fn from(e: MixedLocalSearchError) -> Self {
        Self::MixedSearch(e)
    }
}
/// Sample one joint pool of movable basic slots and list value identities.
/// Enabled fine families enumerate basic then list moves, changing one component
/// per attempt. Both components contribute to the coupled score and acceptor.
/// Ruin counts apply to the combined pool. Pins, unsampled basic values, and
/// unsampled list owners/relative order are preserved. Missing optional values
/// are eligible under the same rules as the individual basic/list searches.
///
/// Reconstruction scores the coupled model. Sequential and ValueQueue placement
/// finish components in `order`; pooled placement compares both components.
/// Acceptance entity identities are basic slot indices followed by list entity
/// indices; value identities distinguish Basic from List even for equal values.
/// Move equality compares final basic edits and changed lists. Relative tabu sizes
/// use basic-slot plus list-entity population. These are native search semantics.
/// Shared work limits and best retention follow [`ruin_recreate_search_from`].
pub fn ruin_recreate_mixed_search_from<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    ruin_recreate_mixed_search_from_observed(problem, network, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_mixed_search_from_with_observer<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    ruin_recreate_mixed_search_from_observed(
        problem,
        network,
        initial,
        config,
        Some(&mut observer),
        None,
    )
}
pub(crate) fn ruin_recreate_mixed_search_from_observed<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    network
        .validate(problem, &initial)
        .map_err(MixedLocalSearchError::from)?;
    run(problem, initial, config, observer, shared, |p, a, c| {
        greedy_mixed_construction_from(p, network, a, c).map_err(MixedLocalSearchError::from)
    })
}
/// Stream counterpart, rebuilding the coupled graph for initialization and each
/// repair or fine-move score. Factory work consumes the cooperative time budget. Validation and
/// zero-budget/pre-cancellation checks precede factory invocation.
pub fn ruin_recreate_mixed_stream_search_from<
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    ruin_recreate_mixed_stream_search_from_observed(problem, graph, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_mixed_stream_search_from_with_observer<
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    ruin_recreate_mixed_stream_search_from_observed(
        problem,
        graph,
        initial,
        config,
        Some(&mut observer),
        None,
    )
}
pub(crate) fn ruin_recreate_mixed_stream_search_from_observed<
    'g,
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, MixedStreamSources<B, L>), GraphError>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    run(problem, initial, config, observer, shared, |p, a, mut c| {
        let start = c.construction.max_time.map(|_| Instant::now());
        let (graph, sources) =
            graph().map_err(|e| MixedLocalSearchError::from(MixedStreamScoringError::from(e)))?;
        if let (Some(limit), Some(start)) = (c.construction.max_time, start) {
            c.construction.max_time = Some(limit.saturating_sub(start.elapsed()));
        }
        greedy_mixed_stream_construction_from(p, graph, sources, a, c)
            .map_err(MixedLocalSearchError::from)
    })
}
fn run<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    config: MixedRuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, MixedAssignment<B, L>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    mut rebuild: impl FnMut(
        &MixedProblem<B, L>,
        MixedAssignment<B, L>,
        MixedConstructionConfig,
    ) -> Result<MixedConstructionResult<B, L, S>, MixedLocalSearchError>,
) -> Result<MixedRuinRecreateResult<B, L, S>, RuinRecreateError> {
    crate::ruin_engine::run(
        problem,
        initial,
        config.search,
        observer,
        shared,
        |p, a, construction| {
            let r = rebuild(
                p,
                a,
                MixedConstructionConfig {
                    construction,
                    order: config.order,
                },
            )?;
            Ok(crate::ruin_engine::Repair {
                assignment: r.assignment,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
            })
        },
    )
    .map(|r| r.map(|(assignment, score)| ScoredMixedAssignment { assignment, score }))
}
impl<B: Eq + Clone, L: Eq + Clone> crate::ruin_engine::RepairModel for MixedProblem<B, L> {
    type Assignment = MixedAssignment<B, L>;
    type FineMove = MixedMove<B, L>;
    type Movement = (Move<B>, Vec<(usize, Vec<L>)>);
    type Value = MixedPlanningValue<B, L>;
    fn validate_complete(&self, a: &Self::Assignment) -> Result<(), RuinRecreateError> {
        self.validate(a)
            .map_err(|e| MixedLocalSearchError::from(MixedScoringError::from(e)))?;
        if !self.is_complete(a) {
            return Err(RuinRecreateError::IncompleteInitialAssignment);
        }
        Ok(())
    }
    fn initial(&self) -> Self::Assignment {
        self.initial_assignment()
    }
    fn candidate_count(&self) -> usize {
        self.basic().variables().len() + self.lists().values().len()
    }
    fn population(&self) -> usize {
        self.basic().population() + self.lists().population()
    }
    fn movable(&self, a: &Self::Assignment) -> Vec<usize> {
        let offset = self.basic().population();
        let mut result = self.basic().movable(a.basic());
        result.extend(
            self.lists()
                .movable(a.lists())
                .into_iter()
                .map(|i| offset + i),
        );
        result
    }
    fn repair(&self, a: &Self::Assignment, selected: &[usize]) -> Result<Self, RuinRecreateError> {
        let offset = self.basic().population();
        let basics: Vec<_> = selected.iter().copied().filter(|&i| i < offset).collect();
        let lists: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&i| i >= offset)
            .map(|i| i - offset)
            .collect();
        Ok(Self::new(
            self.basic().repair(a.basic(), &basics)?,
            self.lists().repair(a.lists(), &lists)?,
        ))
    }
    fn validate_neighborhood(
        &self,
        neighborhood: RepairNeighborhood<'_>,
    ) -> Result<(), RuinRecreateError> {
        if let RepairNeighborhood::Basic(c) = neighborhood {
            return c
                .validate(self.basic())
                .map_err(RuinRecreateError::BasicNeighborhood);
        }
        if let RepairNeighborhood::Mixed(c) = neighborhood {
            return c
                .validate(self)
                .map_err(RuinRecreateError::MixedNeighborhood);
        }
        self.basic().validate_neighborhood(neighborhood)?;
        self.lists().validate_neighborhood(neighborhood)
    }
    fn fine_moves<'a>(
        &'a self,
        a: &'a Self::Assignment,
        neighborhood: RepairNeighborhood<'a>,
    ) -> Result<crate::ruin_engine::FineMoves<'a, Self::FineMove>, RuinRecreateError> {
        if let RepairNeighborhood::Basic(c) = neighborhood {
            self.validate(a).map_err(|e| {
                RuinRecreateError::MixedNeighborhood(MixedNeighborhoodError::Model(e))
            })?;
            let moves = basic_moves_configured(self.basic(), a.basic(), c)
                .map_err(RuinRecreateError::BasicNeighborhood)?;
            return Ok(crate::ruin_engine::FineMoves::Checked(Box::new(moves.map(
                |basic| {
                    Some(MixedMove {
                        basic,
                        lists: ListMove::Composite(vec![]),
                    })
                },
            ))));
        }
        if let RepairNeighborhood::Mixed(c) = neighborhood {
            let moves =
                mixed_moves_configured(self, a, c).map_err(RuinRecreateError::MixedNeighborhood)?;
            if self.lists().dependencies.is_none() {
                return Ok(crate::ruin_engine::FineMoves::All(Box::new(moves)));
            }
            let checker = self.lists().move_checker(a.lists()).map_err(|e| {
                RuinRecreateError::MixedNeighborhood(MixedNeighborhoodError::Model(
                    MixedError::Lists(e),
                ))
            })?;
            return Ok(crate::ruin_engine::FineMoves::Checked(Box::new(moves.map(
                move |movement| {
                    (!matches!(
                        checker.is_doable(&movement.lists),
                        Err(ListError::DependencyCycle { .. })
                    ))
                    .then_some(movement)
                },
            ))));
        }
        let basic = self
            .basic()
            .fine_moves(a.basic(), neighborhood)?
            .map(|movement| {
                movement.map(|basic| MixedMove {
                    basic,
                    lists: ListMove::Composite(vec![]),
                })
            });
        let lists = self
            .lists()
            .fine_moves(a.lists(), neighborhood)?
            .map(|movement| {
                movement.map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                })
            });
        Ok(crate::ruin_engine::FineMoves::Checked(Box::new(
            basic.chain(lists),
        )))
    }
    fn apply_fine(
        &self,
        a: &Self::Assignment,
        m: &Self::FineMove,
    ) -> Result<Self::Assignment, RuinRecreateError> {
        let mut next = a.clone();
        self.apply(&mut next, m)
            .map_err(|e| MixedLocalSearchError::from(MixedScoringError::from(e)))?
            .commit();
        Ok(next)
    }
    fn metadata(
        &self,
        old: &Self::Assignment,
        new: &Self::Assignment,
    ) -> (Vec<usize>, Self::Movement, Vec<Self::Value>) {
        let (mut entities, basic_move, basic_values) =
            self.basic().metadata(old.basic(), new.basic());
        let (list_entities, list_move, list_values) =
            self.lists().metadata(old.lists(), new.lists());
        entities.extend(
            list_entities
                .into_iter()
                .map(|i| self.basic().population() + i),
        );
        let values = basic_values
            .into_iter()
            .map(MixedPlanningValue::Basic)
            .chain(list_values.into_iter().map(MixedPlanningValue::List))
            .collect();
        (entities, (basic_move, list_move), values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruin_engine::RepairModel;
    #[test]
    fn mixed_repair_identity_namespaces_do_not_collide() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            ListProblem::new(
                vec![0, 1, 2],
                vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
            )
            .unwrap(),
        );
        let old = p.initial_assignment();
        let new = MixedAssignment::new(
            Assignment::new(vec![Some(1)]),
            ListAssignment::new(vec![vec![0, 2], vec![1]]),
        );
        let (entities, movement, values) = p.metadata(&old, &new);
        assert_eq!(p.population(), 3);
        assert_eq!(p.movable(&old), vec![0, 1, 2, 3]);
        assert_eq!(entities, vec![0, 1, 2]);
        assert_eq!(
            values,
            vec![
                MixedPlanningValue::Basic(Some(1)),
                MixedPlanningValue::List(1),
                MixedPlanningValue::List(2)
            ]
        );
        assert_eq!(movement.1, vec![(0, vec![0, 2]), (1, vec![1])]);
        let repair = p.repair(&old, &[0, 2]).unwrap();
        assert_eq!(repair.initial_assignment().basic().values(), &[None]);
        assert_eq!(
            repair.initial_assignment().lists().lists(),
            &[vec![0, 2], vec![]]
        );
    }
}
