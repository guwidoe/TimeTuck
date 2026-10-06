//! Range metadata estimates and fresh initialization snapshots.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListEntity, ManagedRangeError, ManagedRangeKind,
    ManagedRangeTarget, ManagedValueRanges, ProblemScale,
};

/// Fresh counts from an assignment validated against one field's managed ranges.
/// Native list registry values are counted as shadow entities. Basic fields have
/// no shadow entities. Optional basic None is initialized; missing list values
/// are reported whether or not leaving values unassigned is allowed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RangeInitializationStatistics {
    pub genuine_entity_count: usize,
    pub shadow_entity_count: usize,
    pub uninitialized_variable_count: usize,
    pub unassigned_value_count: usize,
}
/// Approximate metadata size, independent of current assignments and pins.
/// Shared value counts are counted once; entity providers sum their range sizes.
/// Basic optional choices include None. List range restrictions use independent
/// eligibility fractions; their estimate is not an exact count or an upper bound.
/// A zero-sized required range uses the upstream zero-log reporting convention,
/// exposed separately by `has_empty_required_range`; scale never proves feasibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManagedRangeStatistics {
    pub entity_count: usize,
    pub variable_count: usize,
    pub approximate_value_count: usize,
    pub scale: ProblemScale,
    pub has_empty_required_range: bool,
}
impl ManagedRangeStatistics {
    /// Combine fields on the same logical entity rows, counting entities once.
    /// An empty collection describes an empty model. Different entity types must
    /// be combined by the caller with their own explicit row/type metadata.
    pub fn combine_fields(fields: &[Self]) -> Result<Self, ManagedRangeError> {
        let n = fields.first().map_or(0, |f| f.entity_count);
        let mut out = Self {
            entity_count: n,
            variable_count: 0,
            approximate_value_count: 0,
            scale: ProblemScale::from_log10(0.0),
            has_empty_required_range: false,
        };
        let mut log = 0.0;
        for field in fields {
            if field.entity_count != n {
                return Err(ManagedRangeError::EntityCount {
                    expected: n,
                    actual: field.entity_count,
                });
            }
            out.variable_count = out
                .variable_count
                .checked_add(field.variable_count)
                .ok_or(ManagedRangeError::CountOverflow)?;
            out.approximate_value_count = out
                .approximate_value_count
                .checked_add(field.approximate_value_count)
                .ok_or(ManagedRangeError::CountOverflow)?;
            out.has_empty_required_range |= field.has_empty_required_range;
            log += field.scale.log10();
        }
        out.scale = ProblemScale::from_log10(if out.has_empty_required_range {
            0.0
        } else {
            log
        });
        Ok(out)
    }
}
fn log_add(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let high = a.max(b);
    high + (1.0 + 10f64.powf(a.min(b) - high)).log10()
}
impl<V: Clone + Eq> ManagedValueRanges<V> {
    /// Compute counts and a base-10 search-space estimate from current providers.
    /// Optional lists sum estimates for all assigned subsets in logarithmic space;
    /// unassigned values have no ordering. O(n^2) time for optional n-value lists.
    pub fn statistics(&self) -> Result<ManagedRangeStatistics, ManagedRangeError> {
        let n = self.entity_count();
        let solution = self.get(ManagedRangeTarget::Solution)?;
        let entities = (0..n)
            .map(|i| self.get(ManagedRangeTarget::Entity(i)))
            .collect::<Result<Vec<_>, _>>()?;
        let value_count = if self.has_entity_providers() {
            entities.iter().try_fold(0usize, |a, r| {
                a.checked_add(r.values().len())
                    .ok_or(ManagedRangeError::CountOverflow)
            })?
        } else {
            solution.values().len()
        };
        let mut empty = false;
        let log = match self.kind() {
            ManagedRangeKind::Basic { .. } => {
                empty = entities.iter().any(|r| r.values().is_empty());
                if empty {
                    0.0
                } else {
                    entities
                        .iter()
                        .map(|r| (r.values().len() as f64).log10())
                        .sum()
                }
            }
            ManagedRangeKind::List { allows_unassigned } => {
                let weights: Vec<f64> = solution
                    .values()
                    .iter()
                    .map(|value| {
                        if n == 0 {
                            return f64::NEG_INFINITY;
                        }
                        let eligible = entities
                            .iter()
                            .filter(|r| r.values().contains(value))
                            .count();
                        (eligible as f64 / n as f64).log10()
                    })
                    .collect();
                if allows_unassigned {
                    // Elementary symmetric sums give subset eligibility weights.
                    let mut subsets = vec![f64::NEG_INFINITY; weights.len() + 1];
                    subsets[0] = 0.0;
                    for (i, weight) in weights.iter().enumerate() {
                        for k in (1..=i + 1).rev() {
                            subsets[k] = log_add(subsets[k], subsets[k - 1] + weight);
                        }
                    }
                    let mut total = 0.0;
                    let mut arrangement = 0.0;
                    for (k, subset) in subsets.iter().enumerate().skip(1) {
                        arrangement += (n as f64 + (k - 1) as f64).log10();
                        total = log_add(total, arrangement + subset);
                    }
                    total
                } else {
                    empty = weights.contains(&f64::NEG_INFINITY);
                    if empty {
                        0.0
                    } else {
                        weights
                            .iter()
                            .enumerate()
                            .map(|(i, w)| (n as f64 + i as f64).log10() + w)
                            .sum()
                    }
                }
            }
        };
        Ok(ManagedRangeStatistics {
            entity_count: n,
            variable_count: n,
            approximate_value_count: value_count,
            scale: ProblemScale::from_log10(log),
            has_empty_required_range: empty,
        })
    }
    /// Validate and inspect a fresh basic assignment. No cached initialization
    /// counters survive a reset; passing the new assignment refreshes the snapshot.
    pub fn basic_initialization(
        &self,
        assignment: &Assignment<V>,
    ) -> Result<RangeInitializationStatistics, ManagedRangeError> {
        if !matches!(self.kind(), ManagedRangeKind::Basic { .. }) {
            return Err(ManagedRangeError::WrongVariableKind);
        }
        let variables = (0..self.entity_count())
            .map(|i| self.variable(i))
            .collect::<Result<Vec<_>, _>>()?;
        let p = FiniteProblem::new(variables).map_err(ManagedRangeError::Model)?;
        p.validate(assignment).map_err(ManagedRangeError::Model)?;
        let missing = p
            .variables()
            .iter()
            .zip(assignment.values())
            .filter(|(v, a)| !v.allows_unassigned() && a.is_none())
            .count();
        Ok(RangeInitializationStatistics {
            genuine_entity_count: self.entity_count(),
            uninitialized_variable_count: missing,
            ..Default::default()
        })
    }
    pub fn list_initialization(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Result<RangeInitializationStatistics, ManagedRangeError> {
        let p = self.list_problem(vec![ListEntity::new(vec![]); self.entity_count()])?;
        p.validate(assignment).map_err(ManagedRangeError::List)?;
        let assigned = assignment.lists().iter().map(Vec::len).sum::<usize>();
        Ok(RangeInitializationStatistics {
            genuine_entity_count: self.entity_count(),
            shadow_entity_count: p.values().len(),
            unassigned_value_count: p.values().len() - assigned,
            ..Default::default()
        })
    }
}
