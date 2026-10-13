//! Recorded typed basic-variable restoration through the live state registry.
use crate::{
    PlanningType, PlanningVariableKind, PlanningVariableRef, VariableStateError,
    VariableStateRegistry,
};
/// An immutable restore instruction for one canonical field and entity.
pub struct BasicRestoreAction<'s, E, V> {
    pub variable: PlanningVariableRef<'s>,
    pub entity: E,
    pub value: Option<V>,
}
impl<'s, E: 'static, V: 'static> BasicRestoreAction<'s, E, V> {
    pub fn new(
        variable: PlanningVariableRef<'s>,
        entity: E,
        value: Option<V>,
    ) -> Result<Self, VariableStateError> {
        if !matches!(variable.kind(), PlanningVariableKind::Basic { .. })
            || variable.value_type() != PlanningType::of::<V>()
        {
            return Err(VariableStateError {
                variable: variable.name().into(),
                reason: "restore action requires a matching basic value type".into(),
            });
        }
        Ok(Self {
            variable,
            entity,
            value,
        })
    }
}
/// Captures each actual old value before a successful registry change. Repeated
/// writes to one entity remain distinct actions. Failed writes add no action.
/// Storage reservation and capture finish before mutation; all ordinary registry
/// inverse indexes and before/after trackers continue to update normally.
pub struct BasicChangeRecorder<'s, E, V> {
    actions: Vec<BasicRestoreAction<'s, E, V>>,
}
impl<E, V> Default for BasicChangeRecorder<'_, E, V> {
    fn default() -> Self {
        Self { actions: vec![] }
    }
}
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> BasicChangeRecorder<'s, E, V> {
    pub fn change(
        &mut self,
        registry: &mut VariableStateRegistry<'s>,
        variable: PlanningVariableRef<'s>,
        entity: E,
        value: Option<V>,
    ) -> Result<(), VariableStateError> {
        let old = registry
            .basic::<E, V>(variable)?
            .snapshot()
            .remove(&entity)
            .ok_or_else(|| VariableStateError {
                variable: variable.name().into(),
                reason: "unknown entity".into(),
            })?;
        let action = BasicRestoreAction::new(variable, entity.clone(), old)?;
        self.actions.reserve(1);
        registry.change_basic(variable, entity, value)?;
        self.actions.push(action);
        Ok(())
    }
    pub fn into_undo(self) -> RecordedBasicUndo<'s, E, V> {
        RecordedBasicUndo::new(self.actions)
    }
}
/// Executes real registry writes in reverse recording order. Each successful
/// action advances the cursor before notifying the caller. A registry error
/// retains the failed action and remaining prefix for retry. Observer panics
/// leave the just-completed action consumed. This is sequential replay, not an
/// atomic multi-action transaction; observers see actual intermediate states.
/// Creating an undo from actions does not validate against a particular registry.
/// Execution enforces schema ownership, bound key types and entity registration. No callbacks run
/// for an empty/already completed replay. Domain/range policy is the caller's
/// responsibility, as for direct registry changes; this is not a scored move.
pub struct RecordedBasicUndo<'s, E, V> {
    actions: Vec<BasicRestoreAction<'s, E, V>>,
    remaining: usize,
}
impl<'s, E, V> RecordedBasicUndo<'s, E, V> {
    pub fn new(actions: Vec<BasicRestoreAction<'s, E, V>>) -> Self {
        let remaining = actions.len();
        Self { actions, remaining }
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
}
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> RecordedBasicUndo<'s, E, V> {
    pub fn execute(
        &mut self,
        registry: &mut VariableStateRegistry<'s>,
        mut applied: impl FnMut(&BasicRestoreAction<'s, E, V>, &mut VariableStateRegistry<'s>),
    ) -> Result<(), VariableStateError> {
        while self.remaining != 0 {
            let action = &self.actions[self.remaining - 1];
            registry.change_basic(action.variable, action.entity.clone(), action.value.clone())?;
            self.remaining -= 1;
            applied(action, registry);
        }
        Ok(())
    }
}
