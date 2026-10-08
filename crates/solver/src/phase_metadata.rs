//! Inspection of configured phase roles, shared with runtime initialization.
use crate::{MixedSolverPhase, MixedSolverSequence, SolverPhase, SolverPhaseKind, SolverSequence};

/// An owned description of one configured phase, before executing a solve.
/// `is_last_initializing` marks the phase immediately before the first local or
/// ruin/recreate search. It is false throughout a sequence with no improvement
/// phase or with improvement first. Exact search can initialize native models.
///
/// This describes configuration, not execution: limits, failure or an immovable
/// problem can prevent a phase from running or an initialization event from firing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolverPhaseMetadata {
    pub index: usize,
    pub kind: SolverPhaseKind,
    pub is_last_initializing: bool,
}
impl SolverPhaseKind {
    /// Whether a phase of this kind can construct an initial assignment. This
    /// describes capability even when the phase occurs after an improvement.
    /// Initial-only scoring is not a configured phase and cannot initialize.
    pub fn can_initialize(self) -> bool {
        matches!(self, Self::Construction | Self::Custom | Self::Exact)
    }
}
impl<S> SolverPhase<'_, S> {
    pub fn kind(&self) -> SolverPhaseKind {
        match self {
            Self::Construction(_) => SolverPhaseKind::Construction,
            Self::LocalSearch(_) => SolverPhaseKind::LocalSearch,
            Self::RuinRecreate(_) => SolverPhaseKind::RuinRecreate,
            Self::Exact(_) => SolverPhaseKind::Exact,
            Self::Custom(_) => SolverPhaseKind::Custom,
        }
    }
}
impl<S> MixedSolverPhase<'_, S> {
    pub fn kind(&self) -> SolverPhaseKind {
        match self {
            Self::Construction(_) => SolverPhaseKind::Construction,
            Self::LocalSearch(_) => SolverPhaseKind::LocalSearch,
            Self::RuinRecreate(_) => SolverPhaseKind::RuinRecreate,
            Self::Exact(_) => SolverPhaseKind::Exact,
            Self::Custom(_) => SolverPhaseKind::Custom,
        }
    }
}

pub(crate) fn initialization_index(
    kinds: impl IntoIterator<Item = SolverPhaseKind>,
) -> Option<usize> {
    kinds
        .into_iter()
        .position(|kind| {
            matches!(
                kind,
                SolverPhaseKind::LocalSearch | SolverPhaseKind::RuinRecreate
            )
        })
        .and_then(|index| index.checked_sub(1))
}
impl<S> SolverSequence<'_, S> {
    /// The configured initialization boundary, if any. Recomputed from current
    /// phases, so editing a sequence cannot leave stale metadata behind.
    pub fn initialization_phase(&self) -> Option<SolverPhaseMetadata> {
        let index = initialization_index(self.phases.iter().map(SolverPhase::kind))?;
        Some(SolverPhaseMetadata {
            index,
            kind: self.phases[index].kind(),
            is_last_initializing: true,
        })
    }
    /// Inspect all configured phases in order, without constructing score state.
    /// Empty sequences yield no metadata; solving still rejects an empty plan.
    pub fn phase_metadata(&self) -> impl ExactSizeIterator<Item = SolverPhaseMetadata> + '_ {
        let last = self.initialization_phase().map(|phase| phase.index);
        self.phases
            .iter()
            .enumerate()
            .map(move |(index, phase)| SolverPhaseMetadata {
                index,
                kind: phase.kind(),
                is_last_initializing: last == Some(index),
            })
    }
}
impl<S> MixedSolverSequence<'_, S> {
    /// Mixed-model counterpart of [`SolverSequence::initialization_phase`].
    pub fn initialization_phase(&self) -> Option<SolverPhaseMetadata> {
        let index = initialization_index(self.phases.iter().map(MixedSolverPhase::kind))?;
        Some(SolverPhaseMetadata {
            index,
            kind: self.phases[index].kind(),
            is_last_initializing: true,
        })
    }
    /// Mixed-model counterpart of [`SolverSequence::phase_metadata`].
    pub fn phase_metadata(&self) -> impl ExactSizeIterator<Item = SolverPhaseMetadata> + '_ {
        let last = self.initialization_phase().map(|phase| phase.index);
        self.phases
            .iter()
            .enumerate()
            .map(move |(index, phase)| SolverPhaseMetadata {
                index,
                kind: phase.kind(),
                is_last_initializing: last == Some(index),
            })
    }
}
