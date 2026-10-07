//! Model-aware native default move families.
use crate::{
    Assignment, EntitySwapError, EntitySwapOrder, EntitySwapProvider, FiniteProblem,
    ListAssignment, ListError, ListMove, ListProblem, MixedAssignment, MixedError, MixedMove,
    MixedProblem, ModelError, Move, MoveProvider, MoveUnion, PillarError, UnionOrder,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultMoveFamily {
    Change,
    Assign,
    Unassign,
    Swap,
    ListChange,
    ListSwap,
    TwoOpt,
    TailSwap,
    ListAssign,
    ListUnassign,
}
use DefaultMoveFamily::*;
const BASIC: &[DefaultMoveFamily] = &[Change, Swap];
const OPTIONAL_BASIC: &[DefaultMoveFamily] = &[Change, Assign, Unassign, Swap];
const LIST: &[DefaultMoveFamily] = &[ListChange, ListSwap, TwoOpt, TailSwap];
const OPTIONAL_LIST: &[DefaultMoveFamily] = &[
    ListChange,
    ListSwap,
    TwoOpt,
    TailSwap,
    ListAssign,
    ListUnassign,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultNeighborhoodError {
    Basic(ModelError),
    Lists(ListError),
    EntitySwap(EntitySwapError),
}
impl fmt::Display for DefaultNeighborhoodError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic(e) => e.fmt(f),
            Self::Lists(e) => e.fmt(f),
            Self::EntitySwap(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for DefaultNeighborhoodError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Basic(e) => e,
            Self::Lists(e) => e,
            Self::EntitySwap(e) => e,
        })
    }
}

/// Automatically choose the native default families from model optionality.
/// Basic changes stay between assigned values. Optional slots additionally get
/// separate assign/unassign families. Lists get relocations, swaps, within-list
/// 2-opt and tail swaps (including reversing tails), plus optional assign/unassign.
/// Required missing values should be initialized by construction first.
/// Checked basic_moves/list_moves/mixed_moves return configuration errors; the
/// MoveProvider adapter expects valid assignments and configured entity rows.
///
/// Use as a MoveProvider with the existing *_with_provider search APIs or phase
/// providers. Each opening validates its immutable snapshot. Families open lazily,
/// and every finite family is drained once. Sequential order follows families();
/// seeded random order gives each live family equal weight, preserving order
/// within that family. This is native finite enumeration, not Java random traces.
/// Random seed advances by completed_steps; duplicate successors remain possible.
///
/// Basic slots are independent single-column entities unless with_entity_rows()
/// supplies aligned rows for atomic row swaps. Row pins/domains are respected;
/// rows affect only swap candidates. Other slots still receive change candidates.
/// List structural dependencies are checked on candidate copies when configured.
/// No candidate list is materialized; one next() may scan rejected candidates.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![1,2]).with_initial(1).allowing_unassigned()])?;
/// let a=p.initial_assignment();
/// let mut provider=DefaultNeighborhood::default();
/// assert_eq!(provider.basic_families(&p), &[DefaultMoveFamily::Change,
///     DefaultMoveFamily::Assign, DefaultMoveFamily::Unassign, DefaultMoveFamily::Swap]);
/// assert_eq!(provider.moves(&p,&a,0).count(),2);
/// # Ok::<(), ModelError>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct DefaultNeighborhood {
    order: UnionOrder,
    entity_swaps: Option<EntitySwapProvider>,
}
impl DefaultNeighborhood {
    pub fn new(order: UnionOrder) -> Self {
        Self {
            order,
            entity_swaps: None,
        }
    }
    pub fn with_entity_rows(mut self, rows: Vec<Vec<usize>>) -> Result<Self, PillarError> {
        let order = EntitySwapOrder::Unordered((0..rows.len()).collect());
        self.entity_swaps = Some(EntitySwapProvider::new(rows, order)?);
        Ok(self)
    }
    pub fn basic_families<V: Eq>(&self, p: &FiniteProblem<V>) -> &'static [DefaultMoveFamily] {
        if p.variables().iter().any(|v| v.allows_unassigned()) {
            OPTIONAL_BASIC
        } else {
            BASIC
        }
    }
    pub fn list_families<V: Eq + Clone>(&self, p: &ListProblem<V>) -> &'static [DefaultMoveFamily] {
        if p.allows_unassigned() {
            OPTIONAL_LIST
        } else {
            LIST
        }
    }
    pub fn mixed_families<B: Eq + Clone, V: Eq + Clone>(
        &self,
        p: &MixedProblem<B, V>,
    ) -> Vec<DefaultMoveFamily> {
        self.basic_families(p.basic())
            .iter()
            .chain(self.list_families(p.lists()))
            .copied()
            .collect()
    }
    fn order_at(&self, step: u64) -> UnionOrder {
        match self.order {
            UnionOrder::Sequential => UnionOrder::Sequential,
            UnionOrder::Random { seed } => UnionOrder::Random {
                seed: seed.wrapping_add(step),
            },
        }
    }
    fn validate_basic<V: Eq + Clone>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
    ) -> Result<(), DefaultNeighborhoodError> {
        p.validate(a).map_err(DefaultNeighborhoodError::Basic)?;
        if let Some(swaps) = &self.entity_swaps {
            swaps
                .pair_count_upper_bound(p, a)
                .map_err(DefaultNeighborhoodError::EntitySwap)?;
        }
        Ok(())
    }
    fn basic_family<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        family: DefaultMoveFamily,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
        if family == Swap {
            return match &self.entity_swaps {
                Some(swaps) => Box::new(
                    swaps
                        .select(p, a, step)
                        .expect("validated entity rows")
                        .map(|c| c.movement),
                ),
                None => Box::new(crate::swap_moves(p, a).expect("validated assignment")),
            };
        }
        let slots = (0..p.variables().len()).filter(move |&i| {
            !p.variables()[i].is_pinned()
                && match family {
                    Change => a.values()[i].is_some(),
                    Assign => p.variables()[i].allows_unassigned() && a.values()[i].is_none(),
                    Unassign => p.variables()[i].allows_unassigned() && a.values()[i].is_some(),
                    _ => unreachable!("basic family"),
                }
        });
        Box::new(slots.flat_map(move |i| {
            crate::basic_value_order::targets(p, i, &[], family != Unassign, family == Unassign)
                .filter(move |value| *value != a.values()[i].as_ref())
                .map(move |value| Move::Change {
                    variable: i,
                    value: value.cloned(),
                })
        }))
    }

    fn list_family<'a, V: Eq + Clone>(
        &self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        family: DefaultMoveFamily,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a> {
        let source: Box<dyn Iterator<Item = ListMove<V>> + 'a> = match family {
            ListChange => Box::new(crate::list_change_moves(p, a).expect("validated assignment")),
            ListSwap => Box::new(crate::list_swap_moves(p, a).expect("validated assignment")),
            TwoOpt => {
                Box::new(crate::list_two_opt_moves(p, a, false).expect("validated assignment"))
            }
            TailSwap => {
                Box::new(crate::list_tail_swap_moves(p, a, true).expect("validated assignment"))
            }
            ListAssign => Box::new(crate::list_assign_moves(p, a).expect("validated assignment")),
            ListUnassign => {
                Box::new(crate::list_unassign_moves(p, a).expect("validated assignment"))
            }
            _ => unreachable!("list family"),
        };
        if p.dependencies.is_none() {
            return source;
        }
        Box::new(source.filter(move |movement| {
            let mut copy = a.clone();
            p.apply(&mut copy, movement).is_ok()
        }))
    }
    pub fn basic_moves<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Result<impl Iterator<Item = Move<V>> + 'a, DefaultNeighborhoodError> {
        self.validate_basic(p, a)?;
        let children = self.basic_families(p).iter().map(move |&family| {
            let lazy =
                std::iter::once_with(move || self.basic_family(p, a, family, step)).flatten();
            (lazy, 1)
        });
        Ok(MoveUnion::new(children, self.order_at(step)))
    }
    pub fn list_moves<'a, V: Eq + Clone>(
        &'a self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Result<impl Iterator<Item = ListMove<V>> + 'a, DefaultNeighborhoodError> {
        p.validate(a).map_err(DefaultNeighborhoodError::Lists)?;
        let children = self.list_families(p).iter().map(move |&family| {
            let lazy = std::iter::once_with(move || self.list_family(p, a, family)).flatten();
            (lazy, 1)
        });
        Ok(MoveUnion::new(children, self.order_at(step)))
    }
    pub fn mixed_moves<'a, B: Eq + Clone, V: Eq + Clone>(
        &'a self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        step: u64,
    ) -> Result<impl Iterator<Item = MixedMove<B, V>> + 'a, DefaultNeighborhoodError> {
        p.validate(a).map_err(|e| match e {
            MixedError::Basic(e) => DefaultNeighborhoodError::Basic(e),
            MixedError::Lists(e) => DefaultNeighborhoodError::Lists(e),
        })?;
        self.validate_basic(p.basic(), a.basic())?;
        let children = self.mixed_families(p).into_iter().map(move |family| {
            let lazy =
                std::iter::once_with(move || -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a> {
                    match family {
                        Change | Assign | Unassign | Swap => Box::new(
                            self.basic_family(p.basic(), a.basic(), family, step)
                                .map(|basic| MixedMove {
                                    basic,
                                    lists: ListMove::Composite(vec![]),
                                }),
                        ),
                        _ => {
                            Box::new(self.list_family(p.lists(), a.lists(), family).map(|lists| {
                                MixedMove {
                                    basic: Move::Composite(vec![]),
                                    lists,
                                }
                            }))
                        }
                    }
                })
                .flatten();
            (lazy, 1)
        });
        Ok(MoveUnion::new(children, self.order_at(step)))
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>> for DefaultNeighborhood {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        Box::new(
            self.basic_moves(p, a, step)
                .expect("default provider requires a valid basic assignment and entity rows"),
        )
    }
}
impl<V: Eq + Clone> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for DefaultNeighborhood
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            self.list_moves(p, a, step)
                .expect("default provider requires a valid list assignment"),
        )
    }
}
impl<B: Eq + Clone, V: Eq + Clone>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for DefaultNeighborhood
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        Box::new(
            self.mixed_moves(p, a, step)
                .expect("default provider requires a valid mixed assignment and entity rows"),
        )
    }
}
