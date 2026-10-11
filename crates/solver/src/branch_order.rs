//! Stable index permutations for bounded exact phases.
use crate::{FiniteProblem, ListProblem, MixedProblem};
use std::{error::Error, fmt};

/// Branch-layer and child priorities for [`crate::BoundedPhaseConfig`]. These
/// permutations affect bounded exact phases only; standalone bounded functions
/// and complete-assignment enumeration retain their existing order.
///
/// Every configured vector must be a full permutation of its model indices,
/// including pinned and already assigned entries. Validation precedes solver
/// callbacks, even at zero budget. Pins and completion-fixed entries are then
/// filtered without changing the surviving order. Unspecified dimensions retain
/// model order. Domain entries identify canonical model values, not new payloads.
///
/// Optional unassigned/omission choices remain last. List insertion positions
/// remain ascending within each entity. Mixed trees still place list values
/// before basic variables. [`crate::NodeExploration`] independently chooses which
/// generated partial node to expand; sibling scores are calculated on generation.
///
/// ```
/// use timetuck::*;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![10_i64, 20]); 2])?;
/// let scores = ConstraintNetwork::<_, SimpleScore>::new(2);
/// let result = solve(&problem, &scores, SolverSequence {
///     phases: vec![SolverPhase::Exact(ExactPhaseConfig {
///         strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
///             order: Some(BranchOrder {
///                 basic_variables: Some(vec![1, 0]),
///                 basic_values: Some(vec![vec![1, 0], vec![1, 0]]),
///                 ..Default::default()
///             }.into()),
///             exploration: NodeExploration::InputOrderDepthFirst,
///             ..Default::default()
///         }), ..Default::default()
///     })], ..Default::default()
/// })?;
/// assert_eq!(result.assignment.values(), &[Some(20), Some(20)]);
/// assert!(result.is_optimal());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Changing order can change ties, finite-budget results and pruning work, but
/// never the eligible full search space or admissibility requirements for bounds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BranchOrder {
    pub basic_variables: Option<Vec<usize>>,
    /// One full domain-index permutation per basic variable, in model variable
    /// order (regardless of basic_variables). Optional None is not a domain index.
    pub basic_values: Option<Vec<Vec<usize>>>,
    pub list_values: Option<Vec<usize>>,
    pub list_entities: Option<Vec<usize>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchOrderDimension {
    BasicVariables,
    BasicDomains,
    BasicValues { variable: usize },
    ListValues,
    ListEntities,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BranchOrderError {
    Unavailable {
        dimension: BranchOrderDimension,
    },
    Length {
        dimension: BranchOrderDimension,
        expected: usize,
        actual: usize,
    },
    OutOfRange {
        dimension: BranchOrderDimension,
        position: usize,
        value: usize,
        length: usize,
    },
    Duplicate {
        dimension: BranchOrderDimension,
        position: usize,
        value: usize,
    },
}
impl fmt::Display for BranchOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid branch order: {self:?}")
    }
}
impl Error for BranchOrderError {}
fn permutation(
    order: &[usize],
    length: usize,
    dimension: BranchOrderDimension,
) -> Result<(), BranchOrderError> {
    if order.len() != length {
        return Err(BranchOrderError::Length {
            dimension,
            expected: length,
            actual: order.len(),
        });
    }
    let mut seen = vec![false; length];
    for (position, &value) in order.iter().enumerate() {
        let Some(slot) = seen.get_mut(value) else {
            return Err(BranchOrderError::OutOfRange {
                dimension,
                position,
                value,
                length,
            });
        };
        if std::mem::replace(slot, true) {
            return Err(BranchOrderError::Duplicate {
                dimension,
                position,
                value,
            });
        }
    }
    Ok(())
}
impl BranchOrder {
    fn basic<V: Eq>(&self, p: &FiniteProblem<V>) -> Result<(), BranchOrderError> {
        use BranchOrderDimension as D;
        if let Some(order) = &self.basic_variables {
            permutation(order, p.variables().len(), D::BasicVariables)?;
        }
        if let Some(rows) = &self.basic_values {
            if rows.len() != p.variables().len() {
                return Err(BranchOrderError::Length {
                    dimension: D::BasicDomains,
                    expected: p.variables().len(),
                    actual: rows.len(),
                });
            }
            for (variable, (row, meta)) in rows.iter().zip(p.variables()).enumerate() {
                permutation(row, meta.domain().len(), D::BasicValues { variable })?;
            }
        }
        Ok(())
    }
    fn lists<V: Eq + Clone>(&self, p: &ListProblem<V>) -> Result<(), BranchOrderError> {
        use BranchOrderDimension as D;
        if let Some(order) = &self.list_values {
            permutation(order, p.values().len(), D::ListValues)?;
        }
        if let Some(order) = &self.list_entities {
            permutation(order, p.entities().len(), D::ListEntities)?;
        }
        Ok(())
    }
    fn without_basic(&self) -> Result<(), BranchOrderError> {
        let dimension = if self.basic_variables.is_some() {
            Some(BranchOrderDimension::BasicVariables)
        } else if self.basic_values.is_some() {
            Some(BranchOrderDimension::BasicDomains)
        } else {
            None
        };
        dimension.map_or(Ok(()), |dimension| {
            Err(BranchOrderError::Unavailable { dimension })
        })
    }
    fn without_lists(&self) -> Result<(), BranchOrderError> {
        let dimension = if self.list_values.is_some() {
            Some(BranchOrderDimension::ListValues)
        } else if self.list_entities.is_some() {
            Some(BranchOrderDimension::ListEntities)
        } else {
            None
        };
        dimension.map_or(Ok(()), |dimension| {
            Err(BranchOrderError::Unavailable { dimension })
        })
    }
}
pub(crate) trait OrderedBranchModel {
    fn validate_branch_order(&self, order: &BranchOrder) -> Result<(), BranchOrderError>;
}
impl<V: Eq> OrderedBranchModel for FiniteProblem<V> {
    fn validate_branch_order(&self, order: &BranchOrder) -> Result<(), BranchOrderError> {
        order.without_lists()?;
        order.basic(self)
    }
}
impl<V: Eq + Clone> OrderedBranchModel for ListProblem<V> {
    fn validate_branch_order(&self, order: &BranchOrder) -> Result<(), BranchOrderError> {
        order.without_basic()?;
        order.lists(self)
    }
}
impl<B: Eq + Clone, V: Eq + Clone> OrderedBranchModel for MixedProblem<B, V> {
    fn validate_branch_order(&self, order: &BranchOrder) -> Result<(), BranchOrderError> {
        order.basic(self.basic())?;
        order.lists(self.lists())
    }
}
/// Input is validated once by the plan. Reorder only the remaining layers.
pub(crate) fn reorder_remaining(remaining: &mut Vec<usize>, order: Option<&[usize]>) {
    if let Some(order) = order {
        let mut present = vec![false; order.len()];
        for &i in remaining.iter() {
            present[i] = true;
        }
        remaining.clear();
        remaining.extend(order.iter().copied().filter(|&i| present[i]));
    }
}
