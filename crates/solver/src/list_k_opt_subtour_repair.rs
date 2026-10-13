//! Optional combined-tour even-k proposals with callback-selected subtour splicing.
//! Selected movable suffixes use actual combined-cycle adjacency. No surplus
//! Java endpoint-buffer/virtual-port metadata is inferred.
use crate::*;
use std::{error::Error, fmt};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListKOptRepairError {
    InvalidConfig,
    InvalidDraw,
    UnsupportedK(usize),
    MissingValue,
    RepairBudget,
    InvalidRepairChoice,
    Topology(ListKOptError),
}
impl fmt::Display for ListKOptRepairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "combined subtour repair: {self:?}")
    }
}
impl Error for ListKOptRepairError {}
impl From<ListKOptError> for ListKOptRepairError {
    fn from(e: ListKOptError) -> Self {
        Self::Topology(e)
    }
}
#[derive(Clone, Debug)]
pub struct ListKOptSubtourRepairSelector {
    sizes: ListKOptConfig,
    weights: Vec<u64>,
    mass: u64,
}
fn retained(removed: &[(ListPosition, ListPosition)], a: ListPosition, b: ListPosition) -> bool {
    !removed
        .iter()
        .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
}
impl ListKOptSubtourRepairSelector {
    /// Prefix weights select initial consecutive k values; maximum additionally
    /// bounds actual cuts added by repairs. Unweighted sizes are not selected.
    pub fn new(sizes: ListKOptConfig, weights: Vec<u64>) -> Result<Self, ListKOptRepairError> {
        let mass = weights
            .iter()
            .try_fold(0u64, |sum, &w| sum.checked_add(w))
            .ok_or(ListKOptRepairError::InvalidConfig)?;
        if weights.is_empty() || weights.len() > sizes.maximum() - sizes.minimum() + 1 || mass == 0
        {
            return Err(ListKOptRepairError::InvalidConfig);
        }
        Ok(Self {
            sizes,
            weights,
            mass,
        })
    }
    /// Initial origin/value/direction callbacks read the current assignment.
    /// On each repair the next value chooses a live row neighborhood. An
    /// independent draw chooses one retained original edge; a retained edge selected
    /// near existing cut boundaries and the live neighborhood supplies a bridge in
    /// a different actual combined component. Cross-splicing joins those paths;
    /// no expected values, names or reference partner tails are used.
    // Separate borrowed callbacks retain distinct selector draw/lifecycle roles.
    #[allow(clippy::too_many_arguments)]
    pub fn select<V: Eq + Clone>(
        &self,
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        draw: u64,
        mut origin: impl FnMut() -> Option<V>,
        mut value: impl FnMut() -> Option<V>,
        mut forward: impl FnMut() -> bool,
        mut repair_cut: impl FnMut(
            &[Vec<ListPosition>],
            &[(ListPosition, ListPosition)],
        ) -> Option<(ListPosition, bool)>,
    ) -> Result<ListKOptRepairProposal<V>, ListKOptRepairError> {
        problem.validate(before).map_err(ListKOptError::from)?;
        if draw >= self.mass {
            return Err(ListKOptRepairError::InvalidDraw);
        }
        let mut cumulative = 0;
        let mut k = self.sizes.minimum();
        for (i, &w) in self.weights.iter().enumerate() {
            cumulative += w;
            if draw < cumulative {
                k += i;
                break;
            }
        }
        if !k.is_multiple_of(2) {
            return Err(ListKOptRepairError::UnsupportedK(k));
        }
        let locate = |selected: &V| {
            before.lists().iter().enumerate().find_map(|(entity, row)| {
                row.iter()
                    .position(|v| v == selected)
                    .map(|index| ListPosition { entity, index })
            })
        };
        // Draw all owners before resolving adjacency. A row-end cut uses the
        // next selected suffix, never that row's nonexistent local wrap edge.
        let mut starts = vec![];
        let mut entities = vec![];
        for index in 0..k {
            let selected = (if index == 0 { origin() } else { value() })
                .ok_or(ListKOptRepairError::MissingValue)?;
            let at = locate(&selected).ok_or(ListKOptRepairError::MissingValue)?;
            let meta = &problem.entities()[at.entity];
            if meta.is_pinned() || at.index < meta.pinned_prefix() {
                return Err(ListKOptRepairError::InvalidRepairChoice);
            }
            if !entities.contains(&at.entity) {
                entities.push(at.entity);
            }
            starts.push((at, forward()));
        }
        let mut positions = vec![];
        for &entity in &entities {
            positions.extend(
                (problem.entities()[entity].pinned_prefix()..before.lists()[entity].len())
                    .map(|index| ListPosition { entity, index }),
            );
        }
        let neighbor = |at: ListPosition,
                        direction: bool|
         -> Result<ListPosition, ListKOptRepairError> {
            let i = positions
                .iter()
                .position(|&p| p == at)
                .ok_or(ListKOptRepairError::InvalidRepairChoice)?;
            Ok(positions[(i + if direction { 1 } else { positions.len() - 1 }) % positions.len()])
        };
        let mut removed = starts
            .into_iter()
            .map(|(at, direction)| Ok((at, neighbor(at, direction)?)))
            .collect::<Result<Vec<_>, ListKOptRepairError>>()?;
        let mut added: Vec<_> = (0..k).map(|i| (2 * i + 1, 2 * ((i + 1) % k))).collect();
        let coordinate = |p: ListPosition| {
            positions
                .iter()
                .position(|&x| x == p)
                .ok_or(ListKOptRepairError::InvalidRepairChoice)
        };
        let topology = |cuts: &[(ListPosition, ListPosition)],
                        added: &[(usize, usize)]|
         -> Result<ListKOptTopology, ListKOptRepairError> {
            let cuts: Vec<_> = cuts
                .iter()
                .map(|&(a, b)| Ok((coordinate(a)?, coordinate(b)?)))
                .collect::<Result<_, ListKOptRepairError>>()?;
            let endpoints: Vec<_> = cuts.iter().flat_map(|&(a, b)| [a, b]).collect();
            let added: Vec<_> = added
                .iter()
                .map(|&(a, b)| (endpoints[a], endpoints[b]))
                .collect();
            Ok(ListKOptTopology::new(0..positions.len(), &cuts, &added)?)
        };
        let mut combined = topology(&removed, &added)?;
        let original_components = combined.cycle_count();
        while combined.cycle_count() > 1 {
            if removed.len() + 2 > self.sizes.maximum() {
                return Err(ListKOptRepairError::RepairBudget);
            }
            let selected = value().ok_or(ListKOptRepairError::MissingValue)?;
            let position = locate(&selected).ok_or(ListKOptRepairError::MissingValue)?;
            let components: Vec<Vec<ListPosition>> = combined
                .cycles()
                .iter()
                .map(|cycle| cycle.iter().map(|&i| positions[i]).collect())
                .collect();
            let (start, direction) = repair_cut(&components, &removed)
                .ok_or(ListKOptRepairError::InvalidRepairChoice)?;
            let end = neighbor(start, direction)?;
            let other = combined
                .cycle_id(coordinate(start)?)
                .ok_or(ListKOptRepairError::InvalidRepairChoice)?;
            if combined.cycle_id(coordinate(end)?) != Some(other) || !retained(&removed, start, end)
            {
                return Err(ListKOptRepairError::InvalidRepairChoice);
            }
            // The extra selector value identifies a live row neighborhood. Pick
            // a nearby retained edge in that row, then fall back to other rows.
            // Its original edge must belong to a different
            // actual combined component from the independently drawn repair edge.
            // Prefer a cut-adjacent retained edge to concentrate changes near
            // existing boundaries, but inspect every original retained edge.
            // Both directions permit reverse initial draws and cross-row seams.
            let endpoints: Vec<_> = removed.iter().flat_map(|&(a, b)| [a, b]).collect();
            let anchor = positions
                .iter()
                .copied()
                .flat_map(|a| {
                    [true, false]
                        .into_iter()
                        .map(move |direction| (a, direction))
                })
                .filter_map(|(a, direction)| {
                    let b = neighbor(a, direction).ok()?;
                    let component = combined.cycle_id(coordinate(a).ok()?)?;
                    (component != other
                        && combined.cycle_id(coordinate(b).ok()?) == Some(component)
                        && retained(&removed, a, b))
                    .then_some((a, b))
                })
                .min_by_key(|&(a, b)| {
                    (
                        !endpoints.contains(&a),
                        a.entity != position.entity,
                        a.index.abs_diff(position.index),
                        a.entity,
                        a.index,
                        b.entity,
                        b.index,
                    )
                })
                .ok_or(ListKOptRepairError::InvalidRepairChoice)?;
            let offset = removed.len() * 2;
            removed.extend([anchor, (start, end)]);
            added.extend([(offset, offset + 3), (offset + 1, offset + 2)]);
            let next = topology(&removed, &added)?;
            if next.cycle_count() + 1 != combined.cycle_count() {
                return Err(ListKOptRepairError::InvalidRepairChoice);
            }
            combined = next;
        }
        let physical = ListKOptCombinedRepairPlan {
            entities: entities.clone(),
            rows: entities
                .iter()
                .map(|&e| before.lists()[e].clone())
                .collect(),
            prefixes: entities
                .iter()
                .map(|&e| problem.entities()[e].pinned_prefix())
                .collect(),
            positions,
            topology: combined,
        };
        let endpoints = removed
            .iter()
            .flat_map(|&(a, b)| {
                [
                    before.lists()[a.entity][a.index].clone(),
                    before.lists()[b.entity][b.index].clone(),
                ]
            })
            .collect();
        let mut partners = vec![0; removed.len() * 2];
        for (a, b) in added {
            partners[a] = b;
            partners[b] = a;
        }
        Ok(ListKOptRepairProposal {
            removed,
            endpoints,
            partners,
            physical,
            initial_components: original_components,
        })
    }
}
#[derive(Clone, Debug)]
pub struct ListKOptRepairProposal<V> {
    removed: Vec<(ListPosition, ListPosition)>,
    endpoints: Vec<V>,
    partners: Vec<usize>,
    physical: ListKOptCombinedRepairPlan<V>,
    initial_components: usize,
}
impl<V: Eq + Clone> ListKOptRepairProposal<V> {
    pub fn k(&self) -> usize {
        self.removed.len()
    }
    pub fn removed_edges(&self) -> &[(ListPosition, ListPosition)] {
        &self.removed
    }
    pub fn removed_values(&self) -> &[V] {
        &self.endpoints
    }
    /// Exactly two active occurrence ports per actual removed edge; no Java
    /// sentinel or API-visible surplus partner storage is represented.
    pub fn active_endpoint_partners(&self) -> &[usize] {
        &self.partners
    }
    pub fn endpoint_partners(&self) -> &[usize] {
        self.active_endpoint_partners()
    }
    pub fn physical_plan(&self) -> &ListKOptCombinedRepairPlan<V> {
        &self.physical
    }
    pub fn initial_component_count(&self) -> usize {
        self.initial_components
    }
}

/// Snapshot-checked actual combined-tour geometry; explicit lengths select rows.
#[derive(Clone, Debug)]
pub struct ListKOptCombinedRepairPlan<V> {
    entities: Vec<usize>,
    rows: Vec<Vec<V>>,
    prefixes: Vec<usize>,
    positions: Vec<ListPosition>,
    topology: ListKOptTopology,
}
impl<V: Eq + Clone> ListKOptCombinedRepairPlan<V> {
    pub fn physical_cycle_count(&self) -> usize {
        self.topology.cycle_count()
    }
    pub fn cycle_positions(
        &self,
        first: ListPosition,
        reverse: bool,
    ) -> Result<Vec<ListPosition>, ListKOptError> {
        let first = self
            .positions
            .iter()
            .position(|&p| p == first)
            .ok_or(ListKOptError::FirstSuccessor)?;
        let cycle = &self.topology.cycles()[0];
        let at = cycle
            .iter()
            .position(|&i| i == first)
            .ok_or(ListKOptError::FirstSuccessor)?;
        let successor = cycle[(at + if reverse { cycle.len() - 1 } else { 1 }) % cycle.len()];
        self.topology
            .order_from(first, successor)
            .map(|order| order.into_iter().map(|i| self.positions[i]).collect())
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
        for (i, &entity) in self.entities.iter().enumerate() {
            if entity >= problem.entities().len() || entity >= before.lists().len() {
                return Err(ListKOptError::Entities);
            }
            if problem.entities()[entity].is_pinned()
                || problem.entities()[entity].pinned_prefix() != self.prefixes[i]
                || before.lists()[entity] != self.rows[i]
            {
                return Err(ListKOptError::InvalidEdge);
            }
        }
        let order = self.cycle_positions(first, reverse)?;
        let coordinate = |p| {
            self.positions
                .iter()
                .position(|&x| x == p)
                .ok_or(ListKOptError::FirstSuccessor)
        };
        problem.k_opt_repartition_move(
            before,
            &self.entities,
            self.topology.removed_edges(),
            self.topology.added_edges(),
            (coordinate(order[0])?, coordinate(order[1])?),
            lengths,
        )
    }
}
