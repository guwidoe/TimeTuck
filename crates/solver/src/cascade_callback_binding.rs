//! Actual typed callback registration bound to existing list cascade execution.
use crate::{
    ListAssignment, ListCascadeCache, ListCascadeContext, ListCascadeError, ListProblem,
    PlanningType, PlanningVariableKind, PlanningVariableRef,
};
use std::{any::Any, collections::HashMap, error::Error, fmt};
type Evaluate<V, S> = dyn for<'a> FnMut(ListCascadeContext<'a, V, S>) -> Option<S>;
/// A native callback whose actual Rust input/output types are runtime-checkable.
pub struct ListCascadeCallback<V, S>(Box<Evaluate<V, S>>);
impl<V, S> ListCascadeCallback<V, S> {
    pub fn new(
        callback: impl for<'a> FnMut(ListCascadeContext<'a, V, S>) -> Option<S> + 'static,
    ) -> Self {
        Self(Box::new(callback))
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum CascadeCallbackBindingError {
    Property {
        entity: PlanningType,
        property: String,
        method: String,
        reason: &'static str,
    },
}
impl fmt::Display for CascadeCallbackBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Property {
            entity,
            property,
            method,
            reason,
        } = self;
        write!(
            f,
            "The entity class ({}) has a cascading annotated property ({property}), but the method \"{method}\" cannot be found with the required native callback signature: {reason}",
            entity.name()
        )
    }
}
impl Error for CascadeCallbackBindingError {}
#[derive(Default)]
pub struct CascadeCallbackRegistry {
    members: HashMap<(PlanningType, String), Box<dyn Any>>,
}
impl CascadeCallbackRegistry {
    pub fn register<V: 'static, S: 'static>(
        &mut self,
        entity: PlanningType,
        name: impl Into<String>,
        callback: ListCascadeCallback<V, S>,
    ) {
        self.members
            .insert((entity, name.into()), Box::new(callback));
    }
    /// A real callable with another Rust signature remains a callable, but fails
    /// the required typed binding. No user-supplied validity flag is accepted.
    pub fn register_other<T: Any>(
        &mut self,
        entity: PlanningType,
        name: impl Into<String>,
        callback: T,
    ) {
        self.members
            .insert((entity, name.into()), Box::new(callback));
    }
    pub fn register_data<T: Any>(
        &mut self,
        entity: PlanningType,
        name: impl Into<String>,
        value: T,
    ) {
        self.members.insert((entity, name.into()), Box::new(value));
    }
    /// Validate actual property metadata and registered callable type before any
    /// callback/cascade initialization. Missing, data-only and wrong-signature
    /// bindings report native entity/property/requested method identity. This
    /// explicit registry is not Java reflection or overload discovery.
    pub fn bind<V: 'static, S: 'static>(
        &mut self,
        property: PlanningVariableRef<'_>,
        method: &str,
    ) -> Result<BoundCascadeCallback<'_, V, S>, CascadeCallbackBindingError> {
        let entity = property.entity().entity_type();
        let fail = |reason| CascadeCallbackBindingError::Property {
            entity,
            property: property.name().into(),
            method: method.into(),
            reason,
        };
        if property.kind() != PlanningVariableKind::Shadow
            || property.value_type() != PlanningType::of::<S>()
            || entity != PlanningType::of::<V>()
        {
            return Err(fail("property type or owner mismatch"));
        }
        let member = self
            .members
            .get_mut(&(entity, method.into()))
            .ok_or_else(|| fail("missing registration"))?;
        let callback = member
            .downcast_mut::<ListCascadeCallback<V, S>>()
            .ok_or_else(|| fail("member is data or has wrong callback input/output types"))?;
        Ok(BoundCascadeCallback { callback })
    }
}
pub struct BoundCascadeCallback<'a, V, S> {
    callback: &'a mut ListCascadeCallback<V, S>,
}
impl<V: Eq + Ord + Clone, S: Eq> BoundCascadeCallback<'_, V, S> {
    pub fn initialize(
        &mut self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<ListCascadeCache<V, S>, ListCascadeError<V>> {
        ListCascadeCache::new(problem, assignment, |context| (self.callback.0)(context))
    }
    pub fn update(
        &mut self,
        cascade: &mut ListCascadeCache<V, S>,
        assignment: &ListAssignment<V>,
        dirty: &[V],
    ) -> Result<Vec<V>, ListCascadeError<V>> {
        cascade.update(assignment, dirty, |context| (self.callback.0)(context))
    }
}
