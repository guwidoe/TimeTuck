//! Entity-row swaps over aligned selected basic-variable columns.
use crate::{
    Assignment, FiniteProblem, ListMove, MixedAssignment, MixedMove, MixedProblem, ModelError,
    Move, MoveError, MoveProvider, Pillar, PillarError, random::SplitMix64,
};
use std::{error::Error, fmt};

/// Ordered row IDs. Unordered visits pairs of positions in one order; Cartesian
/// visits both orientations when present; Paired zips independently pin-filtered
/// orders. Repeated row IDs preserve input multiplicity; self-pairs are omitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntitySwapOrder {
    Unordered(Vec<usize>),
    Cartesian { left: Vec<usize>, right: Vec<usize> },
    Paired { left: Vec<usize>, right: Vec<usize> },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntitySwapError {
    Model(ModelError),
    Schema(PillarError),
    UnknownEntity { entity: usize, count: usize },
}
impl fmt::Display for EntitySwapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity swap: {self:?}")
    }
}
impl Error for EntitySwapError {}
/// Captured row identities and their atomic ordinary move. Rebuild after edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitySwapCandidate<V> {
    pub left: usize,
    pub right: usize,
    pub movement: Move<V>,
}
/// Swap aligned variable columns of two entities without grouping equal-valued
/// entities into a pillar. Row order identifies entities and is retained. Rows
/// must be disjoint, nonempty and equally wide; an empty schema is allowed.
/// A pin in any selected column excludes its row. Unselected columns are untouched.
/// Each changed value must fit the opposite variable's domain; transferring None
/// requires that recipient to allow unassignment. Equal selected tuples yield
/// no-ops only when requested, including required variables already both missing.
///
/// Deterministic selection is lazy, finite and preserves configured order. Optional
/// sampling draws pairs with replacement from the same pin-filtered orders and
/// considers at most the configured attempts. Invalid and unchanged pairs consume
/// attempts. Reopening uses seed+completed_steps (wrapping), so partial consumption
/// does not alter later openings. Empty dimensions draw nothing. A missed sample
/// does not prove local optimality, and native random traces differ from Java.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1,2]).with_initial(1),
///     Variable::new(vec![1,2]).with_initial(2)])?;
/// let provider = EntitySwapProvider::new(vec![vec![0],vec![1]],
///     EntitySwapOrder::Unordered(vec![0,1]))?;
/// assert_eq!(provider.select(&p, &p.initial_assignment(), 0)?.count(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct EntitySwapProvider {
    rows: Vec<Vec<usize>>,
    order: EntitySwapOrder,
    include_noops: bool,
    sampling: Option<(u64, u64)>,
}
impl EntitySwapProvider {
    pub fn new(rows: Vec<Vec<usize>>, order: EntitySwapOrder) -> Result<Self, PillarError> {
        if !rows.is_empty() {
            Pillar::from_rows(rows.clone())?;
        }
        Ok(Self {
            rows,
            order,
            include_noops: false,
            sampling: None,
        })
    }
    pub fn with_noops(mut self, include: bool) -> Self {
        self.include_noops = include;
        self
    }
    pub fn with_sampling(mut self, seed: u64, attempts_per_step: u64) -> Self {
        self.sampling = Some((seed, attempts_per_step));
        self
    }
    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
    pub(crate) fn orders<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
    ) -> Result<(Vec<usize>, Vec<usize>), EntitySwapError> {
        p.validate(a).map_err(EntitySwapError::Model)?;
        for &variable in self.rows.iter().flatten() {
            if variable >= p.variables().len() {
                return Err(EntitySwapError::Schema(PillarError::Move(MoveError {
                    variable,
                    variable_count: p.variables().len(),
                })));
            }
        }
        let filter = |order: &[usize]| -> Result<Vec<usize>, EntitySwapError> {
            let mut out = vec![];
            for &entity in order {
                let row = self
                    .rows
                    .get(entity)
                    .ok_or(EntitySwapError::UnknownEntity {
                        entity,
                        count: self.rows.len(),
                    })?;
                if !row.iter().any(|&v| p.variables()[v].is_pinned()) {
                    out.push(entity);
                }
            }
            Ok(out)
        };
        match &self.order {
            EntitySwapOrder::Unordered(order) => {
                let order = filter(order)?;
                Ok((order.clone(), order))
            }
            EntitySwapOrder::Cartesian { left, right }
            | EntitySwapOrder::Paired { left, right } => Ok((filter(left)?, filter(right)?)),
        }
    }
    /// Upper bound after pin filtering, before self/range/no-op rejection. This
    /// counts the configured pair space, not the length of a sampled iterator.
    pub fn pair_count_upper_bound<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
    ) -> Result<u128, EntitySwapError> {
        let (left, right) = self.orders(p, a)?;
        let n = left.len() as u128;
        Ok(match self.order {
            EntitySwapOrder::Unordered(_) => n * n.saturating_sub(1) / 2,
            EntitySwapOrder::Cartesian { .. } => n * right.len() as u128,
            EntitySwapOrder::Paired { .. } => left.len().min(right.len()) as u128,
        })
    }
    pub(crate) fn compatible<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
        left: usize,
        right: usize,
    ) -> bool {
        let slots = self.rows[left].iter().zip(&self.rows[right]);
        let accepts = |to: usize, from: usize| {
            if a.values()[to] == a.values()[from] {
                return true;
            }
            match &a.values()[from] {
                Some(v) => p.variables()[to].domain().contains(v),
                None => p.variables()[to].allows_unassigned(),
            }
        };
        if !slots.clone().all(|(&l, &r)| accepts(l, r) && accepts(r, l)) {
            return false;
        }
        true
    }
    fn candidate<V: Clone + Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
        left: usize,
        right: usize,
    ) -> Option<EntitySwapCandidate<V>> {
        if left == right {
            return None;
        }
        let slots = self.rows[left].iter().zip(&self.rows[right]);
        let changed = slots.clone().any(|(&l, &r)| a.values()[l] != a.values()[r]);
        if !changed && !self.include_noops {
            return None;
        }
        if !self.compatible(p, a, left, right) {
            return None;
        }
        Some(EntitySwapCandidate {
            left,
            right,
            movement: Move::Composite(
                slots
                    .map(|(&left, &right)| Move::Swap { left, right })
                    .collect(),
            ),
        })
    }
    /// Validate the model/schema/orders at opening; generate candidates on demand.
    pub fn select<'a, V: Clone + Eq>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        completed_steps: u64,
    ) -> Result<impl Iterator<Item = EntitySwapCandidate<V>> + 'a, EntitySwapError> {
        let (left, right) = self.orders(p, a)?;
        let unordered = matches!(self.order, EntitySwapOrder::Unordered(_));
        let paired = matches!(self.order, EntitySwapOrder::Paired { .. });
        let pairs: Box<dyn Iterator<Item = (usize, usize)> + 'a> =
            if let Some((seed, attempts)) = self.sampling {
                let mut random = SplitMix64::new(seed.wrapping_add(completed_steps));
                let n = if paired {
                    left.len().min(right.len())
                } else {
                    left.len()
                };
                let attempts = if n == 0 || right.is_empty() || (unordered && n < 2) {
                    0
                } else {
                    attempts
                };
                Box::new((0..attempts).map(move |_| {
                    let mut i = if n == 1 {
                        0
                    } else {
                        random.below_wide(n as u128) as usize
                    };
                    let mut j = if paired {
                        i
                    } else if unordered {
                        let mut j = random.below_wide((n - 1) as u128) as usize;
                        if j >= i {
                            j += 1;
                        }
                        j
                    } else if right.len() == 1 {
                        0
                    } else {
                        random.below_wide(right.len() as u128) as usize
                    };
                    if unordered && i > j {
                        std::mem::swap(&mut i, &mut j);
                    }
                    (left[i], right[j])
                }))
            } else {
                let (mut i, mut j) = (0, usize::from(unordered));
                Box::new(std::iter::from_fn(move || {
                    if i >= left.len() || j >= right.len() {
                        return None;
                    }
                    let result = (left[i], right[j]);
                    if paired {
                        i += 1;
                        j += 1;
                    } else {
                        j += 1;
                        if j == right.len() {
                            i += 1;
                            j = if unordered { i + 1 } else { 0 };
                        }
                    }
                    Some(result)
                }))
            };
        Ok(pairs.filter_map(move |(l, r)| self.candidate(p, a, l, r)))
    }
}
impl<V: Clone + Eq> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>> for EntitySwapProvider {
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
            self.select(p, a, step)
                .expect("entity swap provider requires valid model/schema/orders")
                .map(|c| c.movement),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for EntitySwapProvider
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
        p.validate(a)
            .expect("entity swap provider requires valid mixed assignment");
        Box::new(
            self.select(p.basic(), a.basic(), step)
                .expect("entity swap provider requires valid schema/orders")
                .map(|c| MixedMove {
                    basic: c.movement,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
