//! Current typed lists and value locations fed by native list source events.
use super::{
    CheckedSolutionStream, SolutionStreamError, SolutionStreamObject, SolutionStreamRegistry,
};
use crate::{
    AdditiveScore, BoundListSolution, ConcatId, GraphError, ListStreamSources, ListValueRow,
    StreamGraph, StreamGraphBuilder, StreamPort,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct WorkingListObject<T, V> {
    object: SolutionStreamObject<T>,
    variable: Option<Arc<str>>,
    values: Vec<V>,
}
impl<T, V: Clone> Clone for WorkingListObject<T, V> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            variable: self.variable.clone(),
            values: self.values.clone(),
        }
    }
}
impl<T: 'static, V> WorkingListObject<T, V> {
    pub fn object(&self) -> &T {
        self.object.get()
    }
    pub fn allocation(&self) -> &Arc<dyn Any + Send + Sync> {
        self.object.allocation()
    }
    pub fn list(&self, name: &str) -> Option<&[V]> {
        (self.variable.as_deref() == Some(name)).then_some(self.values.as_slice())
    }
    pub fn list_variable(&self) -> Option<&str> {
        self.variable.as_deref()
    }
}
/// The object is the original canonical allocation. Position and neighbor state
/// comes from the current native list assignment, independent of object getters.
pub struct WorkingListValue<T, V> {
    object: SolutionStreamObject<T>,
    state: ListValueRow<V>,
}
impl<T, V: Clone> Clone for WorkingListValue<T, V> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            state: self.state.clone(),
        }
    }
}
impl<T: 'static, V> WorkingListValue<T, V> {
    pub fn object(&self) -> &T {
        self.object.get()
    }
    pub fn allocation(&self) -> &Arc<dyn Any + Send + Sync> {
        self.object.allocation()
    }
    pub fn state(&self) -> &ListValueRow<V> {
        &self.state
    }
}
#[derive(Debug)]
pub enum WorkingListUpdateError {
    Source(SolutionStreamError),
    Graph(GraphError),
}
impl std::fmt::Display for WorkingListUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "working list source: {self:?}")
    }
}
impl std::error::Error for WorkingListUpdateError {}
impl From<GraphError> for WorkingListUpdateError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}
impl From<SolutionStreamError> for WorkingListUpdateError {
    fn from(e: SolutionStreamError) -> Self {
        Self::Source(e)
    }
}
pub type WorkingListModelPort<T, V> = StreamPort<ConcatId<usize, usize>, WorkingListObject<T, V>>;
pub type WorkingListValuePort<T, V> = StreamPort<usize, WorkingListValue<T, V>>;
pub struct WorkingListModelStream<T, V> {
    sources: ListStreamSources<V>,
    constant: CheckedSolutionStream<T>,
    rows: Vec<(usize, SolutionStreamObject<T>)>,
    port: WorkingListModelPort<T, V>,
}
impl<T, V> WorkingListModelStream<T, V> {
    pub fn sources(&self) -> &ListStreamSources<V> {
        &self.sources
    }
    pub fn port(&self) -> &WorkingListModelPort<T, V> {
        &self.port
    }
}
impl<T: 'static, V> WorkingListModelStream<T, V> {
    /// Initialize immutable facts/shadow-only objects while both native list
    /// sources remain empty for the score director to load and protect.
    pub fn initialize_constants<S: AdditiveScore + 'static>(
        &self,
        graph: &mut StreamGraph<'_, S>,
    ) -> Result<(), GraphError> {
        self.constant.refresh_membership(graph, self.rows.clone())
    }
}
impl SolutionStreamRegistry {
    pub fn working_list_model<T: 'static, V: Clone + 'static, S: AdditiveScore + 'static>(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
        bound: &BoundListSolution<V>,
    ) -> Result<WorkingListModelStream<T, V>, WorkingListUpdateError> {
        if !Arc::ptr_eq(&self.layout, bound.facts().descriptor()) {
            return Err(SolutionStreamError::DifferentDescriptor.into());
        }
        let constant = self.source::<T, S>(builder)?;
        let owners: HashMap<_, _> = bound
            .entities()
            .iter()
            .filter_map(|e| {
                Some((
                    e.list_index()?,
                    constant.row(e.entity_type(), e.object())?.1,
                ))
            })
            .collect();
        let owner_keys: HashSet<_> = owners
            .values()
            .map(|o| Arc::as_ptr(o.allocation()) as *const () as usize)
            .collect();
        let mut seen = HashSet::new();
        let rows = constant
            .list_bound(bound)?
            .into_iter()
            .filter(|(key, _)| !owner_keys.contains(key) && seen.insert(*key))
            .collect();
        let sources = ListStreamSources::new(builder);
        let owners = Arc::new(owners);
        let select = owners.clone();
        let selected = builder.filter(sources.entities.port(), move |(index, _)| {
            select.contains_key(index)
        })?;
        let variable: Arc<str> = Arc::from(bound.list_variable());
        let current = builder.map(&selected, move |(index, values)| WorkingListObject {
            object: owners[index].clone(),
            variable: Some(variable.clone()),
            values: values.clone(),
        })?;
        let constants = builder.map(constant.port(), |object| WorkingListObject {
            object: object.clone(),
            variable: None,
            values: Vec::new(),
        })?;
        let port = builder.concat(&current, &constants)?;
        Ok(WorkingListModelStream {
            sources,
            constant,
            rows,
            port,
        })
    }
    /// Attach a projected canonical value allocation to the same native sources
    /// used by the list score director. Compile identity routing once; each move
    /// maps only the actual affected value rows. No inverse/index getter is read.
    pub fn working_list_values<
        T: 'static,
        C: Any + Send + Sync + Eq,
        S: AdditiveScore + 'static,
    >(
        &self,
        builder: &mut StreamGraphBuilder<'_, S>,
        bound: &BoundListSolution<Arc<C>>,
        sources: &ListStreamSources<Arc<C>>,
        initialized_only: bool,
    ) -> Result<WorkingListValuePort<T, Arc<C>>, WorkingListUpdateError> {
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
                .values()
                .iter()
                .map(|value| SolutionStreamObject {
                    allocation: value.clone(),
                    path: path.clone(),
                    marker: std::marker::PhantomData,
                })
                .collect(),
        );
        let selected = builder.filter(sources.values.port(), move |row| {
            !initialized_only || row.position.is_some()
        })?;
        Ok(builder.map(&selected, move |row| WorkingListValue {
            object: objects[row.range_index].clone(),
            state: row.clone(),
        })?)
    }
}
