//! Checked preparation of compound basic-variable edits on an isolated draft.
use crate::{Assignment, FiniteProblem, ModelError, Move, MoveError};
use std::{collections::BTreeSet, error::Error, fmt};

/// Keep child order and nesting, returning a sole child directly. Empty input
/// becomes an empty Composite. Empty children inside larger composites remain
/// present; this constructor does not apply moves or decide their doability.
pub fn compose_basic_moves<V>(moves: impl IntoIterator<Item = Move<V>>) -> Move<V> {
    let mut moves: Vec<_> = moves.into_iter().collect();
    if moves.len() == 1 {
        moves.pop().unwrap()
    } else {
        Move::Composite(moves)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicMoveBuildError {
    Model(ModelError),
    DuplicateCycleVariable(usize),
}
impl From<ModelError> for BasicMoveBuildError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}
impl fmt::Display for BasicMoveBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(f),
            Self::DuplicateCycleVariable(index) => {
                write!(f, "variable {index} occurs twice in a cycle")
            }
        }
    }
}
impl Error for BasicMoveBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}
/// Build interdependent custom moves against a private, validated draft. Each
/// operation sees earlier edits and validates domains/pins on its final state.
/// A failed operation preserves the draft and all earlier steps. push accepts a
/// nested Composite for operations requiring temporarily invalid intermediate
/// values. Required slots may be cleared; completeness remains a separate check.
///
/// Cloning forks the draft. finish returns an ordinary native Move usable by
/// every basic scoring backend or as the basic component of a mixed move. Replay
/// it against the same starting assignment: there is no implicit rebasing or
/// stale-state detection. Preparation copies the assignment once, and cycles
/// clone their selected values. This is a custom-move API, not constant-time editing.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![
///     Variable::new(vec![1,2,3]).with_initial(1),
///     Variable::new(vec![1,2,3]).with_initial(2),
///     Variable::new(vec![1,2,3]).with_initial(3)])?;
/// let mut assignment = p.initial_assignment();
/// let mut draft = BasicMoveBuilder::new(&p, &assignment)?;
/// draft.cycle(&[0,1,2])?;
/// draft.swap(0,1)?;
/// assert_eq!(draft.assignment().values(), &[Some(1),Some(3),Some(2)]);
/// p.apply(&mut assignment, &draft.finish())?.commit();
/// assert_eq!(assignment.values(), &[Some(1),Some(3),Some(2)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct BasicMoveBuilder<'p, V> {
    problem: &'p FiniteProblem<V>,
    draft: Assignment<V>,
    steps: Vec<Move<V>>,
}
impl<'p, V: Eq + Clone> BasicMoveBuilder<'p, V> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<Self, BasicMoveBuildError> {
        problem.validate(assignment)?;
        Ok(Self {
            problem,
            draft: assignment.clone(),
            steps: vec![],
        })
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.draft
    }
    pub fn steps(&self) -> &[Move<V>] {
        &self.steps
    }
    /// Preserve the supplied move shape; execute it on the draft atomically.
    pub fn push(&mut self, movement: Move<V>) -> Result<(), BasicMoveBuildError> {
        self.steps.reserve(1);
        // new validates the draft, successful edits preserve it, and no mutable
        // assignment reference escapes. Only touched slots require revalidation.
        self.problem
            .apply_validated(&mut self.draft, &movement)?
            .commit();
        self.steps.push(movement);
        Ok(())
    }
    pub fn change(&mut self, variable: usize, value: Option<V>) -> Result<(), BasicMoveBuildError> {
        self.push(Move::Change { variable, value })
    }
    pub fn swap(&mut self, left: usize, right: usize) -> Result<(), BasicMoveBuildError> {
        self.push(Move::Swap { left, right })
    }
    /// Apply writes in input order, validating only the final state of this
    /// operation. Repeated indices retain their intermediate writes and last value.
    pub fn change_many(
        &mut self,
        writes: impl IntoIterator<Item = (usize, Option<V>)>,
    ) -> Result<(), BasicMoveBuildError> {
        self.push(compose_basic_moves(
            writes
                .into_iter()
                .map(|(variable, value)| Move::Change { variable, value }),
        ))
    }
    /// Rotate values one position to the right in the supplied variable order.
    /// Read all values from the current draft before applying any write. Repeated
    /// indices are rejected; empty/singleton cycles are validated no-ops.
    pub fn cycle(&mut self, variables: &[usize]) -> Result<(), BasicMoveBuildError> {
        let mut seen = BTreeSet::new();
        let mut values = Vec::with_capacity(variables.len());
        for &variable in variables {
            if !seen.insert(variable) {
                return Err(BasicMoveBuildError::DuplicateCycleVariable(variable));
            }
            values.push(
                self.draft
                    .get(variable)
                    .ok_or(ModelError::Move(MoveError {
                        variable,
                        variable_count: self.draft.values().len(),
                    }))?
                    .clone(),
            );
        }
        if !values.is_empty() {
            values.rotate_right(1);
        }
        self.change_many(variables.iter().copied().zip(values))
    }
    pub fn finish(self) -> Move<V> {
        compose_basic_moves(self.steps)
    }
}
