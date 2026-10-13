//! Optional registered-field move inspection, retaining actual pre-execution values.
use crate::{PlanningVariableRef, VariableStateError, VariableStateRegistry};

/// A prepared native basic change or swap. Entity keys and canonical variable
/// identities are retained; old values (including genuine None) are captured
/// once from the live registry. Description and execution use the same metadata.
/// This optional API does not alter solver moves, tabu identity or scoring.
/// Columns currently share one native value type; heterogeneous domains can use
/// a tagged value type or prepare separate typed moves.
pub struct InspectedBasicMove<'s, E, V> {
    variables: Vec<PlanningVariableRef<'s>>,
    left: Vec<E>,
    right: Vec<E>,
    old_left: Vec<Option<V>>,
    old_right: Vec<Option<V>>,
    change: Option<Option<V>>,
    pillar: bool,
}
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> InspectedBasicMove<'s, E, V> {
    fn capture(
        registry: &mut VariableStateRegistry<'s>,
        variables: &[PlanningVariableRef<'s>],
        entities: &[E],
    ) -> Result<Vec<Option<V>>, VariableStateError> {
        if variables.is_empty() || entities.is_empty() {
            return Err(VariableStateError {
                variable: "basic move".into(),
                reason: "variables and entity groups must be nonempty".into(),
            });
        }
        variables
            .iter()
            .map(|&variable| {
                let snapshot = registry.basic::<E, V>(variable)?.snapshot();
                let first = snapshot
                    .get(&entities[0])
                    .ok_or_else(|| VariableStateError {
                        variable: variable.name().into(),
                        reason: "unknown entity".into(),
                    })?;
                for entity in entities {
                    if snapshot.get(entity) != Some(first) {
                        return Err(VariableStateError {
                            variable: variable.name().into(),
                            reason: "pillar members must have the same registered value".into(),
                        });
                    }
                }
                Ok(first.clone())
            })
            .collect()
    }
    pub fn change(
        registry: &mut VariableStateRegistry<'s>,
        variable: PlanningVariableRef<'s>,
        entity: E,
        to: Option<V>,
    ) -> Result<Self, VariableStateError> {
        Self::prepare_change(registry, variable, vec![entity], to, false)
    }
    pub fn pillar_change(
        registry: &mut VariableStateRegistry<'s>,
        variable: PlanningVariableRef<'s>,
        entities: Vec<E>,
        to: Option<V>,
    ) -> Result<Self, VariableStateError> {
        Self::prepare_change(registry, variable, entities, to, true)
    }
    fn prepare_change(
        registry: &mut VariableStateRegistry<'s>,
        variable: PlanningVariableRef<'s>,
        entities: Vec<E>,
        to: Option<V>,
        pillar: bool,
    ) -> Result<Self, VariableStateError> {
        let old_left = Self::capture(registry, &[variable], &entities)?;
        Ok(Self {
            variables: vec![variable],
            left: entities,
            right: vec![],
            old_left,
            old_right: vec![],
            change: Some(to),
            pillar,
        })
    }
    pub fn swap(
        registry: &mut VariableStateRegistry<'s>,
        variables: Vec<PlanningVariableRef<'s>>,
        left: E,
        right: E,
    ) -> Result<Self, VariableStateError> {
        Self::prepare_swap(registry, variables, vec![left], vec![right], false)
    }
    pub fn pillar_swap(
        registry: &mut VariableStateRegistry<'s>,
        variables: Vec<PlanningVariableRef<'s>>,
        left: Vec<E>,
        right: Vec<E>,
    ) -> Result<Self, VariableStateError> {
        Self::prepare_swap(registry, variables, left, right, true)
    }
    fn prepare_swap(
        registry: &mut VariableStateRegistry<'s>,
        variables: Vec<PlanningVariableRef<'s>>,
        left: Vec<E>,
        right: Vec<E>,
        pillar: bool,
    ) -> Result<Self, VariableStateError> {
        let old_left = Self::capture(registry, &variables, &left)?;
        let old_right = Self::capture(registry, &variables, &right)?;
        Ok(Self {
            variables,
            left,
            right,
            old_left,
            old_right,
            change: None,
            pillar,
        })
    }
    pub fn variables(&self) -> &[PlanningVariableRef<'s>] {
        &self.variables
    }
    pub fn left_entities(&self) -> &[E] {
        &self.left
    }
    pub fn right_entities(&self) -> &[E] {
        &self.right
    }
    /// Outer None means swap; Some(None) means a genuine change to unassigned.
    pub fn change_destination(&self) -> Option<&Option<V>> {
        self.change.as_ref()
    }
    pub fn original_left_values(&self) -> &[Option<V>] {
        &self.old_left
    }
    pub fn original_right_values(&self) -> &[Option<V>] {
        &self.old_right
    }
    /// Apply through ordinary registered writes. Preflight rejects foreign,
    /// unbound or changed source fields before any write. Publication is sequential,
    /// as direct registry writes are; it is not an atomic scored transaction.
    pub fn execute(
        &self,
        registry: &mut VariableStateRegistry<'s>,
    ) -> Result<(), VariableStateError> {
        let left = Self::capture(registry, &self.variables, &self.left)?;
        let right = if self.change.is_none() {
            Self::capture(registry, &self.variables, &self.right)?
        } else {
            vec![]
        };
        if left != self.old_left || right != self.old_right {
            return Err(VariableStateError {
                variable: "basic move".into(),
                reason: "prepared source values changed".into(),
            });
        }
        for (index, &variable) in self.variables.iter().enumerate() {
            let destination = match &self.change {
                Some(to) => to,
                None => &self.old_right[index],
            };
            for entity in &self.left {
                registry.change_basic(variable, entity.clone(), destination.clone())?;
            }
            if self.change.is_none() {
                for entity in &self.right {
                    registry.change_basic(
                        variable,
                        entity.clone(),
                        self.old_left[index].clone(),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn describe(
        &self,
        mut entity_label: impl FnMut(&E) -> String,
        mut value_label: impl FnMut(&V) -> String,
    ) -> String {
        let mut group = |entities: &[E]| {
            let labels = entities
                .iter()
                .map(&mut entity_label)
                .collect::<Vec<_>>()
                .join(", ");
            if self.pillar {
                format!("[{labels}]")
            } else {
                labels
            }
        };
        let left = group(&self.left);
        let right = group(&self.right);
        let mut value = |v: &Option<V>| v.as_ref().map_or_else(|| "null".into(), &mut value_label);
        let old = self
            .old_left
            .iter()
            .map(&mut value)
            .collect::<Vec<_>>()
            .join(", ");
        if let Some(to) = &self.change {
            format!("{left} {{{old} -> {}}}", value(to))
        } else {
            let target = self
                .old_right
                .iter()
                .map(value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{left} {{{old}}} <-> {right} {{{target}}}")
        }
    }
}
