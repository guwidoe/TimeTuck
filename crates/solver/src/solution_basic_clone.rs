//! Typed application-result cloning from an actual native solve snapshot.
use crate::*;
use std::collections::HashSet;

type CloneBasic<'a, D, V, S> = dyn FnMut(&D, &Assignment<V>, S) -> D + 'a;
type ReadCloneScore<'a, D, S> = dyn Fn(&D) -> Option<S> + 'a;

#[derive(Debug)]
pub enum BasicSolutionCloneError {
    Binding(BasicSolutionAccessError),
    Solve(SolverError<LocalSearchError>),
    Scoring(ScoringError),
    NoScoredResult,
    ScoreTypeMismatch,
    ScoreCorruption(String),
    SharedEntity { clone_index: usize },
    EntityLayoutChanged,
    MissingCloneFactory(PlanningType),
    DuplicateCloneFactory(PlanningType),
    UnknownCloneEntity,
    CloneTypeMismatch,
    AssignmentChanged,
}
impl std::fmt::Display for BasicSolutionCloneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Solve(e) => e.fmt(f),
            Self::Scoring(e) => e.fmt(f),
            Self::ScoreCorruption(message) => f.write_str(message),
            Self::SharedEntity { clone_index } => write!(
                f,
                "Cloning corruption: the same entity is retained at clone index {clone_index}"
            ),
            Self::NoScoredResult => f.write_str("no scored result to clone"),
            Self::ScoreTypeMismatch => {
                f.write_str("clone score type differs from the registered solution score")
            }
            Self::EntityLayoutChanged => f.write_str("cloning changed the bound entity layout"),
            Self::MissingCloneFactory(t) => {
                write!(f, "missing entity clone factory for {}", t.name())
            }
            Self::DuplicateCloneFactory(t) => {
                write!(f, "duplicate entity clone factory for {}", t.name())
            }
            Self::UnknownCloneEntity => {
                f.write_str("entity allocation is not part of this clone map")
            }
            Self::CloneTypeMismatch => f.write_str("entity clone type mismatch"),
            Self::AssignmentChanged => {
                f.write_str("cloning changed the retained planning assignment")
            }
        }
    }
}
impl std::error::Error for BasicSolutionCloneError {}

pub struct ClonedBasicSolution<D, V, S> {
    pub solution: D,
    pub binding: BoundBasicSolution<V>,
    pub native: SolverResult<Assignment<V>, S>,
    /// Additional initial score evaluation used by clone preflight.
    pub clone_score_evaluations: u64,
}

/// Project a retained native assignment into independently owned application
/// entities. Immutable domain facts may stay shared. The callback receives the
/// actual assignment/score, since application input objects stay immutable while
/// the solver works. Rebinding validates all registered fields/ranges, score
/// equality, entity allocation isolation and the exact retained assignment.
///
/// This is an explicit custom clone policy, not reflective automatic deep cloning.
/// The application remains responsible for unregistered fields and backlinks.
/// Getters and clone callbacks execute only at the solve/clone boundary. Errors
/// publish no clone; callback side effects and interior mutations are not undone.
pub struct BasicSolutionCloner<'a, D, V, S> {
    clone: Box<CloneBasic<'a, D, V, S>>,
    score: Box<ReadCloneScore<'a, D, S>>,
}
impl<'a, D: 'static, V: Eq + Clone + 'static, S: AdditiveScore + 'static>
    BasicSolutionCloner<'a, D, V, S>
{
    pub fn new(
        clone: impl FnMut(&D, &Assignment<V>, S) -> D + 'a,
        score: impl Fn(&D) -> Option<S> + 'a,
    ) -> Self {
        Self {
            clone: Box::new(clone),
            score: Box::new(score),
        }
    }

    /// Validate a clone of the actual initially scored input before search, then
    /// run native construction/search and clone its retained result. This detects
    /// corruption even when infeasibility permits search to skip initial scoring.
    /// Preflight scoring/cloning happens outside native phase work/limits. Includes
    /// scored partial results; an unscored native result returns NoScoredResult.
    pub fn solve(
        &mut self,
        accessors: &SolutionBasicAccessors<'_, D, V>,
        source: &D,
        network: &ConstraintNetwork<'_, V, S>,
        config: SolverConfig<'_, S>,
    ) -> Result<ClonedBasicSolution<D, V, S>, BasicSolutionCloneError> {
        if !accessors.layout().members().iter().any(
            |m| matches!(m.kind(), SolutionMemberKind::Score(t) if *t == PlanningType::of::<S>()),
        ) {
            return Err(BasicSolutionCloneError::ScoreTypeMismatch);
        }
        let original = accessors
            .bind(source)
            .map_err(BasicSolutionCloneError::Binding)?;
        let initial = original.problem().initial_assignment();
        let score = ScoreDirector::new(original.problem(), network, initial.clone())
            .map_err(BasicSolutionCloneError::Scoring)?
            .score();
        self.clone_snapshot(accessors, source, &original, &initial, score)?;
        let native =
            solve(original.problem(), network, config).map_err(BasicSolutionCloneError::Solve)?;
        let score = native
            .score
            .clone()
            .ok_or(BasicSolutionCloneError::NoScoredResult)?;
        let (solution, binding) =
            self.clone_snapshot(accessors, source, &original, &native.assignment, score)?;
        Ok(ClonedBasicSolution {
            solution,
            binding,
            native,
            clone_score_evaluations: 1,
        })
    }

    fn clone_snapshot(
        &mut self,
        accessors: &SolutionBasicAccessors<'_, D, V>,
        source: &D,
        original: &BoundBasicSolution<V>,
        assignment: &Assignment<V>,
        score: S,
    ) -> Result<(D, BoundBasicSolution<V>), BasicSolutionCloneError> {
        let solution = (self.clone)(source, assignment, score.clone());
        let cloned_score = (self.score)(&solution);
        if cloned_score.as_ref() != Some(&score) {
            return Err(BasicSolutionCloneError::ScoreCorruption(format!(
                "Cloning corruption: the original's score {score:?} differs from the clone's score {cloned_score:?}"
            )));
        }
        let binding = validate_basic_clone(accessors, original, &solution, assignment)?;
        Ok((solution, binding))
    }
}

pub(crate) fn validate_basic_clone<D: 'static, V: Eq + Clone + 'static>(
    accessors: &SolutionBasicAccessors<'_, D, V>,
    original: &BoundBasicSolution<V>,
    solution: &D,
    assignment: &Assignment<V>,
) -> Result<BoundBasicSolution<V>, BasicSolutionCloneError> {
    let binding = accessors
        .bind(solution)
        .map_err(BasicSolutionCloneError::Binding)?;
    let allocations: HashSet<_> = original
        .entities()
        .iter()
        .map(|e| std::sync::Arc::as_ptr(e.object()) as *const ())
        .collect();
    for (clone_index, e) in binding.entities().iter().enumerate() {
        if allocations.contains(&(std::sync::Arc::as_ptr(e.object()) as *const ())) {
            return Err(BasicSolutionCloneError::SharedEntity { clone_index });
        }
    }
    if original.entities().len() != binding.entities().len()
        || original
            .entities()
            .iter()
            .zip(binding.entities())
            .any(|(a, b)| a.entity_type() != b.entity_type() || a.slots() != b.slots())
        || accessors.layout().members().iter().any(|m| {
            matches!(m.kind(), SolutionMemberKind::Entities { .. })
                && original.collection(m.name()) != binding.collection(m.name())
        })
        || original
            .problem()
            .variables()
            .iter()
            .zip(binding.problem().variables())
            .any(|(a, b)| {
                a.domain() != b.domain()
                    || a.is_pinned() != b.is_pinned()
                    || a.allows_unassigned() != b.allows_unassigned()
            })
    {
        return Err(BasicSolutionCloneError::EntityLayoutChanged);
    }
    if binding.problem().initial_assignment() != *assignment {
        return Err(BasicSolutionCloneError::AssignmentChanged);
    }
    Ok(binding)
}
