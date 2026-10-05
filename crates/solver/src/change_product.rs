//! Simultaneous basic changes with a separate destination for each selected slot.
use crate::{Assignment, FiniteProblem, ModelError, Move, MoveProduct, SubPillarSelection};

/// Enumerate selected unpinned variable subsets, then the Cartesian product of
/// their alternative values. Every selected slot changes; no duplicate successor
/// states arise from redundant writes. Optional slots may select None last;
/// required slots never become unassigned. Domains need not overlap.
/// Subset bounds and consecutive selection apply to eligible slots in model order.
/// Singletons are supported; pins and slots without alternatives are excluded.
/// Search can use `Neighborhood::ChangeProduct` with the same selection policy.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0),
///                                Variable::new(vec![0,2]).with_initial(0)])?;
/// let a = p.initial_assignment();
/// let moves: Vec<_> = change_product_moves(&p, &a,
///     SubPillarSelection::combinations(2,2)?)?.collect();
/// let mut next = a.clone();
/// p.apply(&mut next, &moves[0])?.commit();
/// assert_eq!(next.values(), &[Some(1), Some(2)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn change_product_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    selection: SubPillarSelection,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    Ok(ordered(p, a, selection, 0..p.variables().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    selection: SubPillarSelection,
    order: impl Iterator<Item = usize>,
) -> impl Iterator<Item = Move<V>> + 'a {
    let eligible: Vec<_> = order
        .filter(|&i| {
            let v = &p.variables()[i];
            !v.is_pinned()
                && (v.domain().iter().any(|x| Some(x) != a.values()[i].as_ref())
                    || (v.allows_unassigned() && a.values()[i].is_some()))
        })
        .collect();
    selection.indices(eligible.len()).flat_map(move |indices| {
        let members: Vec<_> = indices.into_iter().map(|i| eligible[i]).collect();
        MoveProduct::new(members.len(), false, move |index| {
            crate::neighborhood::change_moves_ordered(p, a, std::iter::once(members[index]))
        })
        .map(Move::Composite)
    })
}
