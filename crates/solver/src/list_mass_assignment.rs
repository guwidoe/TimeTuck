//! Missing-only list group assignment and assigned-only group unassignment.
use crate::{
    ListAssignment, ListError, ListMassChange, ListPosition, ListProblem, ListSampleSize,
    SubPillarSelection,
};
pub(crate) fn validate_optional<V: Eq + Clone>(p: &ListProblem<V>) -> Result<(), ListError> {
    if !p.allows_unassigned() {
        return Err(ListError::OptionalValuesRequired);
    }
    Ok(())
}
fn movable<V: Eq + Clone>(p: &ListProblem<V>, a: &ListAssignment<V>, v: &V) -> bool {
    a.location(v).is_some_and(|at| {
        !p.entities()[at.entity].is_pinned() && at.index >= p.entities()[at.entity].pinned_prefix()
    })
}
fn candidate<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    members: Vec<V>,
    destination: Option<ListPosition>,
) -> ListMassChange<V> {
    let movement = p
        .gather_move(a, &members, destination)
        .expect("validated list assignment group");
    ListMassChange::from_parts(members, destination, movement)
}
/// Insert ordered groups of at least two missing identities consecutively at an
/// original insertion boundary. Requires optional values. Pins, prefixes and every
/// member's receiving range constrain destinations. All-sized groups use the full
/// missing population before receiver filtering. Order is receiver/index, size,
/// then registry permutation. Enumeration is lazy; no permutation list is stored.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![0,1,2],vec![ListEntity::new(vec![0])])?.allowing_unassigned();
/// let a=p.initial_assignment();
/// let moves:Vec<_>=list_mass_assign_moves(&p,&a,ListSampleSize::all())?.collect();
/// assert_eq!(moves.len(),4);
/// assert!(moves.iter().all(|m|!m.members().contains(&0)));
/// # Ok::<(), ListError>(())
/// ```
pub fn list_mass_assign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: ListSampleSize,
) -> Result<impl Iterator<Item = ListMassChange<V>> + 'a, ListError> {
    p.validate(a)?;
    validate_optional(p)?;
    Ok(ordered(p, a, size, false, 0..p.entities().len()))
}
/// Unassign movable assigned identities, in increasing subset size and registry
/// order. Singleton groups are valid. Missing identities, full pins and pinned
/// prefixes never join. Requires optional values even for empty requested work.
pub fn list_mass_unassign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: ListSampleSize,
) -> Result<impl Iterator<Item = ListMassChange<V>> + 'a, ListError> {
    p.validate(a)?;
    validate_optional(p)?;
    Ok(ordered(p, a, size, true, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: ListSampleSize,
    unassign: bool,
    order: I,
) -> Box<dyn Iterator<Item = ListMassChange<V>> + 'a> {
    let eligible: Vec<_> = p
        .values()
        .iter()
        .filter(|v| {
            if unassign {
                movable(p, a, v)
            } else {
                a.location(v).is_none()
            }
        })
        .collect();
    let Some((minimum, maximum)) =
        size.bounds_with_minimum(eligible.len(), if unassign { 1 } else { 2 })
    else {
        return Box::new(std::iter::empty());
    };
    if unassign {
        return Box::new(
            SubPillarSelection::combinations(minimum, maximum)
                .expect("positive group size")
                .indices(eligible.len())
                .map(move |indices| {
                    candidate(
                        p,
                        a,
                        indices.iter().map(|&i| eligible[i].clone()).collect(),
                        None,
                    )
                }),
        );
    }
    Box::new(
        crate::list_neighborhood::insertions(p, a, order).flat_map(move |destination| {
            let accepted: Vec<_> = eligible
                .iter()
                .copied()
                .filter(|v| p.accepts(destination.entity, v))
                .collect();
            crate::list_gather_neighborhood::Selections::bounded(accepted.len(), minimum, maximum)
                .map(move |indices| {
                    candidate(
                        p,
                        a,
                        indices.iter().map(|&i| accepted[i].clone()).collect(),
                        Some(destination),
                    )
                })
        }),
    )
}
/// Bounded seeded draws with replacement between groups and no duplicate identity
/// within a group. Sample size is uniform over clipped positive bounds, then a
/// uniform subset is selected, ordered by registry index. Every requested draw is
/// produced when the eligible population meets the bounds, including repeated
/// singletons. Empty populations yield none. The input remains one fixed snapshot;
/// regenerate after mutation. No Java PRNG or distribution parity is claimed.
pub fn sample_list_mass_unassign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: ListSampleSize,
    seed: u64,
    draws: usize,
) -> Result<impl Iterator<Item = ListMassChange<V>> + 'a, ListError> {
    p.validate(a)?;
    validate_optional(p)?;
    let eligible: Vec<_> = p.values().iter().filter(|v| movable(p, a, v)).collect();
    let bounds = size.bounds_with_minimum(eligible.len(), 1);
    let mut random = crate::random::SplitMix64::new(seed);
    Ok((0..if bounds.is_some() { draws } else { 0 }).map(move |_| {
        let (minimum, maximum) = bounds.expect("eligible population");
        let count = minimum + random.below((maximum - minimum + 1) as u64) as usize;
        let mut indices: Vec<_> = (0..eligible.len()).collect();
        random.shuffle(&mut indices);
        indices.truncate(count);
        indices.sort_unstable();
        candidate(
            p,
            a,
            indices.iter().map(|&i| eligible[i].clone()).collect(),
            None,
        )
    }))
}
