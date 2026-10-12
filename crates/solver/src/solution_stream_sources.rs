//! Schema-checked typed stream sources retaining actual solution allocations.
mod working_list;
mod working_mixed;
use crate::{
    AdditiveScore, BoundBasicSolution, BoundSolutionFacts, ComposedPlanningSolution, PlanningType,
    SolutionMemberKind, StreamGraphBuilder, StreamPort, StreamSource,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    marker::PhantomData,
    sync::Arc,
};
pub use working_list::{
    WorkingListModelPort, WorkingListModelStream, WorkingListObject, WorkingListUpdateError,
    WorkingListValue, WorkingListValuePort,
};
pub use working_mixed::{
    WorkingMixedModelPort, WorkingMixedModelStream, WorkingMixedObject, WorkingMixedUpdateError,
};
type Project = dyn Send + Sync + for<'a> Fn(&'a dyn Any) -> &'a dyn Any;
type SourcePaths = HashMap<PlanningType, Arc<Vec<Arc<Project>>>>;
type WorkingRow<T, V> = (usize, WorkingBasicObject<T, V>);
pub type WorkingBasicRows<T, V> = Vec<WorkingRow<T, V>>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SolutionStreamError {
    UnknownType {
        requested: PlanningType,
        registered: Vec<PlanningType>,
    },
    UnknownProjectionOwner(PlanningType),
    ProjectionCycle,
    DuplicateProjection,
    DifferentDescriptor,
    MissingFactSnapshot,
    BasicAssignment(crate::ModelError),
    ListAssignment(crate::ListError),
    WrongListValueType(PlanningType),
    WrongWorkingValueType {
        entity: PlanningType,
        variable: String,
        expected: PlanningType,
        actual: PlanningType,
    },
    UnknownEntityRow(usize),
}
impl std::fmt::Display for SolutionStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType {
                requested,
                registered,
            } => write!(
                f,
                "{} is not assignable from any registered solution source: {registered:?}",
                requested.name()
            ),
            _ => write!(f, "solution stream source: {self:?}"),
        }
    }
}
impl std::error::Error for SolutionStreamError {}
/// A graph tuple owns the original allocation and borrows its actual projected
/// subobject on access. Cloning copies Arc handles, never domain payloads. Shared
/// interior mutation is not a snapshot and must not occur while a graph uses it.
pub struct SolutionStreamObject<T> {
    allocation: Arc<dyn Any + Send + Sync>,
    path: Arc<Vec<Arc<Project>>>,
    marker: PhantomData<fn() -> T>,
}
impl<T> Clone for SolutionStreamObject<T> {
    fn clone(&self) -> Self {
        Self {
            allocation: self.allocation.clone(),
            path: self.path.clone(),
            marker: PhantomData,
        }
    }
}
impl<T: 'static> SolutionStreamObject<T> {
    pub fn get(&self) -> &T {
        let mut value: &dyn Any = self.allocation.as_ref();
        for project in self.path.iter() {
            value = project(value);
        }
        value.downcast_ref().expect("validated source projection")
    }
    pub fn allocation(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.allocation
    }
}
/// The graph is constructed through real typed source handles. Every requested
/// type must be registered as an entity/fact or have an executable projection
/// from one. Provider-only members do not implicitly become problem facts.
/// Projection paths resolve once at source construction, before any data reads.
pub struct SolutionStreamRegistry {
    layout: Arc<ComposedPlanningSolution>,
    concrete: Vec<PlanningType>,
    projections: HashMap<PlanningType, Vec<(PlanningType, Arc<Project>)>>,
}
pub struct CheckedSolutionStream<T> {
    layout: Arc<ComposedPlanningSolution>,
    source: StreamSource<usize, SolutionStreamObject<T>>,
    paths: HashMap<PlanningType, Arc<Vec<Arc<Project>>>>,
}
impl SolutionStreamRegistry {
    pub fn new(layout: Arc<ComposedPlanningSolution>) -> Self {
        let mut concrete: Vec<_> = layout
            .schema()
            .entities()
            .map(|e| e.entity_type())
            .collect();
        for member in layout.members() {
            match member.kind() {
                SolutionMemberKind::Facts { allowed_types, .. } => concrete.extend(allowed_types),
                SolutionMemberKind::ValueRange { value_type, .. }
                    if member.is_problem_fact_collection() =>
                {
                    concrete.push(*value_type)
                }
                _ => {}
            }
        }
        let mut seen = HashSet::new();
        concrete.retain(|t| seen.insert(*t));
        Self {
            layout,
            concrete,
            projections: HashMap::new(),
        }
    }
    fn path(&self, from: PlanningType, to: PlanningType) -> Option<Vec<Arc<Project>>> {
        let mut pending = VecDeque::from([(from, vec![])]);
        let mut visited = HashSet::new();
        while let Some((current, path)) = pending.pop_front() {
            if current == to {
                return Some(path);
            }
            if !visited.insert(current) {
                continue;
            }
            for (parent, project) in self.projections.get(&current).into_iter().flatten() {
                let mut next = path.clone();
                next.push(project.clone());
                pending.push_back((*parent, next));
            }
        }
        None
    }
    /// Register a safe borrowed projection of a real embedded/referenced base.
    /// The owner must already be reachable from an actual declared source type.
    pub fn project<C: 'static, P: 'static>(
        &mut self,
        project: impl for<'a> Fn(&'a C) -> &'a P + Send + Sync + 'static,
    ) -> Result<(), SolutionStreamError> {
        let child = PlanningType::of::<C>();
        let parent = PlanningType::of::<P>();
        if !self.concrete.iter().any(|&t| self.path(t, child).is_some()) {
            return Err(SolutionStreamError::UnknownProjectionOwner(child));
        }
        if self.path(parent, child).is_some() {
            return Err(SolutionStreamError::ProjectionCycle);
        }
        if self
            .projections
            .get(&child)
            .is_some_and(|edges| edges.iter().any(|(p, _)| *p == parent))
        {
            return Err(SolutionStreamError::DuplicateProjection);
        }
        self.projections.entry(child).or_default().push((
            parent,
            Arc::new(move |value| {
                project(
                    value
                        .downcast_ref::<C>()
                        .expect("validated projection owner"),
                ) as &dyn Any
            }),
        ));
        Ok(())
    }
    fn resolve_paths<T: 'static>(&self) -> Result<SourcePaths, SolutionStreamError> {
        let requested = PlanningType::of::<T>();
        let paths: HashMap<_, _> = self
            .concrete
            .iter()
            .filter_map(|&t| self.path(t, requested).map(|p| (t, Arc::new(p))))
            .collect();
        if paths.is_empty() {
            return Err(SolutionStreamError::UnknownType {
                requested,
                registered: self.concrete.clone(),
            });
        }
        Ok(paths)
    }
    pub fn source<T: 'static, S: AdditiveScore + 'static>(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
    ) -> Result<CheckedSolutionStream<T>, SolutionStreamError> {
        let paths = self.resolve_paths::<T>()?;
        Ok(CheckedSolutionStream {
            layout: self.layout.clone(),
            source: builder.source(),
            paths,
        })
    }
}
impl<T: 'static> CheckedSolutionStream<T> {
    /// Transactionally refresh membership of immutable object/projection rows.
    /// Existing allocations keep their tuples and avoid redundant score work.
    /// Current planning-field snapshots require their own event feeds; this is
    /// a membership refresh, not an interior-mutation notification.
    pub fn refresh_membership<S: AdditiveScore + 'static>(
        &self,
        graph: &mut crate::StreamGraph<'_, S>,
        rows: Vec<(usize, SolutionStreamObject<T>)>,
    ) -> Result<(), crate::GraphError> {
        let old_order: Vec<_> = graph.source_rows(&self.source)?.keys().copied().collect();
        let old: HashSet<_> = old_order.iter().copied().collect();
        let next: HashSet<_> = rows.iter().map(|(key, _)| *key).collect();
        let mut tx = graph.transaction();
        for key in old_order.iter().filter(|key| !next.contains(key)) {
            tx.apply(&self.source, crate::TupleChange::retract(*key))?;
        }
        let mut inserted = old;
        for (key, row) in rows {
            if inserted.insert(key) {
                tx.apply(&self.source, crate::TupleChange::put(key, row))?;
            }
        }
        tx.commit()
    }
    /// Select entities using current native slots, including shadow-only
    /// entities, while keeping ordinary facts. The object payload is immutable
    /// fact/initial-state data; planning fields are read from the assignment.
    pub fn initialized_basic<V: Eq>(
        &self,
        bound: &BoundBasicSolution<V>,
        assignment: &crate::Assignment<V>,
    ) -> Result<Vec<(usize, SolutionStreamObject<T>)>, SolutionStreamError> {
        let facts = bound
            .facts()
            .ok_or(SolutionStreamError::MissingFactSnapshot)?;
        let mut rows = self.facts(facts)?;
        bound
            .problem()
            .validate(assignment)
            .map_err(SolutionStreamError::BasicAssignment)?;
        rows.extend(
            bound
                .entities()
                .iter()
                .filter(|e| {
                    e.slots()
                        .iter()
                        .all(|(_, slot)| assignment.values()[*slot].is_some())
                })
                .filter_map(|e| self.row(e.entity_type(), e.object())),
        );
        Ok(rows)
    }
    /// Raw list-owner, shadow-only entity and fact objects from the concrete
    /// binding. Current planning lists remain in the explicit native assignment.
    pub fn list_bound<V>(
        &self,
        bound: &crate::BoundListSolution<V>,
    ) -> Result<Vec<(usize, SolutionStreamObject<T>)>, SolutionStreamError> {
        let mut rows = self.facts(bound.facts())?;
        rows.extend(
            bound
                .entities()
                .iter()
                .filter_map(|e| self.row(e.entity_type(), e.object())),
        );
        Ok(rows)
    }
    /// Select real canonical list-value allocations using the live native
    /// assignment. No inverse/index getter is consulted. Both required and
    /// optional list fields exclude currently unassigned values for this view.
    /// Native Eq identity determines canonical model rows; foreign equal-valued
    /// assignment handles cannot replace the actual registered domain object.
    pub fn initialized_list_values<C: Any + Send + Sync + Eq>(
        &self,
        bound: &crate::BoundListSolution<Arc<C>>,
        assignment: &crate::ListAssignment<Arc<C>>,
    ) -> Result<Vec<(usize, SolutionStreamObject<T>)>, SolutionStreamError> {
        if !Arc::ptr_eq(&self.layout, bound.facts().descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor);
        }
        let ty = PlanningType::of::<C>();
        if !self.paths.contains_key(&ty) {
            return Err(SolutionStreamError::WrongListValueType(ty));
        }
        let positions = bound
            .problem()
            .validate_using(assignment, true)
            .map_err(SolutionStreamError::ListAssignment)?;
        let assigned = match positions {
            Some(positions) => positions
                .into_iter()
                .map(|p| p.is_some())
                .collect::<Vec<_>>(),
            None => {
                // Eq-only models return no position table. Preserve their native
                // identity semantics; indexed models use the single-pass result.
                let mut assigned = vec![false; bound.problem().values().len()];
                for value in assignment.lists().iter().flatten() {
                    let index = bound.problem().value_index(value).expect("validated value");
                    assigned[index] = true;
                }
                assigned
            }
        };
        Ok(bound
            .problem()
            .values()
            .iter()
            .zip(assigned)
            .filter(|(_, assigned)| *assigned)
            .filter_map(|(value, _)| {
                let object: Arc<dyn Any + Send + Sync> = value.clone();
                self.row(ty, &object)
            })
            .collect())
    }
    pub fn source(&self) -> &StreamSource<usize, SolutionStreamObject<T>> {
        &self.source
    }
    pub fn port(&self) -> &StreamPort<usize, SolutionStreamObject<T>> {
        self.source.port()
    }
    fn row(
        &self,
        ty: PlanningType,
        object: &Arc<dyn Any + Send + Sync>,
    ) -> Option<(usize, SolutionStreamObject<T>)> {
        self.paths.get(&ty).map(|path| {
            (
                Arc::as_ptr(object) as *const () as usize,
                SolutionStreamObject {
                    allocation: object.clone(),
                    path: path.clone(),
                    marker: PhantomData,
                },
            )
        })
    }
    /// Read canonical fact allocations from the exact descriptor that created
    /// this source, including empty registered collections. Occurrence aliases
    /// have already been deduplicated by the actual fact reader.
    pub fn facts(
        &self,
        facts: &BoundSolutionFacts,
    ) -> Result<Vec<(usize, SolutionStreamObject<T>)>, SolutionStreamError> {
        if !Arc::ptr_eq(&self.layout, facts.descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor);
        }
        Ok(facts
            .records()
            .iter()
            .filter_map(|r| self.row(r.fact_type(), r.object()))
            .collect())
    }
    /// Read both canonical entity and fact allocations. The attached snapshot
    /// proves the basic binding belongs to this actual descriptor. Assignment
    /// filtering remains explicit in the graph; this method does not infer Java
    /// forEach initialization/list-inverse semantics from object field syntax.
    pub fn bound<V>(
        &self,
        bound: &BoundBasicSolution<V>,
    ) -> Result<Vec<(usize, SolutionStreamObject<T>)>, SolutionStreamError> {
        let facts = bound
            .facts()
            .ok_or(SolutionStreamError::MissingFactSnapshot)?;
        let mut rows = self.facts(facts)?;
        rows.extend(
            bound
                .entities()
                .iter()
                .filter_map(|r| self.row(r.entity_type(), r.object())),
        );
        Ok(rows)
    }
}

/// A typed object with owned current genuine-field values. Object payloads remain
/// immutable facts/initial state; `field()` reads the current native assignment.
/// Field-name storage is shared between snapshots, and no domain-object clone
/// bound is required. V cloning follows the native assignment's value semantics.
pub struct WorkingBasicObject<T, V> {
    object: SolutionStreamObject<T>,
    names: Arc<[String]>,
    values: Vec<Option<V>>,
}
impl<T, V: Clone> Clone for WorkingBasicObject<T, V> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            names: self.names.clone(),
            values: self.values.clone(),
        }
    }
}
impl<T: 'static, V> WorkingBasicObject<T, V> {
    pub fn object(&self) -> &T {
        self.object.get()
    }
    pub fn allocation(&self) -> &Arc<dyn Any + Send + Sync> {
        self.object.allocation()
    }
    pub fn field(&self, name: &str) -> Option<&Option<V>> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| &self.values[i])
    }
    pub fn fields(&self) -> impl Iterator<Item = (&str, &Option<V>)> {
        self.names.iter().map(String::as_str).zip(&self.values)
    }
}
pub struct WorkingBasicStream<T, V> {
    layout: Arc<ComposedPlanningSolution>,
    paths: HashMap<PlanningType, Arc<Vec<Arc<Project>>>>,
    names: HashMap<PlanningType, Arc<[String]>>,
    source: StreamSource<usize, WorkingBasicObject<T, V>>,
}
impl SolutionStreamRegistry {
    /// Build a genuine typed source whose tuples carry current planning fields.
    /// Validate relevant basic field types before allocating a graph source node.
    pub fn working_basic_source<T: 'static, V: Clone + 'static, S: AdditiveScore + 'static>(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
    ) -> Result<WorkingBasicStream<T, V>, SolutionStreamError> {
        let paths = self.resolve_paths::<T>()?;
        let mut names = HashMap::new();
        for &ty in paths.keys() {
            if let Ok(entity) = self.layout.schema().entity_type(ty) {
                let mut fields = vec![];
                for variable in entity.genuine_variables() {
                    if !matches!(variable.kind(), crate::PlanningVariableKind::Basic { .. })
                        || variable.value_type() != PlanningType::of::<V>()
                    {
                        return Err(SolutionStreamError::WrongWorkingValueType {
                            entity: ty,
                            variable: variable.name().into(),
                            expected: variable.value_type(),
                            actual: PlanningType::of::<V>(),
                        });
                    }
                    fields.push(variable.name().to_owned());
                }
                names.insert(ty, Arc::from(fields));
            }
        }
        Ok(WorkingBasicStream {
            layout: self.layout.clone(),
            paths,
            names,
            source: builder.source(),
        })
    }
}
impl<T: 'static, V> WorkingBasicStream<T, V> {
    pub fn source(&self) -> &StreamSource<usize, WorkingBasicObject<T, V>> {
        &self.source
    }
    pub fn port(&self) -> &StreamPort<usize, WorkingBasicObject<T, V>> {
        self.source.port()
    }
    fn object(
        &self,
        ty: PlanningType,
        object: &Arc<dyn Any + Send + Sync>,
    ) -> Option<SolutionStreamObject<T>> {
        self.paths.get(&ty).map(|path| SolutionStreamObject {
            allocation: object.clone(),
            path: path.clone(),
            marker: PhantomData,
        })
    }
}
impl<T: 'static, V: Clone + Eq + 'static> WorkingBasicStream<T, V> {
    /// Snapshot current field values, preserving actual entity/fact allocations.
    /// Passing false includes entities with null genuine fields. Whole-assignment
    /// validation precedes publication, including domain and pin constraints.
    pub fn rows(
        &self,
        bound: &BoundBasicSolution<V>,
        assignment: &crate::Assignment<V>,
        initialized_only: bool,
    ) -> Result<WorkingBasicRows<T, V>, SolutionStreamError> {
        let facts = bound
            .facts()
            .ok_or(SolutionStreamError::MissingFactSnapshot)?;
        if !Arc::ptr_eq(&self.layout, facts.descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor);
        }
        bound
            .problem()
            .validate(assignment)
            .map_err(SolutionStreamError::BasicAssignment)?;
        let mut rows = vec![];
        for fact in facts.records() {
            if let Some(object) = self.object(fact.fact_type(), fact.object()) {
                rows.push((
                    Arc::as_ptr(fact.object()) as *const () as usize,
                    WorkingBasicObject {
                        object,
                        names: Arc::from([]),
                        values: vec![],
                    },
                ));
            }
        }
        for (index, _) in bound.entities().iter().enumerate() {
            if let Some(row) = self.entity_row(bound, assignment, index, initialized_only)? {
                rows.push(row);
            }
        }
        Ok(rows)
    }
    fn entity_row(
        &self,
        bound: &BoundBasicSolution<V>,
        assignment: &crate::Assignment<V>,
        index: usize,
        initialized_only: bool,
    ) -> Result<Option<WorkingRow<T, V>>, SolutionStreamError> {
        let entity = bound
            .entities()
            .get(index)
            .ok_or(SolutionStreamError::UnknownEntityRow(index))?;
        let Some(object) = self.object(entity.entity_type(), entity.object()) else {
            return Ok(None);
        };
        let values = entity
            .slots()
            .iter()
            .map(|(_, slot)| assignment.values()[*slot].clone())
            .collect::<Vec<_>>();
        if initialized_only && values.iter().any(Option::is_none) {
            return Ok(None);
        }
        let names = self.names[&entity.entity_type()].clone();
        Ok(Some((
            Arc::as_ptr(entity.object()) as *const () as usize,
            WorkingBasicObject {
                object,
                names,
                values,
            },
        )))
    }
    /// Refresh owned field snapshots transactionally, including updates whose
    /// planning identities compare equal but carry different payloads. Existing
    /// object handles remain stable; changed fields trigger actual graph routing.
    pub fn refresh<S: AdditiveScore + 'static>(
        &self,
        graph: &mut crate::StreamGraph<'_, S>,
        rows: Vec<(usize, WorkingBasicObject<T, V>)>,
    ) -> Result<(), crate::GraphError> {
        let old = graph
            .source_rows(&self.source)?
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let next = rows.iter().map(|(key, _)| *key).collect::<HashSet<_>>();
        let mut tx = graph.transaction();
        for key in old {
            if !next.contains(&key) {
                tx.apply(&self.source, crate::TupleChange::retract(key))?;
            }
        }
        for (key, row) in rows {
            tx.apply(&self.source, crate::TupleChange::put(key, row))?;
        }
        tx.commit()
    }
    /// Update one concrete canonical entity row and leave unrelated source rows
    /// untouched. Validate the full native input before any graph publication.
    pub fn update_entity<S: AdditiveScore + 'static>(
        &self,
        graph: &mut crate::StreamGraph<'_, S>,
        bound: &BoundBasicSolution<V>,
        assignment: &crate::Assignment<V>,
        index: usize,
        initialized_only: bool,
    ) -> Result<(), WorkingBasicUpdateError> {
        let facts = bound.facts().ok_or(WorkingBasicUpdateError::Source(
            SolutionStreamError::MissingFactSnapshot,
        ))?;
        if !Arc::ptr_eq(&self.layout, facts.descriptor()) {
            return Err(WorkingBasicUpdateError::Source(
                SolutionStreamError::DifferentDescriptor,
            ));
        }
        bound.problem().validate(assignment).map_err(|e| {
            WorkingBasicUpdateError::Source(SolutionStreamError::BasicAssignment(e))
        })?;
        graph
            .source_rows(&self.source)
            .map_err(WorkingBasicUpdateError::Graph)?;
        let row = self
            .entity_row(bound, assignment, index, initialized_only)
            .map_err(WorkingBasicUpdateError::Source)?;
        let entity = bound
            .entities()
            .get(index)
            .ok_or(WorkingBasicUpdateError::Source(
                SolutionStreamError::UnknownEntityRow(index),
            ))?;
        if !self.paths.contains_key(&entity.entity_type()) {
            return Ok(());
        }
        let key = Arc::as_ptr(entity.object()) as *const () as usize;
        let mut tx = graph.transaction();
        let change = row.map_or_else(
            || crate::TupleChange::retract(key),
            |(key, row)| crate::TupleChange::put(key, row),
        );
        tx.apply(&self.source, change)
            .map_err(WorkingBasicUpdateError::Graph)?;
        tx.commit().map_err(WorkingBasicUpdateError::Graph)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkingBasicUpdateError {
    Source(SolutionStreamError),
    Graph(crate::GraphError),
}
impl fmt::Display for WorkingBasicUpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "working basic update: {self:?}")
    }
}
impl std::error::Error for WorkingBasicUpdateError {}

pub type WorkingBasicModelPort<T, V> =
    crate::StreamPort<crate::ConcatId<usize, usize>, WorkingBasicObject<T, V>>;
/// Real native planning-variable source wired to typed current entity tuples.
/// Multiple fields are grouped by the actual entity allocation; an entity emits
/// only once all its field occurrences exist, and optionally only when non-null.
/// Constant facts and shadow-only entities use an independent immutable source.
pub struct WorkingBasicModelStream<T, V> {
    variables: crate::PlanningStreamSource<V>,
    constant: WorkingBasicStream<T, V>,
    constant_rows: WorkingBasicRows<T, V>,
    port: WorkingBasicModelPort<T, V>,
}
impl<T, V> WorkingBasicModelStream<T, V> {
    pub fn planning_source(&self) -> &crate::PlanningStreamSource<V> {
        &self.variables
    }
    pub fn port(&self) -> &WorkingBasicModelPort<T, V> {
        &self.port
    }
}
impl<T: 'static, V: Clone + Eq + 'static> WorkingBasicModelStream<T, V> {
    /// Initialize only immutable facts/shadow-only objects. The planning source
    /// stays empty for StreamScoreDirector/solve_stream to load and protect.
    pub fn initialize_constants<S: AdditiveScore + 'static>(
        &self,
        graph: &mut crate::StreamGraph<'_, S>,
    ) -> Result<(), crate::GraphError> {
        self.constant.refresh(graph, self.constant_rows.clone())
    }
}
impl SolutionStreamRegistry {
    /// Bind current typed objects to actual native variable events. All object
    /// and field routing metadata is compiled once; moves reuse native indexed
    /// group updates and graph journals rather than scanning every entity.
    pub fn working_basic_model<T: 'static, V: Clone + Eq + 'static, S: AdditiveScore + 'static>(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
        bound: &BoundBasicSolution<V>,
        initialized_only: bool,
    ) -> Result<WorkingBasicModelStream<T, V>, WorkingBasicUpdateError> {
        let facts = bound.facts().ok_or(WorkingBasicUpdateError::Source(
            SolutionStreamError::MissingFactSnapshot,
        ))?;
        if !Arc::ptr_eq(&self.layout, facts.descriptor()) {
            return Err(WorkingBasicUpdateError::Source(
                SolutionStreamError::DifferentDescriptor,
            ));
        }
        let constant = self
            .working_basic_source::<T, V, S>(builder)
            .map_err(WorkingBasicUpdateError::Source)?;
        let initial = bound.problem().initial_assignment();
        let constant_rows = constant
            .rows(bound, &initial, false)
            .map_err(WorkingBasicUpdateError::Source)?
            .into_iter()
            .filter(|(_, row)| row.values.is_empty())
            .collect();
        let mut by_slot = vec![None; bound.problem().variables().len()];
        let mut entities = HashMap::new();
        for entity in bound.entities() {
            let Some(object) = constant.object(entity.entity_type(), entity.object()) else {
                continue;
            };
            if entity.slots().is_empty() {
                continue;
            }
            let key = Arc::as_ptr(entity.object()) as *const () as usize;
            let names = constant.names[&entity.entity_type()].clone();
            for (_, slot) in entity.slots() {
                by_slot[*slot] = Some(key);
            }
            entities.insert(key, (object, names));
        }
        let by_slot = Arc::new(by_slot);
        let entities = Arc::new(entities);
        let variables = builder.source::<usize, (usize, Option<V>)>();
        let select = by_slot.clone();
        let selected = builder
            .filter(variables.port(), move |(slot, _)| {
                select.get(*slot).is_some_and(Option::is_some)
            })
            .map_err(WorkingBasicUpdateError::Graph)?;
        let groups = builder
            .group_list(&selected, move |(slot, value)| {
                (by_slot[*slot].expect("selected slot"), value.clone())
            })
            .map_err(WorkingBasicUpdateError::Graph)?;
        let lengths = entities.clone();
        let complete = builder
            .filter(&groups, move |(key, values)| {
                values.len() == lengths[key].1.len()
                    && (!initialized_only || values.iter().all(Option::is_some))
            })
            .map_err(WorkingBasicUpdateError::Graph)?;
        let current = builder
            .map(&complete, move |(key, values)| {
                let (object, names) = &entities[key];
                WorkingBasicObject {
                    object: object.clone(),
                    names: names.clone(),
                    values: values.iter().cloned().collect(),
                }
            })
            .map_err(WorkingBasicUpdateError::Graph)?;
        let port = builder
            .concat(&current, constant.port())
            .map_err(WorkingBasicUpdateError::Graph)?;
        Ok(WorkingBasicModelStream {
            variables,
            constant,
            constant_rows,
            port,
        })
    }
}
