//! Lazy list neighborhoods over one immutable assignment snapshot.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange, Neighborhood,
};

use std::sync::{Arc, OnceLock};
mod swap_index;
use swap_index::{SwapPositions, SwapReceivers};

// Defer the snapshot until the iterator is polled. Eq-only registries keep their
// lazy direct lookup; indexed registries scan assigned values once.
pub(crate) fn unassigned_ids<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
) -> impl Iterator<Item = usize> + 'a {
    std::iter::once_with(move || p.indexed_value_locations(a)).flat_map(move |locations| {
        p.values().iter().enumerate().filter_map(move |(id, v)| {
            locations
                .as_ref()
                .map_or_else(|| a.location(v), |rows| rows[id])
                .is_none()
                .then_some(id)
        })
    })
}

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
// Check each receiver once, before expanding its insertion slots. Every validated
// unpinned list has an insertion slot, even after a fully pinned prefix.
pub(crate) fn value_insertions<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: impl Iterator<Item = usize> + 'a,
    value_id: usize,
) -> impl Iterator<Item = ListPosition> + 'a {
    order
        .filter(move |&e| !p.entities()[e].is_pinned() && p.accepts_registered(e, value_id))
        .flat_map(move |entity| {
            (p.entities()[entity].pinned_prefix()..=a.lists()[entity].len())
                .map(move |index| ListPosition { entity, index })
        })
}

pub(crate) fn assigns<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    unassigned_ids(p, a).flat_map(move |id| {
        let value = &p.values()[id];
        value_insertions(p, a, order.clone(), id).map(move |destination| ListMove::Assign {
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
    // Dispatch once per emitted move, never once per rejected candidate, for
    // models whose static range shape cannot benefit from a receiver index.
    if p.has_sparse_entity_ranges() {
        swap_index::SwapTraversal::Indexed(indexed_swap_pairs(p, a, order, unique))
    } else {
        swap_index::SwapTraversal::Direct(direct_swap_pairs(p, a, order, unique))
    }
}
fn direct_swap_pairs<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
    unique: bool,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    // Shared by source positions, but initialized only when a replacement is
    // requested. Assigned swaps and required models need no missing-value scan.
    let missing = p
        .allows_unassigned()
        .then(|| Arc::new(OnceLock::<Vec<usize>>::new()));
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
            let cache = missing.clone();
            let mut cursor = 0;
            let unassigned = std::iter::from_fn(move || {
                let ids = cache
                    .as_ref()?
                    .get_or_init(|| unassigned_ids(p, a).collect());
                let id = *ids.get(cursor)?;
                cursor += 1;
                Some(&p.values()[id])
            })
            .filter(move |v| p.accepts(left.entity, v))
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
fn indexed_swap_pairs<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    order: I,
    unique: bool,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    // Shared by source positions, but initialized only when a replacement is
    // requested. Assigned swaps and required models need no missing-value scan.
    let missing = p
        .allows_unassigned()
        .then(|| Arc::new(OnceLock::<Vec<usize>>::new()));
    // Keep default/Eq-only/small scans allocation-free. The index is built on
    // the second requested source position and lives only for this snapshot.
    // A caller taking only an early first-source proposal never builds it.
    let receivers = p
        .has_sparse_entity_ranges()
        .then(OnceLock::<Option<Arc<SwapReceivers>>>::new);
    positions(p, a, order.clone())
        .enumerate()
        .flat_map(move |(offset, left)| {
            let index = receivers
                .as_ref()
                .filter(|_| offset != 0)
                .and_then(|cache| {
                    cache
                        .get_or_init(|| SwapReceivers::new(p, a, order.clone()).map(Arc::new))
                        .as_ref()
                });
            let candidates = match index {
                Some(index) => {
                    let value = p
                        .value_index(&a.lists()[left.entity][left.index])
                        .expect("validated value");
                    SwapPositions::Indexed(index.positions(value, unique.then_some(offset)))
                }
                None => SwapPositions::Direct(positions(p, a, order.clone()).skip(if unique {
                    offset + 1
                } else {
                    0
                })),
            };
            let assigned = candidates
                .filter(move |&right| {
                    right != left
                        && p.accepts(left.entity, &a.lists()[right.entity][right.index])
                        && p.accepts(right.entity, &a.lists()[left.entity][left.index])
                })
                .map(move |right| ListMove::Swap { left, right });
            let cache = missing.clone();
            let mut cursor = 0;
            let unassigned = std::iter::from_fn(move || {
                let ids = cache
                    .as_ref()?
                    .get_or_init(|| unassigned_ids(p, a).collect());
                let id = *ids.get(cursor)?;
                cursor += 1;
                Some(&p.values()[id])
            })
            .filter(move |v| p.accepts(left.entity, v))
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
        let unique = neighborhood == Neighborhood::UniqueSwap;
        return if p.has_sparse_entity_ranges() {
            Box::new(indexed_swap_pairs(p, a, order, unique))
        } else {
            Box::new(direct_swap_pairs(p, a, order, unique))
        };
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
    let unique = neighborhood == Neighborhood::ChangeAndUniqueSwap;
    if p.has_sparse_entity_ranges() {
        combined_ordered(p, a, neighborhood, order, |order| {
            indexed_swap_pairs(p, a, order, unique)
        })
    } else {
        combined_ordered(p, a, neighborhood, order, |order| {
            direct_swap_pairs(p, a, order, unique)
        })
    }
}

fn combined_ordered<
    'a,
    V: Eq + Clone,
    I: Iterator<Item = usize> + Clone + 'a,
    S: Iterator<Item = ListMove<V>> + 'a,
>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    neighborhood: Neighborhood,
    order: I,
    make_swaps: impl FnOnce(I) -> S,
) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
    let assigned = p
        .allows_unassigned()
        .then(|| assigns(p, a, order.clone()))
        .into_iter()
        .flatten();
    let removed = unassigns(p, a, order.clone());
    let swapped = neighborhood
        .includes_swaps()
        .then(|| make_swaps(order.clone()))
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
        order
            .clone()
            .filter(move |&entity| left.entity < entity && !p.entities()[entity].is_pinned())
            .flat_map(move |entity| {
                let row = &a.lists()[entity];
                let prefix = p.entities()[entity].pinned_prefix();
                // Eligibility of the incoming left suffix is shared by every
                // right cut. Empty/prefix-pinned rows contribute no cuts.
                let eligible = prefix < row.len()
                    && (p.entities()[entity].value_range().is_none()
                        || a.lists()[left.entity][left.index..]
                            .iter()
                            .all(|v| p.accepts(entity, v)));
                let first = if !eligible {
                    row.len()
                } else if p.entities()[left.entity].value_range().is_none() {
                    prefix
                } else {
                    // Every suffix after the last rejected value fits. Work
                    // only on the movable tail, once per source/receiver pair.
                    row[prefix..]
                        .iter()
                        .rposition(|v| !p.accepts(left.entity, v))
                        .map_or(prefix, |offset| prefix + offset + 1)
                };
                (first..row.len()).map(move |index| ListPosition { entity, index })
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

#[cfg(test)]
mod receiver_tests {
    use super::*;
    use crate::ListEntity;

    #[test]
    fn eligible_insertions_preserve_duplicate_order_and_fully_pinned_prefix_append() {
        let values: Vec<_> = (0..5).collect();
        let entities = vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1]),
            ListEntity::new(vec![2, 3])
                .with_pinned_prefix(2)
                .with_value_range(vec![2, 3, 4]),
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![]).with_value_range(vec![4]),
        ];
        for p in [
            ListProblem::new(values.clone(), entities.clone()).unwrap(),
            ListProblem::new_indexed(values, entities).unwrap(),
        ] {
            let a = p.initial_assignment();
            let order = [3, 1, 0, 2, 1];
            for id in 0..5 {
                let expected: Vec<_> = insertions(&p, &a, order.into_iter())
                    .filter(|at| p.accepts(at.entity, &p.values()[id]))
                    .collect();
                assert_eq!(
                    value_insertions(&p, &a, order.into_iter(), id).collect::<Vec<_>>(),
                    expected
                );
            }
            let mut actual = value_insertions(&p, &a, order.into_iter(), 4);
            assert_eq!(
                actual.next(),
                Some(ListPosition {
                    entity: 3,
                    index: 0
                })
            );
            assert_eq!(
                actual.next(),
                Some(ListPosition {
                    entity: 1,
                    index: 2
                })
            );
            assert_eq!(
                actual.next(),
                Some(ListPosition {
                    entity: 1,
                    index: 2
                })
            );
            assert_eq!(actual.next(), None);
            assert_eq!(actual.next(), None);
        }
    }
}

#[cfg(test)]
mod tail_range_tests {
    use super::*;
    use crate::ListEntity;

    // Independent original position-pair enumeration, including encounter order.
    fn reference(
        p: &ListProblem<i32>,
        a: &ListAssignment<i32>,
        order: &[usize],
        reversing: bool,
    ) -> Vec<ListMove<i32>> {
        let mut out = Vec::new();
        for &l in order {
            if p.entities()[l].is_pinned() {
                continue;
            }
            for li in p.entities()[l].pinned_prefix()..a.lists()[l].len() {
                for &r in order {
                    if l >= r || p.entities()[r].is_pinned() {
                        continue;
                    }
                    for ri in p.entities()[r].pinned_prefix()..a.lists()[r].len() {
                        let fits = |owner: usize, values: &[i32]| {
                            p.entities()[owner]
                                .value_range()
                                .is_none_or(|range| values.iter().all(|v| range.contains(v)))
                        };
                        if !fits(r, &a.lists()[l][li..]) || !fits(l, &a.lists()[r][ri..]) {
                            continue;
                        }
                        let left = ListRange {
                            entity: l,
                            start: li,
                            end: a.lists()[l].len(),
                        };
                        let right = ListRange {
                            entity: r,
                            start: ri,
                            end: a.lists()[r].len(),
                        };
                        out.push(ListMove::SubListSwap {
                            left,
                            right,
                            reverse: false,
                        });
                        if reversing && (left.end - left.start > 1 || right.end - right.start > 1) {
                            out.push(ListMove::SubListSwap {
                                left,
                                right,
                                reverse: true,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn tail_ranges_match_every_cut_with_pins_asymmetry_and_repeated_orders() {
        let orders = [vec![0, 1, 2, 3], vec![3, 1, 0, 2, 1, 0], vec![2, 0], vec![]];
        for cut in 0..=6 {
            for mask in 0..64 {
                let rows = [
                    (0..cut).collect::<Vec<_>>(),
                    (cut..6).collect(),
                    vec![6],
                    vec![],
                ];
                let entities = rows
                    .iter()
                    .enumerate()
                    .map(|(owner, row)| {
                        let mut entity = ListEntity::new(row.clone());
                        if mask & 1 != 0 && owner == 2 {
                            entity = entity.with_pinned(true);
                        }
                        if mask & 2 != 0 && !(mask & 1 != 0 && owner == 2) {
                            entity = entity.with_pinned_prefix(row.len().min(1));
                        }
                        if mask & 4 != 0 && owner == 1 {
                            entity = entity.with_pinned_prefix(row.len());
                        }
                        if mask & (8 << (owner % 3)) != 0 {
                            // Original values remain valid; transferred suffixes can
                            // contain multiple rejects before their final accepted run.
                            entity = entity.with_value_range(
                                (0..7)
                                    .rev()
                                    .filter(|v| {
                                        row.contains(v) || (*v as usize + owner).is_multiple_of(3)
                                    })
                                    .collect(),
                            );
                        }
                        entity
                    })
                    .collect::<Vec<_>>();
                for p in [
                    ListProblem::new((0..7).collect(), entities.clone()).unwrap(),
                    ListProblem::new_indexed((0..7).collect(), entities).unwrap(),
                ] {
                    let a = p.initial_assignment();
                    for order in &orders {
                        for reverse in [false, true] {
                            let expected = reference(&p, &a, order, reverse);
                            let actual = tail_swaps(&p, &a, reverse, order.iter().copied())
                                .collect::<Vec<_>>();
                            assert_eq!(actual, expected, "cut={cut}, mask={mask}, order={order:?}");
                            // Abandonment cannot affect the next opening.
                            let _ = tail_swaps(&p, &a, reverse, order.iter().copied())
                                .take(3)
                                .count();
                            assert_eq!(
                                tail_swaps(&p, &a, reverse, order.iter().copied())
                                    .collect::<Vec<_>>(),
                                expected
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unrestricted_tails_do_not_inspect_values_during_enumeration() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static EQUALITIES: AtomicUsize = AtomicUsize::new(0);
        #[derive(Clone)]
        struct Value(usize);
        impl PartialEq for Value {
            fn eq(&self, other: &Self) -> bool {
                EQUALITIES.fetch_add(1, Ordering::Relaxed);
                self.0 == other.0
            }
        }
        impl Eq for Value {}
        let p = ListProblem::new(
            (0..64).map(Value).collect(),
            vec![
                ListEntity::new((0..32).map(Value).collect()),
                ListEntity::new((32..64).map(Value).collect()),
            ],
        )
        .unwrap();
        let a = p.initial_assignment();
        let moves = list_tail_swap_moves(&p, &a, true).unwrap();
        EQUALITIES.store(0, Ordering::Relaxed);
        assert_eq!(moves.count(), 2047);
        assert_eq!(EQUALITIES.load(Ordering::Relaxed), 0);
    }
}
