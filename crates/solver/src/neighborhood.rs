//! Deterministic, lazy basic-variable move enumeration.
use crate::{Assignment, FiniteProblem, ModelError, Move, Variable};

/// Enumerate each non-pinned variable's domain in order, skipping its current
/// value. Optional variables also allow a change to `None`, last in their range.
pub fn change_moves<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    problem.validate(assignment)?;
    Ok(change_moves_ordered(
        problem,
        assignment,
        0..problem.variables().len(),
    ))
}
pub(crate) fn change_moves_ordered<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = Move<V>> + 'a {
    order
        .map(move |i| (i, &problem.variables()[i]))
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
        })
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
    Ok(swap_moves_ordered(
        problem,
        assignment,
        0..problem.variables().len(),
    ))
}
pub(crate) fn swap_moves_ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: I,
) -> impl Iterator<Item = Move<V>> + 'a {
    let inner = order.clone();
    order
        .map(move |i| (i, &problem.variables()[i]))
        .filter(|(_, v)| !v.is_pinned())
        .flat_map(move |(i, a)| {
            inner
                .clone()
                .map(move |j| (j, &problem.variables()[j]))
                .filter_map(move |(j, b)| {
                    (i != j
                        && !b.is_pinned()
                        && assignment.values()[i] != assignment.values()[j]
                        && accepts(a, &assignment.values()[j])
                        && accepts(b, &assignment.values()[i]))
                    .then_some(Move::Swap { left: i, right: j })
                })
        })
}

pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    neighborhood: crate::Neighborhood,
    order: I,
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    if let crate::Neighborhood::MassChange(config) = neighborhood {
        return Box::new(
            crate::mass_change::ordered(problem, assignment, config, order)
                .map(crate::MassChange::into_move),
        );
    }
    if let crate::Neighborhood::Pillar { swaps } = neighborhood {
        return crate::pillar::ordered(problem, assignment, swaps, order);
    }
    if let crate::Neighborhood::SubPillar(config) = neighborhood {
        return crate::subpillar::ordered(problem, assignment, config, order);
    }
    let swaps = neighborhood
        .includes_swaps()
        .then(|| swap_moves_ordered(problem, assignment, order.clone()))
        .into_iter()
        .flatten();
    Box::new(change_moves_ordered(problem, assignment, order).chain(swaps))
}
