//! Borrowed current-value queries distinct from score initialization/feasibility.
use crate::*;
/// The actual current field shape. V is the scalar type or list element type.
pub enum PlanningVariableValue<'a, V> {
    Basic(Option<&'a V>),
    List(&'a [V]),
}
impl PlanningVariableRef<'_> {
    /// Null basic fields are eligible even when unassignment is allowed. List
    /// fields are never reinitialized by this query, including empty lists.
    /// This does not override pins or select a construction heuristic.
    pub fn is_reinitializable<V: 'static>(
        self,
        current: PlanningVariableValue<'_, V>,
    ) -> Result<bool, PlanningSchemaError> {
        self.check_initialization_type::<V>()?;
        match (self.kind(), current) {
            (PlanningVariableKind::Basic { .. }, PlanningVariableValue::Basic(value)) => {
                Ok(value.is_none())
            }
            (PlanningVariableKind::List { .. }, PlanningVariableValue::List(_)) => Ok(false),
            _ => Err(PlanningSchemaError::WrongVariableKind {
                variable: self.name().into(),
            }),
        }
    }
    /// Read one actual entity field through a typed getter. Reject incompatible
    /// entity/value metadata before invoking the getter; no values are cloned.
    pub fn is_reinitializable_on<'e, E: 'static, V: 'static>(
        self,
        entity: &'e E,
        get: impl FnOnce(&'e E) -> PlanningVariableValue<'e, V>,
    ) -> Result<bool, PlanningSchemaError> {
        if self.entity().entity_type() != PlanningType::of::<E>() {
            return Err(PlanningSchemaError::UnknownEntity(PlanningType::of::<E>()));
        }
        self.check_initialization_type::<V>()?;
        self.is_reinitializable(get(entity))
    }
    fn check_initialization_type<V: 'static>(self) -> Result<(), PlanningSchemaError> {
        if !self.is_genuine() {
            return Err(PlanningSchemaError::WrongVariableKind {
                variable: self.name().into(),
            });
        }
        let actual = PlanningType::of::<V>();
        if self.value_type() != actual {
            return Err(PlanningSchemaError::WrongValueType {
                variable: self.name().into(),
                expected: self.value_type(),
                actual,
            });
        }
        Ok(())
    }
}
