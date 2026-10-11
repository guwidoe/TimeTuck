//! Edge-defined reconnection of open movable list suffixes.
use crate::{ListAssignment, ListError, ListKOptError, ListMove, ListPosition, ListProblem};
use std::collections::{BTreeMap, BTreeSet};

/// An endpoint in an immutable list assignment. Start is immediately before the
/// first unpinned value; End is immediately after the last value. An empty suffix
/// has a direct Start--End edge. Value indices are absolute within their list.
/// Boundary nodes are distinct from domain values and from each other.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ListPathEndpoint {
    Start(usize),
    Value(ListPosition),
    End(usize),
}

impl<V: Eq + Clone> ListProblem<V> {
    /// Remove and add undirected edges across selected movable list suffixes.
    /// Each initial suffix is an open path from Start(entity) through its values
    /// to End(entity). Each final component must contain exactly one Start and
    /// one End. Traversal from Start determines both destination owner and order;
    /// End may belong to another original entity. No result lengths or orientation
    /// hints are needed. This supports cross-list tail exchanges, reversals and
    /// segment transfers, including transfers into empty lists.
    ///
    /// Entities must be distinct and not fully pinned. Edges may only reference
    /// their suffix values/boundaries. Equal cut/join counts are required; empty
    /// edits are allowed. Self edges, duplicate cuts/joins, missing cut edges,
    /// duplicate final edges and wrong degrees are rejected. Restoring a cut edge
    /// is allowed. Start--Start/End--End paths return Partition; orphan cycles
    /// return Disconnected. Other graph errors use ListKOptError's edge variants.
    ///
    /// Construction validates the input and complete result, preserving unique
    /// value ownership, prefixes, unselected rows, missing values and assigned
    /// payloads. No assignment or score is published on error or panic. The result
    /// is an ordinary snapshot-bound composite supporting existing scorers/undo;
    /// regenerate it after the assignment changes. Boundary semantics are explicit
    /// native behavior, not Java's implicit cyclic multi-entity k-opt convention.
    ///
    /// Topology storage is linear in selected values plus entities/edits; ordered
    /// edge validation adds logarithmic factors. Relocation construction uses the
    /// shared linear-vector path below 128 values and O(n log n) ranked path for
    /// larger proposals; shared full-model validation may still be quadratic.
    ///
    /// ```
    /// use timetuck::*;
    /// use ListPathEndpoint::Value;
    /// let p=ListProblem::new((0..4).collect(),vec![
    ///     ListEntity::new(vec![0,1]),ListEntity::new(vec![2,3])])?;
    /// let mut a=p.initial_assignment();
    /// let at=|entity,index|Value(ListPosition{entity,index});
    /// let movement=p.reconnect_list_paths(&a,&[0,1],
    ///     &[(at(0,0),at(0,1)),(at(1,0),at(1,1))],
    ///     &[(at(0,0),at(1,1)),(at(1,0),at(0,1))])?;
    /// { let trial=p.apply(&mut a,&movement)?;
    ///   assert_eq!(trial.lists(),&[vec![0,3],vec![2,1]]); }
    /// assert_eq!(a,p.initial_assignment());
    /// # Ok::<(),Box<dyn std::error::Error>>(())
    /// ```
    pub fn reconnect_list_paths(
        &self,
        before: &ListAssignment<V>,
        entities: &[usize],
        removed: &[(ListPathEndpoint, ListPathEndpoint)],
        added: &[(ListPathEndpoint, ListPathEndpoint)],
    ) -> Result<ListMove<V>, ListKOptError> {
        self.validate(before)?;
        let values = crate::list_k_opt_repartition::members(self, before, entities)?;
        if removed.len() != added.len() {
            return Err(ListKOptError::EdgeCount);
        }
        let rows: BTreeMap<_, _> = entities.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        let n = values.len();
        let endpoint = |point: ListPathEndpoint| -> Result<usize, ListKOptError> {
            let entity = match point {
                ListPathEndpoint::Start(e) | ListPathEndpoint::End(e) => e,
                ListPathEndpoint::Value(at) => at.entity,
            };
            let &row = rows.get(&entity).ok_or(ListKOptError::Entities)?;
            match point {
                ListPathEndpoint::Start(_) => Ok(n + 2 * row),
                ListPathEndpoint::End(_) => Ok(n + 2 * row + 1),
                ListPathEndpoint::Value(at) => {
                    let index = at
                        .index
                        .checked_sub(self.entities()[entity].pinned_prefix())
                        .ok_or(ListError::PinChanged { entity })?;
                    values
                        .flat_index(row, index)
                        .ok_or(ListError::ElementIndex(at).into())
                }
            }
        };
        let normalize = |input: &[(ListPathEndpoint, ListPathEndpoint)]| {
            let mut edges = BTreeSet::new();
            for &(a, b) in input {
                let (a, b) = (endpoint(a)?, endpoint(b)?);
                if a == b {
                    return Err(ListKOptError::InvalidEdge);
                }
                if !edges.insert(crate::list_k_opt::edge(a, b)) {
                    return Err(ListKOptError::DuplicateEdge);
                }
            }
            Ok(edges)
        };
        // Resolve all supplied identities before building the proposed topology.
        let removed = normalize(removed)?;
        let added = normalize(added)?;
        let mut edges = BTreeSet::new();
        for (row, start) in values.offsets().enumerate() {
            let end = start + values.segment(row).expect("selected row").len();
            let mut previous = n + 2 * row;
            for value in start..end {
                edges.insert(crate::list_k_opt::edge(previous, value));
                previous = value;
            }
            edges.insert(crate::list_k_opt::edge(previous, n + 2 * row + 1));
        }
        for edge in removed {
            if !edges.remove(&edge) {
                return Err(ListKOptError::InvalidEdge);
            }
        }
        for edge in added {
            if !edges.insert(edge) {
                return Err(ListKOptError::DuplicateEdge);
            }
        }
        let mut neighbors = vec![Vec::new(); n + 2 * entities.len()];
        for (a, b) in edges {
            neighbors[a].push(b);
            neighbors[b].push(a);
        }
        if neighbors
            .iter()
            .enumerate()
            .any(|(i, row)| row.len() != if i < n { 2 } else { 1 })
        {
            return Err(ListKOptError::Degree);
        }
        let mut visited = vec![false; neighbors.len()];
        let mut tour = Vec::with_capacity(n);
        let mut lengths = Vec::with_capacity(entities.len());
        for row in 0..entities.len() {
            let start = n + 2 * row;
            let offset = tour.len();
            if visited[start] {
                return Err(ListKOptError::Partition);
            }
            visited[start] = true;
            let mut previous = start;
            let mut current = neighbors[start][0];
            loop {
                if visited[current] {
                    return Err(ListKOptError::Disconnected);
                }
                visited[current] = true;
                if current >= n {
                    if (current - n).is_multiple_of(2) {
                        return Err(ListKOptError::Partition);
                    }
                    break;
                }
                tour.push(current);
                let adjacent = &neighbors[current];
                let next = if adjacent[0] == previous {
                    adjacent[1]
                } else {
                    adjacent[0]
                };
                previous = current;
                current = next;
            }
            lengths.push(tour.len() - offset);
        }
        if visited.iter().any(|seen| !seen) {
            return Err(ListKOptError::Disconnected);
        }
        crate::list_k_opt_repartition::repartition(self, before, entities, &values, &tour, &lengths)
    }
}
