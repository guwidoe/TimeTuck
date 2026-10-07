//! Build owned models while retaining diagnostics tied to the caller's input.
use crate::{
    FiniteProblem, ListEntity, ListError, ListPosition, ListProblem, MixedProblem,
    MixedRangeValidationError, ModelError, RangeValidationError, RangeViolationLocation,
    ValueRangeViolation, Variable,
};

/// Clone a borrowed specification into an owned basic model. On failure the
/// diagnostic borrows the original input, so rejected payload identities remain
/// available. Native constructor validation and error priority are unchanged.
pub fn build_basic_with_diagnostics<V: Eq + Clone>(
    variables: &[Variable<V>],
) -> Result<FiniteProblem<V>, RangeValidationError<'_, ModelError, V>> {
    FiniteProblem::new(variables.to_vec()).map_err(|cause| {
        let violation = if let ModelError::ValueOutsideDomain { variable } = &cause {
            let meta = &variables[*variable];
            meta.initial().map(|value| ValueRangeViolation {
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

/// Build an owned list model from borrowed registry/entity specifications.
/// `allows_unassigned` is the list variable's policy; it never admits null list
/// elements. Partial starting lists are valid under either policy. Errors about
/// malformed metadata retain their native cause without assignment details.
pub fn build_list_with_diagnostics<'a, V: Eq + Clone>(
    values: &'a [V],
    entities: &'a [ListEntity<V>],
    allows_unassigned: bool,
) -> Result<ListProblem<V>, RangeValidationError<'a, ListError, V>> {
    ListProblem::new(values.to_vec(), entities.to_vec())
        .map(|model| {
            if allows_unassigned {
                model.allowing_unassigned()
            } else {
                model
            }
        })
        .map_err(|cause| {
            let rejected = match &cause {
                ListError::ValueOutsideDomain => {
                    entities.iter().enumerate().find_map(|(entity, meta)| {
                        meta.initial()
                            .iter()
                            .enumerate()
                            .find(|(_, value)| !values.contains(value))
                            .map(|(index, value)| (entity, index, value, values))
                    })
                }
                ListError::ValueOutsideEntityRange { entity } => {
                    let meta = &entities[*entity];
                    let allowed = meta.value_range().unwrap_or(values);
                    meta.initial()
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
                allows_unassigned,
            });
            RangeValidationError { cause, violation }
        })
}

/// Build basic then list components with separate payload types and namespaces.
/// No model is returned unless both components validate. Borrowed inputs remain
/// intact on errors or clone panics; external effects inside Clone are excluded.
pub fn build_mixed_with_diagnostics<'a, B: Eq + Clone, L: Eq + Clone>(
    variables: &'a [Variable<B>],
    values: &'a [L],
    entities: &'a [ListEntity<L>],
    allows_unassigned_lists: bool,
) -> Result<MixedProblem<B, L>, MixedRangeValidationError<'a, B, L>> {
    let basic =
        build_basic_with_diagnostics(variables).map_err(MixedRangeValidationError::Basic)?;
    let lists = build_list_with_diagnostics(values, entities, allows_unassigned_lists)
        .map_err(MixedRangeValidationError::Lists)?;
    Ok(MixedProblem::new(basic, lists))
}
