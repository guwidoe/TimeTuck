//! Dependent entity partners and bounded random swaps with a no-progress fallback.
use crate::{
    Assignment, EntitySwapCandidate, EntitySwapError, EntitySwapOrder, EntitySwapProvider,
    FiniteProblem, ListMove, MixedAssignment, MixedMove, MixedProblem, Move, MoveProvider,
    PillarError, random::SplitMix64,
};
use std::{error::Error, fmt};

/// Every output draws one left occurrence and probes at most this many right
/// occurrences. Rejected probes consume the budget. Reopening uses seed+step.
#[derive(Clone, Copy, Debug)]
pub struct EntitySwapSampling {
    pub seed: u64,
    pub max_candidates: u64,
    pub max_partner_attempts: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidEntitySwapDraw {
    pub index: usize,
    pub size: usize,
}
impl fmt::Display for InvalidEntitySwapDraw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "entity swap draw {} is outside 0..{}",
            self.index, self.size
        )
    }
}
impl Error for InvalidEntitySwapDraw {}

fn draw_index(
    size: usize,
    draw: &mut impl FnMut(usize) -> usize,
) -> Result<usize, InvalidEntitySwapDraw> {
    let index = draw(size);
    if index < size {
        Ok(index)
    } else {
        Err(InvalidEntitySwapDraw { index, size })
    }
}

/// Sample a left entity, then search for a compatible right entity. If the
/// bounded search fails, emit a self-swap of that left entity. This fallback is
/// deliberately a no-op, not evidence that no improving move exists.
///
/// Rows use aligned, disjoint basic-variable columns. Pins exclude whole rows.
/// Orders retain duplicate occurrences. Random partners must accept every incoming
/// assigned value and share at least one value in an aligned column's domains.
/// Consequently two entirely missing rows with disjoint domains are not random
/// partners, while a common optional column can make missing multi-column rows
/// reachable. Distinct equal-valued rows may produce no-ops.
///
/// Finite partner cursors apply current-value legality only; missing equal values
/// remain compatible even if their domains are disjoint. Random probes additionally
/// apply range overlap. Each scan borrows its assignment; reopen after changes.
/// Storage is linear in the input orders, never the Cartesian product. No Java
/// random trace equivalence is promised. Use EntitySwapProvider for bounded pair
/// sampling that omits failed attempts instead of yielding fallback self-swaps.
#[derive(Clone, Debug)]
pub struct ReachableEntitySwapProvider {
    schema: EntitySwapProvider,
    sampling: EntitySwapSampling,
}
impl ReachableEntitySwapProvider {
    pub fn new(
        rows: Vec<Vec<usize>>,
        left: Vec<usize>,
        right: Vec<usize>,
        sampling: EntitySwapSampling,
    ) -> Result<Self, PillarError> {
        Ok(Self {
            schema: EntitySwapProvider::new(rows, EntitySwapOrder::Cartesian { left, right })?,
            sampling,
        })
    }
    pub fn rows(&self) -> &[Vec<usize>] {
        self.schema.rows()
    }
    /// Configured Cartesian size after pins, before reachability filtering. A
    /// positive candidate budget may still yield self-swaps when the right is empty.
    pub fn pair_count_upper_bound<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
    ) -> Result<u128, EntitySwapError> {
        self.schema.pair_count_upper_bound(p, a)
    }
    /// Bind an explicit recorded left identity. It need not occur in the configured
    /// left order. A pinned left yields no partners, including no random fallback.
    pub fn partners<'a, V: Eq>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        left: usize,
    ) -> Result<EntitySwapPartners<'a, V>, EntitySwapError> {
        let (_, right) = self.schema.orders(p, a)?;
        let row = self
            .rows()
            .get(left)
            .ok_or(EntitySwapError::UnknownEntity {
                entity: left,
                count: self.rows().len(),
            })?;
        Ok(EntitySwapPartners {
            provider: self,
            problem: p,
            assignment: a,
            left,
            right,
            active: !row.iter().any(|&v| p.variables()[v].is_pinned()),
        })
    }
    fn reachable<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
        left: usize,
        right: usize,
    ) -> bool {
        left != right
            && self.schema.compatible(p, a, left, right)
            && self.rows()[left]
                .iter()
                .zip(&self.rows()[right])
                .any(|(&l, &r)| {
                    p.variables()[l]
                        .domain()
                        .iter()
                        .any(|v| p.variables()[r].contains_value(v))
                })
    }
    fn movement<V>(&self, left: usize, right: usize) -> EntitySwapCandidate<V> {
        EntitySwapCandidate {
            left,
            right,
            movement: Move::Composite(
                self.rows()[left]
                    .iter()
                    .zip(&self.rows()[right])
                    .map(|(&left, &right)| Move::Swap { left, right })
                    .collect(),
            ),
        }
    }
    /// Inject bounded indices for deterministic sampling. Each callback receives
    /// the pin-filtered order length, including one-element orders. An invalid
    /// index yields one error then ends the iterator; callback panics propagate.
    pub fn select_with<'a, V: Eq, F: FnMut(usize) -> usize + 'a>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        mut draw: F,
    ) -> Result<
        impl Iterator<Item = Result<EntitySwapCandidate<V>, InvalidEntitySwapDraw>> + 'a,
        EntitySwapError,
    > {
        let (left, right) = self.schema.orders(p, a)?;
        let mut remaining = if left.is_empty() {
            0
        } else {
            self.sampling.max_candidates
        };
        Ok(std::iter::from_fn(move || {
            if remaining == 0 {
                return None;
            }
            remaining -= 1;
            let result = (|| {
                let l = left[draw_index(left.len(), &mut draw)?];
                for _ in 0..self.sampling.max_partner_attempts {
                    if right.is_empty() {
                        break;
                    }
                    let r = right[draw_index(right.len(), &mut draw)?];
                    if self.reachable(p, a, l, r) {
                        return Ok(self.movement(l, r));
                    }
                }
                Ok(self.movement(l, l))
            })();
            if result.is_err() {
                remaining = 0;
            }
            Some(result)
        }))
    }
    pub fn select<'a, V: Eq>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Result<impl Iterator<Item = EntitySwapCandidate<V>> + 'a, EntitySwapError> {
        let mut random = SplitMix64::new(self.sampling.seed.wrapping_add(step));
        Ok(self
            .select_with(p, a, move |n| random.below_wide(n as u128) as usize)?
            .map(|r| r.expect("native bounded random draw")))
    }
}

/// Immutable dependent selection. Input rows and assignment cannot change while
/// this view is alive. Independent finite and random traversals may be reopened.
pub struct EntitySwapPartners<'a, V> {
    provider: &'a ReachableEntitySwapProvider,
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    left: usize,
    right: Vec<usize>,
    active: bool,
}
impl<V: Eq> EntitySwapPartners<'_, V> {
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.right.iter().copied().filter(|&r| {
            self.active
                && self.left != r
                && self
                    .provider
                    .schema
                    .compatible(self.problem, self.assignment, self.left, r)
        })
    }
    /// Materialize only this left entity's finite partners for reversible traversal.
    pub fn cursor(&self) -> EntitySwapPartnerCursor {
        EntitySwapPartnerCursor {
            partners: self.iter().collect(),
            position: 0,
        }
    }
    /// Probe raw right occurrences, then apply a caller's eligibility filter.
    /// Rejections consume attempts. Exhaustion returns the left itself even when
    /// excluded by that filter. None means the left is pinned; no draws then occur.
    pub fn sample_with(
        &self,
        attempts: u64,
        mut draw: impl FnMut(usize) -> usize,
        mut accept: impl FnMut(usize) -> bool,
    ) -> Result<Option<usize>, InvalidEntitySwapDraw> {
        if !self.active {
            return Ok(None);
        }
        for _ in 0..attempts {
            if self.right.is_empty() {
                break;
            }
            let right = self.right[draw_index(self.right.len(), &mut draw)?];
            if accept(right)
                && self
                    .provider
                    .reachable(self.problem, self.assignment, self.left, right)
            {
                return Ok(Some(right));
            }
        }
        Ok(Some(self.left))
    }
}

/// Owned identity snapshot with a reversible position between items. It retains
/// duplicate occurrences. Rebuild after model/assignment/eligibility changes.
#[derive(Clone, Debug)]
pub struct EntitySwapPartnerCursor {
    partners: Vec<usize>,
    position: usize,
}
impl EntitySwapPartnerCursor {
    pub fn has_next(&self) -> bool {
        self.position < self.partners.len()
    }
    pub fn has_previous(&self) -> bool {
        self.position > 0
    }
    pub fn next_partner(&mut self) -> Option<usize> {
        let result = self.partners.get(self.position).copied()?;
        self.position += 1;
        Some(result)
    }
    pub fn previous_partner(&mut self) -> Option<usize> {
        self.position = self.position.checked_sub(1)?;
        Some(self.partners[self.position])
    }
    /// Reset to the initial gap, equivalent to opening at index zero.
    pub fn rewind(&mut self) {
        self.position = 0;
    }
}

impl<V: Eq> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>> for ReachableEntitySwapProvider {
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
                .expect("reachable entity swaps require a valid model/schema/orders")
                .map(|c| c.movement),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for ReachableEntitySwapProvider
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
            .expect("reachable entity swaps require a valid mixed assignment");
        Box::new(
            self.select(p.basic(), a.basic(), step)
                .expect("reachable entity swaps require a valid schema/orders")
                .map(|c| MixedMove {
                    basic: c.movement,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
