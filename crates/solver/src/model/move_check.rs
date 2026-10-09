//! Read-only final-state checks for basic planning moves.
use super::{Assignment, FiniteProblem, ModelError, Move, MoveError};
use std::collections::BTreeMap;

impl<V: Eq> FiniteProblem<V> {
    /// Check whether a valid move changes the assignment according to value `Eq`.
    ///
    /// The source is validated first. All move indices are checked in execution
    /// order, then final domains and pins are checked in variable-index order.
    /// Composite children may temporarily violate domains or pins if their final
    /// state restores them, just as with `apply`. Clearing a required variable is
    /// allowed for partial planning states. An empty composite or one which
    /// restores the original values returns false.
    ///
    /// This does not mutate, clone values, calculate scores or invoke move
    /// listeners. Primitive moves need no scratch allocation; composites retain
    /// borrowed values only for written variables. Invalid moves return their
    /// ordinary model error, including configured range diagnostics.
    ///
    /// ```
    /// use timetuck::*;
    /// let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)])?;
    /// let assignment = problem.initial_assignment();
    /// assert!(!problem.is_move_doable(&assignment, &Move::Change { variable: 0, value: Some(1) })?);
    /// assert!(problem.is_move_doable(&assignment, &Move::Change { variable: 0, value: Some(2) })?);
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn is_move_doable(
        &self,
        assignment: &Assignment<V>,
        movement: &Move<V>,
    ) -> Result<bool, ModelError> {
        self.move_checker(assignment)?.is_doable(movement)
    }

    /// Validate an immutable planning snapshot once for repeated candidate checks.
    /// The checker borrows both model and assignment, preventing either from being
    /// mutated while it is in use. It neither caches candidate results nor scores.
    pub fn move_checker<'a>(
        &'a self,
        assignment: &'a Assignment<V>,
    ) -> Result<BasicMoveChecker<'a, V>, ModelError> {
        self.validate(assignment)?;
        Ok(BasicMoveChecker {
            problem: self,
            assignment,
        })
    }
}

/// Read-only candidate checks against one validated basic assignment.
///
/// Obtain this through [`FiniteProblem::move_checker`]. Each query checks indices,
/// final domains and pins only for written variables. The source snapshot remains
/// valid for the borrow's lifetime; no values are cloned and no scores calculated.
/// Unchanged and restoring moves return false; invalid moves return an error.
///
/// ```
/// use timetuck::*;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)])?;
/// let assignment = problem.initial_assignment();
/// let checker = problem.move_checker(&assignment)?;
/// assert!(!checker.is_doable(&Move::Change { variable: 0, value: Some(1) })?);
/// assert!(checker.is_doable(&Move::Change { variable: 0, value: Some(2) })?);
/// # Ok::<(), ModelError>(())
/// ```
pub struct BasicMoveChecker<'a, V> {
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
}

impl<V: Eq> BasicMoveChecker<'_, V> {
    /// Check final change and validity with the same rules as
    /// [`FiniteProblem::is_move_doable`], without revalidating the source snapshot.
    pub fn is_doable(&self, movement: &Move<V>) -> Result<bool, ModelError> {
        let assignment = self.assignment;
        let problem = self.problem;
        let read = |variable| {
            assignment.values().get(variable).ok_or(MoveError {
                variable,
                variable_count: assignment.values().len(),
            })
        };
        match movement {
            Move::Change { variable, value } => {
                let previous = read(*variable)?;
                validate_final_move_value(problem, *variable, value)?;
                return Ok(previous != value);
            }
            Move::Swap { left, right } => {
                let a = read(*left)?;
                let b = read(*right)?;
                if left <= right {
                    validate_final_move_value(problem, *left, b)?;
                    if left != right {
                        validate_final_move_value(problem, *right, a)?;
                    }
                } else {
                    validate_final_move_value(problem, *right, a)?;
                    validate_final_move_value(problem, *left, b)?;
                }
                return Ok(a != b);
            }
            Move::Composite(_) => {}
        }
        let mut values = BTreeMap::<usize, &Option<V>>::new();
        let mut pending = vec![movement];
        while let Some(step) = pending.pop() {
            match step {
                Move::Change { variable, value } => {
                    read(*variable)?;
                    values.insert(*variable, value);
                }
                Move::Swap { left, right } => {
                    let a = values.get(left).copied().map_or_else(|| read(*left), Ok)?;
                    let b = values
                        .get(right)
                        .copied()
                        .map_or_else(|| read(*right), Ok)?;
                    values.insert(*left, b);
                    values.insert(*right, a);
                }
                Move::Composite(children) => pending.extend(children.iter().rev()),
            }
        }
        for (&variable, &value) in &values {
            validate_final_move_value(problem, variable, value)?;
        }
        Ok(values
            .into_iter()
            .any(|(variable, value)| &assignment.values()[variable] != value))
    }
}

fn validate_final_move_value<V: Eq>(
    problem: &FiniteProblem<V>,
    index: usize,
    value: &Option<V>,
) -> Result<(), ModelError> {
    let variable = &problem.variables[index];
    if let Some(value) = value
        && !variable.domain.contains(value)
    {
        return Err(variable.range_error(index, value));
    }
    if variable.pinned && variable.initial != *value {
        return Err(ModelError::PinnedValueChanged { variable: index });
    }
    Ok(())
}
