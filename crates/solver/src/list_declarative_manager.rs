//! Combine built-in list shadows and declarative session outputs atomically.
use crate::solution_manager::{check_projection, project_list_shadows};
use crate::{
    ListAssignment, ListError, ListProblem, ListSolutionShadows, MixedAssignment, MixedError,
    MixedProblem, ShadowSession, ShadowSolutionManager, SolutionRefreshError,
};
use std::{convert::Infallible, error::Error, fmt};

/// One publication unit: structural fields follow the model's value-universe
/// order; `derived` is the application's owned projection of its shadow graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListDerivedShadows<V, H> {
    pub builtin: ListSolutionShadows<V>,
    pub derived: H,
}
#[derive(Debug)]
pub enum ModelShadowError<M, V, E> {
    Model(M),
    Builtin(SolutionRefreshError<Infallible, V>),
    Adapter(E),
}
impl<M: fmt::Display, V: fmt::Debug, E: fmt::Display> fmt::Display for ModelShadowError<M, V, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Builtin(e) => e.fmt(f),
            Self::Adapter(e) => write!(f, "declarative solution adapter: {e}"),
        }
    }
}
impl<M: Error + 'static, V: fmt::Debug + 'static, E: Error + 'static> Error
    for ModelShadowError<M, V, E>
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Builtin(e) => Some(e),
            Self::Adapter(e) => Some(e),
        }
    }
}

pub type ListDeclarativeSolutionManager<'a, 's, K, V, H, S, E> = ShadowSolutionManager<
    'a,
    's,
    K,
    ListAssignment<V>,
    ListDerivedShadows<V, H>,
    S,
    ModelShadowError<ListError, V, E>,
>;
pub type MixedDeclarativeSolutionManager<'a, 's, K, B, V, H, S, E> = ShadowSolutionManager<
    'a,
    's,
    K,
    MixedAssignment<B, V>,
    ListDerivedShadows<V, H>,
    S,
    ModelShadowError<MixedError, V, E>,
>;

/// Bind list validation and built-in projection to a declarative session.
/// Graph readers use the current assignment; callers register dependencies and
/// capture immutable domain facts when building the session. No shared registry
/// or field setter is required. Invalid assignments are rejected before graph
/// callbacks. Missing sources and, by default, inconsistent graphs are rejected
/// atomically. The returned manager can opt into AllowInconsistent for scoring
/// projections that explicitly retain and penalize inconsistency flags.
///
/// Everything/ShadowsOnly materialize both built-in and custom shadows. ScoreOnly
/// requires current built-in shadows and runs the custom validator against the
/// existing derived projection, without evaluating the graph. The validator must
/// enforce any domain-specific requirements on that projection. Scoring receives
/// both kinds of shadows. All publication and rollback rules of
/// [`ShadowSolutionManager`] apply, including preservation of the old score for
/// ShadowsOnly and exclusion of external callback side effects.
pub fn list_declarative_solution_manager<'a, 's, K: Eq + Clone, V: Eq + Clone + 'a, H, S, E>(
    problem: &'a ListProblem<V>,
    session: ShadowSession<'a, 's, K, ListAssignment<V>>,
    mut derive: impl FnMut(
        &ListAssignment<V>,
        &ShadowSession<'a, 's, K, ListAssignment<V>>,
    ) -> Result<H, E>
    + 'a,
    mut validate: impl FnMut(&ListAssignment<V>, &ListDerivedShadows<V, H>) -> Result<(), E> + 'a,
    mut calculate: impl FnMut(&ListAssignment<V>, &ListDerivedShadows<V, H>) -> Result<S, E> + 'a,
) -> ListDeclarativeSolutionManager<'a, 's, K, V, H, S, E> {
    ShadowSolutionManager::new(
        session,
        move |assignment, graph| {
            let derived = derive(assignment, graph).map_err(ModelShadowError::Adapter)?;
            Ok(ListDerivedShadows {
                builtin: project_list_shadows(problem, assignment),
                derived,
            })
        },
        move |assignment, shadows| {
            check_projection(problem, assignment, &shadows.builtin)
                .map_err(ModelShadowError::Builtin)?;
            validate(assignment, shadows).map_err(ModelShadowError::Adapter)
        },
        move |assignment, shadows| {
            calculate(assignment, shadows).map_err(ModelShadowError::Adapter)
        },
    )
    .with_preflight(move |assignment| {
        problem
            .validate(assignment)
            .map_err(ModelShadowError::Model)
    })
}

/// Mixed-model counterpart, validating basic and list assignments before any
/// callback. Session readers and adapters can use either assignment component.
pub fn mixed_declarative_solution_manager<
    'a,
    's,
    K: Eq + Clone,
    B: Eq + Clone + 'a,
    V: Eq + Clone + 'a,
    H,
    S,
    E,
>(
    problem: &'a MixedProblem<B, V>,
    session: ShadowSession<'a, 's, K, MixedAssignment<B, V>>,
    mut derive: impl FnMut(
        &MixedAssignment<B, V>,
        &ShadowSession<'a, 's, K, MixedAssignment<B, V>>,
    ) -> Result<H, E>
    + 'a,
    mut validate: impl FnMut(&MixedAssignment<B, V>, &ListDerivedShadows<V, H>) -> Result<(), E> + 'a,
    mut calculate: impl FnMut(&MixedAssignment<B, V>, &ListDerivedShadows<V, H>) -> Result<S, E> + 'a,
) -> MixedDeclarativeSolutionManager<'a, 's, K, B, V, H, S, E> {
    ShadowSolutionManager::new(
        session,
        move |assignment, graph| {
            let derived = derive(assignment, graph).map_err(ModelShadowError::Adapter)?;
            Ok(ListDerivedShadows {
                builtin: project_list_shadows(problem.lists(), assignment.lists()),
                derived,
            })
        },
        move |assignment, shadows| {
            check_projection(problem.lists(), assignment.lists(), &shadows.builtin)
                .map_err(ModelShadowError::Builtin)?;
            validate(assignment, shadows).map_err(ModelShadowError::Adapter)
        },
        move |assignment, shadows| {
            calculate(assignment, shadows).map_err(ModelShadowError::Adapter)
        },
    )
    .with_preflight(move |assignment| {
        problem
            .validate(assignment)
            .map_err(ModelShadowError::Model)
    })
}
