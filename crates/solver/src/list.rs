//! Ordered, disjoint list variables with atomic reversible edits.
//!
//! Values use stable `Eq` identities. Owner, index and neighbor views are derived
//! from the lists, so they cannot become stale after a move or rollback.

mod assigned_membership;
pub(crate) mod move_check;
mod span_movement;
pub(crate) mod undo;
mod value_index;
#[cfg(test)]
mod value_metadata_tests;
pub(crate) use value_index::ValueLocations;

use std::{error::Error, fmt, ops::Deref, sync::OnceLock};

/// One entity's initial ordered values and immutable pinning metadata.
#[derive(Clone, Debug)]
pub struct ListEntity<V> {
    initial: Vec<V>,
    pinned_prefix: usize,
    pinned: bool,
    value_range: Option<Vec<V>>,
    range_diagnostic: Option<crate::range_report::RangeDiagnostic<V>>,
}
impl<V> ListEntity<V> {
    /// Attach labels and a formatter used only when an assigned value violates
    /// its range. Reports own rendered strings and survive candidate rollback.
    /// The renderer must be deterministic and must not mutate planning state.
    /// Cloned/resumed models retain this metadata. Ordinary errors are unchanged
    /// when no diagnostic metadata is attached; no Debug/Display bound is added.
    pub fn with_range_diagnostics(
        mut self,
        entity: impl Into<String>,
        variable: impl Into<String>,
        render: impl Fn(&V) -> String + Send + Sync + 'static,
    ) -> Self {
        self.range_diagnostic = Some(crate::range_report::RangeDiagnostic::new(
            entity, variable, render,
        ));
        self
    }

    pub fn new(initial: Vec<V>) -> Self {
        Self {
            initial,
            pinned_prefix: 0,
            pinned: false,
            value_range: None,
            range_diagnostic: None,
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

/// A half-open span in the assignment before a move is applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ListRange {
    pub entity: usize,
    pub start: usize,
    pub end: usize,
}

/// Read-only built-in shadow information for an assigned element.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListElement<'a, V> {
    pub position: ListPosition,
    pub previous: Option<&'a V>,
    pub next: Option<&'a V>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ListAssignment<V> {
    pub(crate) lists: Vec<Vec<V>>,
    pub(crate) index_owner: crate::retained_list_index::IndexOwner,
}
impl<V: fmt::Debug> fmt::Debug for ListAssignment<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListAssignment")
            .field("lists", &self.lists)
            .finish()
    }
}
impl<V> ListAssignment<V> {
    /// Construction is unchecked; `ListProblem::validate` enforces the model.
    pub fn new(lists: Vec<Vec<V>>) -> Self {
        Self {
            lists,
            index_owner: Default::default(),
        }
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
    /// Optional rendered context; the original classification remains in cause.
    RangeViolation {
        cause: Box<Self>,
        report: Box<crate::ValueRangeReport>,
    },
    /// Invalid dependency declaration (indices refer to the value registry).
    Dependency(crate::DependencyError<usize>),
    DependencyCount {
        expected: usize,
        actual: usize,
    },
    /// Assigned values in or downstream from a dependency cycle.
    DependencyCycle {
        values: Vec<usize>,
    },
    /// Startup repair would have to remove this pinned value.
    PinnedDependencyCycle {
        value: usize,
        position: ListPosition,
    },
    OptionalValuesRequired,
    InvalidSampleSize {
        minimum: usize,
        maximum: usize,
    },
    UnassignmentNotAllowed,
    DuplicateDomainValue,
    DuplicateAssignedValue,
    MissingEntityValueRange {
        entity: usize,
    },
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
    OverlappingRanges {
        left: ListRange,
        right: ListRange,
    },
}
impl ListError {
    pub fn range_report(&self) -> Option<&crate::ValueRangeReport> {
        match self {
            Self::RangeViolation { report, .. } => Some(report),
            _ => None,
        }
    }
    /// Recover the original classification without discarding the owned report.
    pub fn without_range_report(&self) -> &Self {
        let mut cause = self;
        while let Self::RangeViolation { cause: inner, .. } = cause {
            cause = inner;
        }
        cause
    }
}
impl fmt::Display for ListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RangeViolation { report, .. } => report.fmt(f),
            _ => write!(f, "invalid list planning state or move: {self:?}"),
        }
    }
}
impl Error for ListError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RangeViolation { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }
}

/// A global value registry, with optional per-entity range restrictions. Assigned
/// values appear exactly once.
/// Missing required values are permitted in partial states; completeness is
/// checked separately. Optionality applies to the entire list variable.
#[derive(Clone, Debug)]
pub struct ListProblem<V> {
    values: Vec<V>,
    value_index: Option<std::sync::Arc<value_index::ValueIndex<V>>>,
    entities: Vec<ListEntity<V>>,
    allows_unassigned: bool,
    pub(crate) dependencies: Option<crate::list_dependencies::ListDependencies>,
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
            value_index: None,
            entities,
            allows_unassigned: false,
            dependencies: None,
        };
        problem.validate(&problem.initial_assignment())?;
        Ok(problem)
    }
    /// Build the registry from explicit per-entity ranges. Values shared by several
    /// ranges occur once, in first-encounter order across entities and their ranges.
    /// Each entity must supply a range (an empty range is valid). All ordinary
    /// duplicate, assignment, range and pin validation still applies. Comparisons
    /// use Eq; equal values must denote the same planning identity.
    ///
    /// ```
    /// use timetuck::{ListEntity, ListProblem};
    /// let p = ListProblem::from_entity_ranges(vec![
    ///     ListEntity::new(vec![]).with_value_range(vec![2, 1]),
    ///     ListEntity::new(vec![]).with_value_range(vec![1, 3]),
    /// ])?;
    /// assert_eq!(p.values(), &[2, 1, 3]);
    /// assert_eq!(p.entities()[1].value_range(), Some([1, 3].as_slice()));
    /// # Ok::<(), timetuck::ListError>(())
    /// ```
    pub fn from_entity_ranges(entities: Vec<ListEntity<V>>) -> Result<Self, ListError> {
        let mut values = Vec::new();
        for (entity, meta) in entities.iter().enumerate() {
            let range = meta
                .value_range
                .as_ref()
                .ok_or(ListError::MissingEntityValueRange { entity })?;
            for value in range {
                if !values.contains(value) {
                    values.push(value.clone());
                }
            }
        }
        Self::new(values, entities)
    }

    pub fn allowing_unassigned(mut self) -> Self {
        self.allows_unassigned = true;
        self
    }
    pub fn values(&self) -> &[V] {
        &self.values
    }
    /// Find a value's stable registry index using the model's identity semantics.
    /// Indexed models use hashing with equality checks for collisions; ordinary
    /// models retain a linear equality search.
    pub fn value_index(&self, value: &V) -> Option<usize> {
        match &self.value_index {
            Some(index) => index.find(&self.values, value),
            None => self.values.iter().position(|v| v == value),
        }
    }
    pub(crate) fn contains_value(&self, value: &V) -> bool {
        match &self.value_index {
            Some(index) => index.find(&self.values, value).is_some(),
            None => self.values.contains(value),
        }
    }
    pub fn entities(&self) -> &[ListEntity<V>] {
        &self.entities
    }
    pub fn allows_unassigned(&self) -> bool {
        self.allows_unassigned
    }
    /// Return a model starting from these validated lists, preserving registry,
    /// entity ranges, optionality and all pins. Partial assignments are allowed.
    /// Original pin baselines are checked before installing any new initial rows.
    pub fn with_initial_assignment(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Result<Self, ListError> {
        self.validate(assignment)?;
        let mut model = self.clone();
        for (entity, list) in model.entities.iter_mut().zip(assignment.lists()) {
            entity.initial = list.clone();
        }
        Ok(model)
    }

    pub fn initial_assignment(&self) -> ListAssignment<V> {
        ListAssignment::new(self.entities.iter().map(|e| e.initial.clone()).collect())
    }
    pub fn validate(&self, assignment: &ListAssignment<V>) -> Result<(), ListError> {
        self.validate_using(assignment, false).map(drop)
    }
    pub(crate) fn validate_using(
        &self,
        assignment: &ListAssignment<V>,
        positions: bool,
    ) -> Result<Option<Vec<Option<ListPosition>>>, ListError> {
        let positions = self.validate_layout_using(assignment, positions)?;
        let values = self.inconsistent_values_unchecked(assignment)?;
        if values.is_empty() {
            Ok(positions)
        } else {
            Err(ListError::DependencyCycle { values })
        }
    }
    pub(crate) fn validate_layout(&self, assignment: &ListAssignment<V>) -> Result<(), ListError> {
        self.validate_layout_using(assignment, false).map(drop)
    }
    fn validate_layout_using(
        &self,
        assignment: &ListAssignment<V>,
        positions: bool,
    ) -> Result<Option<Vec<Option<ListPosition>>>, ListError> {
        if assignment.lists.len() != self.entities.len() {
            return Err(ListError::ListCount {
                expected: self.entities.len(),
                actual: assignment.lists.len(),
            });
        }
        let mut seen = Vec::new();
        let mut seen_ids = self
            .value_index
            .as_ref()
            .map(|_| value_index::AssignedIds::new(self.values.len(), assignment, positions));
        for (entity, (list, meta)) in assignment.lists.iter().zip(&self.entities).enumerate() {
            if (meta.pinned && *list != meta.initial)
                || (!meta.pinned
                    && list.get(..meta.pinned_prefix) != Some(&meta.initial[..meta.pinned_prefix]))
            {
                return Err(ListError::PinChanged { entity });
            }
            for (index, value) in list.iter().enumerate() {
                let position = ListPosition { entity, index };
                let id = match &self.value_index {
                    Some(index) => index.find(&self.values, value),
                    None => self.values.contains(value).then_some(0),
                }
                .ok_or_else(|| self.range_error(position, value, ListError::ValueOutsideDomain))?;
                let accepted = match &self.value_index {
                    Some(index) => index.accepts(entity, id),
                    None => self.accepts(entity, value),
                };
                if !accepted {
                    return Err(self.range_error(
                        position,
                        value,
                        ListError::ValueOutsideEntityRange { entity },
                    ));
                }
                if let Some(seen_ids) = &mut seen_ids {
                    if !seen_ids.insert(id, position) {
                        return Err(ListError::DuplicateAssignedValue);
                    }
                } else {
                    if seen.contains(&value) {
                        return Err(ListError::DuplicateAssignedValue);
                    }
                    seen.push(value);
                }
            }
        }
        Ok(seen_ids.and_then(value_index::AssignedIds::into_positions))
    }
    fn range_error(&self, position: ListPosition, value: &V, cause: ListError) -> ListError {
        let meta = &self.entities[position.entity];
        let Some(d) = &meta.range_diagnostic else {
            return cause;
        };
        let allowed = if matches!(cause, ListError::ValueOutsideEntityRange { .. }) {
            meta.value_range.as_deref().unwrap_or(&self.values)
        } else {
            &self.values
        };
        ListError::RangeViolation {
            cause: Box::new(cause),
            report: d.report(
                crate::RangeViolationLocation::List { position },
                value,
                allowed,
            ),
        }
    }
    // Callers already validate entity indices and global value membership.
    pub(crate) fn accepts(&self, entity: usize, value: &V) -> bool {
        let Some(range) = &self.entities[entity].value_range else {
            return true;
        };
        match &self.value_index {
            Some(index) => self
                .value_index(value)
                .is_some_and(|id| index.accepts(entity, id)),
            None => range.contains(value),
        }
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
    /// Snapshot only the lists referenced by the move, once per entity even for
    /// nested composites. Large primitive swaps and changes retain an inverse edit.
    /// Composite operations use current indices and validate
    /// pins/ranges once at the final state. Errors and ordinary panic unwinding
    /// restore the changed lists without cloning values during rollback.
    /// Validate external input in full. Primitive edits preserve registry membership
    /// and uniqueness, so final layout checks inspect only referenced entities.
    /// Opt-in dependency-cycle checks still examine the global dependency graph.
    /// Span edits move existing payloads; they do not promise per-leaf Clone or
    /// Drop calls. Snapshot fallback clones each referenced row once; compact
    /// edits clone rows only when their pre-move contents are requested.
    pub fn apply<'a>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Result<AppliedListMove<'a, V>, ListError> {
        self.apply_recording(assignment, movement, |_| {})
    }
    fn apply_recording<'a>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
        record: impl FnMut(&V),
    ) -> Result<AppliedListMove<'a, V>, ListError> {
        self.validate(assignment)?;
        self.apply_validated_recording(assignment, movement, record)
    }
    // Only for owners that validated the assignment on entry and retain exclusive
    // control of every subsequent edit (including rollback and winner installs).
    pub(crate) fn apply_validated<'a>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Result<AppliedListMove<'a, V>, ListError> {
        self.apply_validated_recording(assignment, movement, |_| {})
    }
    fn apply_validated_recording<'a>(
        &self,
        assignment: &'a mut ListAssignment<V>,
        movement: &ListMove<V>,
        mut record: impl FnMut(&V),
    ) -> Result<AppliedListMove<'a, V>, ListError> {
        // Save before any mutation. Invalid entities are diagnosed by the normal
        // operation checks, retaining their ordering relative to earlier errors.
        let (entities, assignments) = movement.application_entities();
        let mut membership = assigned_membership::AssignedMembership::new(self, assignments);
        let saved = undo::ListUndo::new(assignment, movement, entities);
        let mut transaction = AppliedListMove {
            assignment,
            saved: Some(saved),
            original: OnceLock::new(),
        };
        let mut pending = vec![movement];
        while let Some(step) = pending.pop() {
            match step {
                ListMove::Assign { value, destination } => {
                    transaction.insertion(*destination, false)?;
                    if membership.contains(self, &transaction, value) {
                        return Err(ListError::AlreadyAssigned);
                    }
                    if !self.contains_value(value) {
                        return Err(self.range_error(
                            *destination,
                            value,
                            ListError::ValueOutsideDomain,
                        ));
                    }
                    record(value);
                    transaction.assignment.lists[destination.entity]
                        .insert(destination.index, value.clone());
                    membership.insert(self, value);
                }
                ListMove::Change {
                    source,
                    destination,
                } => {
                    transaction.element_index(*source)?;
                    transaction.insertion(*destination, source.entity == destination.entity)?;
                    transaction.saved.as_mut().unwrap().change_recording(
                        transaction.assignment,
                        *source,
                        *destination,
                        &mut record,
                    );
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
                    if *entity == destination.entity {
                        for value in &transaction.assignment.lists[*entity][*start..*end] {
                            record(value);
                        }
                        span_movement::relocate(
                            &mut transaction.assignment.lists[*entity],
                            *start,
                            *end,
                            destination.index,
                            *reverse,
                        );
                    } else {
                        let mut span: Vec<_> = transaction.assignment.lists[*entity]
                            .drain(*start..*end)
                            .collect();
                        for value in &span {
                            record(value);
                        }
                        if *reverse {
                            span.reverse();
                        }
                        transaction.assignment.lists[destination.entity]
                            .splice(destination.index..destination.index, span);
                    }
                }
                ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } => {
                    for range in [left, right] {
                        if range.start >= range.end
                            || range.end > transaction.entity(range.entity)?.len()
                        {
                            return Err(ListError::InvalidRange {
                                entity: range.entity,
                                start: range.start,
                                end: range.end,
                            });
                        }
                    }
                    if left.entity == right.entity
                        && left.start < right.end
                        && right.start < left.end
                    {
                        return Err(ListError::OverlappingRanges {
                            left: *left,
                            right: *right,
                        });
                    }
                    for value in transaction.entity(left.entity)?[left.start..left.end]
                        .iter()
                        .chain(&transaction.entity(right.entity)?[right.start..right.end])
                    {
                        record(value);
                    }
                    span_movement::exchange(
                        &mut transaction.assignment.lists,
                        *left,
                        *right,
                        *reverse,
                    );
                }
                ListMove::Swap { left, right } => {
                    transaction.element_index(*left)?;
                    transaction.element_index(*right)?;
                    record(&transaction.assignment.lists[left.entity][left.index]);
                    record(&transaction.assignment.lists[right.entity][right.index]);
                    undo::swap(transaction.assignment, *left, *right);
                    transaction.saved.as_mut().unwrap().mark_swapped();
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
                    for value in &list[*start..*end] {
                        record(value);
                        membership.remove(self, value);
                    }
                    list.drain(*start..*end);
                }
                ListMove::Composite(steps) => pending.extend(steps.iter().rev()),
            }
        }
        // Primitive edits preserve global value membership and uniqueness:
        // Assign checks both explicitly; all other variants only move/remove
        // existing values, and span swaps reject overlap before mutation.
        // Only referenced lists can change their pins or receiver membership.
        for entity in transaction.saved.as_ref().unwrap().entities() {
            let list = &transaction.assignment.lists[entity];
            let meta = &self.entities[entity];
            if (meta.pinned && *list != meta.initial)
                || (!meta.pinned
                    && list.get(..meta.pinned_prefix) != Some(&meta.initial[..meta.pinned_prefix]))
            {
                return Err(ListError::PinChanged { entity });
            }
            // The source was validated before entering this transaction. Staying
            // values still belong to their row's range; only primitive arrivals
            // can violate it. Composites keep a full final-row check because later
            // leaves can move or remove earlier arrivals. Pins remain first, in
            // sorted entity order, and diagnostics use final positions/payloads.
            if meta.value_range.is_none() {
                continue;
            }
            let arrivals = movement.arrivals(entity, list.len());
            for index in arrivals {
                let value = &list[index];
                if !self.accepts(entity, value) {
                    return Err(self.range_error(
                        ListPosition { entity, index },
                        value,
                        ListError::ValueOutsideEntityRange { entity },
                    ));
                }
            }
        }
        // Dynamic predecessor cycles may involve untouched lists as well.
        let values = self.inconsistent_values_unchecked(&transaction)?;
        if !values.is_empty() {
            return Err(ListError::DependencyCycle { values });
        }
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
    /// Exchange two nonempty disjoint spans using pre-move indices. Spans may
    /// have different lengths; reverse=true reverses both exchanged spans.
    /// Empty spans and overlapping same-list spans are rejected atomically.
    SubListSwap {
        left: ListRange,
        right: ListRange,
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
    // Called only after successful primitive application, so all spans refer to
    // final row coordinates. A same-row permutation has no arrivals.
    fn arrivals(&self, entity: usize, final_length: usize) -> std::ops::Range<usize> {
        match self {
            Self::Assign { destination, .. } if destination.entity == entity => {
                destination.index..destination.index + 1
            }
            Self::Change {
                source,
                destination,
            } if source.entity != destination.entity && destination.entity == entity => {
                destination.index..destination.index + 1
            }
            Self::Swap { left, right } if left.entity != right.entity => {
                let index = if left.entity == entity {
                    left.index
                } else {
                    right.index
                };
                index..index + 1
            }
            Self::SubListChange {
                entity: source,
                start,
                end,
                destination,
                ..
            } if *source != destination.entity && destination.entity == entity => {
                destination.index..destination.index + (end - start)
            }
            Self::SubListSwap { left, right, .. } if left.entity != right.entity => {
                if left.entity == entity {
                    left.start..left.start + (right.end - right.start)
                } else {
                    right.start..right.start + (left.end - left.start)
                }
            }
            Self::Composite(_) => 0..final_length,
            _ => 0..0,
        }
    }
    pub fn touched_entities(&self) -> Vec<usize> {
        self.application_entities().0
    }
    fn application_entities(&self) -> (Vec<usize>, usize) {
        let mut assignments = 0;
        let mut pending = vec![self];
        let mut entities = Vec::new();
        while let Some(step) = pending.pop() {
            match step {
                Self::Assign { destination, .. } => {
                    entities.push(destination.entity);
                    assignments += 1;
                }
                Self::Change {
                    source,
                    destination,
                } => entities.extend([source.entity, destination.entity]),
                Self::SubListSwap { left, right, .. } => {
                    entities.extend([left.entity, right.entity])
                }
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
        (entities, assignments)
    }
}

#[must_use = "dropping rolls back the lists; call commit to keep the move"]
pub struct AppliedListMove<'a, V> {
    assignment: &'a mut ListAssignment<V>,
    saved: Option<undo::ListUndo<V>>,
    original: OnceLock<ListAssignment<V>>,
}
impl<V> Deref for AppliedListMove<'_, V> {
    type Target = ListAssignment<V>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}
impl<V> AppliedListMove<'_, V> {
    pub(crate) fn record_retained(
        &self,
        changes: &mut crate::retained_scoped_phase::AssignmentChanges,
    ) {
        changes.lists.record(
            self.saved
                .as_ref()
                .expect("live list transaction")
                .entities(),
        );
    }

    pub(crate) fn snapshot(&self) -> crate::candidate_snapshot::ListSnapshot<V>
    where
        V: Clone,
    {
        crate::candidate_snapshot::ListSnapshot::capture(
            self.assignment,
            self.saved
                .as_ref()
                .expect("live list transaction")
                .entity_count(),
            self.saved
                .as_ref()
                .expect("live list transaction")
                .entities(),
        )
    }

    // Saved rows are unique; cardinality changes telescope over the whole
    // composite, including values assigned and removed again in one trial.
    pub(crate) fn assigned_count_delta(&self) -> Option<i64> {
        let saved = self.saved.as_ref().expect("live list transaction");
        let rows = self.assignment.lists.len();
        // A sequential length count beats scattered touched-row reads for tiny
        // assignments and dense edits, without allocating an auxiliary index.
        if rows <= 32 || saved.entity_count() > rows / 8 {
            return None;
        }
        Some(saved.assigned_delta(self.assignment))
    }

    /// The pre-move list, borrowing the saved list for a touched entity and the
    /// unchanged assignment otherwise. A compact edit materializes its original
    /// touched rows on the first request and reuses them on subsequent requests.
    /// Clone may panic during that first request; rollback stays armed.
    pub fn original_list(&self, entity: usize) -> Option<&[V]> {
        self.saved
            .as_ref()
            .expect("live list transaction")
            .original_list(self.assignment, entity)
    }
    /// Materialize the full pre-move assignment only on demand, then reuse it.
    /// Prefer original_list when inspecting particular entities to avoid cloning
    /// unrelated lists. Rollback does not require this full snapshot.
    pub fn original(&self) -> &ListAssignment<V>
    where
        V: Clone,
    {
        self.original.get_or_init(|| ListAssignment {
            index_owner: Default::default(),
            lists: (0..self.assignment.lists.len())
                .map(|entity| self.original_list(entity).unwrap().to_vec())
                .collect(),
        })
    }
    pub fn commit(mut self) {
        self.assignment.index_owner.changed(
            self.saved
                .as_ref()
                .expect("live list transaction")
                .entities(),
        );
        self.saved = None;
    }

    // A containing transaction takes over rollback after both components pass.
    pub(crate) fn into_undo_lists(mut self) -> undo::ListUndo<V> {
        self.assignment.index_owner.changed(
            self.saved
                .as_ref()
                .expect("live list transaction")
                .entities(),
        );
        self.saved.take().expect("live list transaction")
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
        if let Some(mut saved) = self.saved.take() {
            saved.rollback(self.assignment);
        }
    }
}

impl<V: Eq + Clone> ListProblem<V> {
    /// Capture moved list values, including assigned and removed values, once in
    /// first-touch order. Composite children use their current indices. Shifted
    /// neighbors are not captured merely because their indices change. Full model
    /// validation is identical to apply; the input remains unchanged on all paths.
    /// Validate before cloning captured outputs. Primitive metadata clones only
    /// distinct captured values; composites replay borrowed references in touched
    /// rows. Full source validation and dependency checks may still visit the model.
    /// This is pre-move metadata: capture it before executing the move and retain
    /// the owned result when inspecting an already changed assignment.
    pub fn planning_values(
        &self,
        before: &ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Result<Vec<V>, ListError> {
        self.move_checker(before)?.is_doable(movement)?;
        Ok(self.scored_planning_values(before, movement))
    }
}
