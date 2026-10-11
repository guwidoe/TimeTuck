//! Stateful construction of compound list moves over an isolated draft.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListEditError {
    Model(ListError),
    SamePosition(ListPosition),
    SameEntity {
        entity: usize,
        swap: bool,
    },
    SignedIndex {
        entity: usize,
        index: isize,
        len: usize,
    },
    InvalidShift {
        source: ListPosition,
        offset: isize,
    },
}
impl From<ListError> for ListEditError {
    fn from(value: ListError) -> Self {
        Self::Model(value)
    }
}
impl fmt::Display for ListEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::SamePosition(p) => {
                write!(f, "source and replacement are the same position: {p:?}")
            }
            Self::SameEntity { entity, swap } => {
                write!(f, "cross-list entities must be different (entity {entity})")?;
                if *swap {
                    f.write_str("; use same-list swap (swapValuesInList)")?;
                }
                Ok(())
            }
            Self::SignedIndex { entity, index, len } => write!(
                f,
                "list index {index} is out of bounds for entity {entity} with length {len}"
            ),
            Self::InvalidShift { source, offset } => {
                write!(f, "invalid shift of {source:?} by {offset}")
            }
        }
    }
}
impl Error for ListEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            _ => None,
        }
    }
}

/// Prepare a compound move without changing the solver's working assignment.
/// Coordinates always refer to the current draft; relocation destinations use
/// post-removal indices. Each method is atomic and validates pins/value ranges at
/// the end of that method. Use push(Composite(...)) for a group requiring temporary
/// intermediate pin/range violations. Failed methods preserve all earlier edits.
///
/// This baseline owns a full draft and uses the model's snapshot transactions;
/// it is intended for custom compound moves, not constant-time list editing.
/// Cloning a builder forks an independent draft. finish returns a normal move for
/// any list scoring backend; apply it to the same starting state used by new.
/// Required models may temporarily have missing values; is_complete remains the
/// separate final-solution check, just as with ListProblem::apply.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![0,1,2,3], vec![
///     ListEntity::new(vec![0,1]), ListEntity::new(vec![2])])?;
/// let mut a = p.initial_assignment();
/// let mut draft = ListMoveBuilder::new(&p, &a)?;
/// draft.assign(3, ListPosition { entity: 0, index: 1 })?;
/// draft.relocate(ListPosition { entity: 0, index: 2 },
///                ListPosition { entity: 1, index: 1 })?;
/// assert_eq!(draft.assignment().lists(), &[vec![0,3], vec![2,1]]);
/// p.apply(&mut a, &draft.finish())?.commit();
/// assert!(p.is_complete(&a));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct ListMoveBuilder<'p, V> {
    problem: &'p ListProblem<V>,
    draft: ListAssignment<V>,
    steps: Vec<ListMove<V>>,
}
impl<'p, V: Eq + Clone> ListMoveBuilder<'p, V> {
    pub fn new(
        problem: &'p ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<Self, ListEditError> {
        problem.validate(assignment)?;
        Ok(Self {
            problem,
            draft: assignment.clone(),
            steps: vec![],
        })
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.draft
    }
    pub fn steps(&self) -> &[ListMove<V>] {
        &self.steps
    }

    /// Append a primitive or nested composite. Current coordinates are retained
    /// verbatim, so replay of all successful calls reproduces the draft.
    pub fn push(&mut self, movement: ListMove<V>) -> Result<(), ListEditError> {
        self.steps.reserve(1);
        self.problem.apply(&mut self.draft, &movement)?.commit();
        self.steps.push(movement);
        Ok(())
    }
    /// No edits produce an empty Composite; one edit is returned directly.
    /// Multiple successful edits retain their order, including nested groups.
    pub fn finish(mut self) -> ListMove<V> {
        if self.steps.len() == 1 {
            self.steps.pop().unwrap()
        } else {
            ListMove::Composite(self.steps)
        }
    }
    pub fn assign(&mut self, value: V, destination: ListPosition) -> Result<(), ListEditError> {
        self.push(ListMove::Assign { value, destination })
    }
    /// Insert previously unassigned values in input order, as one atomic edit.
    /// An empty input is a validated no-op; duplicate/already assigned values fail.
    pub fn assign_many(
        &mut self,
        values: impl IntoIterator<Item = V>,
        destination: ListPosition,
    ) -> Result<(), ListEditError> {
        self.insertion(destination)?;
        let steps = values
            .into_iter()
            .enumerate()
            .map(|(offset, value)| {
                destination
                    .index
                    .checked_add(offset)
                    .ok_or(ListError::InsertionIndex(destination))
                    .map(|index| ListMove::Assign {
                        value,
                        destination: ListPosition {
                            index,
                            ..destination
                        },
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if steps.is_empty() {
            return Ok(());
        }
        self.push(ListMove::Composite(steps))
    }
    /// Replace a slot with an unassigned value, returning the displaced value.
    /// At the end of a list (including an empty list), append and return None.
    pub fn set_unassigned(
        &mut self,
        value: V,
        destination: ListPosition,
    ) -> Result<Option<V>, ListEditError> {
        self.insertion(destination)?;
        if self.draft.location(&value).is_some() {
            return Err(ListError::AlreadyAssigned.into());
        }
        let displaced = self.draft.lists()[destination.entity]
            .get(destination.index)
            .cloned();
        if displaced.is_some() {
            self.push(ListMove::Composite(vec![
                ListMove::Unassign {
                    entity: destination.entity,
                    start: destination.index,
                    end: destination.index + 1,
                },
                ListMove::Assign { value, destination },
            ]))?;
        } else {
            self.assign(value, destination)?;
        }
        Ok(displaced)
    }
    pub fn relocate(
        &mut self,
        source: ListPosition,
        destination: ListPosition,
    ) -> Result<(), ListEditError> {
        self.push(ListMove::Change {
            source,
            destination,
        })
    }
    /// Move an existing element by a signed offset within its list. Zero, overflow,
    /// and shifts outside the final list are errors. Returns the moved value.
    pub fn shift(&mut self, source: ListPosition, offset: isize) -> Result<V, ListEditError> {
        let value = self.element(source)?.clone();
        let index = source
            .index
            .checked_add_signed(offset)
            .filter(|&i| offset != 0 && i < self.draft.lists()[source.entity].len())
            .ok_or(ListEditError::InvalidShift { source, offset })?;
        self.relocate(source, ListPosition { index, ..source })?;
        Ok(value)
    }
    /// Move source into the replacement's place, unassigning the displaced value.
    /// Both coordinates are in the pre-edit draft; same-list shifts are adjusted.
    pub fn replace(
        &mut self,
        source: ListPosition,
        replacement: ListPosition,
    ) -> Result<V, ListEditError> {
        self.element(source)?;
        let displaced = self.element(replacement)?.clone();
        if source == replacement {
            return Err(ListEditError::SamePosition(source));
        }
        let adjusted_source = ListPosition {
            index: source.index
                - usize::from(
                    source.entity == replacement.entity && source.index > replacement.index,
                ),
            ..source
        };
        let destination = ListPosition {
            index: replacement.index
                - usize::from(
                    source.entity == replacement.entity && source.index < replacement.index,
                ),
            ..replacement
        };
        self.push(ListMove::Composite(vec![
            ListMove::Unassign {
                entity: replacement.entity,
                start: replacement.index,
                end: replacement.index + 1,
            },
            ListMove::Change {
                source: adjusted_source,
                destination,
            },
        ]))?;
        Ok(displaced)
    }
    /// Checked signed external coordinates, in the pre-edit draft. Index failures
    /// preserve all earlier edits; no native move is built until both are valid.
    pub fn replace_signed(
        &mut self,
        entity: usize,
        source: isize,
        replacement: isize,
    ) -> Result<V, ListEditError> {
        let len = self.list(entity)?.len();
        let check = |index| {
            usize::try_from(index)
                .ok()
                .filter(|&i| i < len)
                .ok_or(ListEditError::SignedIndex { entity, index, len })
        };
        self.replace(
            ListPosition {
                entity,
                index: check(source)?,
            },
            ListPosition {
                entity,
                index: check(replacement)?,
            },
        )
    }
    /// Explicit cross-list operation. General relocate_span also supports
    /// within-list edits; this boundary requires two distinct owners.
    pub fn relocate_between_lists(
        &mut self,
        range: ListRange,
        destination: ListPosition,
        reverse: bool,
    ) -> Result<(), ListEditError> {
        if range.entity == destination.entity {
            return Err(ListEditError::SameEntity {
                entity: range.entity,
                swap: false,
            });
        }
        self.relocate_span(range, destination, reverse)
    }
    pub fn swap_between_lists(
        &mut self,
        left: ListRange,
        right: ListRange,
        reverse: bool,
    ) -> Result<(), ListEditError> {
        if left.entity == right.entity {
            return Err(ListEditError::SameEntity {
                entity: left.entity,
                swap: true,
            });
        }
        self.swap_spans(left, right, reverse)
    }
    pub fn relocate_span(
        &mut self,
        range: ListRange,
        destination: ListPosition,
        reverse: bool,
    ) -> Result<(), ListEditError> {
        self.nonempty(range)?;
        self.push(ListMove::SubListChange {
            entity: range.entity,
            start: range.start,
            end: range.end,
            destination,
            reverse,
        })
    }
    pub fn swap_spans(
        &mut self,
        left: ListRange,
        right: ListRange,
        reverse: bool,
    ) -> Result<(), ListEditError> {
        self.nonempty(left)?;
        self.nonempty(right)?;
        self.push(ListMove::SubListSwap {
            left,
            right,
            reverse,
        })
    }
    pub fn unassign(&mut self, range: ListRange) -> Result<(), ListEditError> {
        self.nonempty(range)?;
        self.push(ListMove::Unassign {
            entity: range.entity,
            start: range.start,
            end: range.end,
        })
    }
    fn list(&self, entity: usize) -> Result<&[V], ListError> {
        self.draft.list(entity).ok_or(ListError::EntityIndex {
            entity,
            count: self.problem.entities().len(),
        })
    }
    fn element(&self, position: ListPosition) -> Result<&V, ListError> {
        self.list(position.entity)?
            .get(position.index)
            .ok_or(ListError::ElementIndex(position))
    }
    fn insertion(&self, position: ListPosition) -> Result<(), ListError> {
        if position.index > self.list(position.entity)?.len() {
            Err(ListError::InsertionIndex(position))
        } else {
            Ok(())
        }
    }
    fn nonempty(&self, range: ListRange) -> Result<(), ListError> {
        if range.start >= range.end || range.end > self.list(range.entity)?.len() {
            Err(ListError::InvalidRange {
                entity: range.entity,
                start: range.start,
                end: range.end,
            })
        } else {
            Ok(())
        }
    }
}
