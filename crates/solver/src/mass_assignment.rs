//! Group assignment and unassignment, distinct from arbitrary common-target changes.
use crate::{Assignment, FiniteProblem, MassChange, ModelError, SubPillarSelection};

/// Source group size before destination filtering. Exact sizes larger than the
/// eligible population yield none; range maximums clip. Zero groups yield none.
/// Assignment additionally excludes singleton groups; unassignment permits them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MassAssignmentSize {
    minimum: usize,
    maximum: usize,
    all: bool,
}
impl MassAssignmentSize {
    pub const fn exactly(size: usize) -> Self {
        Self {
            minimum: size,
            maximum: size,
            all: false,
        }
    }
    pub fn between(minimum: usize, maximum: usize) -> Result<Self, ModelError> {
        if minimum > maximum {
            return Err(ModelError::InvalidSampleSize { minimum, maximum });
        }
        Ok(Self {
            minimum,
            maximum,
            all: false,
        })
    }
    pub const fn all() -> Self {
        Self {
            all: true,
            ..Self::exactly(0)
        }
    }
    fn bounds(self, population: usize, unassign: bool) -> Option<(usize, usize)> {
        let (minimum, maximum) = if self.all {
            (population, population)
        } else {
            (self.minimum, self.maximum.min(population))
        };
        let minimum = minimum.max(if unassign { 1 } else { 2 });
        (minimum <= maximum).then_some((minimum, maximum))
    }
}
pub(crate) fn validate_unassignment<V: Eq>(p: &FiniteProblem<V>) -> Result<(), ModelError> {
    if let Some(variable) = p.variables().iter().position(|v| !v.allows_unassigned()) {
        return Err(ModelError::NullCrossingRequired { variable });
    }
    Ok(())
}
fn eligible<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    unassign: bool,
    order: impl Iterator<Item = usize>,
) -> Vec<usize> {
    order
        .filter(|&i| !p.variables()[i].is_pinned() && a.values()[i].is_some() == unassign)
        .collect()
}
/// Enumerate groups of at least two missing, unpinned basic slots, including
/// missing required slots in a partial assignment. A common non-null target must
/// belong to every member's domain. All-sized groups use the entire missing source
/// population. Order is increasing subset size, member position, first-member
/// domain. Does not assign pinned missing slots or touch already assigned slots.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]);2])?;
/// let a=p.initial_assignment();
/// let moves:Vec<_>=mass_assign_moves(&p,&a,MassAssignmentSize::all())?.collect();
/// assert_eq!(moves.len(),2);
/// # Ok::<(), ModelError>(())
/// ```
pub fn mass_assign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    size: MassAssignmentSize,
) -> Result<impl Iterator<Item = MassChange<V>> + 'a, ModelError> {
    p.validate(a)?;
    Ok(ordered(p, a, size, false, 0..p.variables().len()))
}
/// Enumerate assigned, unpinned groups in increasing size and member order, each
/// once. Singleton groups are valid. Every model slot must allow unassignment,
/// including pinned slots; empty models are valid. Missing slots never join.
pub fn mass_unassign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    size: MassAssignmentSize,
) -> Result<impl Iterator<Item = MassChange<V>> + 'a, ModelError> {
    p.validate(a)?;
    validate_unassignment(p)?;
    Ok(ordered(p, a, size, true, 0..p.variables().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    size: MassAssignmentSize,
    unassign: bool,
    order: impl Iterator<Item = usize>,
) -> Box<dyn Iterator<Item = MassChange<V>> + 'a> {
    let eligible = eligible(p, a, unassign, order);
    let Some((minimum, maximum)) = size.bounds(eligible.len(), unassign) else {
        return Box::new(std::iter::empty());
    };
    Box::new(
        SubPillarSelection::combinations(minimum, maximum)
            .expect("positive group bounds")
            .indices(eligible.len())
            .flat_map(move |indices| {
                let members: Vec<_> = indices.into_iter().map(|i| eligible[i]).collect();
                let first = members[0];
                p.variables()[first]
                    .domain()
                    .iter()
                    .take(if unassign { 0 } else { usize::MAX })
                    .map(Some)
                    .chain(unassign.then_some(None))
                    .filter_map(move |destination| {
                        if !crate::pillar::accepts(p, &members, destination) {
                            return None;
                        }
                        Some(MassChange::new(members.clone(), destination.cloned()))
                    })
            }),
    )
}
/// Bounded seeded draws with replacement between draws, without replacement
/// within a group. The snapshot stays unchanged. Size is chosen uniformly from
/// the clipped allowed range; a uniform subset is then selected and sorted by ID.
/// Empty/insufficient populations yield no draws. A nonempty population can supply
/// every requested draw, including repeated singleton groups. This does not claim
/// Java PRNG or sampling-distribution parity. Memory is linear in source size.
pub fn sample_mass_unassign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    size: MassAssignmentSize,
    seed: u64,
    draws: usize,
) -> Result<impl Iterator<Item = MassChange<V>> + 'a, ModelError> {
    p.validate(a)?;
    validate_unassignment(p)?;
    let eligible = eligible(p, a, true, 0..p.variables().len());
    let bounds = size.bounds(eligible.len(), true);
    let mut random = crate::random::SplitMix64::new(seed);
    Ok((0..if bounds.is_some() { draws } else { 0 }).map(move |_| {
        let (minimum, maximum) = bounds.expect("nonempty sample population");
        let count = minimum + random.below((maximum - minimum + 1) as u64) as usize;
        let mut members = eligible.clone();
        random.shuffle(&mut members);
        members.truncate(count);
        members.sort_unstable();
        MassChange::new(members, None)
    }))
}
