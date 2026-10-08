//! Typed named recording/replay wiring for configured selectors.
use crate::{
    ConfiguredSelection, ConfiguredSelectionPolicy, RecordingSelection, ReplayingSelection,
    SelectionBuildError, SelectionBuilder, SelectionSource, SelectionSourceRequest,
};
use std::{collections::BTreeMap, error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionRegistrationError {
    EmptyId,
    DuplicateId(String),
    UnknownId(String),
    ReplayOverrides,
    Build(SelectionBuildError),
}
impl fmt::Display for SelectionRegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selection registration: {self:?}")
    }
}
impl Error for SelectionRegistrationError {}
impl From<SelectionBuildError> for SelectionRegistrationError {
    fn from(e: SelectionBuildError) -> Self {
        Self::Build(e)
    }
}
/// A local typed registry owns replay handles, never the recording source. The
/// caller drives lifecycle only through the returned recorder; replay views do
/// not send duplicate events. All IDs here share one payload type. Distinct
/// registries separate entity/value/sublist namespaces without runtime downcasts.
/// Removing a name prevents future lookups; existing replay handles remain valid.
/// Replacing an ID requires explicit removal. Cloning a view never clones payloads.
pub struct SelectionRegistry<T> {
    entries: BTreeMap<String, ReplayingSelection<T>>,
}
impl<T> Default for SelectionRegistry<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}
impl<T> SelectionRegistry<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    fn validate_new(&self, id: &str) -> Result<(), SelectionRegistrationError> {
        if id.is_empty() {
            Err(SelectionRegistrationError::EmptyId)
        } else if self.entries.contains_key(id) {
            Err(SelectionRegistrationError::DuplicateId(id.to_owned()))
        } else {
            Ok(())
        }
    }
    pub fn register<C, S: SelectionSource<C, T>>(
        &mut self,
        id: impl Into<String>,
        source: S,
    ) -> Result<RecordingSelection<C, T, S>, SelectionRegistrationError> {
        let id = id.into();
        self.validate_new(&id)?;
        let recorder = RecordingSelection::new(source);
        self.entries.insert(id, recorder.replaying());
        Ok(recorder)
    }
    pub fn replay(&self, id: &str) -> Result<ReplayingSelection<T>, SelectionRegistrationError> {
        if id.is_empty() {
            return Err(SelectionRegistrationError::EmptyId);
        }
        self.entries
            .get(id)
            .cloned()
            .ok_or_else(|| SelectionRegistrationError::UnknownId(id.to_owned()))
    }
    pub fn remove(&mut self, id: &str) -> Option<ReplayingSelection<T>> {
        self.entries.remove(id)
    }
}
pub type RegisteredSelection<C, T, S> = RecordingSelection<C, T, ConfiguredSelection<C, T, S>>;
impl<C, T> SelectionBuilder<C, T> {
    /// A replay inherits its recorder's behavior. Reject non-default configuration
    /// and explicit optional settings/callbacks before registry lookup; limits belong on the recording side.
    pub fn build_replaying(
        &self,
        id: &str,
        registry: &SelectionRegistry<T>,
    ) -> Result<ReplayingSelection<T>, SelectionRegistrationError> {
        if self.has_replay_overrides() {
            return Err(SelectionRegistrationError::ReplayOverrides);
        }
        registry.replay(id)
    }
    /// Validate the name before constructing the source. Failed source/config
    /// construction leaves the registry unchanged. The recorder owns the entire
    /// configured pipeline and is the sole recipient of lifecycle events.
    pub fn build_recording<S: SelectionSource<C, T>>(
        self,
        policy: ConfiguredSelectionPolicy,
        factory: impl FnOnce(SelectionSourceRequest) -> S,
        id: impl Into<String>,
        registry: &mut SelectionRegistry<T>,
    ) -> Result<RegisteredSelection<C, T, S>, SelectionRegistrationError>
    where
        T: Clone,
    {
        let id = id.into();
        registry.validate_new(&id)?;
        let source = self.build(policy, factory)?;
        registry.register(id, source)
    }
}
