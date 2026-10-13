//! Reusable basic ruin selection identity, independent of reconstruction writes.
use crate::{
    Assignment, BasicRuinPlan, BasicRuinPlanError, FiniteProblem, MoveRebaseError,
    MoveRebaseNamespace, PlanningVariableKind, PlanningVariableRef,
};
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicRuinSelectionError {
    NotBasic,
    DuplicateEntity { entity: usize },
}
impl fmt::Display for BasicRuinSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "basic ruin selection: {self:?}")
    }
}
impl Error for BasicRuinSelectionError {}
/// One declared field, ordered native entity slots, and an insertion-ordered value
/// set. Values describe the selection's planning footprint; they need not occur in
/// the current selected slots. Eq identity uses the schema field, ordered entities
/// and unordered values, never repair output. Use identity-aware V when object
/// identity matters. Entity slots are explicitly bound to this field by the caller.
/// Reconstruction policy, seed and budget belong to the caller, not this identity.
#[derive(Clone, Debug)]
pub struct BasicRuinSelection<'s, V> {
    variable: PlanningVariableRef<'s>,
    entities: Vec<usize>,
    values: Vec<V>,
}
impl<V: Eq> PartialEq for BasicRuinSelection<'_, V> {
    fn eq(&self, other: &Self) -> bool {
        self.variable == other.variable
            && self.entities == other.entities
            && self.values.len() == other.values.len()
            && self.values.iter().all(|v| other.values.contains(v))
    }
}
impl<V: Eq> Eq for BasicRuinSelection<'_, V> {}
impl<'s, V: Eq> BasicRuinSelection<'s, V> {
    pub fn new(
        variable: PlanningVariableRef<'s>,
        entities: Vec<usize>,
        values: impl IntoIterator<Item = V>,
    ) -> Result<Self, BasicRuinSelectionError> {
        if !matches!(variable.kind(), PlanningVariableKind::Basic { .. }) {
            return Err(BasicRuinSelectionError::NotBasic);
        }
        for (i, &entity) in entities.iter().enumerate() {
            if entities[..i].contains(&entity) {
                return Err(BasicRuinSelectionError::DuplicateEntity { entity });
            }
        }
        let mut unique = vec![];
        for value in values {
            if !unique.contains(&value) {
                unique.push(value);
            }
        }
        Ok(Self {
            variable,
            entities,
            values: unique,
        })
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn planning_entities(&self) -> &[usize] {
        &self.entities
    }
    pub fn planning_values(&self) -> &[V] {
        &self.values
    }
    /// Look up each selected entity and each distinct footprint value once. Field
    /// metadata is retained; map into a corresponding destination model. Collisions
    /// and lookup errors return without changing this selection. Callback side effects
    /// are not rolled back. Rebase does not read source or destination assignments.
    pub fn rebase<U: Eq, E>(
        &self,
        mut entity_lookup: impl FnMut(usize) -> Result<usize, E>,
        mut value_lookup: impl FnMut(&V) -> Result<U, E>,
    ) -> Result<BasicRuinSelection<'s, U>, MoveRebaseError<E>> {
        let mut entities = Vec::with_capacity(self.entities.len());
        for &entity in &self.entities {
            let mapped = entity_lookup(entity).map_err(MoveRebaseError::Lookup)?;
            if entities.contains(&mapped) {
                return Err(MoveRebaseError::Collision(
                    MoveRebaseNamespace::BasicVariable,
                ));
            }
            entities.push(mapped);
        }
        let mut values = Vec::with_capacity(self.values.len());
        for value in &self.values {
            let mapped = value_lookup(value).map_err(MoveRebaseError::Lookup)?;
            if values.contains(&mapped) {
                return Err(MoveRebaseError::Collision(MoveRebaseNamespace::BasicValue));
            }
            values.push(mapped);
        }
        Ok(BasicRuinSelection {
            variable: self.variable,
            entities,
            values,
        })
    }
}
impl<V: Eq + Clone> BasicRuinSelection<'_, V> {
    /// Bind current native slots to the existing executable repair engine. Normal
    /// plan validation checks domains, pins, completeness and selection bounds.
    pub fn prepare<'a>(
        &self,
        problem: &FiniteProblem<V>,
        before: &'a Assignment<V>,
    ) -> Result<BasicRuinPlan<'a, V>, BasicRuinPlanError> {
        BasicRuinPlan::new(problem, before, &self.entities)
    }
}
