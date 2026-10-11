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
    change_moves_with_values(problem, assignment, order, &[])
}
pub(crate) fn change_moves_with_values<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: impl Iterator<Item = usize> + 'a,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    order
        .map(move |i| (i, &problem.variables()[i]))
        .filter(|(_, v)| !v.is_pinned())
        .flat_map(move |(i, _)| {
            crate::basic_value_order::targets(problem, i, values, true, true)
                .filter(move |value| *value != assignment.values()[i].as_ref())
                .map(move |value| Move::Change {
                    variable: i,
                    value: value.cloned(),
                })
        })
}
fn accepts<V: Eq>(variable: &Variable<V>, value: &Option<V>) -> bool {
    match value {
        Some(v) => variable.contains_value(v),
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
/// Enumerate each eligible unordered pair once, in variable encounter order.
/// Pins, mutual ranges and optional values follow `swap_moves`. The first
/// orientation is retained; no set of previously emitted pairs is allocated.
pub fn unique_swap_moves<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    problem.validate(assignment)?;
    Ok(swap_pairs(
        problem,
        assignment,
        0..problem.variables().len(),
        true,
    ))
}
pub(crate) fn swap_moves_ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: I,
) -> impl Iterator<Item = Move<V>> + 'a {
    swap_pairs(problem, assignment, order, false)
}
fn swap_pairs<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: I,
    unique: bool,
) -> impl Iterator<Item = Move<V>> + 'a {
    let inner = order.clone();
    order
        .enumerate()
        .map(move |(offset, i)| (offset, i, &problem.variables()[i]))
        .filter(|(_, _, v)| !v.is_pinned())
        .flat_map(move |(offset, i, a)| {
            inner
                .clone()
                .skip(if unique { offset + 1 } else { 0 })
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
    ordered_with_values(problem, assignment, neighborhood, order, &[])
}
pub(crate) fn ordered_with_values<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    neighborhood: crate::Neighborhood,
    order: I,
    values: &'a [crate::BasicValueOrder],
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    match neighborhood {
        crate::Neighborhood::Assign | crate::Neighborhood::Unassign => {
            return Box::new(crate::assignment_neighborhood::single_with_values(
                problem,
                assignment,
                matches!(neighborhood, crate::Neighborhood::Unassign),
                order,
                values,
            ));
        }
        crate::Neighborhood::PillarChange(config) => {
            return Box::new(crate::assignment_neighborhood::pillars_with_values(
                problem, assignment, config, false, order, values,
            ));
        }
        crate::Neighborhood::PillarUnassign => {
            return Box::new(crate::assignment_neighborhood::pillars_with_values(
                problem,
                assignment,
                crate::PillarChangeConfig::default(),
                true,
                order,
                values,
            ));
        }
        _ => {}
    }
    if let crate::Neighborhood::ChangeProduct(selection) = neighborhood {
        return Box::new(crate::change_product::ordered(
            problem, assignment, selection, order,
        ));
    }
    if matches!(
        neighborhood,
        crate::Neighborhood::Swap | crate::Neighborhood::UniqueSwap
    ) {
        return Box::new(swap_pairs(
            problem,
            assignment,
            order,
            neighborhood == crate::Neighborhood::UniqueSwap,
        ));
    }
    if let crate::Neighborhood::MassAssign(size) | crate::Neighborhood::MassUnassign(size) =
        neighborhood
    {
        return Box::new(
            crate::mass_assignment::ordered(
                problem,
                assignment,
                size,
                matches!(neighborhood, crate::Neighborhood::MassUnassign(_)),
                order,
            )
            .map(crate::MassChange::into_move),
        );
    }
    if let crate::Neighborhood::MassChange(config) = neighborhood {
        return Box::new(
            crate::mass_change::ordered(problem, assignment, config, order)
                .map(crate::MassChange::into_move),
        );
    }
    if let crate::Neighborhood::Pillar { swaps } = neighborhood {
        return crate::pillar::ordered_with_values(problem, assignment, swaps, order, values);
    }
    if let crate::Neighborhood::SubPillar(config) = neighborhood {
        return crate::subpillar::ordered_with_values(problem, assignment, config, order, values);
    }
    let swaps = neighborhood
        .includes_swaps()
        .then(|| {
            swap_pairs(
                problem,
                assignment,
                order.clone(),
                neighborhood == crate::Neighborhood::ChangeAndUniqueSwap,
            )
        })
        .into_iter()
        .flatten();
    Box::new(change_moves_with_values(problem, assignment, order, values).chain(swaps))
}
