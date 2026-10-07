//! Borrowed value-range diagnostics for rejected assignment snapshots.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListPosition, ListProblem,
    MixedAssignment, MixedProblem, ModelError,
};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeViolationLocation {
    Basic { variable: usize },
    List { position: ListPosition },
}

/// Exact rejected payload and the range that rejected it, in declared order.
/// References borrow the model and candidate: inspecting an error clones neither.
/// List optionality describes whether values may remain unassigned; it does not
/// introduce a null element into the list's range.
#[derive(Debug, Eq, PartialEq)]
pub struct ValueRangeViolation<'a, V> {
    pub location: RangeViolationLocation,
    pub value: &'a V,
    pub allowed: &'a [V],
    pub allows_unassigned: bool,
}
impl<V> ValueRangeViolation<'_, V> {
    /// Supply application labels and a value renderer without storing display
    /// names in the solver's identity space. Every allowed value is displayed;
    /// sparse ranges are never presented as a continuous interval.
    pub fn describe(
        &self,
        entity: &str,
        variable: &str,
        mut render: impl FnMut(&V) -> String,
    ) -> String {
        let value = render(self.value);
        let allowed = self
            .allowed
            .iter()
            .map(render)
            .collect::<Vec<_>>()
            .join(", ");
        let optional = if self.allows_unassigned {
            "; unassigned allowed"
        } else {
            ""
        };
        format!(
            "entity {entity}, variable {variable}: value {value} outside allowed range [{allowed}]{optional}"
        )
    }
}

/// The ordinary validator's error, optionally enriched with a range violation.
/// Shape, pin, duplicate and dependency errors retain their original priority
/// and do not acquire unrelated range details from later candidate values.
#[derive(Debug, Eq, PartialEq)]
pub struct RangeValidationError<'a, E, V> {
    pub cause: E,
    pub violation: Option<ValueRangeViolation<'a, V>>,
}
impl<E: fmt::Display, V: fmt::Debug> fmt::Display for RangeValidationError<'_, E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)?;
        if let Some(v) = &self.violation {
            write!(
                f,
                "; {:?}: value {:?}, allowed {:?}, allows unassigned {}",
                v.location, v.value, v.allowed, v.allows_unassigned
            )?;
        }
        Ok(())
    }
}
impl<E: Error + 'static, V: fmt::Debug> Error for RangeValidationError<'_, E, V> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

/// Validate a candidate and report the exact basic slot, payload and domain.
/// Required unassigned slots remain valid partial states, as in ordinary validate.
pub fn validate_basic_with_diagnostics<'a, V: Eq>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
) -> Result<(), RangeValidationError<'a, ModelError, V>> {
    problem.validate(assignment).map_err(|cause| {
        let violation =
            if let ModelError::ValueOutsideDomain { variable } = cause.without_range_report() {
                let meta = &problem.variables()[*variable];
                assignment.values()[*variable]
                    .as_ref()
                    .map(|value| ValueRangeViolation {
                        location: RangeViolationLocation::Basic {
                            variable: *variable,
                        },
                        value,
                        allowed: meta.domain(),
                        allows_unassigned: meta.allows_unassigned(),
                    })
            } else {
                None
            };
        RangeValidationError { cause, violation }
    })
}

/// Validate a list candidate and locate the first value rejected by the ordinary
/// validator. Global membership errors report the global registry; per-owner
/// errors report that owner's declared range. Positions refer to the candidate.
pub fn validate_list_with_diagnostics<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
) -> Result<(), RangeValidationError<'a, ListError, V>> {
    problem.validate(assignment).map_err(|cause| {
        let rejected = match cause.without_range_report() {
            ListError::ValueOutsideDomain => {
                assignment
                    .lists()
                    .iter()
                    .enumerate()
                    .find_map(|(entity, row)| {
                        row.iter()
                            .enumerate()
                            .find(|(_, value)| !problem.values().contains(value))
                            .map(|(index, value)| (entity, index, value, problem.values()))
                    })
            }
            ListError::ValueOutsideEntityRange { entity } => {
                let allowed = problem.entities()[*entity]
                    .value_range()
                    .unwrap_or(problem.values());
                assignment.lists()[*entity]
                    .iter()
                    .enumerate()
                    .find(|(_, value)| !allowed.contains(value))
                    .map(|(index, value)| (*entity, index, value, allowed))
            }
            _ => None,
        };
        let violation = rejected.map(|(entity, index, value, allowed)| ValueRangeViolation {
            location: RangeViolationLocation::List {
                position: ListPosition { entity, index },
            },
            value,
            allowed,
            allows_unassigned: problem.allows_unassigned(),
        });
        RangeValidationError { cause, violation }
    })
}

/// Basic and list domains can have unrelated payload types; keep both namespaces.
#[derive(Debug, Eq, PartialEq)]
pub enum MixedRangeValidationError<'a, B, L> {
    Basic(RangeValidationError<'a, ModelError, B>),
    Lists(RangeValidationError<'a, ListError, L>),
}
impl<B: fmt::Debug, L: fmt::Debug> fmt::Display for MixedRangeValidationError<'_, B, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic(e) => write!(f, "mixed basic component: {e}"),
            Self::Lists(e) => write!(f, "mixed list component: {e}"),
        }
    }
}
impl<B: fmt::Debug, L: fmt::Debug> Error for MixedRangeValidationError<'_, B, L> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Borrowed diagnostic payloads cannot escape through Error::source.
        // The original owned cause can, and remains available as a typed field.
        Some(match self {
            Self::Basic(e) => &e.cause,
            Self::Lists(e) => &e.cause,
        })
    }
}
/// Preserve basic-before-list validation, including non-range errors. Scoring or
/// search adapters can call this before accepting an externally supplied state.
pub fn validate_mixed_with_diagnostics<'a, B: Eq + Clone, L: Eq + Clone>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
) -> Result<(), MixedRangeValidationError<'a, B, L>> {
    validate_basic_with_diagnostics(problem.basic(), assignment.basic())
        .map_err(MixedRangeValidationError::Basic)?;
    validate_list_with_diagnostics(problem.lists(), assignment.lists())
        .map_err(MixedRangeValidationError::Lists)
}
