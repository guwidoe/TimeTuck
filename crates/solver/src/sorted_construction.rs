//! Phase and step scoped entity and value sorting for native construction providers.
use crate::*;
use std::{cmp::Ordering, sync::Arc};

#[derive(Clone)]
enum BasicKey<V> {
    Entity(usize),
    Value(V),
    Pool,
}
struct BasicFactory<V> {
    entities: Vec<usize>,
    domains: Vec<Arc<[V]>>,
    optional: OptionalConstruction,
}
impl<V: Eq + Clone> PlacementMoveFactory<FiniteProblem<V>, Assignment<V>, BasicKey<V>, Move<V>>
    for BasicFactory<V>
{
    fn candidates<'a>(
        &mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        key: &BasicKey<V>,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        let entities = match key {
            BasicKey::Entity(e) => vec![*e],
            _ => self.entities.clone(),
        };
        let value = match key {
            BasicKey::Value(v) => Some(v.clone()),
            _ => None,
        };
        let optional = self.optional;
        // Copy only the selected value orders; move values are cloned lazily below.
        let rows = entities
            .into_iter()
            .filter(|&e| {
                !p.variables()[e].is_pinned()
                    && a.values()[e].is_none()
                    && (!p.variables()[e].allows_unassigned()
                        || optional != OptionalConstruction::Preserve)
            })
            .map(|e| {
                let values = if let Some(v) = &value {
                    if p.variables()[e].contains_value(v) {
                        Arc::from([v.clone()])
                    } else {
                        Arc::from([])
                    }
                } else {
                    self.domains[e].clone()
                };
                (e, values)
            })
            .collect::<Vec<_>>();
        Box::new(rows.into_iter().flat_map(|(variable, values)| {
            (0..values.len()).map(move |i| Move::Change {
                variable,
                value: Some(values[i].clone()),
            })
        }))
    }
}
/// A phase-scoped stable order over basic entities and domain values. Sorting
/// changes candidate traversal without rearranging the model or its variable IDs.
/// Equal comparisons retain model/domain encounter order. Sequential placements
/// visit sorted entities once; ValueQueue cycles distinct sorted values; Pooled
/// compares all eligible pairs in entity-major order at every placement.
///
/// Comparators run only at construction of this provider. Use from_factory to
/// derive them from the phase's initial assignment. Rebuild the provider for new
/// phase-dependent priorities; reset rewinds the same prepared order. Candidate
/// eligibility always uses the current assignment, including pins and ranges.
/// Pass the same optional policy to ProviderConstructionConfig. Use only with the
/// bound problem; a different problem is rejected with a panic before enumeration.
/// Initial sorting happens before the search call, outside its cooperative time
/// budget. Use with_step_sorting to refresh candidate priorities during search.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![1,2]);2])?;
/// let n=ConstraintNetwork::<_,SimpleScore>::new(2);
/// let mut order=SortedBasicConstruction::new(&p,ConstructionPlacement::Sequential,
///     OptionalConstruction::Preserve,|a,b|b.cmp(&a),|a,b|b.cmp(a));
/// let r=construction_from_with_provider(&p,&n,p.initial_assignment(),
///     ProviderConstructionConfig{max_steps:Some(1),..Default::default()},&mut order)?;
/// assert_eq!(r.assignment.values(),&[None,Some(2)]);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct SortedBasicConstruction<'p, V> {
    problem: &'p FiniteProblem<V>,
    queue: QueuedMoveProvider<BasicKey<V>, BasicFactory<V>>,
}
impl<'p, V: Eq + Clone> SortedBasicConstruction<'p, V> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        placement: ConstructionPlacement,
        optional: OptionalConstruction,
        mut entities: impl FnMut(usize, usize) -> Ordering,
        mut values: impl FnMut(&V, &V) -> Ordering,
    ) -> Self {
        let mut entity_order = (0..problem.variables().len()).collect::<Vec<_>>();
        entity_order.sort_by(|a, b| entities(*a, *b));
        let domains = problem
            .variables()
            .iter()
            .map(|v| {
                let mut domain = v.domain().to_vec();
                domain.sort_by(&mut values);
                Arc::from(domain)
            })
            .collect();
        let (keys, order) = match placement {
            ConstructionPlacement::Sequential => (
                entity_order.iter().copied().map(BasicKey::Entity).collect(),
                PlacementQueueOrder::Once,
            ),
            ConstructionPlacement::ValueQueue => {
                let mut unique = vec![];
                for v in problem.variables().iter().flat_map(|v| v.domain()) {
                    if !unique.contains(v) {
                        unique.push(v.clone());
                    }
                }
                unique.sort_by(&mut values);
                (
                    unique.into_iter().map(BasicKey::Value).collect(),
                    PlacementQueueOrder::Cycle,
                )
            }
            ConstructionPlacement::Pooled => (vec![BasicKey::Pool], PlacementQueueOrder::Cycle),
        };
        Self {
            problem,
            queue: QueuedMoveProvider::new(
                keys,
                order,
                BasicFactory {
                    entities: entity_order,
                    domains,
                    optional,
                },
            ),
        }
    }
    /// Invoke a comparator factory once using the phase's actual initial state.
    pub fn from_factory<EC, VC>(
        problem: &'p FiniteProblem<V>,
        initial: &Assignment<V>,
        placement: ConstructionPlacement,
        optional: OptionalConstruction,
        factory: impl FnOnce(&FiniteProblem<V>, &Assignment<V>) -> (EC, VC),
    ) -> Self
    where
        EC: FnMut(usize, usize) -> Ordering,
        VC: FnMut(&V, &V) -> Ordering,
    {
        let (entities, values) = factory(problem, initial);
        Self::new(problem, placement, optional, entities, values)
    }
    pub fn reset(&mut self) {
        self.queue.reset();
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for SortedBasicConstruction<'_, V>
{
    fn event(&mut self, event: crate::MoveProviderEvent) {
        self.queue.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.queue.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.problem),
            "sorted construction used with a different problem"
        );
        self.queue.moves(p, a, step)
    }
}
#[derive(Clone)]
enum ListKey<V> {
    Value(V),
    Pool,
}
struct ListFactory<V> {
    entities: Vec<usize>,
    values: Arc<[V]>,
    optional: OptionalConstruction,
}
impl<V: Eq + Clone> PlacementMoveFactory<ListProblem<V>, ListAssignment<V>, ListKey<V>, ListMove<V>>
    for ListFactory<V>
{
    fn candidates<'a>(
        &mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        key: &ListKey<V>,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        if p.allows_unassigned() && self.optional == OptionalConstruction::Preserve {
            return Box::new(std::iter::empty());
        }
        let values = match key {
            ListKey::Value(v) => Arc::from([v.clone()]),
            ListKey::Pool => self.values.clone(),
        };
        let entities = self.entities.clone();
        Box::new(
            (0..values.len())
                .filter_map(move |i| {
                    if a.location(&values[i]).is_none() {
                        Some(values[i].clone())
                    } else {
                        None
                    }
                })
                .flat_map(move |value| {
                    crate::list_neighborhood::insertions(p, a, entities.clone().into_iter())
                        .filter_map(move |destination| {
                            if p.accepts(destination.entity, &value) {
                                Some(ListMove::Assign {
                                    value: value.clone(),
                                    destination,
                                })
                            } else {
                                None
                            }
                        })
                }),
        )
    }
}
/// Stable phase-scoped list-value order and destination-entity order. Sequential
/// and ValueQueue each visit every sorted value once; Pooled rescans all missing
/// values in value-major order. Within an owner, insertion indices stay ascending
/// from its pinned prefix. Pins, receiver ranges and current assignment ownership
/// are read at every placement. Comparator/factory/reset rules match
/// SortedBasicConstruction. Use with list_construction_from_with_provider or its
/// stream counterpart, or bind this provider to a solver construction phase.
pub struct SortedListConstruction<'p, V> {
    problem: &'p ListProblem<V>,
    queue: QueuedMoveProvider<ListKey<V>, ListFactory<V>>,
}
impl<'p, V: Eq + Clone> SortedListConstruction<'p, V> {
    pub fn new(
        problem: &'p ListProblem<V>,
        placement: ConstructionPlacement,
        optional: OptionalConstruction,
        mut entities: impl FnMut(usize, usize) -> Ordering,
        mut values: impl FnMut(&V, &V) -> Ordering,
    ) -> Self {
        let mut entity_order = (0..problem.entities().len()).collect::<Vec<_>>();
        entity_order.sort_by(|a, b| entities(*a, *b));
        let mut value_order = problem.values().to_vec();
        value_order.sort_by(&mut values);
        let (keys, order) = if placement == ConstructionPlacement::Pooled {
            (vec![ListKey::Pool], PlacementQueueOrder::Cycle)
        } else {
            (
                value_order.iter().cloned().map(ListKey::Value).collect(),
                PlacementQueueOrder::Once,
            )
        };
        Self {
            problem,
            queue: QueuedMoveProvider::new(
                keys,
                order,
                ListFactory {
                    entities: entity_order,
                    values: value_order.into(),
                    optional,
                },
            ),
        }
    }
    pub fn from_factory<EC, VC>(
        problem: &'p ListProblem<V>,
        initial: &ListAssignment<V>,
        placement: ConstructionPlacement,
        optional: OptionalConstruction,
        factory: impl FnOnce(&ListProblem<V>, &ListAssignment<V>) -> (EC, VC),
    ) -> Self
    where
        EC: FnMut(usize, usize) -> Ordering,
        VC: FnMut(&V, &V) -> Ordering,
    {
        let (entities, values) = factory(problem, initial);
        Self::new(problem, placement, optional, entities, values)
    }
    pub fn reset(&mut self) {
        self.queue.reset();
    }
}
impl<V: Eq + Clone> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for SortedListConstruction<'_, V>
{
    fn event(&mut self, event: crate::MoveProviderEvent) {
        self.queue.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.queue.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.problem),
            "sorted construction used with a different problem"
        );
        self.queue.moves(p, a, step)
    }
}

/// Refresh candidate ordering from the current assignment once per completed-step
/// counter. Phase queue keys stay fixed: sequential basic construction refreshes
/// domain ordering, value-queue construction refreshes receiver ordering, and
/// pooled construction refreshes both. A retried/discarded scan with the same
/// counter reuses its order. reset starts a fresh step cache and rewinds phase keys.
///
/// Factories and comparators run inside provider.moves, so their time counts
/// toward the search's cooperative budget. They cannot be interrupted mid-call.
/// A panic before publication preserves the previous order and queue position;
/// callback side effects are not rolled back. Equal comparisons use original
/// model/domain encounter order at each refresh, not the preceding sorted order.
pub struct StepSortedBasicConstruction<'p, V, F> {
    inner: SortedBasicConstruction<'p, V>,
    factory: F,
    last_step: Option<u64>,
}
/// List counterpart of StepSortedBasicConstruction. Phase value keys stay fixed;
/// destination owners refresh each step. Pooled placement also refreshes value
/// ordering. Insertion indices remain ascending from each owner's pinned prefix.
pub struct StepSortedListConstruction<'p, V, F> {
    inner: SortedListConstruction<'p, V>,
    factory: F,
    last_step: Option<u64>,
}
impl<'p, V: Eq + Clone> SortedBasicConstruction<'p, V> {
    /// Add a factory for per-step candidate comparators. The returned comparators
    /// may own priorities derived from the current assignment.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2])?;
    /// let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    /// let mut order = SortedBasicConstruction::new(&p, ConstructionPlacement::Sequential,
    ///     OptionalConstruction::Preserve, |a, b| a.cmp(&b), |a, b| a.cmp(b))
    ///     .with_step_sorting(|_, a| {
    ///         let preferred = a.values().iter().flatten().count() as i32;
    ///         (|a: usize, b: usize| a.cmp(&b),
    ///          move |a: &i32, b: &i32| (a != &preferred).cmp(&(b != &preferred)))
    ///     });
    /// let result = construction_from_with_provider(&p, &n, p.initial_assignment(),
    ///     ProviderConstructionConfig::default(), &mut order)?;
    /// assert_eq!(result.assignment.values(), &[Some(0), Some(1)]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_step_sorting<F, EC, VC>(self, factory: F) -> StepSortedBasicConstruction<'p, V, F>
    where
        F: FnMut(&FiniteProblem<V>, &Assignment<V>) -> (EC, VC),
        EC: FnMut(usize, usize) -> Ordering,
        VC: FnMut(&V, &V) -> Ordering,
    {
        StepSortedBasicConstruction {
            inner: self,
            factory,
            last_step: None,
        }
    }
}
impl<'p, V: Eq + Clone> SortedListConstruction<'p, V> {
    /// Refresh destination and pooled-value priorities from each step's assignment.
    /// Queued value keys retain their phase order. See StepSortedListConstruction.
    pub fn with_step_sorting<F, EC, VC>(self, factory: F) -> StepSortedListConstruction<'p, V, F>
    where
        F: FnMut(&ListProblem<V>, &ListAssignment<V>) -> (EC, VC),
        EC: FnMut(usize, usize) -> Ordering,
        VC: FnMut(&V, &V) -> Ordering,
    {
        StepSortedListConstruction {
            inner: self,
            factory,
            last_step: None,
        }
    }
}
impl<V: Eq + Clone, F> StepSortedBasicConstruction<'_, V, F> {
    pub fn reset(&mut self) {
        self.inner.reset();
        self.last_step = None;
    }
}
impl<V: Eq + Clone, F> StepSortedListConstruction<'_, V, F> {
    pub fn reset(&mut self) {
        self.inner.reset();
        self.last_step = None;
    }
}
impl<V: Eq + Clone, F, EC, VC> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for StepSortedBasicConstruction<'_, V, F>
where
    F: FnMut(&FiniteProblem<V>, &Assignment<V>) -> (EC, VC),
    EC: FnMut(usize, usize) -> Ordering,
    VC: FnMut(&V, &V) -> Ordering,
{
    fn event(&mut self, event: crate::MoveProviderEvent) {
        self.inner.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.inner.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.inner.problem),
            "sorted construction used with a different problem"
        );
        if self.last_step != Some(step) {
            let (mut compare_entities, mut compare_values) = (self.factory)(p, a);
            let mut entities = (0..p.variables().len()).collect::<Vec<_>>();
            entities.sort_by(|a, b| compare_entities(*a, *b));
            let domains = p
                .variables()
                .iter()
                .map(|v| {
                    let mut values = v.domain().to_vec();
                    values.sort_by(&mut compare_values);
                    Arc::from(values)
                })
                .collect();
            let prepared = self.inner.queue.factory_mut();
            prepared.entities = entities;
            prepared.domains = domains;
            self.last_step = Some(step);
        }
        self.inner.moves(p, a, step)
    }
}
impl<V: Eq + Clone, F, EC, VC> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for StepSortedListConstruction<'_, V, F>
where
    F: FnMut(&ListProblem<V>, &ListAssignment<V>) -> (EC, VC),
    EC: FnMut(usize, usize) -> Ordering,
    VC: FnMut(&V, &V) -> Ordering,
{
    fn event(&mut self, event: crate::MoveProviderEvent) {
        self.inner.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.inner.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.inner.problem),
            "sorted construction used with a different problem"
        );
        if self.last_step != Some(step) {
            let (mut compare_entities, mut compare_values) = (self.factory)(p, a);
            let mut entities = (0..p.entities().len()).collect::<Vec<_>>();
            entities.sort_by(|a, b| compare_entities(*a, *b));
            let mut values = p.values().to_vec();
            values.sort_by(&mut compare_values);
            let values = Arc::from(values);
            let prepared = self.inner.queue.factory_mut();
            prepared.entities = entities;
            prepared.values = values;
            self.last_step = Some(step);
        }
        self.inner.moves(p, a, step)
    }
}
