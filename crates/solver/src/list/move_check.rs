//! Candidate checks on borrowed list rows, without applying to live planning state.
use super::{ListAssignment, ListError, ListMove, ListPosition, ListProblem};
use std::collections::BTreeMap;

/// Repeated candidate checks against one validated immutable list snapshot.
///
/// Construct through [`ListProblem::move_checker`]. Queries borrow planning values
/// and copy references only for touched rows when needed. Primitive changes, swaps,
/// assignments, removals and sublist moves on dependency-free models use direct checks without
/// scratch rows.
/// They do not clone planning values,
/// mutate the source, score, or notify listeners. Dependency-enabled models still
/// check the global predecessor graph after validating the candidate's final rows.
/// Equality must remain stable while the checker is borrowed.
pub struct ListMoveChecker<'a, V> {
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
}
impl<V: Eq + Clone> ListProblem<V> {
    /// Validate a source once and borrow it for repeated list candidate checks.
    pub fn move_checker<'a>(
        &'a self,
        assignment: &'a ListAssignment<V>,
    ) -> Result<ListMoveChecker<'a, V>, ListError> {
        self.validate(assignment)?;
        Ok(ListMoveChecker {
            problem: self,
            assignment,
        })
    }

    /// Validate borrowed final rows whose membership is a permutation of a
    /// previously validated assignment. Internal callers must establish entity
    /// indices, unique membership and registry identity before using this path.
    /// Omitted rows are unchanged; pins, ranges and dependencies are checked in
    /// the same order as ordinary final-state candidate validation.
    pub(crate) fn validate_permuted_rows<'a>(
        &'a self,
        before: &'a ListAssignment<V>,
        rows: BTreeMap<usize, Vec<&'a V>>,
    ) -> Result<(), ListError> {
        CandidateRows {
            problem: self,
            before,
            rows,
        }
        .validate_final()
    }

    /// Whether this move is valid and changes the final assignment under value Eq.
    /// Invalid moves return the same errors as native `apply`. Empty and restoring
    /// composites return false. Required values may be temporarily unassigned, as
    /// with ordinary partial planning states. Use `move_checker` for many candidates
    /// to avoid validating the entire source for each query.
    pub fn is_move_doable(
        &self,
        assignment: &ListAssignment<V>,
        movement: &ListMove<V>,
    ) -> Result<bool, ListError> {
        self.move_checker(assignment)?.is_doable(movement)
    }
}
impl<V: Eq + Clone> ListMoveChecker<'_, V> {
    /// Validate indices and assignment identity at each primitive operation, then
    /// final pins/ranges in entity order and dependencies. Intermediate pin/range
    /// violations may be restored by a composite, exactly as for native `apply`.
    pub fn is_doable(&self, movement: &ListMove<V>) -> Result<bool, ListError> {
        if self.problem.dependencies.is_none() {
            match movement {
                ListMove::Assign { value, destination } => {
                    let length = self
                        .assignment
                        .list(destination.entity)
                        .ok_or(ListError::EntityIndex {
                            entity: destination.entity,
                            count: self.assignment.lists().len(),
                        })?
                        .len();
                    if destination.index > length {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    if self
                        .assignment
                        .lists()
                        .iter()
                        .any(|row| row.contains(value))
                    {
                        return Err(ListError::AlreadyAssigned);
                    }
                    if !self.problem.values.contains(value) {
                        return Err(self.problem.range_error(
                            *destination,
                            value,
                            ListError::ValueOutsideDomain,
                        ));
                    }
                    self.validate_point(*destination, Some(value))?;
                    return Ok(true);
                }
                ListMove::Unassign { entity, start, end } => {
                    let length = self
                        .assignment
                        .list(*entity)
                        .ok_or(ListError::EntityIndex {
                            entity: *entity,
                            count: self.assignment.lists().len(),
                        })?
                        .len();
                    if start > end || *end > length {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    if start == end {
                        return Ok(false);
                    }
                    self.validate_point(
                        ListPosition {
                            entity: *entity,
                            index: *start,
                        },
                        None,
                    )?;
                    return Ok(true);
                }
                ListMove::Swap { left, right } => {
                    let a = self.element(*left)?;
                    let b = self.element(*right)?;
                    if left == right {
                        return Ok(false);
                    }
                    self.validate_pair(*left, Some(b), *right, Some(a))?;
                    return Ok(true);
                }
                ListMove::Change {
                    source,
                    destination,
                } => {
                    let value = self.element(*source)?;
                    let length = self
                        .assignment
                        .list(destination.entity)
                        .ok_or(ListError::EntityIndex {
                            entity: destination.entity,
                            count: self.assignment.lists().len(),
                        })?
                        .len()
                        - usize::from(source.entity == destination.entity);
                    if destination.index > length {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    if source == destination {
                        return Ok(false);
                    }
                    self.validate_pair(*source, None, *destination, Some(value))?;
                    return Ok(true);
                }
                ListMove::SubListChange {
                    entity,
                    start,
                    end,
                    destination,
                    reverse,
                } => {
                    let values = self.row(*entity)?;
                    if start > end || *end > values.len() {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    let count = end - start;
                    let remaining = self.row(destination.entity)?.len()
                        - if *entity == destination.entity {
                            count
                        } else {
                            0
                        };
                    if destination.index > remaining {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    if count == 0
                        || (*entity == destination.entity
                            && *start == destination.index
                            && (!reverse || count == 1))
                    {
                        return Ok(false);
                    }
                    let source = ListPosition {
                        entity: *entity,
                        index: *start,
                    };
                    if *entity == destination.entity {
                        self.validate_pair(source, None, *destination, None)?;
                    } else if *entity < destination.entity {
                        self.validate_point(source, None)?;
                        self.validate_arrivals(*destination, &values[*start..*end], *reverse)?;
                    } else {
                        self.validate_arrivals(*destination, &values[*start..*end], *reverse)?;
                        self.validate_point(source, None)?;
                    }
                    return Ok(true);
                }
                ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } => {
                    // Retain the primitive validator's short-circuit error order:
                    // an empty/descending range precedes its entity lookup.
                    for range in [left, right] {
                        if range.start >= range.end || range.end > self.row(range.entity)?.len() {
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
                    let a = ListPosition {
                        entity: left.entity,
                        index: left.start,
                    };
                    let b = ListPosition {
                        entity: right.entity,
                        index: right.start,
                    };
                    if left.entity == right.entity {
                        self.validate_pair(a, None, b, None)?;
                    } else {
                        let left_values =
                            &self.assignment.lists()[left.entity][left.start..left.end];
                        let right_values =
                            &self.assignment.lists()[right.entity][right.start..right.end];
                        if left.entity < right.entity {
                            self.validate_arrivals(a, right_values, *reverse)?;
                            self.validate_arrivals(b, left_values, *reverse)?;
                        } else {
                            self.validate_arrivals(b, left_values, *reverse)?;
                            self.validate_arrivals(a, right_values, *reverse)?;
                        }
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
        let mut draft = CandidateRows {
            problem: self.problem,
            before: self.assignment,
            rows: BTreeMap::new(),
        };
        draft.replay(movement)?;
        draft.validate_final()?;
        Ok(draft
            .rows
            .iter()
            .any(|(&e, row)| !row.iter().copied().eq(self.assignment.lists()[e].iter())))
    }

    fn row(&self, entity: usize) -> Result<&[V], ListError> {
        self.assignment.list(entity).ok_or(ListError::EntityIndex {
            entity,
            count: self.assignment.lists().len(),
        })
    }

    // All staying values already satisfy the entity range. Only arrivals can
    // fail; visit them in final row order to retain payload and index diagnostics.
    fn validate_arrivals(
        &self,
        destination: ListPosition,
        incoming: &[V],
        reverse: bool,
    ) -> Result<(), ListError> {
        self.validate_point(destination, None)?;
        if self.problem.entities[destination.entity]
            .value_range
            .is_none()
        {
            return Ok(());
        }
        for offset in 0..incoming.len() {
            let value = &incoming[if reverse {
                incoming.len() - 1 - offset
            } else {
                offset
            }];
            let position = ListPosition {
                entity: destination.entity,
                index: destination.index + offset,
            };
            if !self.problem.accepts(destination.entity, value) {
                return Err(self.problem.range_error(
                    position,
                    value,
                    ListError::ValueOutsideEntityRange {
                        entity: destination.entity,
                    },
                ));
            }
        }
        Ok(())
    }

    fn element(&self, position: ListPosition) -> Result<&V, ListError> {
        self.assignment
            .list(position.entity)
            .ok_or(ListError::EntityIndex {
                entity: position.entity,
                count: self.assignment.lists().len(),
            })?
            .get(position.index)
            .ok_or(ListError::ElementIndex(position))
    }

    // The validated source contains each identity at most once. A nontrivial
    // primitive changes the pins iff it edits a pinned position. Within one row,
    // all values already satisfy that row's range; across rows only arrivals need
    // range checks. Preserve final validation's entity order (pins before ranges).
    fn validate_pair(
        &self,
        left: ListPosition,
        left_value: Option<&V>,
        right: ListPosition,
        right_value: Option<&V>,
    ) -> Result<(), ListError> {
        if left.entity == right.entity {
            return self.validate_point(
                ListPosition {
                    entity: left.entity,
                    index: left.index.min(right.index),
                },
                None,
            );
        }
        if left.entity < right.entity {
            self.validate_point(left, left_value)?;
            self.validate_point(right, right_value)
        } else {
            self.validate_point(right, right_value)?;
            self.validate_point(left, left_value)
        }
    }

    fn validate_point(
        &self,
        position: ListPosition,
        incoming: Option<&V>,
    ) -> Result<(), ListError> {
        let meta = &self.problem.entities[position.entity];
        if meta.pinned || position.index < meta.pinned_prefix {
            return Err(ListError::PinChanged {
                entity: position.entity,
            });
        }
        if let Some(value) = incoming
            && !self.problem.accepts(position.entity, value)
        {
            return Err(self.problem.range_error(
                position,
                value,
                ListError::ValueOutsideEntityRange {
                    entity: position.entity,
                },
            ));
        }
        Ok(())
    }
}

struct CandidateRows<'a, V> {
    problem: &'a ListProblem<V>,
    before: &'a ListAssignment<V>,
    rows: BTreeMap<usize, Vec<&'a V>>,
}
impl<'a, V: Eq + Clone> CandidateRows<'a, V> {
    fn len(&self, entity: usize) -> Result<usize, ListError> {
        if let Some(row) = self.rows.get(&entity) {
            return Ok(row.len());
        }
        self.before
            .list(entity)
            .map(<[V]>::len)
            .ok_or(ListError::EntityIndex {
                entity,
                count: self.before.lists().len(),
            })
    }
    fn element(&self, p: ListPosition) -> Result<&'a V, ListError> {
        if p.index >= self.len(p.entity)? {
            return Err(ListError::ElementIndex(p));
        }
        Ok(self
            .rows
            .get(&p.entity)
            .map_or_else(|| &self.before.lists()[p.entity][p.index], |r| r[p.index]))
    }
    fn row(&mut self, entity: usize) -> &mut Vec<&'a V> {
        self.rows
            .entry(entity)
            .or_insert_with(|| self.before.lists()[entity].iter().collect())
    }
    fn contains(&self, value: &V) -> bool {
        (0..self.before.lists().len()).any(|e| {
            self.rows.get(&e).map_or_else(
                || self.before.lists()[e].contains(value),
                |r| r.contains(&value),
            )
        })
    }
    fn replay(&mut self, movement: &'a ListMove<V>) -> Result<(), ListError> {
        let mut pending = vec![movement];
        while let Some(step) = pending.pop() {
            match step {
                ListMove::Assign { value, destination } => {
                    if destination.index > self.len(destination.entity)? {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    if self.contains(value) {
                        return Err(ListError::AlreadyAssigned);
                    }
                    if !self.problem.values.contains(value) {
                        return Err(self.problem.range_error(
                            *destination,
                            value,
                            ListError::ValueOutsideDomain,
                        ));
                    }
                    self.row(destination.entity)
                        .insert(destination.index, value);
                }
                ListMove::Change {
                    source,
                    destination,
                } => {
                    self.element(*source)?;
                    let length = self.len(destination.entity)?
                        - usize::from(source.entity == destination.entity);
                    if destination.index > length {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    let value = self.row(source.entity).remove(source.index);
                    self.row(destination.entity)
                        .insert(destination.index, value);
                }
                ListMove::Swap { left, right } => {
                    let a = self.element(*left)?;
                    let b = self.element(*right)?;
                    self.row(left.entity)[left.index] = b;
                    self.row(right.entity)[right.index] = a;
                }
                ListMove::Unassign { entity, start, end } => {
                    let length = self.len(*entity)?;
                    if start > end || *end > length {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    self.row(*entity).drain(*start..*end);
                }
                ListMove::SubListChange {
                    entity,
                    start,
                    end,
                    destination,
                    reverse,
                } => {
                    let length = self.len(*entity)?;
                    if start > end || *end > length {
                        return Err(ListError::InvalidRange {
                            entity: *entity,
                            start: *start,
                            end: *end,
                        });
                    }
                    let remaining = self.len(destination.entity)?
                        - if *entity == destination.entity {
                            end - start
                        } else {
                            0
                        };
                    if destination.index > remaining {
                        return Err(ListError::InsertionIndex(*destination));
                    }
                    let mut values: Vec<_> = self.row(*entity).drain(*start..*end).collect();
                    if *reverse {
                        values.reverse();
                    }
                    self.row(destination.entity)
                        .splice(destination.index..destination.index, values);
                }
                ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } => {
                    for range in [left, right] {
                        if range.start >= range.end || range.end > self.len(range.entity)? {
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
                    let mut a = self.row(left.entity)[left.start..left.end].to_vec();
                    let mut b = self.row(right.entity)[right.start..right.end].to_vec();
                    if *reverse {
                        a.reverse();
                        b.reverse();
                    }
                    if left.entity == right.entity && left.start < right.start {
                        self.row(right.entity).splice(right.start..right.end, a);
                        self.row(left.entity).splice(left.start..left.end, b);
                    } else {
                        self.row(left.entity).splice(left.start..left.end, b);
                        self.row(right.entity).splice(right.start..right.end, a);
                    }
                }
                ListMove::Composite(children) => pending.extend(children.iter().rev()),
            }
        }
        Ok(())
    }
    fn validate_final(&self) -> Result<(), ListError> {
        for (&entity, row) in &self.rows {
            let meta = &self.problem.entities[entity];
            let matches_pin = if meta.pinned {
                row.iter().copied().eq(meta.initial.iter())
            } else {
                row.get(..meta.pinned_prefix).is_some_and(|prefix| {
                    prefix
                        .iter()
                        .copied()
                        .eq(meta.initial[..meta.pinned_prefix].iter())
                })
            };
            if !matches_pin {
                return Err(ListError::PinChanged { entity });
            }
            for (index, &value) in row.iter().enumerate() {
                if !self.problem.accepts(entity, value) {
                    return Err(self.problem.range_error(
                        ListPosition { entity, index },
                        value,
                        ListError::ValueOutsideEntityRange { entity },
                    ));
                }
            }
        }
        if let Some(graph) = &self.problem.dependencies {
            let mut dynamic = vec![];
            let mut assigned = vec![false; self.problem.values.len()];
            for entity in 0..self.before.lists().len() {
                let mut previous = None;
                for index in 0..self.len(entity)? {
                    let value = self.element(ListPosition { entity, index })?;
                    let id = self
                        .problem
                        .values
                        .iter()
                        .position(|v| v == value)
                        .expect("validated registry");
                    assigned[id] = true;
                    if let Some(parent) = previous {
                        dynamic.push((id, parent));
                    }
                    previous = Some(id);
                }
            }
            let state = graph
                .evaluate(&dynamic, |_, _| Some(()))
                .map_err(ListError::Dependency)?;
            let values: Vec<_> = state
                .into_iter()
                .filter_map(|(i, derived)| (derived.inconsistent && assigned[i]).then_some(i))
                .collect();
            if !values.is_empty() {
                return Err(ListError::DependencyCycle { values });
            }
        }
        Ok(())
    }
}
