//! Ordered entity-start and value-anchor selection for list destinations.
use crate::{ListAssignment, ListError, ListPosition, ListProblem, SampleRandom};
use std::{iter::FusedIterator, num::NonZeroUsize};

/// An owned PRE-removal destination order. Duplicate occurrences retain their
/// sampling weight. Rebuild after assignment changes; positions are snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListDestinationOrder {
    positions: Vec<Option<ListPosition>>,
}
impl ListDestinationOrder {
    pub fn len(&self) -> usize {
        self.positions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
    /// Outer None means out of bounds; Some(None) is explicit unassignment.
    pub fn get(&self, index: usize) -> Option<Option<ListPosition>> {
        self.positions.get(index).copied()
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = Option<ListPosition>> + ExactSizeIterator + '_ {
        self.positions.iter().copied()
    }
    /// Uniform occurrence sampling. Empty/singleton orders consume no randomness.
    pub fn sample(&self, random: &mut SampleRandom) -> Option<Option<ListPosition>> {
        let n = NonZeroUsize::new(self.len())?;
        self.get(if n.get() == 1 { 0 } else { random.index(n) })
    }
    /// Repeated injected rank draws, allowing deterministic traversal tests.
    /// Empty orders exhaust without drawing; invalid ranks panic.
    pub fn random_iter_with(
        &self,
        mut draw: impl FnMut(usize) -> usize,
    ) -> impl FusedIterator<Item = Option<ListPosition>> {
        std::iter::from_fn(move || {
            if self.is_empty() {
                return None;
            }
            let i = draw(self.len());
            assert!(
                i < self.len(),
                "destination draw must be less than its bound"
            );
            self.get(i)
        })
        .fuse()
    }
}

/// A borrowed assignment snapshot with independent entity-start and value-anchor
/// orders. Starts use each entity's pinned-prefix boundary; anchors contribute the
/// position after an assigned movable value. Missing and pinned anchors disappear.
/// Entity selection controls starts only; anchors may refer to other entities.
/// All identities are checked before opening; duplicate inputs are retained.
///
/// Group queries intersect receiver ranges and exclude the group's own anchors.
/// Optional unassignment is the first destination even when no entity can receive
/// the group. This is receiver selection, not proof of move doability: callers
/// still validate source pins, same-list index adjustment and dependencies.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![2,1]),ListEntity::new(vec![3])])?;
/// let a=p.initial_assignment();
/// let selector=ListDestinationSelector::new(&p,&a,&[1,0],&[3,1,2],false)?;
/// assert_eq!(selector.all().len(),5);
/// assert_eq!(selector.for_values(&[2])?.len(),4);
/// # Ok::<(),ListError>(())
/// ```
pub struct ListDestinationSelector<'a, V> {
    problem: &'a ListProblem<V>,
    _assignment: &'a ListAssignment<V>,
    starts: Vec<ListPosition>,
    anchors: Vec<(usize, ListPosition)>,
    include_unassigned: bool,
}
impl<'a, V: Clone + Eq> ListDestinationSelector<'a, V> {
    pub fn new(
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        entities: &[usize],
        anchors: &[V],
        include_unassigned: bool,
    ) -> Result<Self, ListError> {
        problem.validate(assignment)?;
        if include_unassigned && !problem.allows_unassigned() {
            return Err(ListError::UnassignmentNotAllowed);
        }
        let mut starts = vec![];
        for &entity in entities {
            let meta = problem
                .entities()
                .get(entity)
                .ok_or(ListError::EntityIndex {
                    entity,
                    count: problem.entities().len(),
                })?;
            if !meta.is_pinned() {
                starts.push(ListPosition {
                    entity,
                    index: meta.pinned_prefix(),
                });
            }
        }
        // Two or three anchor queries do not justify indexing the whole layout.
        let locations = (anchors.len() >= 64 && problem.values().len() >= 64)
            .then(|| problem.indexed_value_locations(assignment))
            .flatten();
        let mut positions = vec![];
        for value in anchors {
            let id = problem
                .value_index(value)
                .ok_or(ListError::ValueOutsideDomain)?;
            let at = locations
                .as_ref()
                .map_or_else(|| assignment.location(value), |rows| rows[id]);
            if let Some(at) = at {
                let meta = &problem.entities()[at.entity];
                if !meta.is_pinned() && at.index >= meta.pinned_prefix() {
                    positions.push((
                        id,
                        ListPosition {
                            entity: at.entity,
                            index: at.index + 1,
                        },
                    ));
                }
            }
        }
        Ok(Self {
            problem,
            _assignment: assignment,
            starts,
            anchors: positions,
            include_unassigned,
        })
    }
    pub fn all(&self) -> ListDestinationOrder {
        self.select(&[])
    }
    pub fn for_values(&self, values: &[V]) -> Result<ListDestinationOrder, ListError> {
        let ids = values
            .iter()
            .map(|value| {
                self.problem
                    .value_index(value)
                    .ok_or(ListError::ValueOutsideDomain)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.select(&ids))
    }
    fn select(&self, ids: &[usize]) -> ListDestinationOrder {
        let reachable = |at: &ListPosition| {
            ids.iter()
                .all(|&id| self.problem.accepts_registered(at.entity, id))
        };
        ListDestinationOrder {
            positions: self
                .include_unassigned
                .then_some(None)
                .into_iter()
                .chain(
                    self.starts
                        .iter()
                        .filter(|p| reachable(p))
                        .copied()
                        .map(Some),
                )
                .chain(
                    self.anchors
                        .iter()
                        .filter(|(id, at)| !ids.contains(id) && reachable(at))
                        .map(|(_, at)| Some(*at)),
                )
                .collect(),
        }
    }
}
