//! Schema-checked typed stream sources retaining actual solution allocations.
use crate::{
    AdditiveScore, BoundBasicSolution, BoundSolutionFacts, ComposedPlanningSolution, PlanningType,
    SolutionMemberKind, StreamGraphBuilder, StreamPort, StreamSource,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet, VecDeque},
    marker::PhantomData,
    sync::Arc,
};
type Project = dyn Send + Sync + for<'a> Fn(&'a dyn Any) -> &'a dyn Any;
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
    pub fn source<T: 'static, S: AdditiveScore + 'static>(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
    ) -> Result<CheckedSolutionStream<T>, SolutionStreamError> {
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
