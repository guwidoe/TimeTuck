//! Common-destination changes for arbitrary groups of basic variable slots.
use crate::{Assignment, FiniteProblem, ModelError, Move, Neighborhood, SubPillarSelection};

/// Group cardinality and null-crossing policy. Unlike pillars, members need not
/// currently have equal values. Zero and singleton groups produce no candidates.
/// Default null crossing follows each variable's optionality: missing required
/// variables are excluded, and null destinations require an entirely optional
/// group. Explicit true requires every model variable to be optional, even pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MassChangeConfig {
    minimum: usize,
    maximum: usize,
    all: bool,
    crossing_null: Option<bool>,
}
impl MassChangeConfig {
    pub const fn exactly(size: usize) -> Self {
        Self {
            minimum: size,
            maximum: size,
            all: false,
            crossing_null: None,
        }
    }
    pub fn between(minimum: usize, maximum: usize) -> Result<Self, ModelError> {
        if minimum > maximum {
            return Err(ModelError::InvalidSampleSize { minimum, maximum });
        }
        Ok(Self {
            minimum,
            maximum,
            ..Self::exactly(0)
        })
    }
    /// Use all source-eligible slots before considering destination ranges.
    pub const fn all() -> Self {
        Self {
            all: true,
            ..Self::exactly(0)
        }
    }
    /// False excludes missing members as well as null destinations.
    pub const fn with_crossing_null(mut self, enabled: bool) -> Self {
        self.crossing_null = Some(enabled);
        self
    }
    pub fn crossing_null(self) -> Option<bool> {
        self.crossing_null
    }
    pub(crate) fn validate<V: Eq>(self, p: &FiniteProblem<V>) -> Result<(), ModelError> {
        if self.crossing_null == Some(true)
            && let Some(variable) = p.variables().iter().position(|v| !v.allows_unassigned())
        {
            return Err(ModelError::NullCrossingRequired { variable });
        }
        Ok(())
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
impl Default for MassChangeConfig {
    fn default() -> Self {
        Self {
            minimum: 2,
            maximum: usize::MAX,
            ..Self::exactly(0)
        }
    }
}
/// Immutable selected-slot and common-destination metadata. The ordinary
/// composite includes every member, including members already at the target.
/// Regenerate candidates after the assignment changes. Domain/pin validation
/// and rollback remain the responsibility of the model or score director.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MassChange<V> {
    members: Vec<usize>,
    destination: Option<V>,
    movement: Move<V>,
}
impl<V> MassChange<V> {
    pub fn members(&self) -> &[usize] {
        &self.members
    }
    pub fn destination(&self) -> Option<&V> {
        self.destination.as_ref()
    }
    pub fn movement(&self) -> &Move<V> {
        &self.movement
    }
    pub fn into_move(self) -> Move<V> {
        self.movement
    }
}
impl<V: Clone> MassChange<V> {
    pub(crate) fn new(members: Vec<usize>, destination: Option<V>) -> Self {
        let movement = Move::Composite(
            members
                .iter()
                .map(|&variable| Move::Change {
                    variable,
                    value: destination.clone(),
                })
                .collect(),
        );
        Self {
            members,
            destination,
            movement,
        }
    }
}
impl Neighborhood {
    pub(crate) fn validate_basic<V: Eq>(self, p: &FiniteProblem<V>) -> Result<(), ModelError> {
        if let Self::MassChange(c) = self {
            c.validate(p)?;
        }
        if let Self::MassUnassign(_) = self {
            crate::mass_assignment::validate_unassignment(p)?;
        }
        Ok(())
    }
}
/// Lazily enumerate subsets by size and source position. Targets follow the first
/// member's domain, with null last if allowed for every member. Every target must
/// be legal for the entire group; unchanged states are skipped. Slots are selected
/// independently of a PillarSchema. No random sampling distribution is implied.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0),
///                                Variable::new(vec![0,1]).with_initial(1)])?;
/// let a = p.initial_assignment();
/// let moves: Vec<_> = mass_change_moves(&p, &a, MassChangeConfig::all())?.collect();
/// assert_eq!(moves.len(), 2);
/// assert!(moves.iter().all(|m| m.members() == [0,1]));
/// # Ok::<(), ModelError>(())
/// ```
pub fn mass_change_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: MassChangeConfig,
) -> Result<impl Iterator<Item = MassChange<V>> + 'a, ModelError> {
    p.validate(a)?;
    config.validate(p)?;
    Ok(ordered(p, a, config, 0..p.variables().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: MassChangeConfig,
    order: impl Iterator<Item = usize>,
) -> Box<dyn Iterator<Item = MassChange<V>> + 'a> {
    let eligible: Vec<_> = order
        .filter(|&i| {
            let v = &p.variables()[i];
            !v.is_pinned()
                && (a.values()[i].is_some()
                    || (config.crossing_null != Some(false) && v.allows_unassigned()))
        })
        .collect();
    let Some((minimum, maximum)) = config.bounds(eligible.len()) else {
        return Box::new(std::iter::empty());
    };
    Box::new(
        SubPillarSelection::combinations(minimum, maximum)
            .expect("positive group bounds")
            .indices(eligible.len())
            .flat_map(move |indices| {
                let members: Vec<_> = indices.into_iter().map(|i| eligible[i]).collect();
                let first = members[0];
                let null = config.crossing_null != Some(false)
                    && members
                        .iter()
                        .all(|&i| p.variables()[i].allows_unassigned());
                p.variables()[first]
                    .domain()
                    .iter()
                    .map(Some)
                    .chain(null.then_some(None))
                    .filter_map(move |destination| {
                        if !crate::pillar::accepts(p, &members, destination)
                            || members
                                .iter()
                                .all(|&i| a.values()[i].as_ref() == destination)
                        {
                            return None;
                        }
                        let movement = Move::Composite(
                            members
                                .iter()
                                .map(|&variable| Move::Change {
                                    variable,
                                    value: destination.cloned(),
                                })
                                .collect(),
                        );
                        Some(MassChange {
                            members: members.clone(),
                            destination: destination.cloned(),
                            movement,
                        })
                    })
            }),
    )
}
