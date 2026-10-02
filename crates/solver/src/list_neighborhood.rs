//! Lazy list neighborhoods over one immutable assignment snapshot.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, Neighborhood};

pub(crate) fn positions<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = ListPosition> + 'a {
    order
        .filter(move |&e| !p.entities()[e].is_pinned())
        .flat_map(move |e| {
            (p.entities()[e].pinned_prefix()..a.lists()[e].len())
                .map(move |index| ListPosition { entity: e, index })
        })
}
pub(crate) fn insertions<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = ListPosition> + 'a {
    order
        .filter(move |&e| !p.entities()[e].is_pinned())
        .flat_map(move |e| {
            (p.entities()[e].pinned_prefix()..=a.lists()[e].len())
                .map(move |index| ListPosition { entity: e, index })
        })
}
pub(crate) fn assigns<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    p.values()
        .iter()
        .filter(move |v| a.location(v).is_none())
        .flat_map(move |value| {
            insertions(p, a, order.clone()).map(move |destination| ListMove::Assign {
                value: value.clone(),
                destination,
            })
        })
}
pub(crate) fn changes<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    positions(p, a, order.clone()).flat_map(move |source| {
        order
            .clone()
            .filter(move |&e| !p.entities()[e].is_pinned())
            .flat_map(move |e| {
                let end = a.lists()[e].len() - usize::from(e == source.entity);
                (p.entities()[e].pinned_prefix()..=end).filter_map(move |index| {
                    let destination = ListPosition { entity: e, index };
                    (source != destination).then_some(ListMove::Change {
                        source,
                        destination,
                    })
                })
            })
    })
}
pub(crate) fn unassigns<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    p.allows_unassigned()
        .then(|| positions(p, a, order))
        .into_iter()
        .flatten()
        .map(|position| ListMove::Unassign {
            entity: position.entity,
            start: position.index,
            end: position.index + 1,
        })
}
pub(crate) fn swaps<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    positions(p, a, order.clone()).flat_map(move |left| {
        let assigned = positions(p, a, order.clone())
            .filter(move |&right| right != left)
            .map(move |right| ListMove::Swap { left, right });
        let unassigned = p
            .allows_unassigned()
            .then(|| p.values().iter())
            .into_iter()
            .flatten()
            .filter(move |v| a.location(v).is_none())
            .map(move |value| {
                ListMove::Composite(vec![
                    ListMove::Unassign {
                        entity: left.entity,
                        start: left.index,
                        end: left.index + 1,
                    },
                    ListMove::Assign {
                        value: value.clone(),
                        destination: left,
                    },
                ])
            });
        assigned.chain(unassigned)
    })
}
/// Assign each currently unassigned value at every unpinned insertion position.
/// Valid for required partial construction and optional complete assignments.
pub fn list_assign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(assigns(p, a, 0..p.entities().len()))
}
/// Relocations only, using destination indices after source removal. No no-ops.
pub fn list_change_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(changes(p, a, 0..p.entities().len()))
}
/// Single-element unassignment for optional lists. Required lists yield no moves.
pub fn list_unassign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(unassigns(p, a, 0..p.entities().len()))
}
/// Ordered assigned-position pairs, then optional replacements at each source.
/// Replacements atomically unassign the old value and insert an unassigned one.
pub fn list_swap_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(swaps(p, a, 0..p.entities().len()))
}
/// Relocations, optional assignments/unassignments, then optional swaps. Preserves
/// completeness when supplied a complete assignment; all pins remain unchanged.
pub fn list_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    neighborhood: Neighborhood,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(ordered(p, a, neighborhood, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    neighborhood: Neighborhood,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    let assigned = p
        .allows_unassigned()
        .then(|| assigns(p, a, order.clone()))
        .into_iter()
        .flatten();
    let removed = unassigns(p, a, order.clone());
    let swapped = (neighborhood == Neighborhood::ChangeAndSwap)
        .then(|| swaps(p, a, order.clone()))
        .into_iter()
        .flatten();
    changes(p, a, order)
        .chain(assigned)
        .chain(removed)
        .chain(swapped)
}
