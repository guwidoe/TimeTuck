//! Independent retained-end partition policy for combined cyclic reconnections.
use crate::{ListAssignment, ListKOptError, ListMove, ListProblem};
/// Reconnect the selected combined movable suffix cycle and split it at retained
/// original suffix-end identities. Assign cyclic sections in selected entity
/// order; rotate the section crossing the anchor so its original first value is
/// first. Every selected suffix must be nonempty. Orientation comes from edge
/// order, and final native range/pin/dependency validation precedes movement.
/// This is an explicit native policy, not universal Java implicit-partition parity.
pub fn list_k_opt_inferred_partition_move<V: Eq + Clone>(
    problem: &ListProblem<V>,
    before: &ListAssignment<V>,
    entities: &[usize],
    removed: &[(usize, usize)],
    added: &[(usize, usize)],
) -> Result<ListMove<V>, ListKOptError> {
    problem.validate(before)?;
    let values = crate::list_k_opt_repartition::members(problem, before, entities)?;
    let first = removed.first().ok_or(ListKOptError::EdgeCount)?.0;
    let successor = added
        .iter()
        .find_map(|&(a, b)| {
            if a == first {
                Some(b)
            } else if b == first {
                Some(a)
            } else {
                None
            }
        })
        .ok_or(ListKOptError::FirstSuccessor)?;
    let mut tour =
        crate::list_k_opt::cycle_order(0, values.len(), removed, added, first, successor)?;
    let anchor = tour
        .iter()
        .position(|&v| v == 0)
        .expect("validated complete cycle");
    tour.rotate_left(anchor);
    let mut ends = vec![false; values.len()];
    let mut offset = 0;
    for &entity in entities {
        let length = before.lists()[entity].len() - problem.entities()[entity].pinned_prefix();
        if length == 0 {
            return Err(ListKOptError::Partition);
        }
        offset += length;
        ends[offset - 1] = true;
    }
    let boundaries: Vec<_> = tour
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| ends[v].then_some(i))
        .collect();
    let first_end = boundaries[0] + 1;
    let last_end = boundaries[boundaries.len() - 1] + 1;
    // The cyclic section straddling the anchor is rotated within the first row.
    // Other sections retain traversal order; every identity occurs exactly once.
    let mut target = Vec::with_capacity(tour.len());
    target.extend_from_slice(&tour[..first_end]);
    target.extend_from_slice(&tour[last_end..]);
    let mut lengths = vec![first_end + tour.len() - last_end];
    for pair in boundaries.windows(2) {
        target.extend_from_slice(&tour[pair[0] + 1..=pair[1]]);
        lengths.push(pair[1] - pair[0]);
    }
    crate::list_k_opt_repartition::repartition(
        problem, before, entities, &values, &target, &lengths,
    )
}
