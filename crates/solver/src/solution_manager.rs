//! Explicit refresh of owned solutions without starting a search.
use crate::{
    AdditiveScore, Assignment, ConstraintNetwork, FiniteProblem, ListAssignment, ListBuiltinValues,
    ListConstraintNetwork, ListProblem, ListScoringError, MixedAssignment, MixedConstraintNetwork,
    MixedProblem, MixedScoringError, ScoringError,
};
use std::{error::Error, fmt};

/// Which materialized outputs to refresh. A shadows-only update deliberately
/// retains the stored score, which may therefore become stale.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SolutionUpdatePolicy {
    #[default]
    Everything,
    ShadowsOnly,
    ScoreOnly,
}
/// Caller-owned planning state, materialized shadows and an optional saved score.
/// Editing assignment or shadows does not implicitly refresh either output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedSolution<A, H, S> {
    pub assignment: A,
    pub shadows: H,
    pub score: Option<S>,
}
impl<A, H, S> ManagedSolution<A, H, S> {
    pub fn new(assignment: A, shadows: H) -> Self {
        Self {
            assignment,
            shadows,
            score: None,
        }
    }
}

/// The first incorrect built-in shadow, in owner/index/previous/next order.
/// Neighbor identities are owned values, including values outside the model's
/// universe. Reporting an invalid neighbor does not itself require a lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListShadowMismatch<V> {
    Owner {
        expected: Option<usize>,
        actual: Option<usize>,
    },
    Index {
        expected: Option<usize>,
        actual: Option<usize>,
    },
    Previous {
        expected: Option<V>,
        actual: Option<V>,
    },
    Next {
        expected: Option<V>,
        actual: Option<V>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionRefreshError<E, V = ()> {
    Backend(E),
    ShadowCount {
        expected: usize,
        actual: usize,
    },
    /// Index in the model's stable value universe.
    ShadowMismatch {
        value: usize,
        mismatch: ListShadowMismatch<V>,
    },
}
impl<E: fmt::Debug, V: fmt::Debug> fmt::Display for SolutionRefreshError<E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solution refresh: {self:?}")
    }
}
impl<E: Error + 'static, V: fmt::Debug> Error for SolutionRefreshError<E, V> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Backend(e) => Some(e),
            _ => None,
        }
    }
}
type Refresh<'a, A, H, E> = dyn FnMut(&A) -> Result<H, E> + 'a;
type Validate<'a, A, H, E> = dyn FnMut(&A, &H) -> Result<(), E> + 'a;
type Calculate<'a, A, H, S, E> = dyn FnMut(&A, &H) -> Result<S, E> + 'a;

/// Refresh one owned solution using typed shadow, validation and scoring adapters.
/// Native constructors below bind the existing constraint networks and models;
/// custom domains can supply a shadow evaluator and a score calculator directly.
///
/// Everything prepares shadows, validates them, then calculates the score.
/// ShadowsOnly prepares/validates shadows without invoking the score calculator.
/// ScoreOnly validates the existing shadows and calculates without invoking the
/// shadow evaluator. All requested outputs are published together after success;
/// errors and ordinary callback panics leave the solution unchanged. No Clone
/// bound is needed on assignments, shadows or scores. Callbacks must not mutate
/// the supplied assignment/shadows through shared interior state; their external
/// side effects and captured caches are not rolled back.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![1,2]).with_initial(1)])?;
/// let n=ConstraintNetwork::<i32,SimpleScore>::new(1);
/// let mut manager=basic_solution_manager(&p,&n);
/// let mut solution=ManagedSolution::new(p.initial_assignment(),());
/// assert_eq!(manager.update(&mut solution)?,Some(&SimpleScore::ZERO));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct SolutionManager<'a, A, H, S, E> {
    refresh: Box<Refresh<'a, A, H, E>>,
    validate: Box<Validate<'a, A, H, E>>,
    calculate: Box<Calculate<'a, A, H, S, E>>,
}
impl<'a, A, H, S, E> SolutionManager<'a, A, H, S, E> {
    pub fn new(
        refresh: impl FnMut(&A) -> Result<H, E> + 'a,
        validate: impl FnMut(&A, &H) -> Result<(), E> + 'a,
        calculate: impl FnMut(&A, &H) -> Result<S, E> + 'a,
    ) -> Self {
        Self {
            refresh: Box::new(refresh),
            validate: Box::new(validate),
            calculate: Box::new(calculate),
        }
    }
    pub fn update<'s>(
        &mut self,
        solution: &'s mut ManagedSolution<A, H, S>,
    ) -> Result<Option<&'s S>, E> {
        self.update_with_policy(solution, SolutionUpdatePolicy::Everything)
    }
    pub fn update_with_policy<'s>(
        &mut self,
        solution: &'s mut ManagedSolution<A, H, S>,
        policy: SolutionUpdatePolicy,
    ) -> Result<Option<&'s S>, E> {
        let next_shadows = if policy == SolutionUpdatePolicy::ScoreOnly {
            None
        } else {
            Some((self.refresh)(&solution.assignment)?)
        };
        let shadows = next_shadows.as_ref().unwrap_or(&solution.shadows);
        (self.validate)(&solution.assignment, shadows)?;
        let next_score = if policy == SolutionUpdatePolicy::ShadowsOnly {
            None
        } else {
            Some((self.calculate)(&solution.assignment, shadows)?)
        };
        // Prepare both outputs before replacing either. Retain displaced objects
        // until publication is complete, including if their destructors panic.
        let old_shadows = next_shadows.map(|h| std::mem::replace(&mut solution.shadows, h));
        let old_score = next_score.and_then(|s| solution.score.replace(s));
        drop((old_shadows, old_score));
        Ok(solution.score.as_ref())
    }
}

pub type BasicSolutionManager<'a, V, S> = SolutionManager<'a, Assignment<V>, (), S, ScoringError>;
/// Full native basic scoring, including required-variable structural penalties.
/// Custom basic shadows can use SolutionManager::new with an explicit H type.
pub fn basic_solution_manager<'a, V: Eq + Clone + 'a, S: AdditiveScore + 'a>(
    problem: &'a FiniteProblem<V>,
    network: &'a ConstraintNetwork<'_, V, S>,
) -> BasicSolutionManager<'a, V, S> {
    SolutionManager::new(
        |_| Ok(()),
        move |a, _| problem.validate(a).map_err(ScoringError::from),
        move |a, _| network.calculate(problem, a),
    )
}

/// Built-in list shadows in the model's stable value-universe order. Unassigned
/// values have all four fields None; an empty vector is an uninitialized snapshot.
pub type ListSolutionShadows<V> = Vec<ListBuiltinValues<usize, V>>;
pub type ListSolutionManager<'a, V, S> = SolutionManager<
    'a,
    ListAssignment<V>,
    ListSolutionShadows<V>,
    S,
    SolutionRefreshError<ListScoringError, V>,
>;
pub type MixedSolutionManager<'a, B, V, S> = SolutionManager<
    'a,
    MixedAssignment<B, V>,
    ListSolutionShadows<V>,
    S,
    SolutionRefreshError<MixedScoringError, V>,
>;

fn project<V: Eq + Clone>(
    problem: &ListProblem<V>,
    a: &ListAssignment<V>,
) -> ListSolutionShadows<V> {
    problem
        .values()
        .iter()
        .map(|v| {
            a.element(v)
                .map_or_else(ListBuiltinValues::default, |at| ListBuiltinValues {
                    owner: Some(at.position.entity),
                    index: Some(at.position.index),
                    previous: at.previous.cloned(),
                    next: at.next.cloned(),
                })
        })
        .collect()
}
fn check_projection<V: Eq + Clone, E>(
    problem: &ListProblem<V>,
    a: &ListAssignment<V>,
    shadows: &[ListBuiltinValues<usize, V>],
) -> Result<(), SolutionRefreshError<E, V>> {
    if shadows.len() != problem.values().len() {
        return Err(SolutionRefreshError::ShadowCount {
            expected: problem.values().len(),
            actual: shadows.len(),
        });
    }
    for (value, (expected, actual)) in project(problem, a).iter().zip(shadows).enumerate() {
        let mismatch = if expected.owner != actual.owner {
            Some(ListShadowMismatch::Owner {
                expected: expected.owner,
                actual: actual.owner,
            })
        } else if expected.index != actual.index {
            Some(ListShadowMismatch::Index {
                expected: expected.index,
                actual: actual.index,
            })
        } else if expected.previous != actual.previous {
            Some(ListShadowMismatch::Previous {
                expected: expected.previous.clone(),
                actual: actual.previous.clone(),
            })
        } else if expected.next != actual.next {
            Some(ListShadowMismatch::Next {
                expected: expected.next.clone(),
                actual: actual.next.clone(),
            })
        } else {
            None
        };
        if let Some(mismatch) = mismatch {
            return Err(SolutionRefreshError::ShadowMismatch { value, mismatch });
        }
    }
    Ok(())
}
/// Refresh owner/index/neighbor shadows and use the existing list constraint
/// network. ScoreOnly rejects stale structural shadows instead of silently
/// repairing them. Errors identify the value's universe index and the first
/// incorrect field with its expected and actual contents. Custom cascading
/// shadows require an explicit adapter.
pub fn list_solution_manager<'a, V: Eq + Clone + 'a, S: AdditiveScore + 'a>(
    problem: &'a ListProblem<V>,
    network: &'a ListConstraintNetwork<'_, V, S>,
) -> ListSolutionManager<'a, V, S> {
    SolutionManager::new(
        move |a| {
            problem
                .validate(a)
                .map_err(|e| SolutionRefreshError::Backend(ListScoringError::from(e)))?;
            Ok(project(problem, a))
        },
        move |a, h| {
            problem
                .validate(a)
                .map_err(|e| SolutionRefreshError::Backend(ListScoringError::from(e)))?;
            check_projection(problem, a, h)
        },
        move |a, _| {
            network
                .calculate(problem, a)
                .map_err(SolutionRefreshError::Backend)
        },
    )
}
/// Mixed counterpart: validates both components, refreshes structural list
/// shadows, and calculates the mixed network's complete structural/ordinary score.
pub fn mixed_solution_manager<'a, B: Eq + Clone + 'a, V: Eq + Clone + 'a, S: AdditiveScore + 'a>(
    problem: &'a MixedProblem<B, V>,
    network: &'a MixedConstraintNetwork<'_, B, V, S>,
) -> MixedSolutionManager<'a, B, V, S> {
    SolutionManager::new(
        move |a| {
            problem
                .validate(a)
                .map_err(|e| SolutionRefreshError::Backend(MixedScoringError::from(e)))?;
            Ok(project(problem.lists(), a.lists()))
        },
        move |a, h| {
            problem
                .validate(a)
                .map_err(|e| SolutionRefreshError::Backend(MixedScoringError::from(e)))?;
            check_projection(problem.lists(), a.lists(), h)
        },
        move |a, _| {
            network
                .calculate(problem, a)
                .map_err(SolutionRefreshError::Backend)
        },
    )
}
