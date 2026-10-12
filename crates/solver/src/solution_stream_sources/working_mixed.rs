//! Group native mixed events into immutable current snapshots of real objects.
use super::{
    CheckedSolutionStream, SolutionStreamError, SolutionStreamObject, SolutionStreamRegistry,
    WorkingListValue, WorkingListValuePort,
};
use crate::{
    AdditiveScore, BoundMixedSolution, ConcatId, GraphError, MixedStreamSources, StreamGraph,
    StreamGraphBuilder, StreamPort,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub struct WorkingMixedObject<T, B, L> {
    object: SolutionStreamObject<T>,
    names: Arc<[String]>,
    basic: Vec<Option<B>>,
    list_name: Option<Arc<str>>,
    list: Arc<[L]>,
}
impl<T, B: Clone, L> Clone for WorkingMixedObject<T, B, L> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            names: self.names.clone(),
            basic: self.basic.clone(),
            list_name: self.list_name.clone(),
            list: self.list.clone(),
        }
    }
}
impl<T: 'static, B, L> WorkingMixedObject<T, B, L> {
    pub fn object(&self) -> &T {
        self.object.get()
    }
    pub fn allocation(&self) -> &Arc<dyn Any + Send + Sync> {
        self.object.allocation()
    }
    pub fn field(&self, name: &str) -> Option<&Option<B>> {
        self.names
            .iter()
            .position(|n| n == name)
            .map(|i| &self.basic[i])
    }
    pub fn fields(&self) -> impl Iterator<Item = (&str, &Option<B>)> {
        self.names.iter().map(String::as_str).zip(&self.basic)
    }
    pub fn list(&self, name: &str) -> Option<&[L]> {
        (self.list_name.as_deref() == Some(name)).then_some(self.list.as_ref())
    }
}
enum Piece<B, L> {
    Basic(Option<B>),
    List(Arc<[L]>),
}
impl<B: Clone, L> Clone for Piece<B, L> {
    fn clone(&self) -> Self {
        match self {
            Self::Basic(v) => Self::Basic(v.clone()),
            Self::List(v) => Self::List(v.clone()),
        }
    }
}
#[derive(Debug)]
pub enum WorkingMixedUpdateError {
    Source(SolutionStreamError),
    Graph(GraphError),
}
impl std::fmt::Display for WorkingMixedUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "working mixed source: {self:?}")
    }
}
impl std::error::Error for WorkingMixedUpdateError {}
impl From<SolutionStreamError> for WorkingMixedUpdateError {
    fn from(e: SolutionStreamError) -> Self {
        Self::Source(e)
    }
}
impl From<GraphError> for WorkingMixedUpdateError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}
pub type WorkingMixedModelPort<T, B, L> =
    StreamPort<ConcatId<usize, usize>, WorkingMixedObject<T, B, L>>;
pub struct WorkingMixedModelStream<T, B, L> {
    sources: MixedStreamSources<B, L>,
    constant: CheckedSolutionStream<T>,
    rows: Vec<(usize, SolutionStreamObject<T>)>,
    port: WorkingMixedModelPort<T, B, L>,
}
impl<T, B, L> WorkingMixedModelStream<T, B, L> {
    pub fn sources(&self) -> &MixedStreamSources<B, L> {
        &self.sources
    }
    pub fn port(&self) -> &WorkingMixedModelPort<T, B, L> {
        &self.port
    }
}
impl<T: 'static, B, L> WorkingMixedModelStream<T, B, L> {
    pub fn initialize_constants<S: AdditiveScore + 'static>(
        &self,
        graph: &mut StreamGraph<'_, S>,
    ) -> Result<(), GraphError> {
        self.constant.refresh_membership(graph, self.rows.clone())
    }
}
struct Shape<T> {
    object: SolutionStreamObject<T>,
    names: Arc<[String]>,
    list_name: Option<Arc<str>>,
}
impl SolutionStreamRegistry {
    pub fn working_mixed_model<
        T: 'static,
        B: Clone + Eq + 'static,
        L: Clone + Eq + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
        bound: &BoundMixedSolution<B, L>,
        initialized_only: bool,
    ) -> Result<WorkingMixedModelStream<T, B, L>, WorkingMixedUpdateError> {
        if !Arc::ptr_eq(&self.layout, bound.facts().descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor.into());
        }
        let constant = self.source::<T, S>(builder)?;
        let mut slots = vec![None; bound.problem().basic().variables().len()];
        let mut lists = HashMap::new();
        let mut shapes = HashMap::new();
        for e in bound.entities() {
            let Some((key, object)) = constant.row(e.entity_type(), e.object()) else {
                continue;
            };
            if e.basic_slots().is_empty() && e.list_field().is_none() {
                continue;
            }
            for (_, slot) in e.basic_slots() {
                slots[*slot] = Some(key);
            }
            if let Some((_, index)) = e.list_field() {
                lists.insert(index, key);
            }
            shapes.insert(
                key,
                Shape {
                    object,
                    names: e
                        .basic_slots()
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>()
                        .into(),
                    list_name: e.list_field().map(|(name, _)| Arc::from(name)),
                },
            );
        }
        let mut seen = HashSet::new();
        let mut rows = constant.facts(bound.facts())?;
        rows.extend(
            bound
                .entities()
                .iter()
                .filter_map(|e| constant.row(e.entity_type(), e.object())),
        );
        rows.retain(|(key, _)| !shapes.contains_key(key) && seen.insert(*key));
        let sources = MixedStreamSources::new(builder);
        let slots = Arc::new(slots);
        let lists = Arc::new(lists);
        let shapes = Arc::new(shapes);
        let select = slots.clone();
        let basic = builder.filter(sources.basic.port(), move |(slot, _)| {
            select.get(*slot).is_some_and(Option::is_some)
        })?;
        let basic = builder.map(&basic, move |(slot, value)| {
            (
                slots[*slot].expect("selected basic slot"),
                Piece::Basic(value.clone()),
            )
        })?;
        let select = lists.clone();
        let list = builder.filter(sources.lists.entities.port(), move |(index, _)| {
            select.contains_key(index)
        })?;
        let list = builder.map(&list, move |(index, values)| {
            (lists[index], Piece::List(Arc::from(values.clone())))
        })?;
        let events = builder.concat(&basic, &list)?;
        let groups = builder.group_list(&events, |(key, piece)| (*key, piece.clone()))?;
        let complete_shapes = shapes.clone();
        let complete = builder.filter(&groups, move |(key, pieces)| {
            let shape = &complete_shapes[key];
            pieces.len() == shape.names.len() + usize::from(shape.list_name.is_some())
                && (!initialized_only || pieces.iter().all(|p| !matches!(p, Piece::Basic(None))))
        })?;
        let current = builder.map(&complete, move |(key, pieces)| {
            let shape = &shapes[key];
            WorkingMixedObject {
                object: shape.object.clone(),
                names: shape.names.clone(),
                basic: pieces
                    .iter()
                    .filter_map(|p| match p {
                        Piece::Basic(v) => Some(v.clone()),
                        Piece::List(_) => None,
                    })
                    .collect(),
                list_name: shape.list_name.clone(),
                list: pieces
                    .iter()
                    .find_map(|p| match p {
                        Piece::List(v) => Some(v.clone()),
                        Piece::Basic(_) => None,
                    })
                    .unwrap_or_else(|| Arc::from([])),
            }
        })?;
        let constants = builder.map(constant.port(), |object| WorkingMixedObject {
            object: object.clone(),
            names: Arc::from([]),
            basic: vec![],
            list_name: None,
            list: Arc::from([]),
        })?;
        let port = builder.concat(&current, &constants)?;
        Ok(WorkingMixedModelStream {
            sources,
            constant,
            rows,
            port,
        })
    }
    pub fn working_mixed_values<
        T: 'static,
        C: Any + Send + Sync + Eq,
        B: Clone + Eq + 'static,
        S: AdditiveScore + 'static,
    >(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
        bound: &BoundMixedSolution<B, Arc<C>>,
        sources: &MixedStreamSources<B, Arc<C>>,
        initialized_only: bool,
    ) -> Result<WorkingListValuePort<T, Arc<C>>, WorkingMixedUpdateError> {
        if !Arc::ptr_eq(&self.layout, bound.facts().descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor.into());
        }
        let paths = self.resolve_paths::<T>()?;
        let ty = crate::PlanningType::of::<C>();
        let path = paths
            .get(&ty)
            .ok_or(SolutionStreamError::WrongListValueType(ty))?;
        let objects: Arc<Vec<_>> = Arc::new(
            bound
                .problem()
                .lists()
                .values()
                .iter()
                .map(|v| SolutionStreamObject {
                    allocation: v.clone(),
                    path: path.clone(),
                    marker: std::marker::PhantomData,
                })
                .collect(),
        );
        let selected = builder.filter(sources.lists.values.port(), move |row| {
            !initialized_only || row.position.is_some()
        })?;
        Ok(builder.map(&selected, move |row| WorkingListValue {
            object: objects[row.range_index].clone(),
            state: row.clone(),
        })?)
    }
}
