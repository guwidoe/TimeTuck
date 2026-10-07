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
    pending: BTreeMap<E, usize>,
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
            pending: BTreeMap::new(),
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
        next.pending.insert(entity.clone(), next.edits.len());
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
