//! Lazy mixed neighborhoods, optionally including simultaneous component moves.
use crate::neighborhood;
use crate::{ListMove, MixedAssignment, MixedError, MixedMove, MixedProblem, Move, Neighborhood};

/// Independently select basic and list move families. None disables a component.
/// A basic-variable selection supplies encounter order and limits candidates to
/// those touching only selected slots. With a pillar schema, whole-row rules
/// still apply; selecting a column does not split a multi-column pillar move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixedNeighborhood<'a> {
    pub basic: Option<Neighborhood>,
    pub lists: Option<Neighborhood>,
    pub basic_variables: Option<&'a [usize]>,
    /// Override model grouping for this family's Pillar/SubPillar neighborhood.
    /// Other basic neighborhood kinds reject a schema override. None retains the
    /// model schema. The override does not mutate the model or other families.
    pub pillar_schema: Option<&'a crate::PillarSchema>,
    /// Emit basic singles, list singles, then their Cartesian product.
    pub joint_moves: bool,
}
impl Default for MixedNeighborhood<'_> {
    fn default() -> Self {
        Self {
            basic: Some(Neighborhood::ChangeAndSwap),
            lists: Some(Neighborhood::ChangeAndSwap),
            basic_variables: None,
            pillar_schema: None,
            joint_moves: false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedNeighborhoodError {
    Model(MixedError),
    Pillar(crate::PillarError),
    SchemaRequiresPillarNeighborhood,
    UnknownBasicVariable { variable: usize, count: usize },
    DuplicateBasicVariable { variable: usize },
}
impl std::fmt::Display for MixedNeighborhoodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Pillar(e) => e.fmt(f),
            Self::SchemaRequiresPillarNeighborhood => {
                f.write_str("schema override requires a Pillar or SubPillar neighborhood")
            }
            Self::UnknownBasicVariable { variable, count } => {
                write!(f, "basic variable {variable} outside {count} slots")
            }
            Self::DuplicateBasicVariable { variable } => {
                write!(f, "basic variable {variable} selected more than once")
            }
        }
    }
}
impl std::error::Error for MixedNeighborhoodError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Pillar(e) => Some(e),
            _ => None,
        }
    }
}
impl From<MixedError> for MixedNeighborhoodError {
    fn from(e: MixedError) -> Self {
        Self::Model(e)
    }
}
impl From<crate::BasicNeighborhoodError> for MixedNeighborhoodError {
    fn from(e: crate::BasicNeighborhoodError) -> Self {
        match e {
            crate::BasicNeighborhoodError::Model(e) => Self::Model(MixedError::Basic(e)),
            crate::BasicNeighborhoodError::Pillar(e) => Self::Pillar(e),
            crate::BasicNeighborhoodError::SchemaRequiresPillarNeighborhood => {
                Self::SchemaRequiresPillarNeighborhood
            }
            crate::BasicNeighborhoodError::UnknownVariable { variable, count } => {
                Self::UnknownBasicVariable { variable, count }
            }
            crate::BasicNeighborhoodError::DuplicateVariable { variable } => {
                Self::DuplicateBasicVariable { variable }
            }
        }
    }
}
impl MixedNeighborhood<'_> {
    pub(crate) fn validate<B: Eq + Clone, L: Eq + Clone>(
        self,
        p: &MixedProblem<B, L>,
    ) -> Result<(), MixedNeighborhoodError> {
        if let Some(n) = self.basic {
            crate::BasicNeighborhood {
                neighborhood: n,
                variables: self.basic_variables,
                pillar_schema: self.pillar_schema,
            }
            .validate(p.basic())
            .map_err(MixedNeighborhoodError::from)?;
        }
        if let Some(n) = self.lists {
            n.validate_list(p.lists()).map_err(MixedError::Lists)?;
        }
        Ok(())
    }
    pub(crate) fn at_step(self, step: u64) -> Self {
        Self {
            basic: self.basic.map(|n| n.at_step(step)),
            lists: self.lists.map(|n| n.at_step(step)),
            ..self
        }
    }
}
/// Enumerate separate basic/list families, retaining domains, pins and the same
/// transactional mixed moves used by local-search providers. Disabled components
/// skip their configuration validation; assignment validation always runs.
///
/// ```
/// use timetuck::*;
/// let p=MixedProblem::new(
///     FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0)])?,
///     ListProblem::new(vec![0,1],vec![ListEntity::new(vec![0,1])])?);
/// let a=p.initial_assignment();
/// let c=MixedNeighborhood{basic:None,lists:Some(Neighborhood::Swap),..Default::default()};
/// let moves=mixed_moves_configured(&p,&a,c)?.collect::<Vec<_>>();
/// assert!(!moves.is_empty());
/// assert!(moves.iter().all(|m|m.basic.touched_variables().is_empty()));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn mixed_moves_configured<'a, B: Eq + Clone, L: Eq + Clone>(
    p: &'a MixedProblem<B, L>,
    a: &'a MixedAssignment<B, L>,
    c: MixedNeighborhood<'a>,
) -> Result<impl Iterator<Item = MixedMove<B, L>> + 'a, MixedNeighborhoodError> {
    p.validate(a)?;
    c.validate(p)?;
    Ok(configured(p, a, c))
}
fn configured<'a, B: Eq + Clone, L: Eq + Clone>(
    p: &'a MixedProblem<B, L>,
    a: &'a MixedAssignment<B, L>,
    c: MixedNeighborhood<'a>,
) -> impl Iterator<Item = MixedMove<B, L>> + 'a {
    let basic = move || -> Box<dyn Iterator<Item = Move<B>> + 'a> {
        let Some(n) = c.basic else {
            return Box::new(std::iter::empty());
        };
        crate::basic_neighborhood::configured(
            p.basic(),
            a.basic(),
            crate::BasicNeighborhood {
                neighborhood: n,
                variables: c.basic_variables,
                pillar_schema: c.pillar_schema,
            },
        )
    };
    let lists = move || -> Box<dyn Iterator<Item = ListMove<L>> + 'a> {
        match c.lists {
            None => Box::new(std::iter::empty()),
            Some(n) => crate::list_neighborhood::ordered(
                p.lists(),
                a.lists(),
                n,
                0..p.lists().entities().len(),
            ),
        }
    };
    let singles = basic()
        .map(|basic| MixedMove {
            basic,
            lists: ListMove::Composite(vec![]),
        })
        .chain(lists().map(|lists| MixedMove {
            basic: Move::Composite(vec![]),
            lists,
        }));
    let joint = c
        .joint_moves
        .then(move || {
            basic().flat_map(move |basic| {
                lists().map(move |lists| MixedMove {
                    basic: basic.clone(),
                    lists,
                })
            })
        })
        .into_iter()
        .flatten();
    singles.chain(joint)
}
/// A weighted union of independently configured mixed families. Each child may
/// disable either component or select a different set of basic variables. Zero
/// weights disable validation/enumeration; duplicate children remain distinct.
/// The step index derives both union and sampled-neighborhood seeds, as in native
/// local search. This can be returned lazily from a MoveProvider implementation.
pub fn mixed_moves_configured_union<'a, B: Eq + Clone, L: Eq + Clone>(
    p: &'a MixedProblem<B, L>,
    a: &'a MixedAssignment<B, L>,
    children: &'a [(MixedNeighborhood<'a>, u32)],
    order: crate::UnionOrder,
    step: u64,
) -> Result<impl Iterator<Item = MixedMove<B, L>> + 'a, MixedNeighborhoodError> {
    p.validate(a)?;
    for &(c, w) in children {
        if w > 0 {
            c.validate(p)?;
        }
    }
    let order = match order {
        crate::UnionOrder::Random { seed } => crate::UnionOrder::Random {
            seed: seed.wrapping_add(step),
        },
        order => order,
    };
    Ok(crate::MoveUnion::new(
        children
            .iter()
            .filter(|(_, w)| *w > 0)
            .map(move |&(c, w)| (configured(p, a, c.at_step(step)), w)),
        order,
    ))
}

fn basics<'a, B: Eq + Clone, L: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    order: I,
) -> impl Iterator<Item = Move<B>> + 'a {
    neighborhood::ordered(problem.basic(), assignment.basic(), neighborhood, order)
}

/// Basic moves (changes/swaps or whole pillars), then list moves, then (when
/// requested) every pair of one basic and one list move. Each component retains its ordering, range,
/// optionality and pin rules. Joint moves use the same immutable starting state;
/// component independence makes their combined final state valid. Complete input
/// stays complete. Ordered swaps can produce duplicate successor states, as in
/// the component neighborhoods. Enumeration is lazy, including the product.
pub fn mixed_moves<'a, B: Eq + Clone, L: Eq + Clone>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    joint_moves: bool,
) -> Result<impl Iterator<Item = MixedMove<B, L>> + 'a, MixedError> {
    problem.validate(assignment)?;
    neighborhood
        .validate_basic(problem.basic())
        .map_err(MixedError::Basic)?;
    neighborhood
        .validate_list(problem.lists())
        .map_err(MixedError::Lists)?;
    Ok(ordered(
        problem,
        assignment,
        neighborhood,
        joint_moves,
        0..problem.basic().variables().len(),
        0..problem.lists().entities().len(),
    ))
}

pub(crate) fn ordered<
    'a,
    B: Eq + Clone,
    L: Eq + Clone,
    I: Iterator<Item = usize> + Clone + 'a,
    J: Iterator<Item = usize> + Clone + 'a,
>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    joint_moves: bool,
    basic_order: I,
    list_order: J,
) -> impl Iterator<Item = MixedMove<B, L>> + 'a {
    let basic =
        basics(problem, assignment, neighborhood, basic_order.clone()).map(|basic| MixedMove {
            basic,
            lists: ListMove::Composite(vec![]),
        });
    let lists = crate::list_neighborhood::ordered(
        problem.lists(),
        assignment.lists(),
        neighborhood,
        list_order.clone(),
    )
    .map(|lists| MixedMove {
        basic: Move::Composite(vec![]),
        lists,
    });
    let joint = joint_moves
        .then(move || {
            basics(problem, assignment, neighborhood, basic_order).flat_map(move |basic| {
                crate::list_neighborhood::ordered(
                    problem.lists(),
                    assignment.lists(),
                    neighborhood,
                    list_order.clone(),
                )
                .map(move |lists| MixedMove {
                    basic: basic.clone(),
                    lists,
                })
            })
        })
        .into_iter()
        .flatten();
    basic.chain(lists).chain(joint)
}

// Basic variable i and list entity i must never collide in variable tabu memory.
pub(crate) fn tabu_keys<B, L>(movement: &MixedMove<B, L>, basic_count: usize) -> Vec<usize> {
    let mut keys = movement.basic.touched_variables();
    keys.extend(movement.lists.touched_entities().into_iter().map(|entity| {
        basic_count
            .checked_add(entity)
            .expect("mixed tabu key fits allocated model")
    }));
    keys
}
