//! Gather selected list identities into a consecutive block.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem};
use std::{error::Error, fmt};

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
        self.validate(before)?;
        for (index, member) in members.iter().enumerate() {
            if members[..index].contains(member) {
                return Err(ListGatherError::DuplicateValue { index });
            }
            if !self.values().contains(member) {
                return Err(ListError::ValueOutsideDomain.into());
            }
        }
        let insertion = if let Some(position) = destination {
            let list = before.list(position.entity).ok_or(ListError::EntityIndex {
                entity: position.entity,
                count: before.lists().len(),
            })?;
            if position.index > list.len() {
                return Err(ListError::InsertionIndex(position).into());
            }
            Some(ListPosition {
                entity: position.entity,
                index: list[..position.index]
                    .iter()
                    .filter(|v| !members.contains(v))
                    .count(),
            })
        } else {
            None
        };
        let mut candidate = before.clone();
        let mut steps = Vec::new();
        let mut gathered = Vec::new();
        for member in members {
            if let Some(position) = candidate.location(member) {
                let value = candidate.lists[position.entity].remove(position.index);
                steps.push(ListMove::Unassign {
                    entity: position.entity,
                    start: position.index,
                    end: position.index + 1,
                });
                if insertion.is_some() {
                    gathered.push(value);
                }
            } else if insertion.is_some() {
                gathered.push(member.clone());
            }
        }
        if let Some(position) = insertion {
            for (offset, value) in gathered.into_iter().enumerate() {
                let destination = ListPosition {
                    entity: position.entity,
                    index: position.index + offset,
                };
                steps.push(ListMove::Assign {
                    value: value.clone(),
                    destination,
                });
                candidate.lists[destination.entity].insert(destination.index, value);
            }
        }
        self.validate(&candidate)?;
        Ok(ListMove::Composite(steps))
    }
}
