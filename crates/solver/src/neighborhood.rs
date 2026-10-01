//! Deterministic, lazy basic-variable move enumeration.
use crate::{Assignment, FiniteProblem, ModelError, Move, Variable};

/// Enumerate each non-pinned variable's domain in order, skipping its current
/// value. Optional variables also allow a change to `None`, last in their range.
pub fn change_moves<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    problem.validate(assignment)?;
    Ok(problem
        .variables()
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_pinned())
        .flat_map(move |(i, variable)| {
            variable
                .domain()
                .iter()
                .map(Some)
                .chain(variable.allows_unassigned().then_some(None))
                .filter(move |value| *value != assignment.values()[i].as_ref())
                .map(move |value| Move::Change {
                    variable: i,
                    value: value.cloned(),
                })
        }))
}
fn accepts<V: Eq>(variable: &Variable<V>, value: &Option<V>) -> bool {
    match value {
        Some(v) => variable.domain().contains(v),
        None => variable.allows_unassigned(),
    }
}
/// Enumerate ordered pairs of unpinned variables with distinct current values
/// and mutually compatible domains. Both `(a,b)` and `(b,a)` are emitted.
/// An unassigned value may move only into an optional variable.
pub fn swap_moves<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    problem.validate(assignment)?;
    Ok(problem
        .variables()
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.is_pinned())
        .flat_map(move |(i, a)| {
            problem
                .variables()
                .iter()
                .enumerate()
                .filter_map(move |(j, b)| {
                    (i != j
                        && !b.is_pinned()
                        && assignment.values()[i] != assignment.values()[j]
                        && accepts(a, &assignment.values()[j])
                        && accepts(b, &assignment.values()[i]))
                    .then_some(Move::Swap { left: i, right: j })
                })
        }))
}
