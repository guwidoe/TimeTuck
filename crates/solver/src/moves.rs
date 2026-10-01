//! Reversible edits to basic planning variables.
//!
//! An applied move is a transaction: dropping it rolls back all writes in reverse
//! order. Call [`AppliedMove::commit`] to keep the edits. The immutable view during
//! a transaction prevents accidental edits that bypass its undo journal.

use std::{error::Error, fmt, ops::Deref};

/// Values for a fixed sequence of planning variables. `None` means unassigned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Assignment<V> {
    pub(crate) values: Vec<Option<V>>,
}

impl<V> Assignment<V> {
    pub fn new(values: Vec<Option<V>>) -> Self {
        Self { values }
    }
    pub fn values(&self) -> &[Option<V>] {
        &self.values
    }
    pub fn get(&self, variable: usize) -> Option<&Option<V>> {
        self.values.get(variable)
    }
}

/// A basic-variable move. Composition executes in the specified order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Move<V> {
    Change { variable: usize, value: Option<V> },
    Swap { left: usize, right: usize },
    Composite(Vec<Self>),
}

impl<V> Move<V> {
    /// Sorted unique variable indices written by this move, including no-op
    /// writes. This is metadata only; applying the move still validates indices.
    pub fn touched_variables(&self) -> Vec<usize> {
        let mut pending = vec![self];
        let mut variables = Vec::new();
        while let Some(movement) = pending.pop() {
            match movement {
                Self::Change { variable, .. } => variables.push(*variable),
                Self::Swap { left, right } => variables.extend([*left, *right]),
                Self::Composite(moves) => pending.extend(moves),
            }
        }
        variables.sort_unstable();
        variables.dedup();
        variables
    }
}

/// A move referenced a variable that is not present in the assignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoveError {
    pub variable: usize,
    pub variable_count: usize,
}
impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "variable {} is outside an assignment of length {}",
            self.variable, self.variable_count
        )
    }
}
impl Error for MoveError {}

/// One write's original value. Repeated writes to a variable remain distinct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriorValue<V> {
    pub variable: usize,
    pub value: Option<V>,
}

/// A reversible transaction. It exposes only a read-only assignment view.
///
/// Rollback also runs during ordinary panic unwinding. As with all Rust drop
/// guards, `mem::forget`, process abort, or a panicking value destructor can prevent
/// cleanup. Pinned/domain constraints are enforced by `FiniteProblem::apply`, not
/// the raw assignment transaction.
#[must_use = "dropping the transaction immediately rolls it back; call commit to keep changes"]
pub struct AppliedMove<'a, V> {
    assignment: &'a mut Assignment<V>,
    journal: Vec<PriorValue<V>>,
}

impl<V> Deref for AppliedMove<'_, V> {
    type Target = Assignment<V>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}

impl<V> AppliedMove<'_, V> {
    /// Original values in write order, including writes with no net change.
    pub fn prior_values(&self) -> &[PriorValue<V>] {
        &self.journal
    }

    /// Keep the candidate assignment and consume this transaction.
    pub fn commit(mut self) {
        self.journal.clear();
    }

    fn write(&mut self, variable: usize, value: Option<V>) -> Result<(), MoveError> {
        let variable_count = self.assignment.values.len();
        let slot = self.assignment.values.get_mut(variable).ok_or(MoveError {
            variable,
            variable_count,
        })?;
        self.journal.push(PriorValue {
            variable,
            value: std::mem::replace(slot, value),
        });
        Ok(())
    }
}

impl<V> Drop for AppliedMove<'_, V> {
    fn drop(&mut self) {
        for change in self.journal.drain(..).rev() {
            self.assignment.values[change.variable] = change.value;
        }
    }
}

impl<V: Clone> Assignment<V> {
    /// Apply a raw move, with automatic rollback on errors or when the guard drops.
    ///
    /// Indices are checked; no domain or pinning rules are assumed here. Use a
    /// finite problem's `apply` method to enforce those rules as well.
    pub fn apply(&mut self, movement: &Move<V>) -> Result<AppliedMove<'_, V>, MoveError> {
        let mut transaction = AppliedMove {
            assignment: self,
            journal: Vec::new(),
        };
        let mut pending = vec![movement];
        while let Some(step) = pending.pop() {
            match step {
                Move::Change { variable, value } => transaction.write(*variable, value.clone())?,
                Move::Swap { left, right } => {
                    let read = |index| {
                        transaction
                            .assignment
                            .values
                            .get(index)
                            .cloned()
                            .ok_or(MoveError {
                                variable: index,
                                variable_count: transaction.assignment.values.len(),
                            })
                    };
                    let a = read(*left)?;
                    let b = read(*right)?;
                    transaction.write(*left, b)?;
                    transaction.write(*right, a)?;
                }
                Move::Composite(steps) => pending.extend(steps.iter().rev()),
            }
        }
        Ok(transaction)
    }
}
