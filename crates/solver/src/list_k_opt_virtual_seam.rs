//! Three-cut reconnection across two declared virtual rows.
//! Supported topology: a local wrap cut and an internal cut on one row,
//! one internal cut on a second row, and three alternating cross links.
use crate::{ListAssignment, ListKOptError, ListMove, ListPosition, ListProblem};
/// Actual list brackets around the compound forward mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualSeamListChange {
    Before {
        entity: usize,
        from: usize,
        to: usize,
    },
    After {
        entity: usize,
        from: usize,
        to: usize,
    },
}
#[derive(Clone, Debug)]
pub struct VirtualSeam3OptPlan<V> {
    entities: [usize; 2],
    shape: [(usize, usize); 2],
    positions: [ListPosition; 6],
    endpoints: [V; 6],
}
impl<V: Eq + Clone> VirtualSeam3OptPlan<V> {
    pub fn prepare(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        removed: &[(ListPosition, ListPosition)],
        partners: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        problem.validate(before)?;
        if removed.len() != 3 || partners.len() != 3 {
            return Err(ListKOptError::EdgeCount);
        }
        let mut wrap = None;
        for (i, &(a, b)) in removed.iter().enumerate() {
            if a.entity != b.entity {
                return Err(ListKOptError::InvalidEdge);
            }
            let meta = problem
                .entities()
                .get(a.entity)
                .ok_or(ListKOptError::Entities)?;
            if meta.is_pinned() {
                return Err(crate::ListError::PinChanged { entity: a.entity }.into());
            }
            let row = &before.lists()[a.entity];
            let prefix = meta.pinned_prefix();
            if a.index < prefix || b.index < prefix || a.index >= row.len() || b.index >= row.len()
            {
                return Err(ListKOptError::InvalidEdge);
            }
            let same_owner_cuts = removed.iter().filter(|(x, _)| x.entity == a.entity).count();
            if same_owner_cuts == 2
                && ((a.index == row.len() - 1 && b.index == prefix)
                    || (b.index == row.len() - 1 && a.index == prefix))
            {
                if wrap.is_some() {
                    return Err(ListKOptError::InvalidEdge);
                }
                wrap = Some((
                    i,
                    if a.index > b.index {
                        (a, b, 2 * i, 2 * i + 1)
                    } else {
                        (b, a, 2 * i + 1, 2 * i)
                    },
                ));
            }
        }
        let (wi, (tail, head, ts, hs)) = wrap.ok_or(ListKOptError::InvalidEdge)?;
        let mut internal = None;
        let mut other = None;
        for (i, &(a, b)) in removed.iter().enumerate() {
            if i == wi {
                continue;
            }
            let (left, right, ls, rs) = if a.index < b.index {
                (a, b, 2 * i, 2 * i + 1)
            } else {
                (b, a, 2 * i + 1, 2 * i)
            };
            if left.index + 1 != right.index {
                return Err(ListKOptError::InvalidEdge);
            }
            if a.entity == head.entity {
                if internal.is_some() {
                    return Err(ListKOptError::InvalidEdge);
                }
                internal = Some((left, right, ls, rs));
            } else {
                if other.is_some() {
                    return Err(ListKOptError::InvalidEdge);
                }
                other = Some((left, right, ls, rs));
            }
        }
        let (left, right, ls, rs) = internal.ok_or(ListKOptError::InvalidEdge)?;
        let (b_left, b_right, bls, brs) = other.ok_or(ListKOptError::InvalidEdge)?;
        if left.index <= head.index || right.index >= tail.index {
            return Err(ListKOptError::InvalidEdge);
        }
        let mut matching = [usize::MAX; 6];
        for &(a, b) in partners {
            if a >= 6 || b >= 6 || a == b {
                return Err(ListKOptError::InvalidEdge);
            }
            if matching[a] != usize::MAX || matching[b] != usize::MAX {
                return Err(ListKOptError::DuplicateEdge);
            }
            matching[a] = b;
            matching[b] = a;
        }
        if matching[hs] != rs || matching[ls] != brs || matching[bls] != ts {
            return Err(ListKOptError::InvalidEdge);
        }
        let positions = [tail, head, left, right, b_left, b_right];
        let entities = [head.entity, b_left.entity];
        crate::list_k_opt_repartition::members(problem, before, &entities)?;
        let shape = entities.map(|e| {
            (
                before.lists()[e].len(),
                problem.entities()[e].pinned_prefix(),
            )
        });
        let endpoints = positions.map(|p| before.lists()[p.entity][p.index].clone());
        Ok(Self {
            entities,
            shape,
            positions,
            endpoints,
        })
    }
    pub fn selected_entities(&self) -> [usize; 2] {
        self.entities
    }
    pub fn movement(
        &self,
        problem: &ListProblem<V>,
        current: &ListAssignment<V>,
    ) -> Result<ListMove<V>, ListKOptError> {
        problem.validate(current)?;
        for (&e, &(n, p)) in self.entities.iter().zip(&self.shape) {
            if problem
                .entities()
                .get(e)
                .is_none_or(|meta| meta.pinned_prefix() != p || current.lists()[e].len() != n)
            {
                return Err(ListKOptError::Partition);
            }
        }
        if self
            .positions
            .iter()
            .zip(&self.endpoints)
            .any(|(p, v)| current.lists()[p.entity][p.index] != *v)
        {
            return Err(ListKOptError::Partition);
        }
        let values = crate::list_k_opt_repartition::members(problem, current, &self.entities)?;
        let n = values.len();
        let a_len = self.shape[0].0 - self.shape[0].1;
        let left = self.positions[2].index - self.shape[0].1;
        let right = left + 1;
        let b_left = a_len + self.positions[4].index - self.shape[1].1;
        let b_right = b_left + 1;
        let mut tour = (0..n).collect::<Vec<_>>();
        // Fix the two added edges incident to the anchor and the second row's
        // left port by interval reversals; orient the completed cycle from its anchor.
        tour[1..=right].reverse();
        let pivot = tour.iter().position(|&v| v == right).expect("ordinal");
        let end = tour.iter().position(|&v| v == b_left).expect("ordinal");
        tour[pivot + 1..=end].reverse();
        tour[1..].reverse();
        // The uncut virtual wrap gap is an owned forward block, rather than a
        // collection of endpoint labels. Preserve its internal payload orientation.
        let gap_len = n - b_right;
        tour[1..1 + gap_len].reverse();
        // Tail ownership transfers at the original cut gaps, independently of the
        // later interval orientation. Pins stay outside the virtual rows.
        let first_length = left + gap_len;
        crate::list_k_opt_repartition::repartition(
            problem,
            current,
            &self.entities,
            &values,
            &tour,
            &[first_length, n - first_length],
        )
    }
    pub fn is_doable(&self, problem: &ListProblem<V>, current: &ListAssignment<V>) -> bool {
        self.movement(problem, current).is_ok()
    }
    pub fn apply_notifying<'a>(
        &self,
        problem: &ListProblem<V>,
        current: &'a mut ListAssignment<V>,
        mut notify: impl FnMut(VirtualSeamListChange, &ListAssignment<V>),
    ) -> Result<crate::AppliedListMove<'a, V>, ListKOptError> {
        let movement = self.movement(problem, current)?;
        for (&e, &(_, prefix)) in self.entities.iter().zip(&self.shape) {
            notify(
                VirtualSeamListChange::Before {
                    entity: e,
                    from: prefix,
                    to: current.lists()[e].len(),
                },
                current,
            );
        }
        let trial = problem.apply(current, &movement)?;
        for (&e, &(_, prefix)) in self.entities.iter().zip(&self.shape) {
            notify(
                VirtualSeamListChange::After {
                    entity: e,
                    from: prefix,
                    to: trial.lists()[e].len(),
                },
                &trial,
            );
        }
        Ok(trial)
    }
    pub fn rebase<W: Eq + Clone>(
        &self,
        problem: &ListProblem<W>,
        current: &ListAssignment<W>,
        mut entity_lookup: impl FnMut(usize) -> Option<usize>,
        mut value_lookup: impl FnMut(&V) -> Option<W>,
    ) -> Result<VirtualSeam3OptPlan<W>, ListKOptError> {
        problem.validate(current)?;
        let owners = [
            entity_lookup(self.entities[0]).ok_or(ListKOptError::Entities)?,
            entity_lookup(self.entities[1]).ok_or(ListKOptError::Entities)?,
        ];
        let mut mapped = Vec::with_capacity(6);
        for v in &self.endpoints {
            mapped.push(value_lookup(v).ok_or(ListKOptError::InvalidEdge)?);
        }
        let mut positions = Vec::with_capacity(6);
        for (i, v) in mapped.iter().enumerate() {
            let owner = owners[usize::from(i >= 4)];
            let row = current.lists().get(owner).ok_or(ListKOptError::Entities)?;
            let index = row
                .iter()
                .position(|x| x == v)
                .ok_or(ListKOptError::InvalidEdge)?;
            positions.push(ListPosition {
                entity: owner,
                index,
            });
        }
        VirtualSeam3OptPlan::prepare(
            problem,
            current,
            &[
                (positions[0], positions[1]),
                (positions[2], positions[3]),
                (positions[4], positions[5]),
            ],
            &[(1, 3), (2, 5), (4, 0)],
        )
    }
}
