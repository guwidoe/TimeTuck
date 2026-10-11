//! Finite basic-variable planning domains, separate from score calculation.

mod domain_index;
pub(crate) mod move_check;

use crate::moves::{AppliedMove, Assignment, Move, MoveError};
use std::{error::Error, fmt};

/// An ordered value range and the initial state of one planning variable.
/// Values are compared by `Eq`; use stable identifiers for large domain objects.
#[derive(Clone, Debug)]
pub struct Variable<V> {
    domain: Vec<V>,
    domain_index: Option<std::sync::Arc<domain_index::DomainIndex<V>>>,
    initial: Option<V>,
    pinned: bool,
    allows_unassigned: bool,
    range_diagnostic: Option<crate::range_report::RangeDiagnostic<V>>,
}
impl<V> Variable<V> {
    /// Attach labels and a formatter used only when an assigned value violates
    /// its range. Reports own rendered strings and survive candidate rollback.
    /// The renderer must be deterministic and must not mutate planning state.
    /// Cloned/resumed models retain this metadata. Ordinary errors are unchanged
    /// when no diagnostic metadata is attached; no Debug/Display bound is added.
    pub fn with_range_diagnostics(
        mut self,
        entity: impl Into<String>,
        variable: impl Into<String>,
        render: impl Fn(&V) -> String + Send + Sync + 'static,
    ) -> Self {
        self.range_diagnostic = Some(crate::range_report::RangeDiagnostic::new(
            entity, variable, render,
        ));
        self
    }

    pub fn new(domain: Vec<V>) -> Self {
        Self {
            domain,
            domain_index: None,
            initial: None,
            pinned: false,
            allows_unassigned: false,
            range_diagnostic: None,
        }
    }
    pub fn with_initial(mut self, value: V) -> Self {
        self.initial = Some(value);
        self
    }
    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
    pub fn allowing_unassigned(mut self) -> Self {
        self.allows_unassigned = true;
        self
    }
    fn range_error(&self, variable: usize, value: &V) -> ModelError {
        let cause = ModelError::ValueOutsideDomain { variable };
        match &self.range_diagnostic {
            None => cause,
            Some(d) => ModelError::RangeViolation {
                cause: Box::new(cause),
                report: d.report(
                    crate::RangeViolationLocation::Basic { variable },
                    value,
                    &self.domain,
                ),
            },
        }
    }
    pub fn domain(&self) -> &[V] {
        &self.domain
    }
    pub fn initial(&self) -> Option<&V> {
        self.initial.as_ref()
    }
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }
    pub fn allows_unassigned(&self) -> bool {
        self.allows_unassigned
    }
}

/// Invalid domain metadata or an assignment that violates the model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    /// Optional rendered context; the original classification remains in cause.
    RangeViolation {
        cause: Box<Self>,
        report: Box<crate::ValueRangeReport>,
    },
    InvalidSampleSize {
        minimum: usize,
        maximum: usize,
    },
    NullCrossingRequired {
        variable: usize,
    },
    DuplicateValue {
        variable: usize,
    },
    ValueOutsideDomain {
        variable: usize,
    },
    PinnedValueChanged {
        variable: usize,
    },
    AssignmentLength {
        expected: usize,
        actual: usize,
    },
    Move(MoveError),
}
impl ModelError {
    pub fn range_report(&self) -> Option<&crate::ValueRangeReport> {
        match self {
            Self::RangeViolation { report, .. } => Some(report),
            _ => None,
        }
    }
    /// Recover the original classification without discarding the owned report.
    pub fn without_range_report(&self) -> &Self {
        let mut cause = self;
        while let Self::RangeViolation { cause: inner, .. } = cause {
            cause = inner;
        }
        cause
    }
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RangeViolation { report, .. } => report.fmt(f),
            Self::InvalidSampleSize { minimum, maximum } => {
                write!(f, "invalid sample bounds {minimum}..={maximum}")
            }
            Self::NullCrossingRequired { variable } => {
                write!(f, "variable {variable} does not allow null crossing")
            }
            Self::DuplicateValue { variable } => {
                write!(f, "variable {variable} has duplicate domain values")
            }
            Self::ValueOutsideDomain { variable } => {
                write!(f, "variable {variable} has a value outside its domain")
            }
            Self::PinnedValueChanged { variable } => write!(
                f,
                "pinned variable {variable} differs from its initial value"
            ),
            Self::AssignmentLength { expected, actual } => {
                write!(f, "expected {expected} variables, found {actual}")
            }
            Self::Move(error) => error.fmt(f),
        }
    }
}
impl Error for ModelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RangeViolation { cause, .. } => Some(cause.as_ref()),
            Self::Move(error) => Some(error),
            _ => None,
        }
    }
}
impl From<MoveError> for ModelError {
    fn from(error: MoveError) -> Self {
        Self::Move(error)
    }
}

/// A validated finite assignment space. Immutable while a search is running.
///
/// A variable may have an empty domain; if it is required, there are no complete
/// assignments. An unpinned required variable may initially be unassigned. Pinning
/// an unassigned required variable preserves a valid but permanently incomplete state.
#[derive(Clone, Debug)]
pub struct FiniteProblem<V> {
    variables: Vec<Variable<V>>,
    pillar_schema: Option<crate::PillarSchema>,
}
impl<V: Eq> FiniteProblem<V> {
    pub fn new(variables: Vec<Variable<V>>) -> Result<Self, ModelError> {
        for (index, variable) in variables.iter().enumerate() {
            variable.validate_metadata(index)?;
        }
        Ok(Self {
            variables,
            pillar_schema: None,
        })
    }
    /// Attach validated selected entity rows for pillar/subpillar neighborhoods.
    /// Ordinary construction, exact search and non-pillar moves are unaffected.
    pub fn with_pillar_schema(
        mut self,
        schema: crate::PillarSchema,
    ) -> Result<Self, crate::PillarError> {
        schema.validate(self.variables.len())?;
        self.pillar_schema = Some(schema);
        Ok(self)
    }
    pub fn without_pillar_schema(mut self) -> Self {
        self.pillar_schema = None;
        self
    }
    pub fn pillar_schema(&self) -> Option<&crate::PillarSchema> {
        self.pillar_schema.as_ref()
    }
    pub fn variables(&self) -> &[Variable<V>] {
        &self.variables
    }

    /// Validate value membership and pins, allowing temporarily unassigned values.
    pub fn validate(&self, assignment: &Assignment<V>) -> Result<(), ModelError> {
        if self.variables.len() != assignment.values.len() {
            return Err(ModelError::AssignmentLength {
                expected: self.variables.len(),
                actual: assignment.values.len(),
            });
        }
        for i in 0..self.variables.len() {
            self.validate_variable(assignment, i)?;
        }
        Ok(())
    }

    fn validate_variable(&self, assignment: &Assignment<V>, i: usize) -> Result<(), ModelError> {
        let variable = &self.variables[i];
        let value = &assignment.values[i];
        if let Some(value) = value
            && !variable.contains_value(value)
        {
            return Err(variable.range_error(i, value));
        }
        if variable.pinned && variable.initial != *value {
            return Err(ModelError::PinnedValueChanged { variable: i });
        }
        Ok(())
    }

    /// Complete means every required variable is assigned, independent of score.
    pub fn is_complete(&self, assignment: &Assignment<V>) -> bool {
        self.validate(assignment).is_ok()
            && self
                .variables
                .iter()
                .zip(&assignment.values)
                .all(|(variable, value)| value.is_some() || variable.allows_unassigned)
    }

    // Construction still fills movable variables when a required null pin makes
    // completeness impossible; an empty movable required range cannot be filled.
    pub(crate) fn has_empty_movable_required_domain(&self) -> bool {
        self.variables
            .iter()
            .any(|v| !v.pinned && !v.allows_unassigned && v.domain.is_empty())
    }

    /// The number of complete assignments, or `None` on `u128` overflow.
    pub fn search_space_size(&self) -> Option<u128> {
        // A zero factor takes precedence over overflow in another dimension.
        if self.variables.iter().any(|v| {
            !v.allows_unassigned
                && if v.pinned {
                    v.initial.is_none()
                } else {
                    v.domain.is_empty()
                }
        }) {
            return Some(0);
        }
        self.variables.iter().try_fold(1_u128, |size, v| {
            let choices = if v.pinned {
                1
            } else {
                v.domain.len() as u128 + u128::from(v.allows_unassigned)
            };
            size.checked_mul(choices)
        })
    }
}
impl<V: Eq + Clone> FiniteProblem<V> {
    /// Return an independent model starting from a validated partial or complete
    /// assignment. Preserve domains, optionality, pins and pillar metadata. This
    /// resumes a solution in a new solve; search history and budgets start afresh.
    /// Pin validation uses this model's original baseline, before replacing it.
    pub fn with_initial_assignment(&self, assignment: &Assignment<V>) -> Result<Self, ModelError> {
        self.validate(assignment)?;
        let mut model = self.clone();
        for (variable, value) in model.variables.iter_mut().zip(assignment.values()) {
            variable.initial = value.clone();
        }
        Ok(model)
    }

    // Callers validate the assignment and selected movable indices first. Retain
    // all model metadata, including diagnostic renderers and pillar schema.
    // Unsampled optional nulls must be frozen just like assigned values.
    pub(crate) fn with_ruined_variables(
        &self,
        assignment: &Assignment<V>,
        selected: &[usize],
    ) -> Self {
        let mut ruined = vec![false; self.variables.len()];
        for &index in selected {
            ruined[index] = true;
        }
        Self {
            variables: self
                .variables
                .iter()
                .zip(assignment.values())
                .enumerate()
                .map(|(index, (variable, value))| Variable {
                    domain: variable.domain.clone(),
                    domain_index: variable.domain_index.clone(),
                    initial: if ruined[index] { None } else { value.clone() },
                    pinned: !ruined[index],
                    allows_unassigned: variable.allows_unassigned,
                    range_diagnostic: variable.range_diagnostic.clone(),
                })
                .collect(),
            pillar_schema: self.pillar_schema.clone(),
        }
    }

    pub fn initial_assignment(&self) -> Assignment<V> {
        Assignment::new(self.variables.iter().map(|v| v.initial.clone()).collect())
    }

    /// Apply a move transaction and validate its final state against domains/pins.
    /// All writes are rolled back if either validation or application fails.
    /// Clearing required variables is allowed for intermediate planning states.
    pub fn apply<'a>(
        &self,
        assignment: &'a mut Assignment<V>,
        movement: &Move<V>,
    ) -> Result<AppliedMove<'a, V>, ModelError> {
        self.validate(assignment)?;
        self.apply_validated(assignment, movement)
    }

    // Owning directors validate at construction/reset and expose immutable state.
    // Raw move application checks every index and cannot change assignment length.
    // Only written slots can violate a domain or pin in the final transaction.
    pub(crate) fn apply_validated<'a>(
        &self,
        assignment: &'a mut Assignment<V>,
        movement: &Move<V>,
    ) -> Result<AppliedMove<'a, V>, ModelError> {
        let transaction = assignment.apply(movement)?;
        match movement {
            Move::Change { variable, .. } => self.validate_variable(&transaction, *variable)?,
            Move::Swap { left, right } => {
                self.validate_variable(&transaction, *left.min(right))?;
                if left != right {
                    self.validate_variable(&transaction, *left.max(right))?;
                }
            }
            Move::Composite(_) => {
                let mut touched: Vec<_> = transaction
                    .prior_values()
                    .iter()
                    .map(|w| w.variable)
                    .collect();
                touched.sort_unstable();
                touched.dedup();
                for variable in touched {
                    self.validate_variable(&transaction, variable)?;
                }
            }
        }
        Ok(transaction)
    }
}
