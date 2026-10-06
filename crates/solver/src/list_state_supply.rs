//! List-state projections, pin queries and typed built-in shadow events.
use crate::{
    ListProblem, ListVariableState, PlanningType, PlanningVariableKind, PlanningVariableRef,
    VariableListLocation, VariableStateError, VariableStateRegistry,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ListPinPolicy {
    pub fully_pinned: bool,
    pub prefix: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ListShadowProperty {
    Owner,
    Index,
    Previous,
    Next,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListBuiltinValue<E, V> {
    Owner(Option<E>),
    Index(Option<usize>),
    Previous(Option<V>),
    Next(Option<V>),
}
impl<E, V> ListBuiltinValue<E, V> {
    pub fn property(&self) -> ListShadowProperty {
        match self {
            Self::Owner(_) => ListShadowProperty::Owner,
            Self::Index(_) => ListShadowProperty::Index,
            Self::Previous(_) => ListShadowProperty::Previous,
            Self::Next(_) => ListShadowProperty::Next,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListBuiltinValues<E, V> {
    pub owner: Option<E>,
    pub index: Option<usize>,
    pub previous: Option<V>,
    pub next: Option<V>,
}
impl<E, V> Default for ListBuiltinValues<E, V> {
    fn default() -> Self {
        Self {
            owner: None,
            index: None,
            previous: None,
            next: None,
        }
    }
}
impl<E: Clone, V: Clone> ListBuiltinValues<E, V> {
    pub fn get(&self, property: ListShadowProperty) -> ListBuiltinValue<E, V> {
        match property {
            ListShadowProperty::Owner => ListBuiltinValue::Owner(self.owner.clone()),
            ListShadowProperty::Index => ListBuiltinValue::Index(self.index),
            ListShadowProperty::Previous => ListBuiltinValue::Previous(self.previous.clone()),
            ListShadowProperty::Next => ListBuiltinValue::Next(self.next.clone()),
        }
    }
    /// Apply a typed event to caller-owned materialized shadow storage.
    pub fn apply(&mut self, value: &ListBuiltinValue<E, V>) {
        match value {
            ListBuiltinValue::Owner(v) => self.owner = v.clone(),
            ListBuiltinValue::Index(v) => self.index = *v,
            ListBuiltinValue::Previous(v) => self.previous = v.clone(),
            ListBuiltinValue::Next(v) => self.next = v.clone(),
        }
    }
    fn at(location: Option<&VariableListLocation<E, V>>) -> Self {
        location.map_or_else(Self::default, |p| Self {
            owner: Some(p.entity.clone()),
            index: Some(p.index),
            previous: p.previous.clone(),
            next: p.next.clone(),
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListBuiltinChange<'s, E, V> {
    pub variable: PlanningVariableRef<'s>,
    pub value: V,
    pub before: ListBuiltinValue<E, V>,
    pub after: ListBuiltinValue<E, V>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListStateUpdate<'s, E, V> {
    pub shadows: Vec<ListBuiltinChange<'s, E, V>>,
    pub notifications: Vec<V>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListSupplyError<E, V> {
    DuplicateValue(V),
    UnknownValue(V),
    AlreadyUnassigned(V),
    UnknownEntity(E),
    InvalidRange {
        from: usize,
        to: usize,
        length: usize,
    },
    Metadata(&'static str),
    InvalidBindings,
    DuplicateSeed(V),
    State(VariableStateError),
}
impl<E: fmt::Debug, V: fmt::Debug> fmt::Display for ListSupplyError<E, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list state supply: {self:?}")
    }
}
impl<E: fmt::Debug, V: fmt::Debug> Error for ListSupplyError<E, V> {}
impl<E, V> From<VariableStateError> for ListSupplyError<E, V> {
    fn from(value: VariableStateError) -> Self {
        Self::State(value)
    }
}
/// A shared list-state view with a fixed value universe and typed outward events.
///
/// Assignment, position and pin queries follow the live state immediately. The
/// emitted shadow projection changes only on refresh. Bind any subset of owner,
/// index, previous and next fields; each must be a distinct shadow on V in the
/// same schema, with native E/usize/V value types. Events carry before/after
/// values and canonical metadata, and can update caller-owned storage or notify
/// ListShadowSession. No reflective Java setter calls are required.
///
/// With all four fields bound, changed shadow events are sufficient and fallback
/// notifications are suppressed. With a partial binding, scanned assigned values
/// still notify even if their projection is unchanged. Unassigned values notify
/// once when removed; untouched unassigned values do not notify on initialization.
/// Pin policy observes current positions; it does not authorize otherwise invalid
/// planning moves. Use ListProblem to validate moves and synchronize its result.
///
/// ```
/// use timetuck::*;
/// struct Solution; struct Route;
/// let schema=PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Route>(
///     vec![PlanningVariableDefinition::list::<u32>("visits",true)])])?;
/// let field=schema.entity::<Route>()?.variable("visits")?;
/// let mut registry=VariableStateRegistry::new(&schema);
/// registry.reset_lists(field,[(0usize,vec![1u32])])?;
/// let mut supply=ListStateSupply::new(registry.list::<usize,u32>(field)?,[1,2,3])?;
/// supply.set_pin(0,ListPinPolicy {fully_pinned:false,prefix:1});
/// assert!(supply.is_pinned(&1));
/// assert_eq!(supply.unassigned_count(),2);
/// assert_eq!(supply.refresh_all()?.notifications,vec![1]);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct ListStateSupply<'s, E, V> {
    state: ListVariableState<'s, E, V>,
    values: BTreeSet<V>,
    pins: BTreeMap<E, ListPinPolicy>,
    bindings: BTreeMap<ListShadowProperty, PlanningVariableRef<'s>>,
    shadows: BTreeMap<V, ListBuiltinValues<E, V>>,
    observed: BTreeMap<V, VariableListLocation<E, V>>,
}
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> ListStateSupply<'s, E, V> {
    pub fn new(
        state: ListVariableState<'s, E, V>,
        values: impl IntoIterator<Item = V>,
    ) -> Result<Self, ListSupplyError<E, V>> {
        let mut universe = BTreeSet::new();
        for v in values {
            if !universe.insert(v.clone()) {
                return Err(ListSupplyError::DuplicateValue(v));
            }
        }
        let shadows = universe
            .iter()
            .cloned()
            .map(|v| (v, ListBuiltinValues::default()))
            .collect();
        let mut supply = Self {
            state,
            values: universe,
            pins: BTreeMap::new(),
            bindings: BTreeMap::new(),
            shadows,
            observed: BTreeMap::new(),
        };
        supply.observed = supply.positions()?;
        Ok(supply)
    }
    /// Import the native model's value universe and pin policies using a complete
    /// one-to-one owner binding. Policies remain tied to stable entity identities.
    pub fn from_problem(
        state: ListVariableState<'s, E, V>,
        problem: &ListProblem<V>,
        bindings: &[(E, usize)],
    ) -> Result<Self, ListSupplyError<E, V>> {
        let mut entities = BTreeSet::new();
        let mut slots = BTreeSet::new();
        if bindings.len() != problem.entities().len() {
            return Err(ListSupplyError::InvalidBindings);
        }
        for (entity, index) in bindings {
            if *index >= problem.entities().len()
                || !entities.insert(entity.clone())
                || !slots.insert(*index)
            {
                return Err(ListSupplyError::InvalidBindings);
            }
        }
        if state.snapshot().keys().any(|e| !entities.contains(e)) {
            return Err(ListSupplyError::InvalidBindings);
        }
        let mut supply = Self::new(state, problem.values().iter().cloned())?;
        for (entity, index) in bindings {
            let e = &problem.entities()[*index];
            supply.set_pin(
                entity.clone(),
                ListPinPolicy {
                    fully_pinned: e.is_pinned(),
                    prefix: e.pinned_prefix(),
                },
            );
        }
        Ok(supply)
    }
    pub fn source(&self) -> PlanningVariableRef<'s> {
        self.state.variable()
    }
    pub fn set_pin(&mut self, entity: E, policy: ListPinPolicy) {
        self.pins.insert(entity, policy);
    }
    pub fn is_assigned(&self, value: &V) -> bool {
        self.values.contains(value) && self.state.is_assigned(value)
    }
    pub fn unassigned_count(&self) -> usize {
        self.values
            .iter()
            .filter(|v| !self.state.is_assigned(v))
            .count()
    }
    pub fn location(&self, value: &V) -> Option<VariableListLocation<E, V>> {
        self.values
            .contains(value)
            .then(|| self.state.location(value))
            .flatten()
    }
    pub fn is_pinned(&self, value: &V) -> bool {
        if self.pins.is_empty() {
            return false;
        }
        self.location(value).is_some_and(|p| {
            self.pins
                .get(&p.entity)
                .is_some_and(|pin| pin.fully_pinned || p.index < pin.prefix)
        })
    }
    pub fn externalize(
        &mut self,
        property: ListShadowProperty,
        variable: PlanningVariableRef<'s>,
    ) -> Result<(), ListSupplyError<E, V>> {
        if self.bindings.contains_key(&property) || self.bindings.values().any(|&v| v == variable) {
            return Err(ListSupplyError::Metadata(
                "shadow field/property already bound",
            ));
        }
        if !std::ptr::eq(
            variable.entity().solution(),
            self.source().entity().solution(),
        ) || variable.entity().entity_type() != PlanningType::of::<V>()
            || variable.kind() != PlanningVariableKind::Shadow
        {
            return Err(ListSupplyError::Metadata(
                "binding must be a shadow on the native list value entity in this schema",
            ));
        }
        let expected = match property {
            ListShadowProperty::Owner => PlanningType::of::<E>(),
            ListShadowProperty::Index => PlanningType::of::<usize>(),
            ListShadowProperty::Previous | ListShadowProperty::Next => PlanningType::of::<V>(),
        };
        if variable.value_type() != expected {
            return Err(ListSupplyError::Metadata(
                "shadow property value type differs from metadata",
            ));
        }
        self.bindings.insert(property, variable);
        Ok(())
    }
    pub fn shadow_values(&self, value: &V) -> Option<&ListBuiltinValues<E, V>> {
        self.shadows.get(value)
    }
    /// Import materialized fields (for example from a cloned solution). A refresh
    /// compares against them and emits only fields whose values really change.
    pub fn seed_shadows(
        &mut self,
        values: impl IntoIterator<Item = (V, ListBuiltinValues<E, V>)>,
    ) -> Result<(), ListSupplyError<E, V>> {
        let mut incoming = BTreeMap::new();
        for (v, fields) in values {
            if !self.values.contains(&v) {
                return Err(ListSupplyError::UnknownValue(v));
            }
            if incoming.insert(v.clone(), fields).is_some() {
                return Err(ListSupplyError::DuplicateSeed(v));
            }
        }
        self.shadows.extend(incoming);
        Ok(())
    }
    fn positions(&self) -> Result<BTreeMap<V, VariableListLocation<E, V>>, ListSupplyError<E, V>> {
        let mut result = BTreeMap::new();
        for (entity, list) in self.state.snapshot() {
            for (index, value) in list.iter().enumerate() {
                if !self.values.contains(value) {
                    return Err(ListSupplyError::UnknownValue(value.clone()));
                }
                result.insert(
                    value.clone(),
                    VariableListLocation {
                        entity: entity.clone(),
                        index,
                        previous: index.checked_sub(1).map(|i| list[i].clone()),
                        next: list.get(index + 1).cloned(),
                    },
                );
            }
        }
        Ok(result)
    }
    fn publish(
        &mut self,
        positions: BTreeMap<V, VariableListLocation<E, V>>,
        selected: BTreeSet<V>,
        touched: BTreeSet<V>,
    ) -> ListStateUpdate<'s, E, V> {
        let mut shadows = self.shadows.clone();
        let mut observed = self.observed.clone();
        let mut update = ListStateUpdate {
            shadows: vec![],
            notifications: vec![],
        };
        for value in selected {
            let now = positions.get(&value);
            let before_position = self.observed.get(&value);
            let new = ListBuiltinValues::at(now);
            let old = shadows.get_mut(&value).expect("validated universe");
            for (&property, &variable) in &self.bindings {
                let before = old.get(property);
                let after = new.get(property);
                if before != after {
                    old.apply(&after);
                    update.shadows.push(ListBuiltinChange {
                        variable,
                        value: value.clone(),
                        before,
                        after,
                    });
                }
            }
            if self.bindings.len() < 4
                && (now.is_some() || before_position.is_some())
                && (touched.contains(&value) || before_position != now)
            {
                update.notifications.push(value.clone());
            }
            if let Some(p) = now {
                observed.insert(value, p.clone());
            } else {
                observed.remove(&value);
            }
        }
        self.shadows = shadows;
        self.observed = observed;
        update
    }
    pub fn refresh_all(&mut self) -> Result<ListStateUpdate<'s, E, V>, ListSupplyError<E, V>> {
        let positions = self.positions()?;
        Ok(self.publish(positions, self.values.clone(), self.values.clone()))
    }
    /// Rescan the suffix after a bracket, including the predecessor when its next
    /// changes. end validates the bracket; index shifts can affect the full suffix.
    /// Also clear old positions removed from that suffix. The bound state already
    /// contains the whole new assignment, so moving between owners is supported.
    pub fn refresh_range(
        &mut self,
        entity: &E,
        from: usize,
        to: usize,
    ) -> Result<ListStateUpdate<'s, E, V>, ListSupplyError<E, V>> {
        let rows = self.state.snapshot();
        let list = rows
            .get(entity)
            .ok_or_else(|| ListSupplyError::UnknownEntity(entity.clone()))?;
        if from > to || to > list.len() {
            return Err(ListSupplyError::InvalidRange {
                from,
                to,
                length: list.len(),
            });
        }
        let positions = self.positions()?;
        let mut selected = BTreeSet::new();
        let mut touched = BTreeSet::new();
        for (value, p) in positions.iter().chain(self.observed.iter()) {
            if &p.entity == entity {
                if p.index >= from.saturating_sub(1) {
                    selected.insert(value.clone());
                }
                if p.index >= from {
                    touched.insert(value.clone());
                }
            }
        }
        Ok(self.publish(positions, selected, touched))
    }
    /// Remove one assigned value from this registry field and publish its new
    /// built-ins. This updates observation state; it does not bypass ListProblem
    /// move validation or mutate a separate native assignment on the caller's behalf.
    pub fn unassign(
        &mut self,
        registry: &mut VariableStateRegistry<'s>,
        value: &V,
    ) -> Result<ListStateUpdate<'s, E, V>, ListSupplyError<E, V>> {
        if !self.values.contains(value) {
            return Err(ListSupplyError::UnknownValue(value.clone()));
        }
        self.positions()?;
        let p = self
            .state
            .location(value)
            .ok_or_else(|| ListSupplyError::AlreadyUnassigned(value.clone()))?;
        let bound = registry.list::<E, V>(self.source())?;
        if !bound.same_instance(&self.state) {
            return Err(ListSupplyError::Metadata(
                "registry does not own this state",
            ));
        }
        let mut rows = self.state.snapshot();
        rows.get_mut(&p.entity)
            .expect("assigned owner")
            .remove(p.index);
        registry.update_lists(self.source(), rows)?;
        self.refresh_range(&p.entity, p.index, p.index)
    }
}
