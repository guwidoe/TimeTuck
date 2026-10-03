//! Lazy list neighborhoods over one immutable assignment snapshot.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange, Neighborhood,
};

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
            insertions(p, a, order.clone())
                .filter(move |position| p.accepts(position.entity, value))
                .map(move |destination| ListMove::Assign {
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
            .filter(move |&e| {
                !p.entities()[e].is_pinned()
                    && p.accepts(e, &a.lists()[source.entity][source.index])
            })
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
            .filter(move |&right| {
                right != left
                    && p.accepts(left.entity, &a.lists()[right.entity][right.index])
                    && p.accepts(right.entity, &a.lists()[left.entity][left.index])
            })
            .map(move |right| ListMove::Swap { left, right });
        let unassigned = p
            .allows_unassigned()
            .then(|| p.values().iter())
            .into_iter()
            .flatten()
            .filter(move |v| a.location(v).is_none() && p.accepts(left.entity, v))
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
    let swapped = neighborhood
        .includes_swaps()
        .then(|| swaps(p, a, order.clone()))
        .into_iter()
        .flatten();
    let max_length = match neighborhood {
        Neighborhood::ChangeSwapAndSubList { max_length } => max_length,
        _ => 0,
    };
    let spans = sublist_changes(p, a, max_length, order.clone());
    let span_swaps = sublist_swaps(p, a, max_length, order.clone());
    changes(p, a, order)
        .chain(assigned)
        .chain(removed)
        .chain(swapped)
        .chain(spans)
        .chain(span_swaps)
}

fn sublist_changes<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_length: usize,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    // Avoid visiting every position when span generation is disabled.
    (max_length >= 2)
        .then(|| positions(p, a, order.clone()))
        .into_iter()
        .flatten()
        .flat_map(move |source| {
            let last = a.lists()[source.entity]
                .len()
                .min(source.index.saturating_add(max_length));
            let order = order.clone();
            (source.index.saturating_add(2)..=last).flat_map(move |end| {
                let span = &a.lists()[source.entity][source.index..end];
                order
                    .clone()
                    .filter(move |&e| {
                        !p.entities()[e].is_pinned() && span.iter().all(|v| p.accepts(e, v))
                    })
                    .flat_map(move |e| {
                        let length =
                            a.lists()[e].len() - if e == source.entity { span.len() } else { 0 };
                        (p.entities()[e].pinned_prefix()..=length).flat_map(move |index| {
                            [false, true].into_iter().filter_map(move |reverse| {
                                (reverse || e != source.entity || index != source.index).then_some(
                                    ListMove::SubListChange {
                                        entity: source.entity,
                                        start: source.index,
                                        end,
                                        destination: ListPosition { entity: e, index },
                                        reverse,
                                    },
                                )
                            })
                        })
                    })
            })
        })
}

/// All legal contiguous-span relocations, then their reversed variants at each
/// destination. Lengths start at two (singletons are ordinary changes) and are
/// capped by max_length. Source and destination pins/ranges are respected.
/// Destination indices are post-removal. Forward same-position no-ops are omitted;
/// distinct moves may still produce the same successor. Generation is lazy.
pub fn list_sublist_change_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_length: usize,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(sublist_changes(p, a, max_length, 0..p.entities().len()))
}

fn ranges<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_length: usize,
    order: I,
) -> impl Iterator<Item = ListRange> + 'a {
    positions(p, a, order).flat_map(move |pos| {
        let last = a.lists()[pos.entity]
            .len()
            .min(pos.index.saturating_add(max_length));
        (pos.index + 1..=last).map(move |end| ListRange {
            entity: pos.entity,
            start: pos.index,
            end,
        })
    })
}
fn sublist_swaps<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_length: usize,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    (max_length >= 2)
        .then(|| ranges(p, a, max_length, order.clone()))
        .into_iter()
        .flatten()
        .flat_map(move |left| {
            ranges(p, a, max_length, order.clone())
                .filter(move |right| {
                    // Canonical owner/index order avoids duplicate symmetric swaps.
                    (left.entity < right.entity
                        || (left.entity == right.entity && left.end <= right.start))
                        && (left.end - left.start > 1 || right.end - right.start > 1)
                        && a.lists()[left.entity][left.start..left.end]
                            .iter()
                            .all(|v| p.accepts(right.entity, v))
                        && a.lists()[right.entity][right.start..right.end]
                            .iter()
                            .all(|v| p.accepts(left.entity, v))
                })
                .flat_map(move |right| {
                    [false, true]
                        .into_iter()
                        .map(move |reverse| ListMove::SubListSwap {
                            left,
                            right,
                            reverse,
                        })
                })
        })
}
/// Lazy disjoint span swaps, with both ordinary and reversed exchanges. Each
/// nonempty span has length <= max_length and at least one has length >= two.
/// Single/single swaps belong to list_swap_moves. Bounds below two yield no moves.
/// Pins and both owners' value ranges are respected; symmetric pairs occur once.
/// Different moves may still have identical successors.
pub fn list_sublist_swap_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    max_length: usize,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(sublist_swaps(p, a, max_length, 0..p.entities().len()))
}
