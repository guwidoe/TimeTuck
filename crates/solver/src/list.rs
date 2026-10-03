//! Ordered, disjoint list variables with atomic reversible edits.
//!
//! Values use stable `Eq` identities. Owner, index and neighbor views are derived
//! from the lists, so they cannot become stale after a move or rollback.

use std::{error::Error, fmt, ops::Deref};

/// One entity's initial ordered values and immutable pinning metadata.
#[derive(Clone, Debug)]
pub struct ListEntity<V> {
    initial: Vec<V>,
    pinned_prefix: usize,
    pinned: bool,
    value_range: Option<Vec<V>>,
}
impl<V> ListEntity<V> {
    pub fn new(initial: Vec<V>) -> Self {
        Self {
            initial,
            pinned_prefix: 0,
            pinned: false,
            value_range: None,
        }
    }
    /// Preserve the first `length` values in order; appending remains possible.
    pub fn with_pinned_prefix(mut self, length: usize) -> Self {
        self.pinned_prefix = length;
        self
    }
    /// Freeze the entire list, including its length. The prefix must be zero.
    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
    /// Restrict this list to these values from the model registry. An empty range
    /// accepts no values; omission accepts every registered value. Range order does
    /// not change search order. The model constructor validates this metadata.
    pub fn with_value_range(mut self, values: Vec<V>) -> Self {
        self.value_range = Some(values);
        self
    }
    pub fn value_range(&self) -> Option<&[V]> {
        self.value_range.as_deref()
    }
    pub fn initial(&self) -> &[V] {
        &self.initial
    }
    pub fn pinned_prefix(&self) -> usize {
        self.pinned_prefix
    }
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ListPosition {
    pub entity: usize,
    pub index: usize,
}

/// Read-only built-in shadow information for an assigned element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListElement<'a, V> {
    pub position: ListPosition,
    pub previous: Option<&'a V>,
    pub next: Option<&'a V>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListAssignment<V> {
    pub(crate) lists: Vec<Vec<V>>,
}
impl<V> ListAssignment<V> {
    /// Construction is unchecked; `ListProblem::validate` enforces the model.
    pub fn new(lists: Vec<Vec<V>>) -> Self {
        Self { lists }
    }
    pub fn lists(&self) -> &[Vec<V>] {
        &self.lists
    }
    pub fn list(&self, entity: usize) -> Option<&[V]> {
        self.lists.get(entity).map(Vec::as_slice)
    }
}
impl<V: Eq> ListAssignment<V> {
    pub fn location(&self, value: &V) -> Option<ListPosition> {
        self.lists.iter().enumerate().find_map(|(entity, list)| {
            list.iter()
                .position(|v| v == value)
                .map(|index| ListPosition { entity, index })
        })
    }
    /// Unassigned values have no owner, index or neighbors. Linear lookup.
    pub fn element(&self, value: &V) -> Option<ListElement<'_, V>> {
        let position = self.location(value)?;
        let list = &self.lists[position.entity];
        Some(ListElement {
            position,
            previous: position.index.checked_sub(1).and_then(|i| list.get(i)),
            next: list.get(position.index + 1),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListError {
    DuplicateDomainValue,
    DuplicateAssignedValue,
    DuplicateEntityRangeValue {
        entity: usize,
    },
    EntityRangeValueOutsideDomain {
        entity: usize,
    },
    ValueOutsideEntityRange {
        entity: usize,
    },
    ValueOutsideDomain,
    ListCount {
        expected: usize,
        actual: usize,
    },
    EntityIndex {
        entity: usize,
        count: usize,
    },
    ElementIndex(ListPosition),
    InsertionIndex(ListPosition),
    InvalidRange {
        entity: usize,
        start: usize,
        end: usize,
    },
    InvalidPin {
        entity: usize,
    },
    PinChanged {
        entity: usize,
    },
    AlreadyAssigned,
}
impl fmt::Display for ListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid list planning state or move: {self:?}")
    }
}
impl Error for ListError {}

/// A global value registry, with optional per-entity range restrictions. Assigned
/// values appear exactly once.
/// Missing required values are permitted in partial states; completeness is
/// checked separately. Optionality applies to the entire list variable.
#[derive(Clone, Debug)]
pub struct ListProblem<V> {
    values: Vec<V>,
    entities: Vec<ListEntity<V>>,
    allows_unassigned: bool,
}
impl<V: Eq + Clone> ListProblem<V> {
    pub fn new(values: Vec<V>, entities: Vec<ListEntity<V>>) -> Result<Self, ListError> {
        for (i, value) in values.iter().enumerate() {
            if values[..i].contains(value) {
                return Err(ListError::DuplicateDomainValue);
            }
        }
        for (entity, meta) in entities.iter().enumerate() {
            if let Some(range) = &meta.value_range {
                for (i, value) in range.iter().enumerate() {
                    if !values.contains(value) {
                        return Err(ListError::EntityRangeValueOutsideDomain { entity });
                    }
                    if range[..i].contains(value) {
                        return Err(ListError::DuplicateEntityRangeValue { entity });
                    }
                }
            }
            if meta.pinned_prefix > meta.initial.len() || (meta.pinned && meta.pinned_prefix != 0) {
                return Err(ListError::InvalidPin { entity });
            }
        }
        let problem = Self {
            values,
            entities,
            allows_unassigned: false,
        };
        problem.validate(&problem.initial_assignment())?;
        Ok(problem)
    }
    pub fn allowing_unassigned(mut self) -> Self {
        self.allows_unassigned = true;
        self
    }
    pub fn values(&self) -> &[V] {
        &self.values
    }
    pub fn entities(&self) -> &[ListEntity<V>] {
        &self.entities
    }
    pub fn allows_unassigned(&self) -> bool {
        self.allows_unassigned
    }
    pub fn initial_assignment(&self) -> ListAssignment<V> {
        ListAssignment::new(self.entities.iter().map(|e| e.initial.clone()).collect())
    }
    pub fn validate(&self, assignment: &ListAssignment<V>) -> Result<(), ListError> {
        if assignment.lists.len() != self.entities.len() {
            return Err(ListError::ListCount {
                expected: self.entities.len(),
                actual: assignment.lists.len(),
            });
        }
        let mut seen = Vec::new();
        for (entity, (list, meta)) in assignment.lists.iter().zip(&self.entities).enumerate() {
            if (meta.pinned && *list != meta.initial)
                || (!meta.pinned
                    && list.get(..meta.pinned_prefix) != Some(&meta.initial[..meta.pinned_prefix]))
            {
                return Err(ListError::PinChanged { entity });
            }
            for value in list {
                if !self.values.contains(value) {
                    return Err(ListError::ValueOutsideDomain);
                }
                if !self.accepts(entity, value) {
                    return Err(ListError::ValueOutsideEntityRange { entity });
                }
                if seen.contains(&value) {
                    return Err(ListError::DuplicateAssignedValue);
                }
                seen.push(value);
            }
        }
        Ok(())
    }
    // Callers already validate entity indices and global value membership.
    pub(crate) fn accepts(&self, entity: usize, value: &V) -> bool {
        self.entities[entity]
            .value_range
            .as_ref()
            .is_none_or(|range| range.contains(value))
    }
    // Valid partial states have no list-length restrictions. Every missing value
    // can be appended independently if any unpinned entity accepts it.
    pub(crate) fn can_complete(&self, assignment: &ListAssignment<V>) -> bool {
        self.allows_unassigned
            || self.values.iter().all(|value| {
                assignment.location(value).is_some()
                    || self
                        .entities
                        .iter()
                        .enumerate()
                        .any(|(e, meta)| !meta.pinned && self.accepts(e, value))
            })
    }
    pub fn is_complete(&self, assignment: &ListAssignment<V>) -> bool {
        self.validate(assignment).is_ok()
            && (self.allows_unassigned
                || assignment.lists.iter().map(Vec::len).sum::<usize>() == self.values.len())
    }
    /// Full-state snapshots provide a correctness baseline, not incremental list
    /// scoring. Composite operations use current indices and validate pins/ranges once
    /// at the final state. Errors and ordinary panic unwinding restore all lists.
    pub fn apply<'a>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Result<AppliedListMove<'a, V>, ListError> {
        self.validate(assignment)?;
        let original = assignment.clone();
        let transaction = AppliedListMove {
            assignment,
            original: Some(original),
        };
        let mut pending = vec![movement];
        while let Some(step) = pending.pop() {
            match step {
                ListMove::Assign { value, destination } => {
                    transaction.insertion(*destination, false)?;
                    if transaction.location(value).is_some() {
                        return Err(ListError::AlreadyAssigned);
                    }
                    if !self.values.contains(value) {
                        return Err(ListError::ValueOutsideDomain);
                    }
                    transaction.assignment.lists[destination.entity]
                        .insert(destination.index, value.clone());
                }
                ListMove::Change {
                    source,
                    destination,
                } => {
                    transaction.element_index(*source)?;
                    transaction.insertion(*destination, source.entity == destination.entity)?;
                    let value = transaction.assignment.lists[source.entity].remove(source.index);
                    transaction.assignment.lists[destination.entity]
                        .insert(destination.index, value);
                }
                ListMove::SubListChange {
                    entity,
                    start,
                    end,
                    destination,
                    reverse,
                } => {
                    let length = transaction.entity(*entity)?.len();
                    if start > end || *end > length {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    let destination_length = transaction.entity(destination.entity)?.len()
                        - if *entity == destination.entity {
                            end - start
                        } else {
                            0
                        };
                    if destination.index > destination_length {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    let mut span: Vec<_> = transaction.assignment.lists[*entity]
                        .drain(*start..*end)
                        .collect();
                    if *reverse {
                        span.reverse();
                    }
                    transaction.assignment.lists[destination.entity]
                        .splice(destination.index..destination.index, span);
                }
                ListMove::Swap { left, right } => {
                    transaction.element_index(*left)?;
                    transaction.element_index(*right)?;
                    if left.entity == right.entity {
                        transaction.assignment.lists[left.entity].swap(left.index, right.index);
                    } else {
                        let (a, b) = if left.entity < right.entity {
                            let (low, high) =
                                transaction.assignment.lists.split_at_mut(right.entity);
                            (&mut low[left.entity][left.index], &mut high[0][right.index])
                        } else {
                            let (low, high) =
                                transaction.assignment.lists.split_at_mut(left.entity);
                            (
                                &mut high[0][left.index],
                                &mut low[right.entity][right.index],
                            )
                        };
                        std::mem::swap(a, b);
                    }
                }
                ListMove::Unassign { entity, start, end } => {
                    let list = transaction.assignment.lists.get_mut(*entity).ok_or(
                        ListError::EntityIndex {
                            entity: *entity,
                            count: self.entities.len(),
                        },
                    )?;
                    if start > end || *end > list.len() {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    list.drain(*start..*end);
                }
                ListMove::Composite(steps) => pending.extend(steps.iter().rev()),
            }
        }
        self.validate(&transaction)?;
        Ok(transaction)
    }
}

/// Within-list changes remove first, then insert at the destination index in
/// the shortened list. Unassignment uses a half-open `[start, end)` range.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ListMove<V> {
    Assign {
        value: V,
        destination: ListPosition,
    },
    Change {
        source: ListPosition,
        destination: ListPosition,
    },
    /// Move [start, end), optionally reversed. Destination is post-removal.
    /// Empty spans are valid native no-ops after validating both positions.
    SubListChange {
        entity: usize,
        start: usize,
        end: usize,
        destination: ListPosition,
        reverse: bool,
    },
    Swap {
        left: ListPosition,
        right: ListPosition,
    },
    Unassign {
        entity: usize,
        start: usize,
        end: usize,
    },
    Composite(Vec<Self>),
}
impl<V> ListMove<V> {
    pub fn touched_entities(&self) -> Vec<usize> {
        let mut pending = vec![self];
        let mut entities = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Self::Assign { destination, .. } => entities.push(destination.entity),
                Self::Change {
                    source,
                    destination,
                } => entities.extend([source.entity, destination.entity]),
                Self::Swap { left, right } => entities.extend([left.entity, right.entity]),
                Self::SubListChange {
                    entity,
                    destination,
                    ..
                } => entities.extend([*entity, destination.entity]),
                Self::Unassign { entity, .. } => entities.push(*entity),
                Self::Composite(steps) => pending.extend(steps),
            }
        }
        entities.sort_unstable();
        entities.dedup();
        entities
    }
}

#[must_use = "dropping rolls back the lists; call commit to keep the move"]
pub struct AppliedListMove<'a, V> {
    assignment: &'a mut ListAssignment<V>,
    original: Option<ListAssignment<V>>,
}
impl<V> Deref for AppliedListMove<'_, V> {
    type Target = ListAssignment<V>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}
impl<V> AppliedListMove<'_, V> {
    pub fn original(&self) -> &ListAssignment<V> {
        self.original.as_ref().expect("live list transaction")
    }
    pub fn commit(mut self) {
        self.original = None;
    }
    fn entity(&self, entity: usize) -> Result<&[V], ListError> {
        self.list(entity).ok_or(ListError::EntityIndex {
            entity,
            count: self.lists.len(),
        })
    }
    fn element_index(&self, position: ListPosition) -> Result<(), ListError> {
        if position.index >= self.entity(position.entity)?.len() {
            return Err(ListError::ElementIndex(position));
        }
        Ok(())
    }
    fn insertion(&self, position: ListPosition, removed_here: bool) -> Result<(), ListError> {
        let length = self.entity(position.entity)?.len() - usize::from(removed_here);
        if position.index > length {
            return Err(ListError::InsertionIndex(position));
        }
        Ok(())
    }
}
impl<V> Drop for AppliedListMove<'_, V> {
    fn drop(&mut self) {
        if let Some(mut original) = self.original.take() {
            std::mem::swap(&mut self.assignment.lists, &mut original.lists);
        }
    }
}
