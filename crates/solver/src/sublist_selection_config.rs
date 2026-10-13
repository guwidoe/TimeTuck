//! Span-specific options validated before looking up a shared replay stream.
use crate::{
    ConfiguredSelection, ConfiguredSelectionPolicy, ListError, ListRange, ListSelectionContext,
    ReplayingSelection, SelectionBuilder, SelectionConfig, SelectionRegistrationError,
    SelectionRegistry, SubListSize, SubListSource,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Default)]
pub struct SubListSelectionConfig {
    pub selection: SelectionConfig,
    pub minimum: Option<usize>,
    pub maximum: Option<usize>,
    pub replay_ref: Option<String>,
}

#[derive(Debug)]
pub enum SubListSelectionBuildError {
    ReplayOverrides,
    ReplayRequiresRegistry,
    Bounds(ListError),
    Registration(SelectionRegistrationError),
}
impl fmt::Display for SubListSelectionBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReplayOverrides => f.write_str("sublist replay has another property configured"),
            Self::ReplayRequiresRegistry => {
                f.write_str("sublist replay requires a recording registry")
            }
            Self::Bounds(e) => e.fmt(f),
            Self::Registration(e) => e.fmt(f),
        }
    }
}
impl Error for SubListSelectionBuildError {}

impl SubListSelectionConfig {
    /// Explicit span bounds, including a bound equal to the default, cannot
    /// override the recorder. Validation happens before any registry lookup.
    pub fn build_replaying(
        &self,
        registry: &SelectionRegistry<ListRange>,
    ) -> Result<ReplayingSelection<ListRange>, SubListSelectionBuildError> {
        if self.minimum.is_some()
            || self.maximum.is_some()
            || self.selection != SelectionConfig::default()
        {
            return Err(SubListSelectionBuildError::ReplayOverrides);
        }
        registry
            .replay(self.replay_ref.as_deref().unwrap_or(""))
            .map_err(SubListSelectionBuildError::Registration)
    }

    /// Build the existing lazy span source. Register this configured source
    /// through `SelectionRegistry::register` to give dependent choices a replay.
    /// Ordinary defaults allow spans from two values through the movable suffix.
    pub fn build<'c, V: Eq + Clone>(
        self,
        policy: ConfiguredSelectionPolicy,
        seed: u64,
    ) -> Result<
        ConfiguredSelection<ListSelectionContext<'c, V>, ListRange, SubListSource>,
        SubListSelectionBuildError,
    > {
        if self.replay_ref.is_some() {
            return Err(SubListSelectionBuildError::ReplayRequiresRegistry);
        }
        let bounds = SubListSize::new(
            self.minimum.unwrap_or(2),
            self.maximum.unwrap_or(usize::MAX),
        )
        .map_err(SubListSelectionBuildError::Bounds)?;
        SelectionBuilder::new(self.selection)
            .build(policy, |request| {
                SubListSource::configured(request, bounds, seed)
            })
            .map_err(|e| SubListSelectionBuildError::Registration(e.into()))
    }
}
