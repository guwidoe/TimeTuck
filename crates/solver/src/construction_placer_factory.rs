//! Typed construction selector inference bound to real native slots and queues.
use crate::*;
use std::{cmp::Ordering, error::Error, fmt, rc::Rc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionPlacerKind {
    PooledEntity,
    QueuedEntity,
    QueuedValue,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ConstructionEntitySort {
    #[default]
    Original,
    DescendingIfAvailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionEntityOrder {
    Original,
    Sorted,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionChangeSelector<'s> {
    pub field: PlanningVariableRef<'s>,
    pub explicit_entity: Option<PlanningType>,
    pub entity_replay: Option<PlanningType>,
    pub value_replay: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionPlacerPlan<'s> {
    pub kind: ConstructionPlacerKind,
    pub entity: PlanningType,
    pub changes: Vec<ConstructionChangeSelector<'s>>,
    pub sort: ConstructionEntitySort,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstructionFactoryError {
    Schema(PlanningSchemaError),
    AmbiguousEntity,
    AmbiguousField,
    NotBasic(String),
    Binding(String),
    Queue(PlacementError),
}
impl fmt::Display for ConstructionFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "construction placer factory: {self:?}")
    }
}
impl Error for ConstructionFactoryError {}
impl From<PlanningSchemaError> for ConstructionFactoryError {
    fn from(e: PlanningSchemaError) -> Self {
        Self::Schema(e)
    }
}
impl From<PlacementError> for ConstructionFactoryError {
    fn from(e: PlacementError) -> Self {
        Self::Queue(e)
    }
}
impl<'s> ConstructionPlacerPlan<'s> {
    pub fn entity_order(&self) -> ConstructionEntityOrder {
        if self.sort == ConstructionEntitySort::Original {
            ConstructionEntityOrder::Original
        } else {
            ConstructionEntityOrder::Sorted
        }
    }

    /// Infer an unambiguous genuine entity/field and attach native recorded-key roles.
    /// Explicit field names preserve order and create separate entity placements.
    pub fn infer(
        schema: &'s PlanningSchema,
        kind: ConstructionPlacerKind,
        entity: Option<PlanningType>,
        fields: &[&str],
        sort: ConstructionEntitySort,
    ) -> Result<Self, ConstructionFactoryError> {
        let selected = if let Some(e) = entity {
            schema.entity_type(e)?
        } else {
            let mut it = schema.genuine_entities();
            let first = it.next().ok_or(ConstructionFactoryError::AmbiguousEntity)?;
            if it.next().is_some() {
                return Err(ConstructionFactoryError::AmbiguousEntity);
            }
            first
        };
        let names = if fields.is_empty() {
            let mut it = selected.genuine_variables();
            let f = it.next().ok_or(ConstructionFactoryError::AmbiguousField)?;
            if it.next().is_some() {
                return Err(ConstructionFactoryError::AmbiguousField);
            }
            vec![f.name()]
        } else {
            fields.to_vec()
        };
        let mut changes = vec![];
        for name in names {
            let field = selected.genuine_variable(name)?;
            if !matches!(field.kind(), PlanningVariableKind::Basic { .. }) {
                return Err(ConstructionFactoryError::NotBasic(name.into()));
            }
            if changes
                .iter()
                .any(|c: &ConstructionChangeSelector<'_>| c.field == field)
            {
                return Err(ConstructionFactoryError::Binding(name.into()));
            }
            changes.push(ConstructionChangeSelector {
                field,
                explicit_entity: None,
                entity_replay: (kind != ConstructionPlacerKind::QueuedValue)
                    .then_some(selected.entity_type()),
                value_replay: kind == ConstructionPlacerKind::QueuedValue,
            });
        }
        if kind == ConstructionPlacerKind::QueuedValue && changes.len() != 1 {
            return Err(ConstructionFactoryError::AmbiguousField);
        }
        Ok(Self {
            kind,
            entity: selected.entity_type(),
            changes,
            sort,
        })
    }
    /// Flattened slots bind each field to the same ordered native entity rows.
    /// The comparator supplies the policy's preferred order, e.g. descending difficulty.
    pub fn build_queue<V: Eq + Clone>(
        &self,
        problem: &FiniteProblem<V>,
        bindings: &[ConstructionFieldBinding],
        sorter: Option<ConstructionEntityComparator>,
    ) -> Result<ConstructionFactoryQueue<V>, ConstructionFactoryError> {
        let mut slots = vec![];
        let mut seen = vec![];
        let mut count = None;
        for change in &self.changes {
            let mut matches = bindings.iter().filter(|b| b.name == change.field.name());
            let b = matches
                .next()
                .ok_or_else(|| ConstructionFactoryError::Binding(change.field.name().into()))?;
            if matches.next().is_some() || count.is_some_and(|n| n != b.slots.len()) {
                return Err(ConstructionFactoryError::Binding(b.name.clone()));
            }
            count = Some(b.slots.len());
            for &slot in &b.slots {
                if slot >= problem.variables().len() || seen.contains(&slot) {
                    return Err(ConstructionFactoryError::Binding(b.name.clone()));
                }
                seen.push(slot);
            }
            slots.push(b.slots.clone());
        }
        let mut values = vec![];
        for &slot in slots.iter().flatten() {
            for v in problem.variables()[slot].domain() {
                if !values.contains(v) {
                    values.push(v.clone());
                }
            }
        }
        let count = count.unwrap_or(0);
        let keys = match self.kind {
            ConstructionPlacerKind::PooledEntity => vec![0],
            ConstructionPlacerKind::QueuedEntity => (0..count).collect(),
            ConstructionPlacerKind::QueuedValue => (0..values.len()).collect(),
        };
        let runtime = Rc::new(Runtime {
            problem: problem.clone(),
            slots,
            values,
            kind: self.kind,
        });
        let children = (0..if self.kind == ConstructionPlacerKind::QueuedEntity {
            self.changes.len()
        } else {
            1
        })
            .map(|column| FactoryConstructionMoves {
                runtime: runtime.clone(),
                column,
            })
            .collect();
        Ok(PlacementQueue::new(
            FactoryConstructionKeys {
                keys,
                sorter: if self.kind == ConstructionPlacerKind::QueuedEntity
                    && self.sort == ConstructionEntitySort::DescendingIfAvailable
                {
                    sorter
                } else {
                    None
                },
            },
            children,
            if self.kind == ConstructionPlacerKind::QueuedEntity {
                PlacementOrder::Once
            } else {
                PlacementOrder::Cycle
            },
            count.max(runtime.values.len()).max(1),
            problem.variables().iter().map(|v| v.domain().len()).sum(),
        )?)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionFieldBinding {
    pub name: String,
    pub slots: Vec<usize>,
}
pub type ConstructionEntityComparator = Rc<dyn Fn(usize, usize) -> Ordering>;
pub struct FactoryConstructionKeys {
    keys: Vec<usize>,
    sorter: Option<ConstructionEntityComparator>,
}
impl<V> SelectionSource<Assignment<V>, usize> for FactoryConstructionKeys {
    fn size(&mut self, _: &Assignment<V>) -> Option<usize> {
        Some(self.keys.len())
    }
    fn open<'a>(&'a mut self, _: &'a Assignment<V>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        let mut keys = self.keys.clone();
        if let Some(sort) = &self.sorter {
            keys.sort_by(|a, b| sort(*a, *b));
        }
        Box::new(keys.into_iter())
    }
}
struct Runtime<V> {
    problem: FiniteProblem<V>,
    slots: Vec<Vec<usize>>,
    values: Vec<V>,
    kind: ConstructionPlacerKind,
}
pub struct FactoryConstructionMoves<V> {
    runtime: Rc<Runtime<V>>,
    column: usize,
}
impl<V: Eq + Clone> PlacementSource<Assignment<V>, usize, Move<V>> for FactoryConstructionMoves<V> {
    fn size(&mut self, _: &Assignment<V>, _: &usize) -> Option<usize> {
        None
    }
    fn open<'a>(
        &'a mut self,
        _: &'a Assignment<V>,
        key: &'a usize,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        let r = &self.runtime;
        match r.kind {
            ConstructionPlacerKind::QueuedEntity => {
                let variable = r.slots[self.column][*key];
                Box::new(
                    r.problem.variables()[variable]
                        .domain()
                        .iter()
                        .map(move |v| Move::Change {
                            variable,
                            value: Some(v.clone()),
                        }),
                )
            }
            ConstructionPlacerKind::QueuedValue => {
                let value = &r.values[*key];
                Box::new(
                    r.slots[0]
                        .iter()
                        .copied()
                        .filter(move |&i| r.problem.variables()[i].domain().contains(value))
                        .map(move |variable| Move::Change {
                            variable,
                            value: Some(value.clone()),
                        }),
                )
            }
            ConstructionPlacerKind::PooledEntity => {
                Box::new(r.slots.iter().flatten().copied().flat_map(move |variable| {
                    r.problem.variables()[variable]
                        .domain()
                        .iter()
                        .map(move |v| Move::Change {
                            variable,
                            value: Some(v.clone()),
                        })
                }))
            }
        }
    }
}
pub type ConstructionFactoryQueue<V> = PlacementQueue<
    Assignment<V>,
    usize,
    Move<V>,
    FactoryConstructionKeys,
    FactoryConstructionMoves<V>,
>;
/// Retained schema plan and immutable native model/slot policy for rebuilding an
/// actual provider with fresh queues and lifecycle. Shared facts are immutable.
#[derive(Clone)]
pub struct ConstructionPlacerRecipe<'s, V> {
    pub plan: ConstructionPlacerPlan<'s>,
    pub problem: FiniteProblem<V>,
    pub bindings: Vec<ConstructionFieldBinding>,
    pub sorter: Option<ConstructionEntityComparator>,
}
type FactoryCapture<V> = fn(&FiniteProblem<V>, &Assignment<V>, u64) -> Assignment<V>;
type FactoryMapper<V> =
    fn(&FiniteProblem<V>, &Assignment<V>, u64, &usize, usize, &Move<V>) -> Option<Move<V>>;
pub type ConstructionFactoryProvider<V> = PlacementQueueProvider<
    Assignment<V>,
    usize,
    Move<V>,
    FactoryConstructionKeys,
    FactoryConstructionMoves<V>,
    FactoryCapture<V>,
    FactoryMapper<V>,
>;
fn factory_capture<V: Clone>(_: &FiniteProblem<V>, a: &Assignment<V>, _: u64) -> Assignment<V> {
    a.clone()
}
fn factory_mapper<V: Eq + Clone>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    _: u64,
    _: &usize,
    _: usize,
    m: &Move<V>,
) -> Option<Move<V>> {
    if let Move::Change { variable, .. } = m
        && a.values()[*variable].is_none()
        && !p.variables()[*variable].is_pinned()
    {
        return Some(m.clone());
    }
    None
}
impl<V: Eq + Clone> ConstructionPlacerRecipe<'_, V> {
    pub fn build_provider(
        &self,
    ) -> Result<ConstructionFactoryProvider<V>, ConstructionFactoryError> {
        let queue = self
            .plan
            .build_queue(&self.problem, &self.bindings, self.sorter.clone())?;
        Ok(PlacementQueueProvider::new(
            queue,
            factory_capture::<V> as FactoryCapture<V>,
            factory_mapper::<V> as FactoryMapper<V>,
        ))
    }
}
