//! Explicit basic-variable reconstruction for custom move providers.
use crate::{Assignment, FiniteProblem, ModelError, Move};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicRuinPlanError {
    Model(ModelError),
    EmptySelection,
    UnknownVariable { variable: usize, count: usize },
    DuplicateVariable { variable: usize },
    PinnedVariable { variable: usize },
    IncompleteInitialAssignment,
    IncompleteReconstruction,
}
impl From<ModelError> for BasicRuinPlanError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}
impl fmt::Display for BasicRuinPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(f),
            _ => write!(f, "invalid basic ruin plan: {self:?}"),
        }
    }
}
impl Error for BasicRuinPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            _ => None,
        }
    }
}

/// Clear an explicit selection of variables for reconstruction into one move.
///
/// The independent repair model clears selected variables and pins every other
/// variable to its supplied value, including optional nulls. Domains, their order,
/// optionality, diagnostics and pillar metadata are retained. Use ordinary or
/// stream construction, exact search, or a custom algorithm against that model.
/// A required variable must be assigned both in the input and finished result;
/// selected optional variables may remain null or become assigned.
///
/// [`Self::finish`] produces an ordinary snapshot-bound [`Move`] suitable for
/// custom providers and all basic scoring backends. [`crate::TemporaryMoves`]
/// provides scored evaluation/undo and guarded recorded replay. The plan never
/// mutates the original assignment or installs a reconstruction's cached score.
/// Nested reconstruction in a provider needs its own explicit budget; its work
/// is not counted as scored proposals in the surrounding search.
#[derive(Debug)]
pub struct BasicRuinPlan<'a, V> {
    before: &'a Assignment<V>,
    repair: FiniteProblem<V>,
    selected: Vec<usize>,
}
impl<'a, V: Eq + Clone> BasicRuinPlan<'a, V> {
    pub fn new(
        problem: &FiniteProblem<V>,
        before: &'a Assignment<V>,
        selected: &[usize],
    ) -> Result<Self, BasicRuinPlanError> {
        problem.validate(before)?;
        if !problem.is_complete(before) {
            return Err(BasicRuinPlanError::IncompleteInitialAssignment);
        }
        if selected.is_empty() {
            return Err(BasicRuinPlanError::EmptySelection);
        }
        let mut seen = vec![false; problem.variables().len()];
        for &variable in selected {
            let count = seen.len();
            let seen = seen
                .get_mut(variable)
                .ok_or(BasicRuinPlanError::UnknownVariable { variable, count })?;
            if std::mem::replace(seen, true) {
                return Err(BasicRuinPlanError::DuplicateVariable { variable });
            }
            if problem.variables()[variable].is_pinned() {
                return Err(BasicRuinPlanError::PinnedVariable { variable });
            }
        }
        Ok(Self {
            before,
            repair: problem.with_ruined_variables(before, selected),
            selected: selected.to_vec(),
        })
    }

    pub fn repair_problem(&self) -> &FiniteProblem<V> {
        &self.repair
    }

    /// Validate a reconstruction and write only selected, Eq-different variables.
    ///
    /// Unselected payloads and Eq-equivalent selected copies keep the original
    /// payload. Changed variables use supplied reconstructed payloads. Child order
    /// follows the supplied selection. An unchanged reconstruction returns an
    /// empty composite; providers can discard it with `is_move_doable`.
    /// Reconstructing an unselected variable violates its repair pin and is an
    /// error. Failed finishing preserves the plan for retry and all caller state.
    pub fn finish(&self, rebuilt: &Assignment<V>) -> Result<Move<V>, BasicRuinPlanError> {
        self.repair.validate(rebuilt)?;
        if !self.repair.is_complete(rebuilt) {
            return Err(BasicRuinPlanError::IncompleteReconstruction);
        }
        Ok(Move::Composite(
            self.selected
                .iter()
                .copied()
                .filter(|&variable| self.before.values()[variable] != rebuilt.values()[variable])
                .map(|variable| Move::Change {
                    variable,
                    value: rebuilt.values()[variable].clone(),
                })
                .collect(),
        ))
    }
}
