//! Snapshot-independent list ruin selection metadata and rebasing.
use crate::{
    ListAssignment, ListProblem, ListRuinPlan, ListRuinPlanError, PlanningVariableKind,
    PlanningVariableRef,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListRuinSelectionError<E> {
    NotList { name: String },
    Lookup(E),
    UnknownEntity { entity: usize, count: usize },
    UnlistedOwner { entity: usize },
    Plan(ListRuinPlanError),
}
impl<E: fmt::Debug> fmt::Display for ListRuinSelectionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list ruin selection: {self:?}")
    }
}
impl<E: fmt::Debug> Error for ListRuinSelectionError<E> {}

/// Semantic list ruin selection: a registered list variable, ordered selected
/// value identities and an entity set. Entity traversal keeps first insertion
/// order, while equality ignores that order. Value order and duplicates remain
/// significant. Identity is independent of reconstruction, scores and generated
/// writes. A native repair algorithm is chosen when preparing/executing; no Java
/// phase builder, solver scope or RNG-state equivalence is implied.
///
/// Generic identity payloads allow applications to retain borrowed handles or
/// reference-counted working objects. Their Eq implementation defines identity;
/// use pointer-aware handles if payload equality should not imply object identity.
#[derive(Clone, Debug)]
pub struct ListRuinSelection<'s, V, E> {
    variable: PlanningVariableRef<'s>,
    values: Vec<V>,
    entities: Vec<E>,
}
impl<'s, V, E: Eq> ListRuinSelection<'s, V, E> {
    pub fn new(
        variable: PlanningVariableRef<'s>,
        values: Vec<V>,
        entities: impl IntoIterator<Item = E>,
    ) -> Result<Self, ListRuinSelectionError<std::convert::Infallible>> {
        if !matches!(variable.kind(), PlanningVariableKind::List { .. }) {
            return Err(ListRuinSelectionError::NotList {
                name: variable.name().into(),
            });
        }
        let mut unique = Vec::new();
        for entity in entities {
            if !unique.contains(&entity) {
                unique.push(entity);
            }
        }
        Ok(Self {
            variable,
            values,
            entities: unique,
        })
    }
}
impl<V: PartialEq, E: Eq> PartialEq for ListRuinSelection<'_, V, E> {
    fn eq(&self, other: &Self) -> bool {
        self.variable == other.variable
            && self.values == other.values
            && self.entities.len() == other.entities.len()
            && self.entities.iter().all(|e| other.entities.contains(e))
    }
}
impl<V: Eq, E: Eq> Eq for ListRuinSelection<'_, V, E> {}
impl<'s, V, E> ListRuinSelection<'s, V, E> {
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn planning_values(&self) -> &[V] {
        &self.values
    }
    pub fn planning_entities(&self) -> &[E] {
        &self.entities
    }
    /// Lookup each ordered value occurrence, then each retained entity once.
    /// Descriptor identity is retained without a lookup. Destination entity set
    /// collisions collapse in first-result order, matching set semantics; ordered
    /// value collisions/duplicates remain ordered occurrences. Errors and panics
    /// stop subsequent callbacks and leave this selection intact. External callback
    /// effects are not rolled back. No assignment or model is mutated.
    pub fn rebase<U, F: Eq, X>(
        &self,
        mut value: impl FnMut(&V) -> Result<U, X>,
        mut entity: impl FnMut(&E) -> Result<F, X>,
    ) -> Result<ListRuinSelection<'s, U, F>, ListRuinSelectionError<X>> {
        let values = self
            .values
            .iter()
            .map(&mut value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ListRuinSelectionError::Lookup)?;
        let mut entities = Vec::new();
        for source in &self.entities {
            let destination = entity(source).map_err(ListRuinSelectionError::Lookup)?;
            if !entities.contains(&destination) {
                entities.push(destination);
            }
        }
        Ok(ListRuinSelection {
            variable: self.variable,
            values,
            entities,
        })
    }
    /// Resolve application identities to the actual target model's registry and
    /// entity slots, then prepare its existing executable reconstruction machinery.
    /// Values resolve first, entities second; no input state changes. Listed entities
    /// must be valid and include every selected assigned owner. They may include
    /// additional empty owners; reconstruction may insert into other legal receivers.
    /// Caller bindings connect this descriptor to the native flattened model.
    pub fn prepare<'a, T: Eq + Clone, X>(
        &self,
        problem: &'a ListProblem<T>,
        before: &'a ListAssignment<T>,
        mut value_index: impl FnMut(&V) -> Result<usize, X>,
        mut entity_index: impl FnMut(&E) -> Result<usize, X>,
    ) -> Result<ListRuinPlan<'a, T>, ListRuinSelectionError<X>> {
        let selected = self
            .values
            .iter()
            .map(&mut value_index)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ListRuinSelectionError::Lookup)?;
        let mut owners = Vec::new();
        for entity in &self.entities {
            let entity = entity_index(entity).map_err(ListRuinSelectionError::Lookup)?;
            if entity >= problem.entities().len() {
                return Err(ListRuinSelectionError::UnknownEntity {
                    entity,
                    count: problem.entities().len(),
                });
            }
            owners.push(entity);
        }
        let plan =
            ListRuinPlan::new(problem, before, &selected).map_err(ListRuinSelectionError::Plan)?;
        let locations = problem.value_locations(before);
        for &value in &selected {
            if let Some(at) = locations[value]
                && !owners.contains(&at.entity)
            {
                return Err(ListRuinSelectionError::UnlistedOwner { entity: at.entity });
            }
        }
        Ok(plan)
    }
}
