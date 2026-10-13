//! Odd sequential k-opt proposals from live selector draws and route positions.
use crate::*;
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SequentialKOptError {
    Weight(ListKOptWeightError),
    InvalidDraw,
    UnsupportedK(usize),
    MissingValue,
    Topology(ListKOptError),
}
impl fmt::Display for SequentialKOptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sequential k-opt: {self:?}")
    }
}
impl Error for SequentialKOptError {}
impl From<ListKOptError> for SequentialKOptError {
    fn from(e: ListKOptError) -> Self {
        Self::Topology(e)
    }
}
/// Optional external-draw selector. Weights select consecutive k values exactly;
/// this entrypoint handles odd sequential proposals only. Even k requires a
/// nonsequential/subtour-repair policy and is explicitly rejected. No PRNG or JVM
/// distribution parity is implied. Existing exhaustive/random providers are unchanged.
#[derive(Clone, Debug)]
pub struct ListKOptSequentialSelector {
    sizes: ListKOptConfig,
    weights: Vec<u64>,
    mass: u64,
}
impl ListKOptSequentialSelector {
    pub fn new(sizes: ListKOptConfig, weights: Vec<u64>) -> Result<Self, SequentialKOptError> {
        let expected = sizes.maximum() - sizes.minimum() + 1;
        if weights.len() != expected {
            return Err(SequentialKOptError::Weight(ListKOptWeightError::Length {
                expected,
                actual: weights.len(),
            }));
        }
        let mass = weights
            .iter()
            .try_fold(0u64, |sum, &w| sum.checked_add(w))
            .ok_or(SequentialKOptError::Weight(ListKOptWeightError::Overflow))?;
        if mass == 0 {
            return Err(SequentialKOptError::Weight(ListKOptWeightError::ZeroMass));
        }
        Ok(Self {
            sizes,
            weights,
            mass,
        })
    }
    pub fn distribution_mass(&self) -> u64 {
        self.mass
    }
    /// Draw order is k distribution, origin, direction, then each value and its
    /// direction. Successor/predecessor uses the actual movable suffix's local
    /// wrap. A missing, pinned or duplicate cut rejects before publication.
    /// Draw callbacks may have side effects and are not rolled back on errors.
    pub fn select<V: Eq + Clone>(
        &self,
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        draw: u64,
        mut origin: impl FnMut() -> Option<V>,
        mut value: impl FnMut() -> Option<V>,
        mut forward: impl FnMut() -> bool,
    ) -> Result<SequentialKOptProposal<V>, SequentialKOptError> {
        problem.validate(before).map_err(ListKOptError::from)?;
        if draw >= self.mass {
            return Err(SequentialKOptError::InvalidDraw);
        }
        let mut cumulative = 0;
        let mut k = self.sizes.minimum();
        for (offset, &weight) in self.weights.iter().enumerate() {
            cumulative += weight;
            if draw < cumulative {
                k += offset;
                break;
            }
        }
        if k.is_multiple_of(2) {
            return Err(SequentialKOptError::UnsupportedK(k));
        }
        let movable = problem
            .entities()
            .iter()
            .zip(before.lists())
            .filter(|(e, _)| !e.is_pinned())
            .map(|(e, row)| row.len() - e.pinned_prefix())
            .sum::<usize>();
        if k > movable {
            return Err(ListKOptError::TooFewValues.into());
        }
        let mut removed = Vec::with_capacity(k);
        let mut endpoints = Vec::with_capacity(2 * k);
        for index in 0..k {
            let selected = if index == 0 { origin() } else { value() }
                .ok_or(SequentialKOptError::MissingValue)?;
            let a = before
                .lists()
                .iter()
                .enumerate()
                .find_map(|(entity, row)| {
                    row.iter()
                        .position(|v| v == &selected)
                        .map(|index| ListPosition { entity, index })
                })
                .ok_or(SequentialKOptError::MissingValue)?;
            let meta = &problem.entities()[a.entity];
            let row = &before.lists()[a.entity];
            let prefix = meta.pinned_prefix();
            if meta.is_pinned() || a.index < prefix {
                return Err(ListKOptError::InvalidEdge.into());
            }
            let index = if forward() {
                if a.index + 1 == row.len() {
                    prefix
                } else {
                    a.index + 1
                }
            } else if a.index == prefix {
                row.len() - 1
            } else {
                a.index - 1
            };
            let b = ListPosition {
                entity: a.entity,
                index,
            };
            removed.push((a, b));
            endpoints.extend([row[a.index].clone(), row[b.index].clone()]);
        }
        // Join the end of each selected edge to the start of the next cut; the
        // last end closes to the first start. Valid test draw orders induce one abstract cycle; arbitrary cut orders may
        // produce several components, which the physical plan keeps inspectable.
        let added: Vec<_> = (0..k).map(|i| (2 * i + 1, 2 * ((i + 1) % k))).collect();
        let mut partners = vec![0; 2 * k];
        for &(a, b) in &added {
            partners[a] = b;
            partners[b] = a;
        }
        let physical = ListKOptLocalCyclePlan::prepare(problem, before, &removed, &added)?;
        Ok(SequentialKOptProposal {
            removed,
            endpoints,
            partners,
            physical,
        })
    }
}
/// Endpoint metadata is the actual proposal's cut/draw order, with zero-based
/// occurrence slots and no Java sentinel. The physical plan preserves original
/// local adjacencies and needs explicit row lengths; it does NOT claim reference
/// virtual-seam conversion/implicit repartition semantics. Disconnected physical
/// components remain inspectable and cannot execute as a single tour.
#[derive(Clone, Debug)]
pub struct SequentialKOptProposal<V> {
    removed: Vec<(ListPosition, ListPosition)>,
    endpoints: Vec<V>,
    partners: Vec<usize>,
    physical: ListKOptLocalCyclePlan<V>,
}
impl<V: Eq + Clone> SequentialKOptProposal<V> {
    pub fn k(&self) -> usize {
        self.removed.len()
    }
    pub fn removed_edges(&self) -> &[(ListPosition, ListPosition)] {
        &self.removed
    }
    pub fn removed_values(&self) -> &[V] {
        &self.endpoints
    }
    pub fn endpoint_partners(&self) -> &[usize] {
        &self.partners
    }
    pub fn physical_plan(&self) -> &ListKOptLocalCyclePlan<V> {
        &self.physical
    }
}
