//! Snapshot-bound, range-aware destinations for list neighborhoods.
use crate::{ListAssignment, ListError, ListPosition, ListProblem, SampleRandom};
use std::{error::Error, fmt, num::NonZeroUsize};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListDestinationError {
    Model(ListError),
    CountOverflow,
}
impl fmt::Display for ListDestinationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list destinations: {self:?}")
    }
}
impl Error for ListDestinationError {}
#[derive(Clone, Copy, Debug)]
struct Span {
    entity: usize,
    start: usize,
    length: usize,
}
/// A compact owned selection of insertion intervals. None is one optional
/// unassignment member, preceding assigned destinations. Ordinal lookup and random
/// selection count each insertion slot equally; entities with longer lists have
/// proportionally more slots. Random selection is with replacement.
///
/// get/sample return an outer None for an empty/out-of-bounds selection and
/// Some(None) for the unassignment member. A singleton does not consume randomness.
#[derive(Clone, Debug)]
pub struct ListDestinationSet {
    spans: Vec<(Span, usize)>,
    len: usize,
    unassigned: bool,
}
impl ListDestinationSet {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, index: usize) -> Option<Option<ListPosition>> {
        if index >= self.len {
            return None;
        }
        if self.unassigned && index == 0 {
            return Some(None);
        }
        let slot = index - usize::from(self.unassigned);
        let row = self.spans.partition_point(|(_, end)| *end <= slot);
        let (span, end) = self.spans[row];
        Some(Some(ListPosition {
            entity: span.entity,
            index: span.start + slot - (end - span.length),
        }))
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = Option<ListPosition>> + ExactSizeIterator + '_ {
        (0..self.len).map(|i| self.get(i).expect("in-bounds destination"))
    }
    /// Reachable movable entities, in model order, once each. A fully pinned
    /// suffix still has an append slot unless the entire entity is pinned.
    pub fn entities(&self) -> impl DoubleEndedIterator<Item = usize> + ExactSizeIterator + '_ {
        self.spans.iter().map(|(span, _)| span.entity)
    }
    pub fn sample(&self, random: &mut SampleRandom) -> Option<Option<ListPosition>> {
        let size = NonZeroUsize::new(self.len)?;
        self.get(if self.len == 1 { 0 } else { random.index(size) })
    }
}
/// Cache eligible receivers for each registered value and insertion intervals for
/// one immutable assignment. The borrows prevent changing the model or assignment
/// while this index is in use. Owned query results remain snapshots of old indices;
/// never apply them to a later assignment without recomputing destinations.
///
/// Full pins are excluded. Prefix pins allow insertion at the first unpinned index,
/// including append after a fully pinned prefix. Queries intersect receiving ranges
/// for groups; duplicate values do not change the set and an empty group accepts all
/// movable entities. Unknown values are errors. Optional unassignment is represented
/// once even when no entity can receive a group. No value objects are cloned.
///
/// These are PRE-removal insertion positions. They describe receiver eligibility,
/// not move doability: callers must respect source pins, adjust same-list indices,
/// and validate dependencies when constructing an actual move.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2],vec![
///     ListEntity::new(vec![1]).with_value_range(vec![1]),
///     ListEntity::new(vec![2]).with_value_range(vec![1,2])])?;
/// let a=p.initial_assignment();
/// let index=ListDestinationIndex::new(&p,&a,false)?;
/// let both=index.for_values(&[1,2])?;
/// assert_eq!(both.entities().collect::<Vec<_>>(),vec![1]);
/// assert_eq!(both.len(),2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ListDestinationIndex<'a, V> {
    problem: &'a ListProblem<V>,
    _assignment: &'a ListAssignment<V>,
    spans: Vec<Span>,
    receivers: Vec<Vec<bool>>,
    unassigned: bool,
}
impl<'a, V: Clone + Eq> ListDestinationIndex<'a, V> {
    pub fn new(
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        include_unassigned: bool,
    ) -> Result<Self, ListDestinationError> {
        problem
            .validate(assignment)
            .map_err(ListDestinationError::Model)?;
        if include_unassigned && !problem.allows_unassigned() {
            return Err(ListDestinationError::Model(
                ListError::UnassignmentNotAllowed,
            ));
        }
        let mut spans = vec![];
        for (entity, meta) in problem.entities().iter().enumerate() {
            if meta.is_pinned() {
                continue;
            }
            let start = meta.pinned_prefix();
            let length = (assignment.lists()[entity].len() - start)
                .checked_add(1)
                .ok_or(ListDestinationError::CountOverflow)?;
            spans.push(Span {
                entity,
                start,
                length,
            });
        }
        let receivers = problem
            .values()
            .iter()
            .map(|v| spans.iter().map(|s| problem.accepts(s.entity, v)).collect())
            .collect();
        Ok(Self {
            problem,
            _assignment: assignment,
            spans,
            receivers,
            unassigned: include_unassigned,
        })
    }
    pub fn for_value(&self, value: &V) -> Result<ListDestinationSet, ListDestinationError> {
        self.for_values(std::slice::from_ref(value))
    }
    pub fn for_values(&self, values: &[V]) -> Result<ListDestinationSet, ListDestinationError> {
        let mut eligible = vec![true; self.spans.len()];
        for value in values {
            let i = self
                .problem
                .value_index(value)
                .ok_or(ListDestinationError::Model(ListError::ValueOutsideDomain))?;
            for (keep, allowed) in eligible.iter_mut().zip(&self.receivers[i]) {
                *keep &= allowed;
            }
        }
        let mut spans = vec![];
        let mut count = 0usize;
        for (&span, keep) in self.spans.iter().zip(eligible) {
            if keep {
                count = count
                    .checked_add(span.length)
                    .ok_or(ListDestinationError::CountOverflow)?;
                spans.push((span, count));
            }
        }
        let len = count
            .checked_add(usize::from(self.unassigned))
            .ok_or(ListDestinationError::CountOverflow)?;
        Ok(ListDestinationSet {
            spans,
            len,
            unassigned: self.unassigned,
        })
    }
}
