//! Shared recording of bounded list edits and transactional notification-aware undo.
use crate::{PlanningType, PlanningVariableKind, PlanningVariableRef};
use std::{cell::RefCell, collections::BTreeMap, error::Error, fmt, rc::Rc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListRecordingError {
    Metadata,
    Busy,
    AlreadyOpen,
    NoOpenBracket,
    WrongStart {
        expected: usize,
        actual: usize,
    },
    InvalidRange {
        from: usize,
        to: usize,
        length: usize,
    },
    WrongLengthChange,
    ChangedOutsideRange,
    UnclosedBracket,
    UnknownEntity,
    StateMismatch,
}
impl fmt::Display for ListRecordingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyOpen => f.write_str("entity already has an open before list change"),
            Self::NoOpenBracket => f.write_str("entity has no matching before list change"),
            Self::WrongStart { expected, actual } => write!(
                f,
                "after start {actual} must match its before list change start {expected}"
            ),
            Self::WrongLengthChange => {
                f.write_str("list actually changed length by a different amount than reported")
            }
            Self::UnclosedBracket => {
                f.write_str("a before list change was never closed by its after list change")
            }
            _ => write!(f, "list recording: {self:?}"),
        }
    }
}
impl Error for ListRecordingError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListChangeNotification<'s, E> {
    Before {
        variable: PlanningVariableRef<'s>,
        entity: E,
        from: usize,
        to: usize,
    },
    After {
        variable: PlanningVariableRef<'s>,
        entity: E,
        from: usize,
        to: usize,
    },
    UpdateShadows,
}
struct Edit<E, V> {
    entity: E,
    from: usize,
    before_to: usize,
    before: Rc<Vec<V>>,
    after: Option<(usize, Rc<Vec<V>>)>,
}
struct Recording<E, V> {
    edits: Vec<Rc<Edit<E, V>>>,
    pending: crate::PendingListChanges<E, usize>,
}
impl<E: Clone, V> Clone for Recording<E, V> {
    fn clone(&self) -> Self {
        Self {
            edits: self.edits.clone(),
            pending: self.pending.clone(),
        }
    }
}
impl<E, V> Default for Recording<E, V> {
    fn default() -> Self {
        Self {
            edits: vec![],
            pending: crate::PendingListChanges::default(),
        }
    }
}
/// Records one canonical list variable with caller-owned stable entity keys.
/// before/after observe immutable snapshots around a real mutation. Matching uses
/// entity identity, so brackets for different entities may interleave; each closed
/// bracket remains a distinct edit, even if its range matches an earlier edit.
///
/// Clone and non_delegating share the live recording, including pending brackets.
/// A non-delegating handle records normally but returns no forward notification.
/// Returned notifications let callers notify their backing implementation once.
/// This recorder observes changes; it does not grant permission to bypass a native
/// ListProblem's value ranges, required assignments or pinned prefixes.
///
/// ```
/// use timetuck::*;
/// use std::collections::BTreeMap;
/// struct Solution;
/// struct Owner;
/// let schema=PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Owner>(vec![
///     PlanningVariableDefinition::list::<i32>("visits",false),
/// ])])?;
/// let field=schema.entity::<Owner>().unwrap().variable("visits").unwrap();
/// let recorder=ListChangeRecorder::<usize,i32>::new(field)?;
/// let undo=recorder.undo_handle(); // Sees edits recorded later, too.
/// let mut lists=BTreeMap::from([(0,vec![1,2,3])]);
/// recorder.before(0,1,2,&lists[&0])?;
/// lists.get_mut(&0).unwrap()[1]=4;
/// recorder.after(0,1,2,&lists[&0])?;
/// let mut events=vec![];
/// undo.execute(&mut lists, |event,_|events.push(event))?;
/// assert_eq!(lists[&0],vec![1,2,3]);
/// assert_eq!(events.len(),3); // One before/after pair, then one shadow refresh.
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct ListChangeRecorder<'s, E, V> {
    variable: PlanningVariableRef<'s>,
    state: Rc<RefCell<Recording<E, V>>>,
    delegate: bool,
}
impl<E, V> Clone for ListChangeRecorder<'_, E, V> {
    fn clone(&self) -> Self {
        Self {
            variable: self.variable,
            state: self.state.clone(),
            delegate: self.delegate,
        }
    }
}
/// A live undo handle, not a frozen snapshot. It sees edits opened and completed
/// after its creation. Successful execution consumes the shared recording; all
/// recorder handles can then record another batch. Old undo handles stay live.
pub struct ListRecordingUndo<'s, E, V> {
    variable: PlanningVariableRef<'s>,
    state: Rc<RefCell<Recording<E, V>>>,
}
impl<E, V> Clone for ListRecordingUndo<'_, E, V> {
    fn clone(&self) -> Self {
        Self {
            variable: self.variable,
            state: self.state.clone(),
        }
    }
}
fn range(from: usize, to: usize, length: usize) -> Result<(), ListRecordingError> {
    if from > to || to > length {
        Err(ListRecordingError::InvalidRange { from, to, length })
    } else {
        Ok(())
    }
}
impl<'s, E: Ord + Clone, V: Eq + Clone + 'static> ListChangeRecorder<'s, E, V> {
    pub fn new(variable: PlanningVariableRef<'s>) -> Result<Self, ListRecordingError> {
        if !matches!(variable.kind(), PlanningVariableKind::List { .. })
            || variable.value_type() != PlanningType::of::<V>()
        {
            return Err(ListRecordingError::Metadata);
        }
        Ok(Self {
            variable,
            state: Rc::new(RefCell::new(Recording::default())),
            delegate: true,
        })
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn non_delegating(&self) -> Self {
        Self {
            delegate: false,
            ..self.clone()
        }
    }
    pub fn undo_handle(&self) -> ListRecordingUndo<'s, E, V> {
        ListRecordingUndo {
            variable: self.variable,
            state: self.state.clone(),
        }
    }
    pub fn before(
        &self,
        entity: E,
        from: usize,
        to: usize,
        list: &[V],
    ) -> Result<Option<ListChangeNotification<'s, E>>, ListRecordingError> {
        let mut current = self
            .state
            .try_borrow_mut()
            .map_err(|_| ListRecordingError::Busy)?;
        if current.pending.contains_key(&entity) {
            return Err(ListRecordingError::AlreadyOpen);
        }
        range(from, to, list.len())?;
        let event = self.delegate.then(|| ListChangeNotification::Before {
            variable: self.variable,
            entity: entity.clone(),
            from,
            to,
        });
        let mut next = current.clone();
        next.pending.insert(entity.clone(), next.edits.len())?;
        next.edits.push(Rc::new(Edit {
            entity,
            from,
            before_to: to,
            before: Rc::new(list.to_vec()),
            after: None,
        }));
        *current = next;
        Ok(event)
    }
    pub fn after(
        &self,
        entity: E,
        from: usize,
        to: usize,
        list: &[V],
    ) -> Result<Option<ListChangeNotification<'s, E>>, ListRecordingError> {
        let mut current = self
            .state
            .try_borrow_mut()
            .map_err(|_| ListRecordingError::Busy)?;
        let &index = current
            .pending
            .get(&entity)
            .ok_or(ListRecordingError::NoOpenBracket)?;
        let edit = &current.edits[index];
        if from != edit.from {
            return Err(ListRecordingError::WrongStart {
                expected: edit.from,
                actual: from,
            });
        }
        range(from, to, list.len())?;
        if edit.before.len() - edit.before_to != list.len() - to {
            return Err(ListRecordingError::WrongLengthChange);
        }
        if edit.before[..from] != list[..from] || edit.before[edit.before_to..] != list[to..] {
            return Err(ListRecordingError::ChangedOutsideRange);
        }
        let event = self.delegate.then(|| ListChangeNotification::After {
            variable: self.variable,
            entity: entity.clone(),
            from,
            to,
        });
        let mut next = current.clone();
        next.edits[index] = Rc::new(Edit {
            entity: entity.clone(),
            from,
            before_to: edit.before_to,
            before: edit.before.clone(),
            after: Some((to, Rc::new(list.to_vec()))),
        });
        next.pending.remove(&entity);
        *current = next;
        Ok(event)
    }
    pub fn undo_changes(
        &self,
        lists: &mut BTreeMap<E, Vec<V>>,
        notify: impl FnMut(ListChangeNotification<'s, E>, &BTreeMap<E, Vec<V>>),
    ) -> Result<(), ListRecordingError> {
        self.undo_handle().execute(lists, notify)
    }
}
impl<'s, E: Ord + Clone, V: Eq + Clone> ListRecordingUndo<'s, E, V> {
    /// Validate the complete reverse replay before notifying. Each callback sees
    /// the working snapshot immediately before/after its edit; the final callback
    /// sees the restored lists and can refresh shadows once. Publish caller lists
    /// and consume records only after every callback succeeds. Errors and ordinary
    /// callback panics leave both unchanged. External callback side effects are not
    /// rolled back. Mutating the shared recorder during replay returns Busy.
    /// Snapshots use V's immutable Clone/Eq semantics; panicking destructors have
    /// the usual Rust transaction limitations. No callbacks run for invalid undo.
    pub fn execute(
        &self,
        lists: &mut BTreeMap<E, Vec<V>>,
        mut notify: impl FnMut(ListChangeNotification<'s, E>, &BTreeMap<E, Vec<V>>),
    ) -> Result<(), ListRecordingError> {
        let mut state = self
            .state
            .try_borrow_mut()
            .map_err(|_| ListRecordingError::Busy)?;
        if !state.pending.is_empty() {
            return Err(ListRecordingError::UnclosedBracket);
        }
        let mut prepared = lists.clone();
        let mut events = vec![];
        for edit in state.edits.iter().rev() {
            let (to, after) = edit
                .after
                .as_ref()
                .ok_or(ListRecordingError::UnclosedBracket)?;
            let actual = prepared
                .get_mut(&edit.entity)
                .ok_or(ListRecordingError::UnknownEntity)?;
            if actual != after.as_ref() {
                return Err(ListRecordingError::StateMismatch);
            }
            *actual = edit.before.as_ref().clone();
            events.push((
                ListChangeNotification::Before {
                    variable: self.variable,
                    entity: edit.entity.clone(),
                    from: edit.from,
                    to: *to,
                },
                ListChangeNotification::After {
                    variable: self.variable,
                    entity: edit.entity.clone(),
                    from: edit.from,
                    to: edit.before_to,
                },
            ));
        }
        let mut working = lists.clone();
        for (edit, (before, after)) in state.edits.iter().rev().zip(events) {
            notify(before, &working);
            working.insert(edit.entity.clone(), edit.before.as_ref().clone());
            notify(after, &working);
        }
        notify(ListChangeNotification::UpdateShadows, &working);
        *lists = prepared;
        *state = Recording::default();
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListRecordingRebaseError<X> {
    Recording(ListRecordingError),
    Lookup(X),
    EntityCollision,
    ValueCollision,
}
impl<X: fmt::Debug> fmt::Display for ListRecordingRebaseError<X> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list recording rebase: {self:?}")
    }
}
impl<X: fmt::Debug> Error for ListRecordingRebaseError<X> {}
impl<'s, E: Ord + Clone, V: Eq + Clone + 'static> ListChangeRecorder<'s, E, V> {
    /// Rebase a snapshot of all recorded edits into an independent recording.
    /// Target metadata must describe a list with the same unassignment policy.
    /// Each distinct source entity/value is looked up once; repeated occurrences
    /// reuse that result. Distinct source identities may not collapse to one target.
    /// Open brackets stay open; completed ranges and notification delegation survive.
    ///
    /// Lookup errors/panics leave the source recording unchanged. While lookups run,
    /// reentrant recording mutations return Busy. The new undo handle affects only
    /// target lists, and later source/target edits are independent. External lookup
    /// side effects cannot be rolled back. Clone/Eq must preserve stable identity.
    pub fn rebase<'t, E2: Ord + Clone, V2: Eq + Clone + 'static, X>(
        &self,
        variable: PlanningVariableRef<'t>,
        mut entity_lookup: impl FnMut(&E) -> Result<E2, X>,
        mut value_lookup: impl FnMut(&V) -> Result<V2, X>,
    ) -> Result<ListChangeRecorder<'t, E2, V2>, ListRecordingRebaseError<X>> {
        if variable.kind() != self.variable.kind() {
            return Err(ListRecordingRebaseError::Recording(
                ListRecordingError::Metadata,
            ));
        }
        let mut rebased =
            ListChangeRecorder::new(variable).map_err(ListRecordingRebaseError::Recording)?;
        let source = self
            .state
            .try_borrow()
            .map_err(|_| ListRecordingRebaseError::Recording(ListRecordingError::Busy))?;
        let mut entities = BTreeMap::<E, E2>::new();
        let mut target_entities = std::collections::BTreeSet::new();
        let mut values = Vec::<(V, V2)>::new();
        let mut target = Recording::default();
        for edit in &source.edits {
            let entity = if let Some(entity) = entities.get(&edit.entity) {
                entity.clone()
            } else {
                let entity =
                    entity_lookup(&edit.entity).map_err(ListRecordingRebaseError::Lookup)?;
                if !target_entities.insert(entity.clone()) {
                    return Err(ListRecordingRebaseError::EntityCollision);
                }
                entities.insert(edit.entity.clone(), entity.clone());
                entity
            };
            let mut map_list = |list: &[V]| -> Result<Rc<Vec<V2>>, ListRecordingRebaseError<X>> {
                let mut mapped = Vec::with_capacity(list.len());
                for value in list {
                    let target_value = if let Some((_, target)) =
                        values.iter().find(|(source, _)| source == value)
                    {
                        target.clone()
                    } else {
                        let target =
                            value_lookup(value).map_err(ListRecordingRebaseError::Lookup)?;
                        if values.iter().any(|(_, existing)| existing == &target) {
                            return Err(ListRecordingRebaseError::ValueCollision);
                        }
                        values.push((value.clone(), target.clone()));
                        target
                    };
                    mapped.push(target_value);
                }
                Ok(Rc::new(mapped))
            };
            let before = map_list(&edit.before)?;
            let after = edit
                .after
                .as_ref()
                .map(|(to, list)| Ok((*to, map_list(list)?)))
                .transpose()?;
            if after.is_none() {
                target
                    .pending
                    .insert(entity.clone(), target.edits.len())
                    .map_err(ListRecordingRebaseError::Recording)?;
            }
            target.edits.push(Rc::new(Edit {
                entity,
                from: edit.from,
                before_to: edit.before_to,
                before,
                after,
            }));
        }
        rebased.state = Rc::new(RefCell::new(target));
        rebased.delegate = self.delegate;
        Ok(rebased)
    }
}
