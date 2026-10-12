//! Validated endpoint-slot topology for cuts in separate movable list cycles.
use crate::{ListAssignment, ListKOptError, ListPosition, ListProblem};
/// A logical endpoint cycle, distinct from the physical combined value cycle.
/// Each removed edge contributes two occurrence slots; adjacent cuts may refer
/// to the same value in different slots. Added links match occurrence slots.
/// This validates local wrap cuts and endpoint connectivity without claiming a
/// physical list rewrite or inferred destination partition. Source order/pins
/// are checked during preparation; scoring and assignments are never changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListKOptEndpointTopology {
    positions: Vec<ListPosition>,
    entities: Vec<usize>,
    order: Vec<usize>,
    inverse: Vec<usize>,
    partners: Vec<usize>,
    retained: Vec<usize>,
    cycles: Vec<Vec<usize>>,
    cycle_ids: Vec<usize>,
}
impl ListKOptEndpointTopology {
    pub fn prepare_local_cycles<V: Eq + Clone>(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        removed: &[(ListPosition, ListPosition)],
        added_slots: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        problem.validate(before)?;
        if removed.len() < 2 || removed.len() != added_slots.len() {
            return Err(ListKOptError::EdgeCount);
        }
        let mut entities = vec![];
        let mut positions = Vec::with_capacity(2 * removed.len());
        let mut cuts = std::collections::BTreeSet::new();
        for &(a, b) in removed {
            if a.entity != b.entity {
                return Err(ListKOptError::InvalidEdge);
            }
            let Some(meta) = problem.entities().get(a.entity) else {
                return Err(ListKOptError::Entities);
            };
            if meta.is_pinned() {
                return Err(crate::ListError::PinChanged { entity: a.entity }.into());
            }
            let row = &before.lists()[a.entity];
            let first = meta.pinned_prefix();
            if a.index < first || b.index < first || a.index >= row.len() || b.index >= row.len() {
                return Err(ListKOptError::InvalidEdge);
            }
            let successor = |i| if i + 1 == row.len() { first } else { i + 1 };
            let origin = if successor(a.index) == b.index {
                a.index
            } else if successor(b.index) == a.index {
                b.index
            } else {
                return Err(ListKOptError::InvalidEdge);
            };
            if !cuts.insert((a.entity, origin)) {
                return Err(ListKOptError::DuplicateEdge);
            }
            if !entities.contains(&a.entity) {
                entities.push(a.entity);
            }
            positions.extend([a, b]);
        }
        let mut starts = std::collections::BTreeMap::new();
        let mut offset = 0usize;
        for &entity in &entities {
            starts.insert(entity, offset);
            offset += before.lists()[entity].len() - problem.entities()[entity].pinned_prefix();
        }
        let coordinate = |p: ListPosition| {
            starts[&p.entity] + p.index - problem.entities()[p.entity].pinned_prefix()
        };
        let mut pairs = (0..removed.len())
            .map(|i| {
                let a = 2 * i;
                let b = a + 1;
                if coordinate(positions[a]) <= coordinate(positions[b]) {
                    (a, b)
                } else {
                    (b, a)
                }
            })
            .collect::<Vec<_>>();
        pairs.sort_by_key(|&(a, _)| coordinate(positions[a]));
        let order = pairs
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect::<Vec<_>>();
        let mut inverse = vec![0; order.len()];
        for (rank, &slot) in order.iter().enumerate() {
            inverse[slot] = rank;
        }
        let mut partners = vec![usize::MAX; order.len()];
        for &(a, b) in added_slots {
            if a >= order.len() || b >= order.len() || a == b {
                return Err(ListKOptError::InvalidEdge);
            }
            if partners[a] != usize::MAX || partners[b] != usize::MAX {
                return Err(ListKOptError::DuplicateEdge);
            }
            partners[a] = b;
            partners[b] = a;
        }
        if partners.contains(&usize::MAX) {
            return Err(ListKOptError::Degree);
        }
        let mut retained = vec![0; order.len()];
        for pair in 0..removed.len() {
            let a = order[2 * pair + 1];
            let b = order[(2 * pair + 2) % order.len()];
            retained[a] = b;
            retained[b] = a;
        }
        // Two perfect matchings make degree-two MULTIGRAPH components. A link
        // may coincide with a retained segment, forming a valid two-slot cycle.
        let mut cycle_ids = vec![usize::MAX; order.len()];
        let mut cycles = vec![];
        for &first in &order {
            if cycle_ids[first] != usize::MAX {
                continue;
            }
            let id = cycles.len();
            let mut cycle = vec![first];
            cycle_ids[first] = id;
            let mut previous = first;
            let mut current = partners[first];
            while current != first {
                cycle_ids[current] = id;
                cycle.push(current);
                let next = if partners[current] == previous {
                    retained[current]
                } else {
                    partners[current]
                };
                previous = current;
                current = next;
            }
            cycles.push(cycle);
        }
        Ok(Self {
            positions,
            entities,
            order,
            inverse,
            partners,
            retained,
            cycles,
            cycle_ids,
        })
    }
    pub fn endpoint_positions(&self) -> &[ListPosition] {
        &self.positions
    }
    pub fn selected_entities(&self) -> &[usize] {
        &self.entities
    }
    /// Endpoint occurrence slots in pair-preserving logical tour order.
    pub fn endpoint_tour_order(&self) -> &[usize] {
        &self.order
    }
    pub fn endpoint_slot_ranks(&self) -> &[usize] {
        &self.inverse
    }
    pub fn endpoint_partners(&self) -> &[usize] {
        &self.partners
    }
    pub fn endpoint_cycle_count(&self) -> usize {
        self.cycles.len()
    }
    /// Orient an endpoint cycle using occurrence slots, not physical value positions.
    pub fn endpoint_cycle_order(
        &self,
        first: usize,
        next: usize,
    ) -> Result<Vec<usize>, ListKOptError> {
        if first >= self.order.len() || next >= self.order.len() {
            return Err(ListKOptError::FirstSuccessor);
        }
        if self.partners[first] != next && self.retained[first] != next {
            return Err(ListKOptError::FirstSuccessor);
        }
        if self.cycles.len() != 1 {
            return Err(ListKOptError::Disconnected);
        }
        let mut output = vec![first];
        let mut previous = first;
        let mut current = next;
        while current != first {
            output.push(current);
            let following = if self.partners[current] == previous {
                self.retained[current]
            } else {
                self.partners[current]
            };
            previous = current;
            current = following;
        }
        Ok(output)
    }
    pub fn endpoint_cycle_ids(&self) -> &[usize] {
        &self.cycle_ids
    }
}
