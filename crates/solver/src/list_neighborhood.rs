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
    swap_pairs(p, a, order, false)
}
fn swap_pairs<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
    unique: bool,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    positions(p, a, order.clone())
        .enumerate()
        .flat_map(move |(offset, left)| {
            let assigned = positions(p, a, order.clone())
                .skip(if unique { offset + 1 } else { 0 })
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
/// Visit each eligible assigned-position pair once, retaining its first
/// orientation. Optional replacements remain once per source/value combination.
/// Enumeration is lazy and does not retain a set of visited pairs.
pub fn list_unique_swap_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(swap_pairs(p, a, 0..p.entities().len(), true))
}
/// Enumerate the selected neighborhood. Gather yields bounded selected groups;
/// MassListChange uses assigned-only groups with explicit size/null policy;
/// TwoOpt yields only reversals/tail swaps. Other modes include relocations and
/// optional assignments/unassignments.
/// Complete inputs remain complete and all pins remain unchanged.
pub fn list_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    neighborhood: Neighborhood,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    neighborhood.validate_list(p)?;
    Ok(ordered(p, a, neighborhood, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    neighborhood: Neighborhood,
    order: I,
) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
    match neighborhood {
        Neighborhood::SubListChange(c) => {
            return Box::new(crate::list_sublist::changes(p, a, c, order));
        }
        Neighborhood::SubListSwap(c) => {
            return Box::new(crate::list_sublist::swaps(p, a, c, order));
        }
        Neighborhood::SubListUnassign(size) => {
            return Box::new(crate::list_sublist::unassigns(p, a, size, order));
        }
        _ => {}
    }
    if matches!(neighborhood, Neighborhood::Swap | Neighborhood::UniqueSwap) {
        return Box::new(swap_pairs(
            p,
            a,
            order,
            neighborhood == Neighborhood::UniqueSwap,
        ));
    }
    // Keep the selected family behind one pointer. Nested mixed products must
    // not carry every possible iterator's state on the caller's stack.
    if let Neighborhood::RepartitionKOpt(config) = neighborhood {
        return Box::new(crate::list_k_opt_repartition::ordered(p, a, config, order));
    }
    if let Neighborhood::SampledKOpt(config) = neighborhood {
        return Box::new(crate::list_k_opt_sampling::ordered(p, a, config, order));
    }
    if let Neighborhood::KOpt(config) = neighborhood {
        return Box::new(crate::list_k_opt::ordered(p, a, config, order));
    }
    if let Neighborhood::MassListAssign(size) | Neighborhood::MassListUnassign(size) = neighborhood
    {
        return Box::new(
            crate::list_mass_assignment::ordered(
                p,
                a,
                size,
                matches!(neighborhood, Neighborhood::MassListUnassign(_)),
                order,
            )
            .map(crate::ListMassChange::into_move),
        );
    }
    if let Neighborhood::MassListChange(config) = neighborhood {
        return Box::new(
            crate::list_mass_change::ordered(p, a, config, order)
                .map(crate::ListMassChange::into_move),
        );
    }
    if let Neighborhood::Gather { max_size } = neighborhood {
        return crate::list_gather_neighborhood::ordered(p, a, max_size, order);
    }
    if let Neighborhood::TwoOpt { cross_entity } = neighborhood {
        return Box::new(two_opt_moves(p, a, cross_entity, order));
    }
    let assigned = p
        .allows_unassigned()
        .then(|| assigns(p, a, order.clone()))
        .into_iter()
        .flatten();
    let removed = unassigns(p, a, order.clone());
    let swapped = neighborhood
        .includes_swaps()
        .then(|| {
            swap_pairs(
                p,
                a,
                order.clone(),
                neighborhood == Neighborhood::ChangeAndUniqueSwap,
            )
        })
        .into_iter()
        .flatten();
    let max_length = match neighborhood {
        Neighborhood::ChangeSwapAndSubList { max_length } => max_length,
        _ => 0,
    };
    let spans = sublist_changes(p, a, max_length, order.clone());
    let span_swaps = sublist_swaps(p, a, max_length, order.clone());
    Box::new(
        changes(p, a, order)
            .chain(assigned)
            .chain(removed)
            .chain(swapped)
            .chain(spans)
            .chain(span_swaps),
    )
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

fn tail_swaps<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    reverse: bool,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    positions(p, a, order.clone()).flat_map(move |left| {
        positions(p, a, order.clone())
            .filter(move |right| {
                left.entity < right.entity
                    && a.lists()[left.entity][left.index..]
                        .iter()
                        .all(|v| p.accepts(right.entity, v))
                    && a.lists()[right.entity][right.index..]
                        .iter()
                        .all(|v| p.accepts(left.entity, v))
            })
            .flat_map(move |right| {
                let left = ListRange {
                    entity: left.entity,
                    start: left.index,
                    end: a.lists()[left.entity].len(),
                };
                let right = ListRange {
                    entity: right.entity,
                    start: right.index,
                    end: a.lists()[right.entity].len(),
                };
                [false, true]
                    .into_iter()
                    .filter(move |&r| {
                        !r || reverse && (left.end - left.start > 1 || right.end - right.start > 1)
                    })
                    .map(move |reverse| ListMove::SubListSwap {
                        left,
                        right,
                        reverse,
                    })
            })
    })
}
fn two_opt_moves<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    cross_entity: bool,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    let tails = cross_entity
        .then(|| tail_swaps(p, a, false, order.clone()))
        .into_iter()
        .flatten();
    positions(p, a, order)
        .flat_map(move |source| {
            (source.index.saturating_add(2)..=a.lists()[source.entity].len()).map(move |end| {
                ListMove::SubListChange {
                    entity: source.entity,
                    start: source.index,
                    end,
                    destination: source,
                    reverse: true,
                }
            })
        })
        .chain(tails)
}
/// Exchange nonempty tails of different owners, once per unordered owner pair.
/// Include reversed exchanges when requested, except for two singleton tails.
/// Pins and both receiving owners' ranges are respected. Empty tails are excluded.
/// This finite lazy provider does not assign or unassign values.
pub fn list_tail_swap_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    include_reversing: bool,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(tail_swaps(p, a, include_reversing, 0..p.entities().len()))
}
/// Reverse each unpinned within-list span of length >=2 in place, followed by
/// optional non-reversing cross-owner tail exchanges. All tails are nonempty;
/// empty owners cannot receive values through this provider. Generation is lazy.
/// Pins and owner ranges are respected. Different moves may share a successor.
pub fn list_two_opt_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    cross_entity: bool,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(two_opt_moves(p, a, cross_entity, 0..p.entities().len()))
}
