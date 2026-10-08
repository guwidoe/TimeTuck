//! Configurable basic-variable families with independent scopes and pillar schemas.
use crate::{Assignment, FiniteProblem, ModelError, Move, Neighborhood, PillarError, PillarSchema};

/// One basic move family. A variable selection defines encounter order and limits
/// every candidate to those slots. Schema rows remain atomic for grouping/pinning;
/// a column selection never splits a row swap. None retains the model's schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasicNeighborhood<'a> {
    pub neighborhood: Neighborhood,
    pub variables: Option<&'a [usize]>,
    /// Only Pillar and SubPillar families accept an override.
    pub pillar_schema: Option<&'a PillarSchema>,
    /// Per-variable complete target permutations; unspecified slots keep domain order.
    pub value_orders: &'a [crate::BasicValueOrder],
}
impl Default for BasicNeighborhood<'_> {
    fn default() -> Self {
        Self {
            neighborhood: Neighborhood::ChangeAndSwap,
            variables: None,
            pillar_schema: None,
            value_orders: &[],
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicNeighborhoodError {
    Model(ModelError),
    Pillar(PillarError),
    ValueOrder(crate::BasicValueOrderError),
    ValueOrderingUnsupported,
    SchemaRequiresPillarNeighborhood,
    UnknownVariable { variable: usize, count: usize },
    DuplicateVariable { variable: usize },
}
impl std::fmt::Display for BasicNeighborhoodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Pillar(e) => e.fmt(f),
            Self::ValueOrder(e) => e.fmt(f),
            Self::ValueOrderingUnsupported => {
                f.write_str("value ordering requires a single-target change or pillar family")
            }
            Self::SchemaRequiresPillarNeighborhood => {
                f.write_str("schema override requires a Pillar or SubPillar neighborhood")
            }
            Self::UnknownVariable { variable, count } => {
                write!(f, "basic variable {variable} outside {count} slots")
            }
            Self::DuplicateVariable { variable } => {
                write!(f, "basic variable {variable} selected more than once")
            }
        }
    }
}
impl std::error::Error for BasicNeighborhoodError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::Pillar(e) => Some(e),
            Self::ValueOrder(e) => Some(e),
            _ => None,
        }
    }
}
impl BasicNeighborhood<'_> {
    pub(crate) fn validate<V: Eq>(
        self,
        p: &FiniteProblem<V>,
    ) -> Result<(), BasicNeighborhoodError> {
        self.neighborhood
            .validate_basic(p)
            .map_err(BasicNeighborhoodError::Model)?;
        if !self.value_orders.is_empty() {
            if !matches!(
                self.neighborhood,
                Neighborhood::Change
                    | Neighborhood::ChangeAndSwap
                    | Neighborhood::ChangeAndUniqueSwap
                    | Neighborhood::Assign
                    | Neighborhood::Unassign
                    | Neighborhood::Pillar { .. }
                    | Neighborhood::SubPillar(_)
                    | Neighborhood::PillarChange(_)
                    | Neighborhood::PillarUnassign
            ) {
                return Err(BasicNeighborhoodError::ValueOrderingUnsupported);
            }
            crate::basic_value_order::validate(p, self.value_orders)
                .map_err(BasicNeighborhoodError::ValueOrder)?;
        }
        if let Some(schema) = self.pillar_schema {
            if !matches!(
                self.neighborhood,
                Neighborhood::Pillar { .. } | Neighborhood::SubPillar(_)
            ) {
                return Err(BasicNeighborhoodError::SchemaRequiresPillarNeighborhood);
            }
            schema
                .validate(p.variables().len())
                .map_err(BasicNeighborhoodError::Pillar)?;
        }
        if let Some(selected) = self.variables {
            for (i, &variable) in selected.iter().enumerate() {
                if variable >= p.variables().len() {
                    return Err(BasicNeighborhoodError::UnknownVariable {
                        variable,
                        count: p.variables().len(),
                    });
                }
                if selected[..i].contains(&variable) {
                    return Err(BasicNeighborhoodError::DuplicateVariable { variable });
                }
            }
        }
        Ok(())
    }
    pub(crate) fn at_step(self, step: u64) -> Self {
        Self {
            neighborhood: self.neighborhood.at_step(step),
            ..self
        }
    }
}
/// Enumerate a selected family without cloning or modifying the model. Pins,
/// domains and optionality still apply. Assignment validation always runs.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0);2])?;
/// let a=p.initial_assignment();
/// let c=BasicNeighborhood{neighborhood:Neighborhood::Change,variables:Some(&[1]),..Default::default()};
/// let moves=basic_moves_configured(&p,&a,c)?.collect::<Vec<_>>();
/// assert_eq!(moves,vec![Move::Change{variable:1,value:Some(1)}]);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn basic_moves_configured<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    c: BasicNeighborhood<'a>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, BasicNeighborhoodError> {
    p.validate(a).map_err(BasicNeighborhoodError::Model)?;
    c.validate(p)?;
    Ok(configured(p, a, c))
}
pub(crate) fn configured<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    c: BasicNeighborhood<'a>,
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    let order = c
        .variables
        .map_or_else(|| (0..p.variables().len()).collect(), |s| s.to_vec());
    let moves = if let Some(schema) = c.pillar_schema {
        let config = match c.neighborhood {
            Neighborhood::Pillar { swaps } => crate::pillar_schema::Settings {
                left: None,
                right: None,
                changes: crate::PillarChangeMode::All,
                swaps,
            },
            Neighborhood::SubPillar(n) => crate::pillar_schema::Settings {
                left: Some(n.selection),
                right: n.swap_selection,
                changes: n.changes,
                swaps: n.swap_selection.is_some(),
            },
            _ => unreachable!("schema neighborhood validated"),
        };
        Box::new(crate::pillar_schema::ordered_with_schema(
            p,
            a,
            config,
            order.into_iter(),
            Some(schema),
            c.value_orders,
        )) as Box<dyn Iterator<Item = Move<V>> + 'a>
    } else {
        crate::neighborhood::ordered_with_values(
            p,
            a,
            c.neighborhood,
            order.into_iter(),
            c.value_orders,
        )
    };
    Box::new(moves.filter(move |m| {
        c.variables
            .is_none_or(|s| m.touched_variables().iter().all(|i| s.contains(i)))
    }))
}
/// Weighted union retaining each child's scope, schema and duplicate occurrences.
/// Zero-weight children are disabled and skip configuration validation. Step
/// indices derive sampled-family/union seeds without changing the supplied config.
pub fn basic_moves_configured_union<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    children: &'a [(BasicNeighborhood<'a>, u32)],
    order: crate::UnionOrder,
    step: u64,
) -> Result<impl Iterator<Item = Move<V>> + 'a, BasicNeighborhoodError> {
    p.validate(a).map_err(BasicNeighborhoodError::Model)?;
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
