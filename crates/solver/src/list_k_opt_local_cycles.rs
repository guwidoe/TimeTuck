//! Executable physical reconnection of separate movable list cycles.
use crate::{
    ListAssignment, ListError, ListKOptEndpointTopology, ListKOptError, ListMove, ListPosition,
    ListProblem,
};

/// A physical local-cycle plan. Unlike logical pair-ranked endpoint topology,
/// every uncut original adjacency is retained, including local wrap edges and
/// interiors with no endpoint. Two ports per value distinguish parallel edges
/// and singleton loops. Added links connect removed-edge occurrence ports.
///
/// Physical expansion uses expected O(n+k) work after endpoint validation,
/// where n is selected suffix membership. Reconstruction
/// traverses actual values, then delegates final range/pin/dependency validation
/// and executable relocations to the existing native repartition machinery.
/// Explicit target lengths choose row boundaries; this does not implement Java's
/// implicit virtual-seam partition policy. Existing combined-cycle APIs and search
/// providers are unchanged.
#[derive(Clone, Debug)]
pub struct ListKOptLocalCyclePlan<V> {
    entities: Vec<usize>,
    rows: Vec<Vec<V>>,
    prefixes: Vec<usize>,
    positions: Vec<ListPosition>,
    links: Vec<[(usize, usize); 2]>,
    cycles: Vec<Vec<usize>>,
}
impl<V: Eq + Clone> ListKOptLocalCyclePlan<V> {
    pub fn prepare(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        removed: &[(ListPosition, ListPosition)],
        added_slots: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        let topology =
            ListKOptEndpointTopology::prepare_local_cycles(problem, before, removed, added_slots)?;
        let entities = topology.selected_entities().to_vec();
        let prefixes = entities
            .iter()
            .map(|&e| problem.entities()[e].pinned_prefix())
            .collect::<Vec<_>>();
        let rows = entities
            .iter()
            .map(|&e| before.lists()[e].clone())
            .collect::<Vec<_>>();
        let mut positions = Vec::new();
        let mut links = Vec::new();
        let mut offsets = std::collections::HashMap::new();
        for (&entity, &prefix) in entities.iter().zip(&prefixes) {
            let offset = positions.len();
            offsets.insert(entity, offset);
            let length = before.lists()[entity].len() - prefix;
            for index in 0..length {
                positions.push(ListPosition {
                    entity,
                    index: prefix + index,
                });
                links.push([
                    (offset + (index + length - 1) % length, 1),
                    (offset + (index + 1) % length, 0),
                ]);
            }
        }
        let coordinate = |p: ListPosition| {
            offsets[&p.entity] + p.index - problem.entities()[p.entity].pinned_prefix()
        };
        let mut ports = Vec::with_capacity(removed.len() * 2);
        for &(a, b) in removed {
            let prefix = problem.entities()[a.entity].pinned_prefix();
            let row = &before.lists()[a.entity];
            let next = if a.index + 1 == row.len() {
                prefix
            } else {
                a.index + 1
            };
            let forward = next == b.index;
            ports.push((coordinate(a), usize::from(forward)));
            ports.push((coordinate(b), usize::from(!forward)));
        }
        for (slot, &partner) in topology.endpoint_partners().iter().enumerate() {
            let (vertex, port) = ports[slot];
            links[vertex][port] = ports[partner];
        }
        let mut visited = vec![false; positions.len()];
        let mut cycles = Vec::new();
        for first in 0..positions.len() {
            if visited[first] {
                continue;
            }
            let tour = traverse(&links, first, 1);
            for &vertex in &tour {
                visited[vertex] = true;
            }
            cycles.push(tour);
        }
        Ok(Self {
            entities,
            rows,
            prefixes,
            positions,
            links,
            cycles,
        })
    }
    pub fn selected_entities(&self) -> &[usize] {
        &self.entities
    }
    pub fn physical_cycle_count(&self) -> usize {
        self.cycles.len()
    }
    /// Complete physical components, each starting at its earliest source member.
    pub fn physical_cycles(&self) -> Vec<Vec<ListPosition>> {
        self.cycles
            .iter()
            .map(|cycle| cycle.iter().map(|&v| self.positions[v]).collect())
            .collect()
    }
    fn order(&self, first: ListPosition, reverse: bool) -> Result<Vec<usize>, ListKOptError> {
        let first = self
            .positions
            .iter()
            .position(|&v| v == first)
            .ok_or(ListKOptError::FirstSuccessor)?;
        if self.cycles.len() != 1 {
            return Err(ListKOptError::Disconnected);
        }
        Ok(traverse(&self.links, first, usize::from(!reverse)))
    }
    /// Orient by the original incoming/outgoing occurrence port at the anchor.
    /// A complete physical cycle is mandatory; disconnected subtours remain
    /// inspectable but cannot masquerade as an executable whole-tour permutation.
    pub fn cycle_positions(
        &self,
        first: ListPosition,
        reverse: bool,
    ) -> Result<Vec<ListPosition>, ListKOptError> {
        Ok(self
            .order(first, reverse)?
            .into_iter()
            .map(|v| self.positions[v])
            .collect())
    }
    pub fn movement(
        &self,
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        first: ListPosition,
        reverse: bool,
        lengths: &[usize],
    ) -> Result<ListMove<V>, ListKOptError> {
        problem.validate(before)?;
        for ((&entity, row), &prefix) in self.entities.iter().zip(&self.rows).zip(&self.prefixes) {
            let meta = problem
                .entities()
                .get(entity)
                .ok_or(ListError::EntityIndex {
                    entity,
                    count: problem.entities().len(),
                })?;
            if &before.lists()[entity] != row || meta.pinned_prefix() != prefix {
                return Err(ListKOptError::Partition);
            }
        }
        let order = self.order(first, reverse)?;
        let values = crate::list_k_opt_repartition::members(problem, before, &self.entities)?;
        crate::list_k_opt_repartition::repartition(
            problem,
            before,
            &self.entities,
            &values,
            &order,
            lengths,
        )
    }
}
fn traverse(links: &[[(usize, usize); 2]], first: usize, mut port: usize) -> Vec<usize> {
    let mut current = first;
    let mut tour = Vec::new();
    loop {
        tour.push(current);
        let (next, incoming) = links[current][port];
        current = next;
        port = 1 - incoming;
        if current == first {
            return tour;
        }
    }
}
