//! Shared per-field variable state and notification tracking.
use crate::{
    Assignment, ListAssignment, PlanningSchema, PlanningType, PlanningVariableKind,
    PlanningVariableRef,
};
use std::{
    any::Any,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    rc::Rc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableStateError {
    pub variable: String,
    pub reason: String,
}
impl fmt::Display for VariableStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "variable ({}): {}", self.variable, self.reason)
    }
}
impl Error for VariableStateError {}
fn error(v: PlanningVariableRef<'_>, reason: &str) -> VariableStateError {
    VariableStateError {
        variable: v.name().into(),
        reason: reason.into(),
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingVariableEvent {
    Before,
    After,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableEventViolation<E> {
    pub entity: E,
    pub missing: MissingVariableEvent,
}
#[derive(Clone, Debug)]
struct EventLog<E> {
    before: BTreeSet<E>,
    after: BTreeSet<E>,
}
impl<E> Default for EventLog<E> {
    fn default() -> Self {
        Self {
            before: BTreeSet::new(),
            after: BTreeSet::new(),
        }
    }
}
/// One independently identified tracker per field. audit filters by canonical
/// field identity and clears the recorded event window after every call. Events
/// track presence, not pairing counts; multiple changes in a window are allowed.
pub struct VariableEventTracker<'s, E> {
    variable: PlanningVariableRef<'s>,
    inner: Rc<RefCell<EventLog<E>>>,
}
impl<E> Clone for VariableEventTracker<'_, E> {
    fn clone(&self) -> Self {
        Self {
            variable: self.variable,
            inner: self.inner.clone(),
        }
    }
}
impl<'s, E: Ord + Clone> VariableEventTracker<'s, E> {
    /// Create an independent event window, including for a shadow field.
    /// Use VariableStateRegistry::tracker to observe registry-generated events.
    pub fn new(variable: PlanningVariableRef<'s>) -> Self {
        Self {
            variable,
            inner: Rc::new(RefCell::new(EventLog::default())),
        }
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
    pub fn before(&self, entity: E) {
        self.inner.borrow_mut().before.insert(entity);
    }
    pub fn after(&self, entity: E) {
        self.inner.borrow_mut().after.insert(entity);
    }
    pub fn audit(
        &self,
        changed: impl IntoIterator<Item = (PlanningVariableRef<'s>, E)>,
    ) -> Vec<VariableEventViolation<E>> {
        let log = std::mem::take(&mut *self.inner.borrow_mut());
        let mut violations = vec![];
        for (variable, entity) in changed {
            if variable != self.variable {
                continue;
            }
            if !log.before.contains(&entity) {
                violations.push(VariableEventViolation {
                    entity: entity.clone(),
                    missing: MissingVariableEvent::Before,
                });
            }
            if !log.after.contains(&entity) {
                violations.push(VariableEventViolation {
                    entity,
                    missing: MissingVariableEvent::After,
                });
            }
        }
        violations
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Rows<E, V> {
    Basic(BTreeMap<E, Option<V>>),
    List(BTreeMap<E, Vec<V>>),
}
#[derive(Clone, Debug)]
struct FieldData<E, V> {
    rows: Rows<E, V>,
    inverse: BTreeMap<V, BTreeSet<E>>,
    locations: BTreeMap<V, (E, usize)>,
}
impl<E: Ord + Clone, V: Ord + Clone> FieldData<E, V> {
    fn build(
        variable: PlanningVariableRef<'_>,
        rows: Rows<E, V>,
    ) -> Result<Self, VariableStateError> {
        let mut inverse: BTreeMap<V, BTreeSet<E>> = BTreeMap::new();
        let mut locations = BTreeMap::new();
        match &rows {
            Rows::Basic(rows) => {
                for (e, v) in rows {
                    if let Some(v) = v {
                        inverse.entry(v.clone()).or_default().insert(e.clone());
                    }
                }
            }
            Rows::List(rows) => {
                for (e, list) in rows {
                    for (index, v) in list.iter().enumerate() {
                        if locations.insert(v.clone(), (e.clone(), index)).is_some() {
                            return Err(error(variable, "list value is assigned more than once"));
                        }
                    }
                }
            }
        }
        Ok(Self {
            rows,
            inverse,
            locations,
        })
    }
}
struct FieldRecord<E, V> {
    state: Rc<RefCell<FieldData<E, V>>>,
    tracker: Rc<RefCell<EventLog<E>>>,
}
/// A cloned handle shares live state. Use snapshot() for a detached assignment.
/// State identity survives field resets and repeated lookups, including when a
/// tracker was requested first. Entity/value keys must have stable Ord identity.
pub struct BasicVariableState<'s, E, V> {
    variable: PlanningVariableRef<'s>,
    inner: Rc<RefCell<FieldData<E, V>>>,
}
impl<E, V> Clone for BasicVariableState<'_, E, V> {
    fn clone(&self) -> Self {
        Self {
            variable: self.variable,
            inner: self.inner.clone(),
        }
    }
}
impl<'s, E: Ord + Clone, V: Ord + Clone> BasicVariableState<'s, E, V> {
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
    pub fn inverse(&self, value: &V) -> Vec<E> {
        self.inner
            .borrow()
            .inverse
            .get(value)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }
    pub fn snapshot(&self) -> BTreeMap<E, Option<V>> {
        match &self.inner.borrow().rows {
            Rows::Basic(rows) => rows.clone(),
            _ => unreachable!("typed field kind"),
        }
    }
    pub fn value(&self, entity: &E) -> Option<Option<V>> {
        match &self.inner.borrow().rows {
            Rows::Basic(rows) => rows.get(entity).cloned(),
            _ => unreachable!("typed field kind"),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableListLocation<E, V> {
    pub entity: E,
    pub index: usize,
    pub previous: Option<V>,
    pub next: Option<V>,
}
pub struct ListVariableState<'s, E, V> {
    variable: PlanningVariableRef<'s>,
    inner: Rc<RefCell<FieldData<E, V>>>,
}
impl<E, V> Clone for ListVariableState<'_, E, V> {
    fn clone(&self) -> Self {
        Self {
            variable: self.variable,
            inner: self.inner.clone(),
        }
    }
}
impl<'s, E: Ord + Clone, V: Ord + Clone> ListVariableState<'s, E, V> {
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
    pub fn is_assigned(&self, value: &V) -> bool {
        self.inner.borrow().locations.contains_key(value)
    }
    pub fn location(&self, value: &V) -> Option<VariableListLocation<E, V>> {
        let data = self.inner.borrow();
        let (entity, index) = data.locations.get(value)?;
        let Rows::List(rows) = &data.rows else {
            unreachable!("typed field kind")
        };
        let list = &rows[entity];
        Some(VariableListLocation {
            entity: entity.clone(),
            index: *index,
            previous: index.checked_sub(1).and_then(|i| list.get(i)).cloned(),
            next: list.get(index + 1).cloned(),
        })
    }
    pub fn snapshot(&self) -> BTreeMap<E, Vec<V>> {
        match &self.inner.borrow().rows {
            Rows::List(rows) => rows.clone(),
            _ => unreachable!("typed field kind"),
        }
    }
}
struct Slot<'s> {
    variable: PlanningVariableRef<'s>,
    data: Box<dyn Any>,
}
/// Field state for a native working solution. Stable handles let scoring,
/// inverse relations and diagnostic consumers share one current relation.
///
/// Linking/lookup never invokes a solution provider: before the first explicit
/// reset/synchronization the field is empty. Loading a field before requesting a
/// handle initializes later lookups immediately. Reset is per field, retains its
/// handles and clears its tracking window. Updates record both events for every
/// changed/added/removed entity and publish inverse/index state immediately.
///
/// This single-threaded registry observes assignments; it does not bypass model
/// validation, pins, range checks or score transactions. Synchronize validated
/// native assignments after apply/undo. Invalid input leaves existing state and
/// tracker events unchanged. Each update rebuilds that field's relation.
///
/// ```
/// use timetuck::*;
/// struct Schedule; struct Job;
/// let schema=PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Job>(
///     vec![PlanningVariableDefinition::basic::<i32>("worker",false)])])?;
/// let field=schema.entity::<Job>()?.variable("worker")?;
/// let mut state=VariableStateRegistry::new(&schema);
/// let handle=state.basic::<usize,i32>(field)?;
/// state.reset_basic(field,[(0usize,Some(1i32))])?;
/// state.change_basic(field,0usize,Some(2i32))?;
/// assert_eq!(handle.inverse(&2),vec![0]);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct VariableStateRegistry<'s> {
    schema: &'s PlanningSchema,
    slots: Vec<Slot<'s>>,
}
impl<'s> VariableStateRegistry<'s> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            slots: vec![],
        }
    }
    fn validate<V: 'static>(
        &self,
        v: PlanningVariableRef<'s>,
        list: bool,
    ) -> Result<(), VariableStateError> {
        if !std::ptr::eq(v.entity().solution(), self.schema) {
            return Err(error(v, "field belongs to another schema"));
        }
        if !matches!(
            (v.kind(), list),
            (PlanningVariableKind::Basic { .. }, false) | (PlanningVariableKind::List { .. }, true)
        ) {
            return Err(error(v, "wrong variable kind"));
        }
        if v.value_type() != PlanningType::of::<V>() {
            return Err(error(v, "value type differs from planning metadata"));
        }
        Ok(())
    }
    fn record<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        list: bool,
    ) -> Result<&mut FieldRecord<E, V>, VariableStateError> {
        self.validate::<V>(v, list)?;
        let index = match self.slots.iter().position(|s| s.variable == v) {
            Some(i) => i,
            None => {
                let rows = if list {
                    Rows::List(BTreeMap::new())
                } else {
                    Rows::Basic(BTreeMap::new())
                };
                let data = FieldRecord::<E, V> {
                    state: Rc::new(RefCell::new(FieldData::build(v, rows)?)),
                    tracker: Rc::new(RefCell::new(EventLog::default())),
                };
                self.slots.push(Slot {
                    variable: v,
                    data: Box::new(data),
                });
                self.slots.len() - 1
            }
        };
        self.slots[index]
            .data
            .downcast_mut()
            .ok_or_else(|| error(v, "entity key type differs from the existing field binding"))
    }
    pub fn basic<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
    ) -> Result<BasicVariableState<'s, E, V>, VariableStateError> {
        Ok(BasicVariableState {
            variable: v,
            inner: self.record::<E, V>(v, false)?.state.clone(),
        })
    }
    pub fn list<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
    ) -> Result<ListVariableState<'s, E, V>, VariableStateError> {
        Ok(ListVariableState {
            variable: v,
            inner: self.record::<E, V>(v, true)?.state.clone(),
        })
    }
    pub fn tracker<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
    ) -> Result<VariableEventTracker<'s, E>, VariableStateError> {
        let list = matches!(v.kind(), PlanningVariableKind::List { .. });
        Ok(VariableEventTracker {
            variable: v,
            inner: self.record::<E, V>(v, list)?.tracker.clone(),
        })
    }
    fn publish<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        rows: Rows<E, V>,
        reset: bool,
    ) -> Result<(), VariableStateError> {
        let list = matches!(rows, Rows::List(_));
        self.validate::<V>(v, list)?;
        let prepared = FieldData::build(v, rows)?;
        let record = self.record::<E, V>(v, list)?;
        let changes = match (&record.state.borrow().rows, &prepared.rows) {
            (Rows::Basic(old), Rows::Basic(new)) => changed_keys(old, new),
            (Rows::List(old), Rows::List(new)) => changed_keys(old, new),
            _ => unreachable!("typed field kind"),
        };
        if reset {
            *record.tracker.borrow_mut() = EventLog::default();
        } else {
            let mut log = record.tracker.borrow_mut();
            log.before.extend(changes.iter().cloned());
            log.after.extend(changes);
        }
        *record.state.borrow_mut() = prepared;
        Ok(())
    }
    pub fn reset_basic<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        rows: impl IntoIterator<Item = (E, Option<V>)>,
    ) -> Result<(), VariableStateError> {
        self.publish(v, Rows::Basic(unique_rows(v, rows)?), true)
    }
    pub fn update_basic<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        rows: impl IntoIterator<Item = (E, Option<V>)>,
    ) -> Result<(), VariableStateError> {
        self.publish(v, Rows::Basic(unique_rows(v, rows)?), false)
    }
    pub fn reset_lists<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        rows: impl IntoIterator<Item = (E, Vec<V>)>,
    ) -> Result<(), VariableStateError> {
        self.publish(v, Rows::List(unique_rows(v, rows)?), true)
    }
    pub fn update_lists<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        rows: impl IntoIterator<Item = (E, Vec<V>)>,
    ) -> Result<(), VariableStateError> {
        self.publish(v, Rows::List(unique_rows(v, rows)?), false)
    }
    pub fn change_basic<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        entity: E,
        value: Option<V>,
    ) -> Result<(), VariableStateError> {
        let state = self.basic::<E, V>(v)?;
        let mut rows = state.snapshot();
        let current = rows
            .get_mut(&entity)
            .ok_or_else(|| error(v, "unknown entity"))?;
        *current = value;
        self.publish(v, Rows::Basic(rows), false)
    }
    /// Bind entity identities to flattened basic-variable slots. Different fields
    /// can select separate slots from one native or mixed assignment.
    pub fn synchronize_basic<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        assignment: &Assignment<V>,
        bindings: &[(E, usize)],
    ) -> Result<(), VariableStateError> {
        let mut seen = BTreeSet::new();
        let mut rows = vec![];
        for (e, i) in bindings {
            if !seen.insert(*i) {
                return Err(error(v, "duplicate assignment slot"));
            }
            rows.push((
                e.clone(),
                assignment
                    .values()
                    .get(*i)
                    .ok_or_else(|| error(v, "assignment slot out of bounds"))?
                    .clone(),
            ));
        }
        self.update_basic(v, rows)
    }
    pub fn synchronize_lists<E: Ord + Clone + 'static, V: Ord + Clone + 'static>(
        &mut self,
        v: PlanningVariableRef<'s>,
        assignment: &ListAssignment<V>,
        bindings: &[(E, usize)],
    ) -> Result<(), VariableStateError> {
        let mut seen = BTreeSet::new();
        let mut rows = vec![];
        for (e, i) in bindings {
            if !seen.insert(*i) {
                return Err(error(v, "duplicate assignment slot"));
            }
            rows.push((
                e.clone(),
                assignment
                    .list(*i)
                    .ok_or_else(|| error(v, "assignment slot out of bounds"))?
                    .to_vec(),
            ));
        }
        self.update_lists(v, rows)
    }
}
fn unique_rows<E: Ord, V>(
    v: PlanningVariableRef<'_>,
    rows: impl IntoIterator<Item = (E, V)>,
) -> Result<BTreeMap<E, V>, VariableStateError> {
    let mut result = BTreeMap::new();
    for (e, value) in rows {
        if result.insert(e, value).is_some() {
            return Err(error(v, "duplicate entity key"));
        }
    }
    Ok(result)
}
fn changed_keys<E: Ord + Clone, V: Eq>(old: &BTreeMap<E, V>, new: &BTreeMap<E, V>) -> BTreeSet<E> {
    old.keys()
        .chain(new.keys())
        .filter(|e| old.get(*e) != new.get(*e))
        .cloned()
        .collect()
}
