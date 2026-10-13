//! Optional owned dataset persistence and commands, independent of solver hot paths.
use crate::{JsonPatch, JsonPatchError};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    time::SystemTime,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerDatasetStatus {
    DatasetCreated,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerDatasetSelector {
    Input,
    Output,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerDatasetMetadata {
    pub id: String,
    pub name: String,
    pub status: WorkerDatasetStatus,
    pub submitted: SystemTime,
    pub tags: BTreeSet<String>,
    pub parent_id: Option<String>,
    pub origin_id: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WorkerDataset {
    pub metadata: WorkerDatasetMetadata,
    pub input: Value,
    pub configuration: Value,
    pub output: Option<Value>,
}
#[derive(Clone, Debug)]
pub struct NewWorkerDataset {
    pub name: String,
    pub tags: BTreeSet<String>,
    pub submitted: SystemTime,
    pub parent_id: Option<String>,
    pub input: Value,
    pub configuration: Value,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerDatasetEventKind {
    Created,
    ValidateCompute,
    SolveStart,
    SolveTerminate,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerDatasetEvent {
    Created { id: String },
    ValidateCompute { id: String, solve: bool },
    SolveStart { id: String },
    SolveTerminate { id: String },
}
impl WorkerDatasetEvent {
    pub fn kind(&self) -> WorkerDatasetEventKind {
        match self {
            Self::Created { .. } => WorkerDatasetEventKind::Created,
            Self::ValidateCompute { .. } => WorkerDatasetEventKind::ValidateCompute,
            Self::SolveStart { .. } => WorkerDatasetEventKind::SolveStart,
            Self::SolveTerminate { .. } => WorkerDatasetEventKind::SolveTerminate,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerFacadeError {
    NotFound(String),
    MissingOutput(String),
    EmptyName,
    IdExhausted,
    Patch(JsonPatchError),
    Event {
        dataset: String,
        kind: WorkerDatasetEventKind,
        reason: String,
    },
}
impl fmt::Display for WorkerFacadeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "worker dataset: {self:?}")
    }
}
impl Error for WorkerFacadeError {}
/// Store implementations commit a complete owned input/configuration/metadata
/// record together and assign identity/origin linkage. Reads return independent
/// snapshots. The facade never treats an emitted command as completed solving.
pub trait WorkerDatasetStore {
    fn get(&self, id: &str) -> Option<WorkerDataset>;
    fn insert(
        &mut self,
        dataset: NewWorkerDataset,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError>;
}
/// In-memory persistence with store-local monotonically allocated IDs. Applications
/// needing global identities/durable storage can supply their own store. Parents
/// are checked and origins derived from actual stored metadata before publication.
#[derive(Default)]
pub struct InMemoryWorkerDatasetStore {
    records: BTreeMap<String, WorkerDataset>,
    next: u64,
}
impl InMemoryWorkerDatasetStore {
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub fn set_output(&mut self, id: &str, output: Value) -> Result<(), WorkerFacadeError> {
        self.records
            .get_mut(id)
            .ok_or_else(|| WorkerFacadeError::NotFound(id.into()))?
            .output = Some(output);
        Ok(())
    }
}
impl WorkerDatasetStore for InMemoryWorkerDatasetStore {
    fn get(&self, id: &str) -> Option<WorkerDataset> {
        self.records.get(id).cloned()
    }
    fn insert(
        &mut self,
        dataset: NewWorkerDataset,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        if dataset.name.trim().is_empty() {
            return Err(WorkerFacadeError::EmptyName);
        }
        let parent = dataset
            .parent_id
            .as_ref()
            .map(|id| {
                self.records
                    .get(id)
                    .ok_or_else(|| WorkerFacadeError::NotFound(id.clone()))
            })
            .transpose()?;
        let next = self
            .next
            .checked_add(1)
            .ok_or(WorkerFacadeError::IdExhausted)?;
        let id = format!("dataset-{next}");
        let metadata = WorkerDatasetMetadata {
            id: id.clone(),
            name: dataset.name,
            status: WorkerDatasetStatus::DatasetCreated,
            submitted: dataset.submitted,
            tags: dataset.tags,
            parent_id: dataset.parent_id.clone(),
            origin_id: parent.map_or_else(|| id.clone(), |p| p.metadata.origin_id.clone()),
        };
        self.records.insert(
            id,
            WorkerDataset {
                metadata: metadata.clone(),
                input: dataset.input,
                configuration: dataset.configuration,
                output: None,
            },
        );
        self.next = next;
        Ok(metadata)
    }
}
/// Inline command/event delivery. An error leaves earlier external delivery and
/// persisted data intact; callers decide their retry/idempotency policy.
pub trait WorkerDatasetEventSink {
    fn emit(&mut self, event: WorkerDatasetEvent) -> Result<(), String>;
}
impl<F: FnMut(WorkerDatasetEvent) -> Result<(), String>> WorkerDatasetEventSink for F {
    fn emit(&mut self, event: WorkerDatasetEvent) -> Result<(), String> {
        self(event)
    }
}
/// Real dataset facade. Creation persists first, emits Created then ValidateCompute;
/// the latter's solve field requests downstream solving after validation. Explicit
/// solve only emits SolveStart after verifying the stored ID. Children own cloned
/// selected JSON and inherit origin through actual parent records. JSON patching
/// delegates the existing atomic patch implementation before storing any child.
pub struct WorkerDatasetFacade<S, E, C> {
    store: S,
    sink: E,
    clock: C,
}
impl<S: WorkerDatasetStore, E: WorkerDatasetEventSink, C: FnMut() -> SystemTime>
    WorkerDatasetFacade<S, E, C>
{
    pub fn new(store: S, sink: E, clock: C) -> Self {
        Self { store, sink, clock }
    }
    pub fn store(&self) -> &S {
        &self.store
    }
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
    pub fn sink(&self) -> &E {
        &self.sink
    }
    pub fn sink_mut(&mut self) -> &mut E {
        &mut self.sink
    }
    pub fn get_dataset(&self, id: &str) -> Result<WorkerDataset, WorkerFacadeError> {
        self.store
            .get(id)
            .ok_or_else(|| WorkerFacadeError::NotFound(id.into()))
    }
    pub fn get_metadata(&self, id: &str) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        Ok(self.get_dataset(id)?.metadata)
    }
    fn emit(&mut self, id: &str, event: WorkerDatasetEvent) -> Result<(), WorkerFacadeError> {
        let kind = event.kind();
        self.sink
            .emit(event)
            .map_err(|reason| WorkerFacadeError::Event {
                dataset: id.into(),
                kind,
                reason,
            })
    }
    fn persist(
        &mut self,
        dataset: NewWorkerDataset,
        solve: bool,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        let metadata = self.store.insert(dataset)?;
        self.emit(
            &metadata.id,
            WorkerDatasetEvent::Created {
                id: metadata.id.clone(),
            },
        )?;
        self.emit(
            &metadata.id,
            WorkerDatasetEvent::ValidateCompute {
                id: metadata.id.clone(),
                solve,
            },
        )?;
        Ok(metadata)
    }
    pub fn create_dataset(
        &mut self,
        name: String,
        tags: BTreeSet<String>,
        input: Value,
        configuration: Value,
        solve: bool,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        let submitted = (self.clock)();
        self.persist(
            NewWorkerDataset {
                name,
                tags,
                submitted,
                parent_id: None,
                input,
                configuration,
            },
            solve,
        )
    }
    fn selected(
        parent: &WorkerDataset,
        selector: WorkerDatasetSelector,
    ) -> Result<Value, WorkerFacadeError> {
        match selector {
            WorkerDatasetSelector::Input => Ok(parent.input.clone()),
            WorkerDatasetSelector::Output => parent
                .output
                .clone()
                .ok_or_else(|| WorkerFacadeError::MissingOutput(parent.metadata.id.clone())),
        }
    }
    pub fn create_from_dataset(
        &mut self,
        parent_id: &str,
        selector: WorkerDatasetSelector,
        name: String,
        tags: BTreeSet<String>,
        configuration: Value,
        solve: bool,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        let parent = self.get_dataset(parent_id)?;
        let input = Self::selected(&parent, selector)?;
        let submitted = (self.clock)();
        self.persist(
            NewWorkerDataset {
                name,
                tags,
                submitted,
                parent_id: Some(parent_id.into()),
                input,
                configuration,
            },
            solve,
        )
    }
    pub fn patch_dataset(
        &mut self,
        parent_id: &str,
        selector: WorkerDatasetSelector,
        name: String,
        configuration: Option<Value>,
        patch: &JsonPatch,
        solve: bool,
    ) -> Result<WorkerDatasetMetadata, WorkerFacadeError> {
        let parent = self.get_dataset(parent_id)?;
        let mut input = Self::selected(&parent, selector)?;
        patch.apply(&mut input).map_err(WorkerFacadeError::Patch)?;
        let submitted = (self.clock)();
        self.persist(
            NewWorkerDataset {
                name,
                tags: parent.metadata.tags,
                submitted,
                parent_id: Some(parent_id.into()),
                input,
                configuration: configuration.unwrap_or(parent.configuration),
            },
            solve,
        )
    }
    pub fn solve_dataset(&mut self, id: &str) -> Result<(), WorkerFacadeError> {
        self.get_dataset(id)?;
        self.emit(id, WorkerDatasetEvent::SolveStart { id: id.into() })
    }
}
