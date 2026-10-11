//! Gather selected list identities into a consecutive block.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem};
use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListGatherError {
    DuplicateValue { index: usize },
    List(ListError),
}
impl From<ListError> for ListGatherError {
    fn from(value: ListError) -> Self {
        Self::List(value)
    }
}
impl fmt::Display for ListGatherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateValue { index } => {
                write!(f, "duplicate gathered value at index {index}")
            }
            Self::List(error) => error.fmt(f),
        }
    }
}
impl Error for ListGatherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::List(error) => Some(error),
            _ => None,
        }
    }
}
impl<V: Eq + Clone> ListProblem<V> {
    /// Gather distinct registered identities in `members` order at an original
    /// insertion boundary. Only members originally before that boundary reduce
    /// its index. Other values keep their relative order. `None` unassigns the
    /// selected members; already unassigned members then have no effect.
    ///
    /// Assigned values retain their existing payload; newly assigned values use
    /// the supplied payload. Empty selections are validated no-ops. Required
    /// models may become partial, like ordinary Unassign moves; use is_complete
    /// to distinguish complete solutions. Pins and ranges constrain the final
    /// state, including no-op gathers of pinned values.
    ///
    /// Returns an ordinary composite whose indices are bound to `before`.
    /// Regenerate it after the assignment changes. Construction never mutates
    /// the input. Applying the result uses existing atomic move/scoring rollback.
    /// Equality of these composites is structural, not Java mass-move equality.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new(vec![0, 1, 2, 3], vec![ListEntity::new(vec![0, 1, 2, 3])])?;
    /// let mut a = p.initial_assignment();
    /// let m = p.gather_move(&a, &[0, 2], Some(ListPosition { entity: 0, index: 2 }))?;
    /// { let trial = p.apply(&mut a, &m)?;
    ///   assert_eq!(trial.list(0), Some(&[1, 0, 2, 3][..])); }
    /// assert_eq!(a.list(0), Some(&[0, 1, 2, 3][..]));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn gather_move(
        &self,
        before: &ListAssignment<V>,
        members: &[V],
        destination: Option<ListPosition>,
    ) -> Result<ListMove<V>, ListGatherError> {
        GatherBuilder::new(self, before)?.gather_move(members, destination)
    }
}

// A borrowed, validated snapshot prevents the sampling fast path from outliving
// its source or being reused after an assignment change. Locations are built once
// per opening; each proposal only copies references for rows it actually edits.
pub(crate) struct GatherBuilder<'a, V> {
    problem: &'a ListProblem<V>,
    before: &'a ListAssignment<V>,
    locations: Vec<Option<ListPosition>>,
}

impl<'a, V: Eq + Clone> GatherBuilder<'a, V> {
    fn new(problem: &'a ListProblem<V>, before: &'a ListAssignment<V>) -> Result<Self, ListError> {
        problem.validate(before)?;
        Ok(Self::from_validated(problem, before))
    }

    // The index validates this exact borrowed source, including optionality and
    // destination count. Keep construction together to avoid a second validation
    // pass without exposing an unchecked builder or accepting a stale index.
    pub(crate) fn for_sampling(
        problem: &'a ListProblem<V>,
        before: &'a ListAssignment<V>,
        unassignment: bool,
    ) -> Result<(Self, crate::ListDestinationIndex<'a, V>), crate::ListDestinationError> {
        let destinations = crate::ListDestinationIndex::new(problem, before, unassignment)?;
        destinations.for_values(&[])?;
        Ok((Self::from_validated(problem, before), destinations))
    }

    fn from_validated(problem: &'a ListProblem<V>, before: &'a ListAssignment<V>) -> Self {
        Self {
            problem,
            before,
            locations: problem.value_locations(before),
        }
    }

    pub(crate) fn locations(&self) -> &[Option<ListPosition>] {
        &self.locations
    }

    pub(crate) fn gather_move(
        &self,
        members: &[V],
        destination: Option<ListPosition>,
    ) -> Result<ListMove<V>, ListGatherError> {
        let mut sources = Vec::with_capacity(members.len());
        for (index, member) in members.iter().enumerate() {
            if members[..index].contains(member) {
                return Err(ListGatherError::DuplicateValue { index });
            }
            let id = self
                .problem
                .value_index(member)
                .ok_or(ListError::ValueOutsideDomain)?;
            sources.push(self.locations[id]);
        }
        let insertion = if let Some(position) = destination {
            let list = self
                .before
                .list(position.entity)
                .ok_or(ListError::EntityIndex {
                    entity: position.entity,
                    count: self.before.lists().len(),
                })?;
            if position.index > list.len() {
                return Err(ListError::InsertionIndex(position).into());
            }
            Some(ListPosition {
                entity: position.entity,
                index: position.index
                    - sources
                        .iter()
                        .flatten()
                        .filter(|source| {
                            source.entity == position.entity && source.index < position.index
                        })
                        .count(),
            })
        } else {
            None
        };
        let mut rows: BTreeMap<usize, Vec<&V>> = BTreeMap::new();
        let mut steps = Vec::new();
        let mut gathered = Vec::new();
        for (member_index, (member, source)) in members.iter().zip(&sources).enumerate() {
            if let Some(source) = source {
                let index = source.index
                    - sources[..member_index]
                        .iter()
                        .flatten()
                        .filter(|earlier| {
                            earlier.entity == source.entity && earlier.index < source.index
                        })
                        .count();
                let row = rows
                    .entry(source.entity)
                    .or_insert_with(|| self.before.lists()[source.entity].iter().collect());
                let value = row.remove(index);
                steps.push(ListMove::Unassign {
                    entity: source.entity,
                    start: index,
                    end: index + 1,
                });
                if insertion.is_some() {
                    gathered.push(value);
                }
            } else if insertion.is_some() {
                gathered.push(member);
            }
        }
        if let Some(position) = insertion {
            let row = rows
                .entry(position.entity)
                .or_insert_with(|| self.before.lists()[position.entity].iter().collect());
            for (offset, value) in gathered.into_iter().enumerate() {
                let destination = ListPosition {
                    entity: position.entity,
                    index: position.index + offset,
                };
                steps.push(ListMove::Assign {
                    value: value.clone(),
                    destination,
                });
                row.insert(destination.index, value);
            }
        }
        // Every supplied identity is registered and distinct; each assigned
        // identity was removed before insertion. Untouched rows retain their
        // validated membership, including when formerly unassigned values arrive.
        self.problem.validate_permuted_rows(self.before, rows)?;
        Ok(ListMove::Composite(steps))
    }
}
