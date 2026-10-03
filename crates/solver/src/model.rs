//! Finite basic-variable planning domains, separate from score calculation.

use crate::moves::{AppliedMove, Assignment, Move, MoveError};
use std::{error::Error, fmt};

/// An ordered value range and the initial state of one planning variable.
/// Values are compared by `Eq`; use stable identifiers for large domain objects.
#[derive(Clone, Debug)]
pub struct Variable<V> {
    domain: Vec<V>,
    initial: Option<V>,
    pinned: bool,
    allows_unassigned: bool,
}
impl<V> Variable<V> {
    pub fn new(domain: Vec<V>) -> Self {
        Self {
            domain,
            initial: None,
            pinned: false,
            allows_unassigned: false,
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
    DuplicateValue { variable: usize },
    ValueOutsideDomain { variable: usize },
    PinnedUnassigned { variable: usize },
    PinnedValueChanged { variable: usize },
    AssignmentLength { expected: usize, actual: usize },
    Move(MoveError),
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateValue { variable } => {
                write!(f, "variable {variable} has duplicate domain values")
            }
            Self::ValueOutsideDomain { variable } => {
                write!(f, "variable {variable} has a value outside its domain")
            }
            Self::PinnedUnassigned { variable } => {
                write!(f, "required variable {variable} is pinned without a value")
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
/// an unassigned required variable is instead a model error.
#[derive(Clone, Debug)]
pub struct FiniteProblem<V> {
    variables: Vec<Variable<V>>,
    pillar_schema: Option<crate::PillarSchema>,
}
impl<V: Eq> FiniteProblem<V> {
    pub fn new(variables: Vec<Variable<V>>) -> Result<Self, ModelError> {
        for (index, variable) in variables.iter().enumerate() {
            for (i, value) in variable.domain.iter().enumerate() {
                if variable.domain[..i].contains(value) {
                    return Err(ModelError::DuplicateValue { variable: index });
                }
            }
            if variable
                .initial
                .as_ref()
                .is_some_and(|value| !variable.domain.contains(value))
            {
                return Err(ModelError::ValueOutsideDomain { variable: index });
            }
            if variable.pinned && variable.initial.is_none() && !variable.allows_unassigned {
                return Err(ModelError::PinnedUnassigned { variable: index });
            }
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
        for (i, (variable, value)) in self.variables.iter().zip(&assignment.values).enumerate() {
            if value
                .as_ref()
                .is_some_and(|value| !variable.domain.contains(value))
            {
                return Err(ModelError::ValueOutsideDomain { variable: i });
            }
            if variable.pinned && variable.initial != *value {
                return Err(ModelError::PinnedValueChanged { variable: i });
            }
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

    /// The number of complete assignments, or `None` on `u128` overflow.
    pub fn search_space_size(&self) -> Option<u128> {
        // A zero factor takes precedence over overflow in another dimension.
        if self
            .variables
            .iter()
            .any(|v| !v.pinned && v.domain.is_empty() && !v.allows_unassigned)
        {
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
        let transaction = assignment.apply(movement)?;
        self.validate(&transaction)?;
        Ok(transaction)
    }
}
