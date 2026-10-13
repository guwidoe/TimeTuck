//! Opt-in evidence from actual calculator state before and after a trial's undo.
use crate::ScoreCorruption;
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};
/// Default preserves the director's existing assert_scores behavior. Explicit
/// Score checks undo even when candidate assertions are disabled. Tracked also
/// requires a calculator-owned immutable state snapshot. Disabled skips only
/// the undo check; candidate/committed-state score checks remain configured.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CalculatorUndoDiagnostics {
    #[default]
    Default,
    Disabled,
    Score,
    Tracked,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CalculatorUndoField {
    pub entity_key: String,
    pub entity_type: String,
    pub entity_description: String,
    pub variable: String,
}
struct CapturedValue {
    value: Box<dyn Any>,
    equal: fn(&dyn Any, &dyn Any) -> bool,
    text: String,
}
/// Owned field values for one trial. Values must be immutable independent
/// snapshots, not aliases to mutable calculator/model storage. Equality uses the
/// original value type; display text is diagnostic only. No field or assignment
/// copies are made unless Tracked checking is explicitly requested.
#[derive(Default)]
pub struct CalculatorStateSnapshot {
    fields: BTreeMap<CalculatorUndoField, CapturedValue>,
}
impl CalculatorStateSnapshot {
    pub fn new() -> Self {
        Self::default()
    }
    /// Duplicate field identity is rejected without replacing the first capture.
    pub fn capture<T: Eq + 'static>(
        &mut self,
        field: CalculatorUndoField,
        value: T,
        text: String,
    ) -> Result<(), crate::MatchRegistryError> {
        if self.fields.contains_key(&field) {
            return Err(crate::MatchRegistryError::Calculator(
                "duplicate calculator undo field".into(),
            ));
        }
        self.fields.insert(
            field,
            CapturedValue {
                value: Box::new(value),
                equal: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
                text,
            },
        );
        Ok(())
    }
    pub fn differences(&self, actual: &Self) -> Vec<CalculatorUndoDifference> {
        self.fields
            .keys()
            .chain(actual.fields.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|field| {
                let before = self.fields.get(field);
                let after = actual.fields.get(field);
                if let (Some(before), Some(after)) = (before, after)
                    && (before.equal)(before.value.as_ref(), after.value.as_ref())
                {
                    return None;
                }
                Some(CalculatorUndoDifference {
                    field: field.clone(),
                    expected: before.map(|v| v.text.clone()),
                    actual: after.map(|v| v.text.clone()),
                })
            })
            .collect()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalculatorUndoDifference {
    pub field: CalculatorUndoField,
    pub expected: Option<String>,
    pub actual: Option<String>,
}
impl fmt::Display for CalculatorUndoDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Actual value ({}) of variable {} on {} entity ({}) differs from expected ({})",
            self.actual.as_deref().unwrap_or("<missing>"),
            self.field.variable,
            self.field.entity_type,
            self.field.entity_description,
            self.expected.as_deref().unwrap_or("<missing>")
        )
    }
}
/// Captured evidence from a failing undo check. No live model references are
/// retained, and creating this report does not repair calculator state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalculatorUndoCorruption {
    pub score: Option<ScoreCorruption>,
    pub fields: Vec<CalculatorUndoDifference>,
}
impl fmt::Display for CalculatorUndoCorruption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("corrupted undoMove")?;
        if let Some(score) = &self.score {
            write!(f, ": {score}")?;
        }
        if !self.fields.is_empty() {
            write!(
                f,
                "\nVariables that are different between before and undo:\n{}",
                crate::format_tracking_list(&self.fields)
            )?;
        }
        Ok(())
    }
}
impl Error for CalculatorUndoCorruption {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.score.as_ref().map(|s| s as &(dyn Error + 'static))
    }
}
