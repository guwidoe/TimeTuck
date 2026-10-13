//! Explicit snapshots and notification diagnostics for move/undo corruption.
use crate::{
    MissingVariableEvent, PlanningSchema, PlanningType, PlanningVariableKind, PlanningVariableRef,
    VariableEventTracker,
};
use std::{any::Any, collections::BTreeMap, error::Error, fmt, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolutionTrackingError {
    Metadata,
    ValueType,
    DuplicateField,
    RegistrationClosed,
    ForeignSnapshot,
    MissingBeforeMove,
    MissingAfterMove,
    MissingAfterUndo,
    MissingRebuild,
    StageAlreadyCaptured,
}
impl fmt::Display for SolutionTrackingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solution tracking: {self:?}")
    }
}
impl Error for SolutionTrackingError {}
#[derive(Clone)]
struct Captured {
    value: Rc<dyn Any>,
    equal: fn(&dyn Any, &dyn Any) -> bool,
    text: String,
}
impl Captured {
    fn new<T: Eq + 'static>(value: T, text: String) -> Self {
        Self {
            value: Rc::new(value),
            equal: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
            text,
        }
    }
}
/// Detached values captured by registered readers. Readers must return owned,
/// immutable values: an Rc pointing to mutable model storage is not a snapshot.
#[derive(Clone)]
pub struct SolutionSnapshot<'s, E> {
    identity: Rc<()>,
    fields: Vec<(PlanningVariableRef<'s>, BTreeMap<E, Captured>)>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableValueViolation<'s, E> {
    pub variable: PlanningVariableRef<'s>,
    pub entity: E,
    pub expected: String,
    pub actual: String,
}
impl<E: fmt::Display> fmt::Display for VariableValueViolation<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self
            .variable
            .entity()
            .entity_type()
            .name()
            .rsplit("::")
            .next()
            .unwrap();
        write!(
            f,
            "Actual value ({}) of variable {} on {name} entity ({}) differs from expected ({})",
            self.actual,
            self.variable.name(),
            self.entity,
            self.expected
        )
    }
}
/// A typed borrowed view into an owned snapshot. Scalar registrations store
/// `Option<V>`; list registrations store `Vec<V>`. Entity keys and metadata are
/// those captured by the registering tracker, independent of later world edits.
#[derive(Debug)]
pub struct SnapshotValue<'v, 's, E, V> {
    pub variable: PlanningVariableRef<'s>,
    pub entity: &'v E,
    pub value: &'v V,
}
type ChangedValue<'v, E> = (&'v E, Option<&'v Captured>, Option<&'v Captured>);
struct ChangedValues<'v, E> {
    before: std::iter::Peekable<std::collections::btree_map::Iter<'v, E, Captured>>,
    after: std::iter::Peekable<std::collections::btree_map::Iter<'v, E, Captured>>,
}
impl<'v, E: Ord> ChangedValues<'v, E> {
    fn new(before: &'v BTreeMap<E, Captured>, after: &'v BTreeMap<E, Captured>) -> Self {
        Self {
            before: before.iter().peekable(),
            after: after.iter().peekable(),
        }
    }
}
impl<'v, E: Ord> Iterator for ChangedValues<'v, E> {
    type Item = ChangedValue<'v, E>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let order = match (self.before.peek(), self.after.peek()) {
                (Some((a, _)), Some((b, _))) => a.cmp(b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => return None,
            };
            match order {
                std::cmp::Ordering::Less => {
                    let (key, value) = self.before.next().unwrap();
                    return Some((key, Some(value), None));
                }
                std::cmp::Ordering::Greater => {
                    let (key, value) = self.after.next().unwrap();
                    return Some((key, None, Some(value)));
                }
                std::cmp::Ordering::Equal => {
                    let (key, a) = self.before.next().unwrap();
                    let (_, b) = self.after.next().unwrap();
                    if !(a.equal)(a.value.as_ref(), b.value.as_ref()) {
                        return Some((key, Some(a), Some(b)));
                    }
                }
            }
        }
    }
}
impl<E: Ord> std::iter::FusedIterator for ChangedValues<'_, E> {}
impl<'s, E: Ord + Clone> SolutionSnapshot<'s, E> {
    /// Actual field registration metadata captured by this snapshot.
    pub fn registered_fields(&self) -> impl Iterator<Item = PlanningVariableRef<'s>> + '_ {
        self.fields.iter().map(|(field, _)| *field)
    }
    /// Get one typed captured value. Unknown fields return Metadata; missing
    /// entities return None; a present value of another type returns ValueType.
    pub fn get<V: 'static>(
        &self,
        variable: PlanningVariableRef<'s>,
        entity: &E,
    ) -> Result<Option<SnapshotValue<'_, 's, E, V>>, SolutionTrackingError> {
        let (_, values) = self
            .fields
            .iter()
            .find(|(field, _)| *field == variable)
            .ok_or(SolutionTrackingError::Metadata)?;
        values
            .get_key_value(entity)
            .map(|(entity, captured)| {
                let value = captured
                    .value
                    .downcast_ref()
                    .ok_or(SolutionTrackingError::ValueType)?;
                Ok(SnapshotValue {
                    variable,
                    entity,
                    value,
                })
            })
            .transpose()
    }
    fn validate_comparison(&self, actual: &Self) -> Result<(), SolutionTrackingError> {
        if !Rc::ptr_eq(&self.identity, &actual.identity)
            || self.fields.len() != actual.fields.len()
            || self
                .fields
                .iter()
                .zip(&actual.fields)
                .any(|((a, _), (b, _))| a != b)
        {
            return Err(SolutionTrackingError::ForeignSnapshot);
        }
        Ok(())
    }
    /// Borrow changed field/entity keys in registration order, then entity order.
    /// Compares typed values; missing entities differ from present null values.
    /// The merge walks both maps without allocating a key set or cloning keys.
    /// Both snapshots must come from the same tracker and registration state.
    pub fn changed_variables<'v>(
        &'v self,
        actual: &'v Self,
    ) -> Result<impl Iterator<Item = (PlanningVariableRef<'s>, &'v E)> + 'v, SolutionTrackingError>
    {
        self.validate_comparison(actual)?;
        Ok(self
            .fields
            .iter()
            .zip(&actual.fields)
            .flat_map(|((field, before), (_, after))| {
                ChangedValues::new(before, after).map(move |(key, _, _)| (*field, key))
            }))
    }
    /// Number of changed registered field/entity pairs, without diagnostic strings
    /// or key allocations. Lists count once per changed list field, not per value.
    /// Register only genuine fields when using this as planning mutation distance.
    pub fn mutation_count(&self, actual: &Self) -> Result<usize, SolutionTrackingError> {
        Ok(self.changed_variables(actual)?.count())
    }
    /// Compare exact typed values, not their formatted labels. Missing entities
    /// are distinct from present entities with an unassigned (null) variable.
    pub fn differences(
        &self,
        actual: &Self,
    ) -> Result<Vec<VariableValueViolation<'s, E>>, SolutionTrackingError> {
        self.validate_comparison(actual)?;
        Ok(self
            .fields
            .iter()
            .zip(&actual.fields)
            .flat_map(|((variable, expected), (_, actual))| {
                ChangedValues::new(expected, actual).map(move |(key, a, b)| {
                    VariableValueViolation {
                        variable: *variable,
                        entity: key.clone(),
                        expected: a.map_or_else(|| "<missing entity>".into(), |v| v.text.clone()),
                        actual: b.map_or_else(|| "<missing entity>".into(), |v| v.text.clone()),
                    }
                })
            })
            .collect())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolutionEventViolation<'s, E> {
    pub variable: PlanningVariableRef<'s>,
    pub entity: E,
    pub missing: MissingVariableEvent,
}
impl<E: fmt::Display> fmt::Display for SolutionEventViolation<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let event = match self.missing {
            MissingVariableEvent::Before => "beforeVariableChanged",
            MissingVariableEvent::After => "afterVariableChanged",
        };
        write!(
            f,
            "Entity ({}) is missing a {event} call for variable ({}).",
            self.entity,
            self.variable.name()
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolutionCorruptionReport<'s, E> {
    pub before_vs_undo: Vec<VariableValueViolation<'s, E>>,
    pub rebuilt_vs_before: Vec<VariableValueViolation<'s, E>>,
    pub rebuilt_vs_undo: Vec<VariableValueViolation<'s, E>>,
    pub forward_events: Vec<SolutionEventViolation<'s, E>>,
    pub undo_events: Vec<SolutionEventViolation<'s, E>>,
}
impl<E> SolutionCorruptionReport<'_, E> {
    pub fn is_empty(&self) -> bool {
        self.before_vs_undo.is_empty()
            && self.rebuilt_vs_before.is_empty()
            && self.rebuilt_vs_undo.is_empty()
            && self.forward_events.is_empty()
            && self.undo_events.is_empty()
    }
}
/// Render at most five items with a remainder count. Structured reports retain all items.
pub fn format_tracking_list(items: &[impl fmt::Display]) -> String {
    let mut text = String::new();
    for item in items.iter().take(5) {
        text.push_str(&format!("  - {item}\n"));
    }
    if items.len() > 5 {
        text.push_str(&format!("  ...({} more)\n", items.len() - 5));
    }
    text
}
impl<E: fmt::Display> fmt::Display for SolutionCorruptionReport<'_, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (heading, items) in [
            (
                "Variables that are different between before and undo:",
                &self.before_vs_undo,
            ),
            (
                "Variables that are different between from scratch and before:",
                &self.rebuilt_vs_before,
            ),
            (
                "Variables that are different between from scratch and undo:",
                &self.rebuilt_vs_undo,
            ),
        ] {
            if !items.is_empty() {
                writeln!(f, "{heading}\n{}", format_tracking_list(items))?;
            }
        }
        for (heading, items) in [
            (
                "Missing shadow variable update events for actual move:",
                &self.forward_events,
            ),
            (
                "Missing shadow variable update events for undo move:",
                &self.undo_events,
            ),
        ] {
            if !items.is_empty() {
                writeln!(f, "{heading}\n{}", format_tracking_list(items))?;
            }
        }
        Ok(())
    }
}
type Reader<'a, E, W> = Box<dyn Fn(&W) -> BTreeMap<E, Captured> + 'a>;
struct Field<'a, 's, E, W> {
    variable: PlanningVariableRef<'s>,
    reader: Reader<'a, E, W>,
    events: VariableEventTracker<'s, E>,
}
/// Optional runtime diagnostic coordinator. Register canonical fields and owned
/// readers, then capture before_move, after_move and after_undo. Feed a detached
/// full recomputation to rebuilt_before/rebuilt_undo; report compares values and
/// separately audits forward/undo event windows. This does not calculate scores.
///
/// Registration returns an independent tracker for manual notifications; supplying
/// a registry tracker with use_events instead observes that registry's events.
/// Event windows are consumed by this diagnostic, so they must not be shared with
/// another consumer that audits or resets them mid-move. Captures call readers
/// before changing diagnostic state. Formatting a report never consumes events.
/// Only registered fields are checked; an empty report does not establish that
/// unregistered fields or the score calculation are correct.
///
/// ```
/// use timetuck::*;
/// use std::collections::BTreeMap;
/// struct Solution;
/// struct Entity;
/// let schema=PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
///     PlanningVariableDefinition::basic::<i32>("value",true),
/// ])])?;
/// let field=schema.entity::<Entity>().unwrap().variable("value").unwrap();
/// let mut tracker=SolutionTracker::new(&schema);
/// let events=tracker.scalar(field, |value:&Option<i32>| BTreeMap::from([(0usize,*value)]), i32::to_string)?;
/// tracker.before_move(&Some(1));
/// events.before(0); events.after(0);
/// tracker.after_move(&Some(2))?;
/// events.before(0); events.after(0);
/// tracker.after_undo(&Some(1))?;
/// tracker.rebuilt_before(&Some(1))?;
/// tracker.rebuilt_undo(&Some(1))?;
/// assert!(tracker.report()?.is_empty());
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct SolutionTracker<'a, 's, E, W> {
    schema: &'s PlanningSchema,
    identity: Rc<()>,
    fields: Vec<Field<'a, 's, E, W>>,
    before: Option<SolutionSnapshot<'s, E>>,
    after: Option<SolutionSnapshot<'s, E>>,
    undo: Option<SolutionSnapshot<'s, E>>,
    rebuilt_before: Option<SolutionSnapshot<'s, E>>,
    rebuilt_undo: Option<SolutionSnapshot<'s, E>>,
    forward_events: Vec<SolutionEventViolation<'s, E>>,
    undo_events: Vec<SolutionEventViolation<'s, E>>,
}
impl<'a, 's, E: Ord + Clone + 'a, W: 'a> SolutionTracker<'a, 's, E, W> {
    pub fn new(schema: &'s PlanningSchema) -> Self {
        Self {
            schema,
            identity: Rc::new(()),
            fields: vec![],
            before: None,
            after: None,
            undo: None,
            rebuilt_before: None,
            rebuilt_undo: None,
            forward_events: vec![],
            undo_events: vec![],
        }
    }
    fn register<V: 'static, T: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        list: bool,
        reader: impl Fn(&W) -> BTreeMap<E, T> + 'a,
        format: impl Fn(&T) -> String + 'a,
    ) -> Result<VariableEventTracker<'s, E>, SolutionTrackingError> {
        if self.before.is_some() {
            return Err(SolutionTrackingError::RegistrationClosed);
        }
        if !std::ptr::eq(variable.entity().solution(), self.schema)
            || variable.value_type() != PlanningType::of::<V>()
            || matches!(variable.kind(), PlanningVariableKind::List { .. }) != list
        {
            return Err(SolutionTrackingError::Metadata);
        }
        if self.fields.iter().any(|f| f.variable == variable) {
            return Err(SolutionTrackingError::DuplicateField);
        }
        let events = VariableEventTracker::new(variable);
        self.fields.push(Field {
            variable,
            events: events.clone(),
            reader: Box::new(move |world| {
                reader(world)
                    .into_iter()
                    .map(|(e, v)| {
                        let text = format(&v);
                        (e, Captured::new(v, text))
                    })
                    .collect()
            }),
        });
        Ok(events)
    }
    /// Register a scalar basic or shadow field. None is rendered as null.
    pub fn scalar<V: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        reader: impl Fn(&W) -> BTreeMap<E, Option<V>> + 'a,
        format: impl Fn(&V) -> String + 'a,
    ) -> Result<VariableEventTracker<'s, E>, SolutionTrackingError> {
        self.register::<V, Option<V>>(variable, false, reader, move |v| {
            v.as_ref().map_or_else(|| "null".into(), &format)
        })
    }
    pub fn list<V: Eq + 'static>(
        &mut self,
        variable: PlanningVariableRef<'s>,
        reader: impl Fn(&W) -> BTreeMap<E, Vec<V>> + 'a,
        format: impl Fn(&V) -> String + 'a,
    ) -> Result<VariableEventTracker<'s, E>, SolutionTrackingError> {
        self.register::<V, Vec<V>>(variable, true, reader, move |v| {
            format!("[{}]", v.iter().map(&format).collect::<Vec<_>>().join(", "))
        })
    }
    pub fn use_events(
        &mut self,
        events: VariableEventTracker<'s, E>,
    ) -> Result<(), SolutionTrackingError> {
        if self.before.is_some() {
            return Err(SolutionTrackingError::RegistrationClosed);
        }
        let field = self
            .fields
            .iter_mut()
            .find(|f| f.variable == events.variable())
            .ok_or(SolutionTrackingError::Metadata)?;
        field.events = events;
        Ok(())
    }
    pub fn snapshot(&self, world: &W) -> SolutionSnapshot<'s, E> {
        SolutionSnapshot {
            identity: self.identity.clone(),
            fields: self
                .fields
                .iter()
                .map(|f| (f.variable, (f.reader)(world)))
                .collect(),
        }
    }
    fn audit(
        &self,
        changes: &[VariableValueViolation<'s, E>],
    ) -> Vec<SolutionEventViolation<'s, E>> {
        self.fields
            .iter()
            .flat_map(|field| {
                field
                    .events
                    .audit(changes.iter().map(|c| (c.variable, c.entity.clone())))
                    .into_iter()
                    .map(|v| SolutionEventViolation {
                        variable: field.variable,
                        entity: v.entity,
                        missing: v.missing,
                    })
            })
            .collect()
    }
    pub fn before_move(&mut self, world: &W) {
        let snapshot = self.snapshot(world);
        self.audit(&[]);
        self.before = Some(snapshot);
        self.after = None;
        self.undo = None;
        self.rebuilt_before = None;
        self.rebuilt_undo = None;
        self.forward_events.clear();
        self.undo_events.clear();
    }
    pub fn after_move(&mut self, world: &W) -> Result<(), SolutionTrackingError> {
        if self.after.is_some() {
            return Err(SolutionTrackingError::StageAlreadyCaptured);
        }
        let before = self
            .before
            .as_ref()
            .ok_or(SolutionTrackingError::MissingBeforeMove)?;
        let snapshot = self.snapshot(world);
        let changes = before.differences(&snapshot)?;
        self.forward_events = self.audit(&changes);
        self.after = Some(snapshot);
        self.undo = None;
        self.undo_events.clear();
        self.rebuilt_undo = None;
        Ok(())
    }
    pub fn after_undo(&mut self, world: &W) -> Result<(), SolutionTrackingError> {
        if self.undo.is_some() {
            return Err(SolutionTrackingError::StageAlreadyCaptured);
        }
        let after = self
            .after
            .as_ref()
            .ok_or(SolutionTrackingError::MissingAfterMove)?;
        let snapshot = self.snapshot(world);
        let changes = after.differences(&snapshot)?;
        self.undo_events = self.audit(&changes);
        self.undo = Some(snapshot);
        self.rebuilt_undo = None;
        Ok(())
    }
    pub fn rebuilt_before(&mut self, world: &W) -> Result<(), SolutionTrackingError> {
        if self.before.is_none() {
            return Err(SolutionTrackingError::MissingBeforeMove);
        }
        self.rebuilt_before = Some(self.snapshot(world));
        Ok(())
    }
    pub fn rebuilt_undo(&mut self, world: &W) -> Result<(), SolutionTrackingError> {
        if self.undo.is_none() {
            return Err(SolutionTrackingError::MissingAfterUndo);
        }
        self.rebuilt_undo = Some(self.snapshot(world));
        Ok(())
    }
    pub fn report(&self) -> Result<SolutionCorruptionReport<'s, E>, SolutionTrackingError> {
        let before = self
            .before
            .as_ref()
            .ok_or(SolutionTrackingError::MissingBeforeMove)?;
        let undo = self
            .undo
            .as_ref()
            .ok_or(SolutionTrackingError::MissingAfterUndo)?;
        let rebuilt_before = self
            .rebuilt_before
            .as_ref()
            .ok_or(SolutionTrackingError::MissingRebuild)?;
        let rebuilt_undo = self
            .rebuilt_undo
            .as_ref()
            .ok_or(SolutionTrackingError::MissingRebuild)?;
        Ok(SolutionCorruptionReport {
            before_vs_undo: before.differences(undo)?,
            rebuilt_vs_before: rebuilt_before.differences(before)?,
            rebuilt_vs_undo: rebuilt_undo.differences(undo)?,
            forward_events: self.forward_events.clone(),
            undo_events: self.undo_events.clone(),
        })
    }
}
