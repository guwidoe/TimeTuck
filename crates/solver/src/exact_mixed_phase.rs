//! Explicit grouped basic and list-owner exact search sharing a finite phase budget.
use crate::*;
use std::{error::Error, fmt};
/// Explicit selector structure: ordered basic entity rows, ordered list owners and
/// registry values. Several independently seeded list roots are a caller policy,
/// not inferred from expected traces or implemented as repeated solver runs.
#[derive(Clone, Debug)]
pub struct ExactMixedPhasePlan {
    pub basic_groups: Vec<Vec<usize>>,
    pub entity_order: Vec<usize>,
    pub value_order: Vec<usize>,
    pub list_roots: usize,
}
#[derive(Debug)]
pub enum ExactMixedModelError<E> {
    InvalidPlan,
    ScoreChangedDuringFinalization,
    Model(MixedError),
    Scoring(E),
}
impl<E: fmt::Debug> fmt::Display for ExactMixedModelError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mixed exact phase: {self:?}")
    }
}
impl<E: fmt::Debug> Error for ExactMixedModelError<E> {}
#[derive(Clone, Debug)]
pub struct ExactMixedPhaseResult<B, L, S> {
    pub best: ExactPhaseBest<MixedAssignment<B, L>, S>,
    pub working: MixedAssignment<B, L>,
    pub working_score: S,
    pub basic_steps: usize,
    pub list_steps: usize,
    pub generated: usize,
}
fn permutation(order: &[usize], n: usize) -> bool {
    let mut copy = order.to_vec();
    copy.sort_unstable();
    copy == (0..n).collect::<Vec<_>>()
}
impl ExactMixedPhasePlan {
    pub fn validate<B: Eq + Clone, L: Eq + Clone>(&self, p: &MixedProblem<B, L>) -> bool {
        if self.list_roots == 0
            || !permutation(&self.entity_order, p.lists().entities().len())
            || !permutation(&self.value_order, p.lists().values().len())
        {
            return false;
        }
        let slots: Vec<_> = self.basic_groups.iter().flatten().copied().collect();
        permutation(&slots, p.basic().variables().len())
            && !self.basic_groups.iter().any(Vec::is_empty)
    }
}
enum Stage {
    Basic,
    List,
}
struct Model<'a, B, L, S, F, U, E> {
    problem: &'a MixedProblem<B, L>,
    plan: &'a ExactMixedPhasePlan,
    start: MixedAssignment<B, L>,
    groups: Vec<Vec<usize>>,
    stage: Stage,
    score: &'a mut F,
    bound: &'a mut U,
    first_restore: bool,
    last_working_score: Option<S>,
    marker: std::marker::PhantomData<(S, E)>,
}
impl<
    B: Eq + Clone,
    L: Eq + Clone,
    S: Score,
    F: FnMut(&MixedAssignment<B, L>) -> Result<S, E>,
    U: FnMut(&MixedAssignment<B, L>, &S) -> Option<S>,
    E: fmt::Debug,
> ExactPhaseModel for Model<'_, B, L, S, F, U, E>
{
    type Assignment = MixedAssignment<B, L>;
    type Move = MixedMove<B, L>;
    type Score = S;
    type Error = ExactMixedModelError<E>;
    fn initial(&mut self) -> Result<Self::Assignment, Self::Error> {
        Ok(self.start.clone())
    }
    fn max_depth(&self) -> usize {
        match self.stage {
            Stage::Basic => self.groups.len(),
            Stage::List => self.plan.entity_order.len(),
        }
    }
    fn moves(
        &mut self,
        depth: usize,
        a: &Self::Assignment,
    ) -> Result<Vec<Self::Move>, Self::Error> {
        let mut moves = vec![];
        match self.stage {
            Stage::Basic => {
                let Some(group) = self.groups.get(depth) else {
                    return Ok(moves);
                };
                let mut products = vec![vec![]];
                for &slot in group {
                    let variable = &self.problem.basic().variables()[slot];
                    if variable.is_pinned() || a.basic().values()[slot].is_some() {
                        continue;
                    }
                    let mut next = vec![];
                    for prefix in products {
                        for value in variable.domain() {
                            let mut children = prefix.clone();
                            children.push(Move::Change {
                                variable: slot,
                                value: Some(value.clone()),
                            });
                            next.push(children);
                        }
                    }
                    products = next;
                }
                for children in products {
                    moves.push(MixedMove {
                        basic: Move::Composite(children),
                        lists: ListMove::Composite(vec![]),
                    });
                }
            }
            Stage::List => {
                for &entity in self.plan.entity_order.iter().skip(depth) {
                    if self.problem.lists().entities()[entity].is_pinned() {
                        continue;
                    }
                    for &index in &self.plan.value_order {
                        let value = &self.problem.lists().values()[index];
                        if a.lists().location(value).is_some() {
                            continue;
                        }
                        let movement = MixedMove {
                            basic: Move::Composite(vec![]),
                            lists: ListMove::Assign {
                                value: value.clone(),
                                destination: ListPosition {
                                    entity,
                                    index: a.lists().lists()[entity].len(),
                                },
                            },
                        };
                        let mut trial = a.clone();
                        if self.problem.apply(&mut trial, &movement).is_ok() {
                            moves.push(movement);
                        }
                    }
                }
            }
        }
        Ok(moves)
    }
    fn apply(&mut self, a: &mut Self::Assignment, m: &Self::Move) -> Result<(), Self::Error> {
        self.problem
            .apply(a, m)
            .map_err(ExactMixedModelError::Model)?
            .commit();
        Ok(())
    }
    fn inverse(&self, before: &Self::Assignment, after: &Self::Assignment) -> Self::Move {
        crate::temporary_moves::inverse_mixed(before, after)
    }
    fn child_depth(&self, parent: usize, m: &Self::Move, _: &Self::Assignment) -> usize {
        match self.stage {
            Stage::Basic => parent + 1,
            Stage::List => match &m.lists {
                ListMove::Assign { destination, .. } => self
                    .plan
                    .entity_order
                    .iter()
                    .position(|&i| i == destination.entity)
                    .expect("generated owner"),
                _ => unreachable!("list assignment"),
            },
        }
    }
    fn is_terminal(&self, depth: usize, a: &Self::Assignment) -> bool {
        match self.stage {
            Stage::Basic => depth >= self.groups.len(),
            Stage::List => self
                .problem
                .lists()
                .values()
                .iter()
                .all(|v| a.lists().location(v).is_some()),
        }
    }
    fn score(&mut self, a: &Self::Assignment) -> Result<S, Self::Error> {
        (self.score)(a).map_err(ExactMixedModelError::Scoring)
    }
    fn optimistic_bound(
        &mut self,
        _: usize,
        a: &Self::Assignment,
        s: &S,
    ) -> Result<Option<S>, Self::Error> {
        Ok((self.bound)(a, s))
    }
    fn restore_mode(&self) -> BranchRestoreMode {
        match self.stage {
            Stage::Basic => BranchRestoreMode::CommonAncestor,
            Stage::List => BranchRestoreMode::RootReplay,
        }
    }
    fn expanded(
        &mut self,
        _: usize,
        _: usize,
        _: usize,
        _: &Self::Assignment,
        score: &Self::Score,
    ) -> Result<(), Self::Error> {
        self.last_working_score = Some(score.clone());
        Ok(())
    }
    fn restored(&mut self, a: &Self::Assignment) -> Result<(), Self::Error> {
        if matches!(self.stage, Stage::List) && self.first_restore {
            self.score(a)?;
        }
        self.first_restore = true;
        Ok(())
    }
}
pub type ExactMixedPhaseOutcome<B, L, S, E> =
    Result<ExactMixedPhaseResult<B, L, S>, ExactPhaseError<ExactMixedModelError<E>>>;
/// Bootstrap once, retain legitimate assignment-bound score caches for brute
/// force, and refresh branch-and-bound phase roots. Basic rows commit simultaneous
/// Cartesian field choices through real mixed transactions. List owner layers
/// permit same-owner appends and only move onward in the declared owner order.
/// The selected basic witness stays fixed during list search; joint optimality
/// requires an objective for which that staged choice is valid.
/// Every restored list node is genuinely rescored; declared roots share one
/// frontier, incumbent and remaining step budget. Final working-state validation
/// and publication each invoke the supplied calculator, while returning retained
/// strict-best witness separately. Callers must provide valid bounds; an incorrect
/// ONLY_DOWN declaration can prune improving states. Existing defaults unchanged.
pub fn run_exact_mixed_phase<B: Eq + Clone, L: Eq + Clone, S: Score, F, U, E: fmt::Debug>(
    problem: &MixedProblem<B, L>,
    plan: &ExactMixedPhasePlan,
    config: ExactMovePhaseConfig,
    mut score: F,
    mut bound: U,
) -> ExactMixedPhaseOutcome<B, L, S, E>
where
    F: FnMut(&MixedAssignment<B, L>) -> Result<S, E>,
    U: FnMut(&MixedAssignment<B, L>, &S) -> Option<S>,
{
    config.validate().map_err(ExactPhaseError::Configuration)?;
    if !plan.validate(problem) {
        return Err(ExactPhaseError::Model(ExactMixedModelError::InvalidPlan));
    }
    let initial = problem.initial_assignment();
    let bootstrap =
        score(&initial).map_err(|e| ExactPhaseError::Model(ExactMixedModelError::Scoring(e)))?;
    let groups: Vec<_> = plan
        .basic_groups
        .iter()
        .filter(|group| {
            group.iter().any(|&i| {
                !problem.basic().variables()[i].is_pinned() && initial.basic().values()[i].is_none()
            })
        })
        .cloned()
        .collect();
    let mut basic_model = Model {
        problem,
        plan,
        start: initial.clone(),
        groups: groups.clone(),
        stage: Stage::Basic,
        score: &mut score,
        bound: &mut bound,
        first_restore: false,
        last_working_score: None,
        marker: std::marker::PhantomData,
    };
    let cache = (config.kind == ExactPhaseKind::BruteForce).then(|| ExactPhaseScoreCache {
        assignment: initial,
        score: bootstrap,
    });
    let basic = run_seeded_exact_phase(&mut basic_model, config, cache, 1, |_, _| {})?;
    let basic_working_score = basic_model
        .last_working_score
        .take()
        .unwrap_or_else(|| basic.best.score.clone());
    drop(basic_model);
    let start = basic.best.assignment.clone();
    let remaining = config.max_steps.saturating_sub(basic.steps);
    if remaining == 0 {
        return Ok(ExactMixedPhaseResult {
            best: basic.best.clone(),
            working: basic.working,
            working_score: basic_working_score,
            basic_steps: basic.steps,
            list_steps: 0,
            generated: basic.generated,
        });
    }
    let cached = if config.kind == ExactPhaseKind::BruteForce {
        Some(ExactPhaseScoreCache {
            assignment: start.clone(),
            score: basic.best.score.clone(),
        })
    } else {
        score(&start).map_err(|e| ExactPhaseError::Model(ExactMixedModelError::Scoring(e)))?;
        None
    };
    let mut list_model = Model {
        problem,
        plan,
        start,
        groups,
        stage: Stage::List,
        score: &mut score,
        bound: &mut bound,
        first_restore: false,
        last_working_score: None,
        marker: std::marker::PhantomData,
    };
    let list = run_seeded_exact_phase(
        &mut list_model,
        ExactMovePhaseConfig {
            max_steps: remaining,
            ..config
        },
        cached,
        plan.list_roots,
        |_, _| {},
    )?;
    let cached_working_score = list_model.last_working_score.take();
    drop(list_model);
    let validated = score(&list.working)
        .map_err(|e| ExactPhaseError::Model(ExactMixedModelError::Scoring(e)))?;
    if cached_working_score.is_some_and(|expected| expected != validated) {
        return Err(ExactPhaseError::Model(
            ExactMixedModelError::ScoreChangedDuringFinalization,
        ));
    }
    let published = score(&list.working)
        .map_err(|e| ExactPhaseError::Model(ExactMixedModelError::Scoring(e)))?;
    if validated != published {
        return Err(ExactPhaseError::Model(
            ExactMixedModelError::ScoreChangedDuringFinalization,
        ));
    }
    Ok(ExactMixedPhaseResult {
        best: list.best,
        working: list.working,
        working_score: published,
        basic_steps: basic.steps,
        list_steps: list.steps,
        generated: basic.generated + list.generated,
    })
}
