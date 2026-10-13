//! Schema inference bound to repeating executable sublist proposal sources.
use crate::{
    ListAssignment, ListMove, ListProblem, ListSelectionContext, MixedAssignment, MixedMove,
    MixedProblem, MoveProvider, PlanningSchema, SchemaMoveConfig, SchemaMoveError, SchemaMoveKind,
    SchemaMoveProvider, SchemaVariableSlots, SelectionSource, SubListChangeConfig,
    SubListSwapConfig,
};
use std::{num::NonZeroUsize, sync::Arc};

#[derive(Clone, Debug)]
pub struct SchemaSubListFactory {
    pub config: SchemaMoveConfig,
}
impl SchemaSubListFactory {
    pub fn change() -> Self {
        Self {
            config: SchemaMoveConfig::new(SchemaMoveKind::SubListChange(
                SubListChangeConfig::default(),
            )),
        }
    }
    pub fn swap() -> Self {
        Self {
            config: SchemaMoveConfig::new(
                SchemaMoveKind::SubListSwap(SubListSwapConfig::default()),
            ),
        }
    }
    pub fn with_reversing(mut self, enabled: bool) -> Self {
        self.config.kind = match self.config.kind {
            SchemaMoveKind::SubListChange(c) => {
                SchemaMoveKind::SubListChange(c.with_reversing(enabled))
            }
            SchemaMoveKind::SubListSwap(c) => {
                SchemaMoveKind::SubListSwap(c.with_reversing(enabled))
            }
            other => other,
        };
        self
    }
    /// Infer list fields and bind actual native row slots. Basic fields are
    /// excluded even in multi-entity schemas. Multiple fields remain separate:
    /// a proposal may touch only rows bound to one field. The native flattened
    /// list model uses one Rust payload type; heterogeneous fields require a
    /// caller-owned typed union and matching schema declarations.
    pub fn bind<'s>(
        &self,
        schema: &'s PlanningSchema,
        bindings: &[SchemaVariableSlots<'s>],
        seed: u64,
        proposals_per_step: NonZeroUsize,
        catalog_limit: NonZeroUsize,
    ) -> Result<SchemaSubListSource, SchemaMoveError> {
        let reversing = match self.config.kind {
            SchemaMoveKind::SubListChange(c) => c.reversing(),
            SchemaMoveKind::SubListSwap(c) => c.reversing(),
            _ => return Err(SchemaMoveError::WrongModel),
        };
        let plan = self.config.resolve(schema)?;
        let provider = plan.bind(bindings)?;
        let rows = plan
            .branches()
            .iter()
            .map(|branch| {
                bindings
                    .iter()
                    .find(|b| b.variable == branch.variables[0])
                    .expect("validated field binding")
                    .slots
                    .clone()
            })
            .collect();
        Ok(SchemaSubListSource {
            provider,
            rows,
            reversing,
            kind: self.config.kind,
            random: crate::random::SplitMix64::new(seed),
            proposals_per_step,
            catalog_limit,
        })
    }
}
/// Repeating random selection from real schema-provider sublist candidates.
/// Every opening rebuilds a finite native move catalog from its current snapshot;
/// it does not score/apply candidates. Productive catalogs repeat without a fixed
/// traversal length; empty catalogs stop. Random state continues across openings.
/// This optional factory trades catalog memory for constant-time subsequent draws;
/// ordinary lazy/sampled providers remain unchanged. No Java RNG trace or selector
/// class/lifecycle ABI is implied. Callers supply an explicit catalog limit;
/// a conservative structural bound rejects oversized openings before generation.
/// A backstop stops after at most limit+1 generated candidates and rejects an oversized
/// catalog before scoring or random draws. Solver bindings also cap each opening.
#[derive(Clone, Debug)]
pub struct SchemaSubListSource {
    provider: SchemaMoveProvider,
    rows: Vec<Vec<usize>>,
    reversing: bool,
    kind: SchemaMoveKind,
    random: crate::random::SplitMix64,
    proposals_per_step: NonZeroUsize,
    catalog_limit: NonZeroUsize,
}
impl SchemaSubListSource {
    pub fn is_never_ending(&self) -> bool {
        true
    }
    pub fn selects_reversing(&self) -> bool {
        self.reversing
    }
    pub fn bound_rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
    pub fn try_proposals<'a, V: Eq + Clone + 'static>(
        &'a mut self,
        problem: &'a ListProblem<V>,
        before: &'a ListAssignment<V>,
    ) -> Result<Box<dyn Iterator<Item = ListMove<V>> + 'a>, SchemaMoveError> {
        let limit = self.catalog_limit.get();
        let candidates = self.provider.try_list_moves(problem, before, 0)?;
        let upper = match self.kind {
            SchemaMoveKind::SubListChange(c) => {
                let spans = crate::SubListSelection::new(problem, before, c.size())
                    .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
                let selected = self
                    .rows
                    .iter()
                    .flatten()
                    .fold(0u128, |n, &e| n.saturating_add(spans.entity_len(e)));
                let destinations = before
                    .lists()
                    .iter()
                    .fold(0u128, |n, row| n.saturating_add(row.len() as u128 + 1));
                selected.saturating_mul(
                    destinations
                        .saturating_mul(if c.reversing() { 2 } else { 1 })
                        .saturating_add(u128::from(problem.allows_unassigned())),
                )
            }
            SchemaMoveKind::SubListSwap(c) => {
                let left = crate::SubListSelection::new(problem, before, c.left_size())
                    .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
                let right = crate::SubListSelection::new(problem, before, c.right_size())
                    .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))?;
                self.rows
                    .iter()
                    .flatten()
                    .fold(0u128, |n, &e| n.saturating_add(left.entity_len(e)))
                    .saturating_mul(right.len())
                    .saturating_mul(if c.reversing() { 2 } else { 1 })
            }
            _ => unreachable!("validated sublist factory kind"),
        };
        if upper > limit as u128 {
            return Err(SchemaMoveError::InvalidModel(format!(
                "sublist catalog exceeds caller limit {limit} (conservative structural bound {upper})"
            )));
        }
        let raw = candidates.take(limit.saturating_add(1)).collect::<Vec<_>>();
        if raw.len() > limit {
            return Err(SchemaMoveError::InvalidModel(format!(
                "sublist catalog exceeds caller limit {limit}"
            )));
        }
        let catalog = raw
            .into_iter()
            .filter(|movement| {
                let touched = movement.touched_entities();
                self.rows
                    .iter()
                    .any(|rows| touched.iter().all(|entity| rows.contains(entity)))
            })
            .collect::<Vec<_>>();
        Ok(Box::new(std::iter::from_fn(move || {
            if catalog.is_empty() {
                return None;
            }
            Some(catalog[self.random.below_wide(catalog.len() as u128) as usize].clone())
        })))
    }
}
impl<V: Eq + Clone + 'static> SelectionSource<ListSelectionContext<'_, V>, ListMove<V>>
    for SchemaSubListSource
{
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &ListSelectionContext<'_, V>) -> Option<usize> {
        None
    }
    fn open<'a>(
        &'a mut self,
        c: &'a ListSelectionContext<'_, V>,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        self.try_proposals(c.problem(), c.assignment())
            .expect("valid schema-bound list source")
    }
}
impl<V: Eq + Clone + 'static> SelectionSource<(Arc<ListProblem<V>>, ListAssignment<V>), ListMove<V>>
    for SchemaSubListSource
{
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &(Arc<ListProblem<V>>, ListAssignment<V>)) -> Option<usize> {
        None
    }
    fn open<'a>(
        &'a mut self,
        c: &'a (Arc<ListProblem<V>>, ListAssignment<V>),
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        self.try_proposals(&c.0, &c.1)
            .expect("valid schema-bound list source")
    }
}
impl<V: Eq + Clone + 'static> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for SchemaSubListSource
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        let limit = self.proposals_per_step.get();
        Box::new(
            self.try_proposals(p, a)
                .expect("valid schema-bound list source")
                .take(limit),
        )
    }
}
impl<B: Eq + Clone, L: Eq + Clone + 'static>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for SchemaSubListSource
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        p.validate(a)
            .expect("valid mixed assignment for schema-bound source");
        let limit = self.proposals_per_step.get();
        Box::new(
            self.try_proposals(p.lists(), a.lists())
                .expect("valid schema-bound list source")
                .take(limit)
                .map(|lists| MixedMove {
                    basic: crate::Move::Composite(vec![]),
                    lists,
                }),
        )
    }
}
