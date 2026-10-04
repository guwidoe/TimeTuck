//! Assigned-only sampled list groups, with optional whole-group unassignment.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, Neighborhood,
    SubPillarSelection,
};

/// Cardinality of an assigned, movable group. Groups smaller than two produce no
/// moves. `all()` means every assigned movable identity before receiver filtering;
/// a receiver that cannot admit that whole group contributes no destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListSampleSize {
    minimum: usize,
    maximum: usize,
    all: bool,
}
impl ListSampleSize {
    pub const fn exactly(size: usize) -> Self {
        Self {
            minimum: size,
            maximum: size,
            all: false,
        }
    }
    pub fn between(minimum: usize, maximum: usize) -> Result<Self, ListError> {
        if minimum > maximum {
            return Err(ListError::InvalidSampleSize { minimum, maximum });
        }
        Ok(Self {
            minimum,
            maximum,
            all: false,
        })
    }
    pub const fn all() -> Self {
        Self {
            minimum: 0,
            maximum: 0,
            all: true,
        }
    }
    fn bounds(self, population: usize) -> Option<(usize, usize)> {
        let (minimum, maximum) = if self.all {
            (population, population)
        } else {
            (self.minimum, self.maximum.min(population))
        };
        let minimum = minimum.max(2);
        (minimum <= maximum).then_some((minimum, maximum))
    }
}
/// Controls assigned-only mass changes. Missing identities never join a sample.
/// The default null policy follows model optionality. Explicitly enabling whole-
/// group unassignment for a required model is a validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListMassChangeConfig {
    sample: ListSampleSize,
    unassignment: Option<bool>,
}
impl ListMassChangeConfig {
    pub const fn new(sample: ListSampleSize) -> Self {
        Self {
            sample,
            unassignment: None,
        }
    }
    pub const fn with_unassignment(mut self, enabled: bool) -> Self {
        self.unassignment = Some(enabled);
        self
    }
    pub fn sample_size(self) -> ListSampleSize {
        self.sample
    }
    pub fn unassignment(self) -> Option<bool> {
        self.unassignment
    }
    pub(crate) fn validate<V: Eq + Clone>(self, p: &ListProblem<V>) -> Result<(), ListError> {
        if self.unassignment == Some(true) && !p.allows_unassigned() {
            return Err(ListError::UnassignmentNotAllowed);
        }
        Ok(())
    }
}
impl Default for ListMassChangeConfig {
    fn default() -> Self {
        Self::new(ListSampleSize {
            minimum: 2,
            maximum: usize::MAX,
            all: false,
        })
    }
}
/// Immutable sample/destination metadata and the resulting snapshot-bound move.
/// The destination is an insertion boundary in the original assignment. Applying
/// the ordinary composite uses the existing atomic validation/scoring rollback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListMassChange<V> {
    members: Vec<V>,
    destination: Option<ListPosition>,
    movement: ListMove<V>,
}
impl<V> ListMassChange<V> {
    pub fn members(&self) -> &[V] {
        &self.members
    }
    pub fn destination(&self) -> Option<ListPosition> {
        self.destination
    }
    pub fn movement(&self) -> &ListMove<V> {
        &self.movement
    }
    pub fn into_move(self) -> ListMove<V> {
        self.movement
    }
}
impl Neighborhood {
    pub(crate) fn validate_list<V: Eq + Clone>(self, p: &ListProblem<V>) -> Result<(), ListError> {
        if let Self::MassListChange(c) = self {
            c.validate(p)?;
        }
        Ok(())
    }
}
/// Lazily enumerate group changes in receiver/index order, then increasing group
/// size and registry-index permutation order. Unassignments follow, once per
/// unordered group. Pins, prefixes and receiver ranges constrain every member.
/// Unchanged results and equivalent adjacent boundaries are skipped; distinct
/// samples can still produce the same successor. No random trace parity is implied.
/// All-sized samples use the global assigned movable population, not the size of
/// a receiver's range. Partial inputs remain partial; no missing value is assigned.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![0,1,2],vec![ListEntity::new(vec![0,1])])?
///     .allowing_unassigned();
/// let a=p.initial_assignment();
/// let moves:Vec<_>=list_mass_change_moves(&p,&a,
///     ListMassChangeConfig::new(ListSampleSize::all()))?.collect();
/// assert!(!moves.is_empty());
/// assert!(moves.iter().all(|m| !m.members().contains(&2)));
/// # Ok::<(), ListError>(())
/// ```
pub fn list_mass_change_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListMassChangeConfig,
) -> Result<impl Iterator<Item = ListMassChange<V>> + 'a, ListError> {
    p.validate(a)?;
    config.validate(p)?;
    Ok(ordered(p, a, config, 0..p.entities().len()))
}
fn eligible<V: Eq + Clone>(p: &ListProblem<V>, a: &ListAssignment<V>, v: &V) -> bool {
    a.location(v).is_some_and(|at| {
        !p.entities()[at.entity].is_pinned() && at.index >= p.entities()[at.entity].pinned_prefix()
    })
}
pub(crate) fn ordered<'a, V: Eq + Clone, I: Iterator<Item = usize> + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListMassChangeConfig,
    order: I,
) -> Box<dyn Iterator<Item = ListMassChange<V>> + 'a> {
    let population = p.values().iter().filter(|v| eligible(p, a, v)).count();
    let Some((minimum, maximum)) = config.sample.bounds(population) else {
        return Box::new(std::iter::empty());
    };
    let inserted = crate::list_neighborhood::insertions(p, a, order).flat_map(move |destination| {
        let preceding = destination
            .index
            .checked_sub(1)
            .map(|i| &a.lists()[destination.entity][i]);
        let eligible: Vec<_> = p
            .values()
            .iter()
            .filter(|v| {
                preceding != Some(*v) && eligible(p, a, v) && p.accepts(destination.entity, v)
            })
            .collect();
        crate::list_gather_neighborhood::Selections::bounded(eligible.len(), minimum, maximum)
            .filter_map(move |indices| {
                let refs: Vec<_> = indices.iter().map(|&i| eligible[i]).collect();
                if crate::list_gather_neighborhood::unchanged(a, &refs, destination) {
                    return None;
                }
                let members: Vec<_> = refs.into_iter().cloned().collect();
                let movement = p
                    .gather_move(a, &members, Some(destination))
                    .expect("validated mass change");
                Some(ListMassChange {
                    members,
                    destination: Some(destination),
                    movement,
                })
            })
    });
    let removed = config
        .unassignment
        .unwrap_or(p.allows_unassigned())
        .then_some(())
        .into_iter()
        .flat_map(move |_| {
            let eligible: Vec<_> = p.values().iter().filter(|v| eligible(p, a, v)).collect();
            SubPillarSelection::combinations(minimum, maximum)
                .expect("positive group bounds")
                .indices(eligible.len())
                .map(move |indices| {
                    let members: Vec<_> = indices.iter().map(|&i| eligible[i].clone()).collect();
                    let movement = p
                        .gather_move(a, &members, None)
                        .expect("validated group unassignment");
                    ListMassChange {
                        members,
                        destination: None,
                        movement,
                    }
                })
        });
    Box::new(inserted.chain(removed))
}
