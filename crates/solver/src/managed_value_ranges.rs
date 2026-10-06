//! Managed, interned value ranges for one native planning field.
use crate::{EnumeratedValueRange, ListEntity, ListError, ListProblem, Variable};
use std::{cmp::Ordering, error::Error, fmt, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedRangeKind {
    Basic { allows_unassigned: bool },
    List { allows_unassigned: bool },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedRangeTarget {
    Solution,
    Entity(usize),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedRangeError {
    UnknownEntity(usize),
    WrongVariableKind,
    EntityCount { expected: usize, actual: usize },
    List(ListError),
    Model(crate::ModelError),
    CountOverflow,
}
impl fmt::Display for ManagedRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed value range: {self:?}")
    }
}
impl Error for ManagedRangeError {}

type RangeCompare<V> = dyn Fn(&V, &V) -> Ordering + Send + Sync;
/// A reusable sorter identity. Clones retain the identity. Create a new sorter
/// when captured solution facts change; reusing it intentionally reuses the result.
pub struct ManagedRangeOrder<V> {
    compare: Arc<RangeCompare<V>>,
}
impl<V> Clone for ManagedRangeOrder<V> {
    fn clone(&self) -> Self {
        Self {
            compare: self.compare.clone(),
        }
    }
}
impl<V> ManagedRangeOrder<V> {
    pub fn new(compare: impl Fn(&V, &V) -> Ordering + Send + Sync + 'static) -> Self {
        Self {
            compare: Arc::new(compare),
        }
    }
}
struct RangeGroup<V> {
    raw: Arc<EnumeratedValueRange<V>>,
    active: Arc<EnumeratedValueRange<Option<V>>>,
    order: Option<ManagedRangeOrder<V>>,
}
/// A registry for one field, with shared or per-entity providers. Distinct values
/// retain first-encounter order. Equal ordered ranges share snapshots and sorting
/// state. Equality must represent stable planning identity.
///
/// Unsorted distinct provider values remain available through `raw`; `get` returns the
/// current selection order. None is added only for optional basic variables and
/// sorts first without calling the user comparator. Updates affect future reads,
/// while existing Arc snapshots and previously built models remain unchanged.
///
/// ```
/// use timetuck::*;
/// let mut ranges=ManagedValueRanges::entity_provided(
///     ManagedRangeKind::Basic { allows_unassigned: true },
///     vec![vec![vec![1,2],vec![2,3]],vec![vec![1,2]]]);
/// assert_eq!(ranges.raw(ManagedRangeTarget::Solution)?.values(), &[1,2,3]);
/// ranges.sort(ManagedRangeTarget::Entity(0), &ManagedRangeOrder::new(|a: &i32,b: &i32| b.cmp(a)))?;
/// # Ok::<(), ManagedRangeError>(())
/// ```
pub struct ManagedValueRanges<V> {
    kind: ManagedRangeKind,
    groups: Vec<RangeGroup<V>>,
    solution: usize,
    entities: Vec<usize>,
    entity_provided: bool,
}
impl<V: Clone + Eq> ManagedValueRanges<V> {
    /// Combine solution-level providers. The range exists even with no entities.
    pub fn shared(kind: ManagedRangeKind, providers: Vec<Vec<V>>, entity_count: usize) -> Self {
        let mut out = Self {
            kind,
            groups: vec![],
            solution: 0,
            entities: vec![],
            entity_provided: false,
        };
        let slot = out.intern(Self::distinct(providers.into_iter().flatten()));
        out.solution = slot;
        out.entities = vec![slot; entity_count];
        out
    }
    /// Each entity supplies a list of providers. Repeated values in or across
    /// providers are deduplicated; the solution view is their ordered union.
    pub fn entity_provided(kind: ManagedRangeKind, providers: Vec<Vec<Vec<V>>>) -> Self {
        let mut out = Self {
            kind,
            groups: vec![],
            solution: 0,
            entities: vec![],
            entity_provided: true,
        };
        let mut union = vec![];
        for entity in providers {
            let values = Self::distinct(entity.into_iter().flatten());
            union.extend(values.iter().cloned());
            let slot = out.intern(values);
            out.entities.push(slot);
        }
        out.solution = out.intern(Self::distinct(union));
        out
    }
    fn distinct(values: impl IntoIterator<Item = V>) -> Vec<V> {
        let mut out = vec![];
        for value in values {
            if !out.contains(&value) {
                out.push(value);
            }
        }
        out
    }
    fn intern(&mut self, values: Vec<V>) -> usize {
        if let Some(slot) = self.groups.iter().position(|g| g.raw.values() == values) {
            return slot;
        }
        let mut choices = vec![];
        if matches!(
            self.kind,
            ManagedRangeKind::Basic {
                allows_unassigned: true
            }
        ) {
            choices.push(None);
        }
        choices.extend(values.iter().cloned().map(Some));
        let slot = self.groups.len();
        self.groups.push(RangeGroup {
            raw: Arc::new(EnumeratedValueRange::new(values)),
            active: Arc::new(EnumeratedValueRange::new(choices)),
            order: None,
        });
        slot
    }
    fn slot(&self, target: ManagedRangeTarget) -> Result<usize, ManagedRangeError> {
        match target {
            ManagedRangeTarget::Solution => Ok(self.solution),
            ManagedRangeTarget::Entity(i) => self
                .entities
                .get(i)
                .copied()
                .ok_or(ManagedRangeError::UnknownEntity(i)),
        }
    }
    pub fn kind(&self) -> ManagedRangeKind {
        self.kind
    }
    /// Whether each entity supplies its own provider set.
    pub fn has_entity_providers(&self) -> bool {
        self.entity_provided
    }
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
    pub fn raw(
        &self,
        target: ManagedRangeTarget,
    ) -> Result<Arc<EnumeratedValueRange<V>>, ManagedRangeError> {
        Ok(self.groups[self.slot(target)?].raw.clone())
    }
    pub fn get(
        &self,
        target: ManagedRangeTarget,
    ) -> Result<Arc<EnumeratedValueRange<Option<V>>>, ManagedRangeError> {
        Ok(self.groups[self.slot(target)?].active.clone())
    }
    /// Stable sorting prepares a replacement before publishing it. Comparator or
    /// value-clone panics leave the active range and its cached sorter unchanged.
    pub fn sort(
        &mut self,
        target: ManagedRangeTarget,
        order: &ManagedRangeOrder<V>,
    ) -> Result<Arc<EnumeratedValueRange<Option<V>>>, ManagedRangeError> {
        let slot = self.slot(target)?;
        let group = &mut self.groups[slot];
        if group
            .order
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(&old.compare, &order.compare))
        {
            return Ok(group.active.clone());
        }
        let mut values = group.active.values().to_vec();
        values.sort_by(|a, b| match (a, b) {
            (Some(a), Some(b)) => (order.compare)(a, b),
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
        });
        let sorted = Arc::new(EnumeratedValueRange::new(values));
        group.active = sorted.clone();
        group.order = Some(order.clone());
        Ok(sorted)
    }
    /// Replace providers and discard sort caches, retaining the field kind.
    pub fn reset_shared(&mut self, providers: Vec<Vec<V>>, entity_count: usize) {
        *self = Self::shared(self.kind, providers, entity_count);
    }
    pub fn reset_entity_provided(&mut self, providers: Vec<Vec<Vec<V>>>) {
        *self = Self::entity_provided(self.kind, providers);
    }
    /// Construct an uninitialized basic slot in the current range order. Callers
    /// can add initial values and pins before constructing a FiniteProblem.
    pub fn variable(&self, entity: usize) -> Result<Variable<V>, ManagedRangeError> {
        let ManagedRangeKind::Basic { allows_unassigned } = self.kind else {
            return Err(ManagedRangeError::WrongVariableKind);
        };
        let mut v = Variable::new(
            self.get(ManagedRangeTarget::Entity(entity))?
                .values()
                .iter()
                .flatten()
                .cloned()
                .collect(),
        );
        if allows_unassigned {
            v = v.allowing_unassigned();
        }
        Ok(v)
    }
    /// Build a list model with these per-entity ranges, keeping supplied initial
    /// lists and pin metadata. This creates a snapshot; later range edits do not
    /// mutate the model or any live scoring session.
    pub fn list_problem(
        &self,
        entities: Vec<ListEntity<V>>,
    ) -> Result<ListProblem<V>, ManagedRangeError> {
        let ManagedRangeKind::List { allows_unassigned } = self.kind else {
            return Err(ManagedRangeError::WrongVariableKind);
        };
        if entities.len() != self.entity_count() {
            return Err(ManagedRangeError::EntityCount {
                expected: self.entity_count(),
                actual: entities.len(),
            });
        }
        let entities = entities
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                let range = self.groups[self.entities[i]]
                    .active
                    .values()
                    .iter()
                    .flatten()
                    .cloned()
                    .collect();
                e.with_value_range(range)
            })
            .collect();
        let values = self.groups[self.solution]
            .active
            .values()
            .iter()
            .flatten()
            .cloned()
            .collect();
        let mut p = ListProblem::new(values, entities).map_err(ManagedRangeError::List)?;
        if allows_unassigned {
            p = p.allowing_unassigned();
        }
        Ok(p)
    }
}
