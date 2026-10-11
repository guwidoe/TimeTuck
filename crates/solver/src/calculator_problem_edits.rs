//! Typed edits to a calculator session's owned working problem.
use crate::{CalculatorSession, IncrementalCalculator, MatchRegistryError};
use std::{error::Error, fmt, marker::PhantomData};

/// Stable keys are resolved independently in these two object namespaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemObjectKind {
    Entity,
    Fact,
}

/// Result of resolving a stable key against the *current* working problem.
/// Implementations must detect duplicate keys rather than choosing an arbitrary
/// object. Handles may be indices: each edit resolves them afresh after prior edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemLookup<H> {
    Missing,
    Unique(H),
    Duplicate,
}

/// Typed notification supplied to both calculator change callbacks. `K` is an
/// application stable key, and `F` identifies a planning variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProblemEdit<K, F> {
    EntityAdded(K),
    EntityRemoved(K),
    VariableChanged { entity: K, variable: F },
    PropertyChanged { kind: ProblemObjectKind, key: K },
    FactAdded(K),
    FactRemoved(K),
}
impl<K, F> ProblemEdit<K, F> {
    fn target(&self) -> (ProblemObjectKind, &K, bool) {
        use ProblemObjectKind::{Entity, Fact};
        match self {
            Self::EntityAdded(key) => (Entity, key, true),
            Self::EntityRemoved(key) | Self::VariableChanged { entity: key, .. } => {
                (Entity, key, false)
            }
            Self::PropertyChanged { kind, key } => (*kind, key, false),
            Self::FactAdded(key) => (Fact, key, true),
            Self::FactRemoved(key) => (Fact, key, false),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProblemEditError {
    Missing(ProblemObjectKind),
    Duplicate(ProblemObjectKind),
    Scoring(MatchRegistryError),
}
impl From<MatchRegistryError> for ProblemEditError {
    fn from(error: MatchRegistryError) -> Self {
        Self::Scoring(error)
    }
}
impl fmt::Display for ProblemEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(kind) => write!(f, "missing {kind:?} stable key"),
            Self::Duplicate(kind) => write!(f, "duplicate {kind:?} stable key"),
            Self::Scoring(error) => error.fmt(f),
        }
    }
}
impl Error for ProblemEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(error) => Some(error),
            _ => None,
        }
    }
}

/// Exclusive edit batch, created by [`CalculatorSession::edit_problem`].
/// Mutations must match their descriptor and preserve stable-key uniqueness.
/// Before/after notifications happen immediately; derived-state propagation is
/// deferred until [`Self::flush`] or successful batch return. Entity membership
/// changes coalesce into one full calculator reset; other edits use its optional
/// `flush_changes` hook. No operation implicitly calculates a score.
///
/// Errors and panics invalidate the session, including errors caught inside the
/// batch. Earlier mutations are not rolled back. Recovery is an explicit
/// [`CalculatorSession::reset_current`] or [`CalculatorSession::reset`].
pub struct ProblemEdits<'a, P, C, K, F, H, L>
where
    C: IncrementalCalculator<P, Change = ProblemEdit<K, F>>,
{
    session: &'a mut CalculatorSession<P, C>,
    lookup: L,
    pending: bool,
    membership: bool,
    poisoned: bool,
    finished: bool,
    marker: PhantomData<fn(K, F) -> H>,
}
impl<P, C: IncrementalCalculator<P>> CalculatorSession<P, C> {
    /// Edit owned problem objects by stable key, then propagate deferred state
    /// once. Lookup runs before calculator notifications or mutation, and must
    /// report missing and duplicate keys accurately. Adding requires a missing
    /// key; every other operation requires a unique current object.
    ///
    /// The calculator uses [`ProblemEdit`] as its `Change` type. Its existing
    /// before/after hooks receive each typed edit. Successful entity additions or
    /// removals request a reset; other edits request `flush_changes` (a no-op by
    /// default). The same calculator instance survives both paths, and scoring is
    /// explicit. An empty successful batch invokes no calculator callbacks.
    ///
    /// Batch errors/panics leave the mutated problem readable but unscorable
    /// until reset. This is not a transactional rollback API. A flush failure is
    /// returned directly; there is no automatic fallback reset.
    pub fn edit_problem<K, F, H, L, R>(
        &mut self,
        lookup: L,
        edit: impl FnOnce(&mut ProblemEdits<'_, P, C, K, F, H, L>) -> Result<R, ProblemEditError>,
    ) -> Result<R, ProblemEditError>
    where
        C: IncrementalCalculator<P, Change = ProblemEdit<K, F>>,
        L: FnMut(&P, ProblemObjectKind, &K) -> ProblemLookup<H>,
    {
        self.require_ready()?;
        let mut batch = ProblemEdits {
            session: self,
            lookup,
            pending: false,
            membership: false,
            poisoned: false,
            finished: false,
            marker: PhantomData,
        };
        let result = edit(&mut batch)?;
        batch.flush()?;
        batch.finished = true;
        Ok(result)
    }
}
impl<P, C, K, F, H, L> ProblemEdits<'_, P, C, K, F, H, L>
where
    C: IncrementalCalculator<P, Change = ProblemEdit<K, F>>,
    L: FnMut(&P, ProblemObjectKind, &K) -> ProblemLookup<H>,
{
    pub fn solution(&self) -> &P {
        self.session.solution()
    }

    fn apply(
        &mut self,
        change: ProblemEdit<K, F>,
        update: impl FnOnce(&mut P, Option<H>),
    ) -> Result<(), ProblemEditError> {
        if self.poisoned {
            return Err(MatchRegistryError::NeedsReset.into());
        }
        // Set before lookup too: even a panic caught by the batch caller poisons
        // this editor and cannot be hidden by subsequently returning Ok.
        self.poisoned = true;
        let (kind, key, adding) = change.target();
        let handle = match ((self.lookup)(self.session.solution(), kind, key), adding) {
            (ProblemLookup::Missing, true) => None,
            (ProblemLookup::Unique(handle), false) => Some(handle),
            (ProblemLookup::Missing, false) => return Err(ProblemEditError::Missing(kind)),
            _ => return Err(ProblemEditError::Duplicate(kind)),
        };
        let membership = matches!(
            change,
            ProblemEdit::EntityAdded(_) | ProblemEdit::EntityRemoved(_)
        );
        self.session
            .change(&change, |problem| update(problem, handle))?;
        drop(change);
        self.pending = true;
        self.membership |= membership;
        self.poisoned = false;
        Ok(())
    }
    pub fn add_entity(
        &mut self,
        key: K,
        update: impl FnOnce(&mut P),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::EntityAdded(key), |p, _| update(p))
    }
    pub fn remove_entity(
        &mut self,
        key: K,
        update: impl FnOnce(&mut P, H),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::EntityRemoved(key), |p, h| {
            update(p, h.expect("resolved handle"))
        })
    }
    pub fn change_variable(
        &mut self,
        entity: K,
        variable: F,
        update: impl FnOnce(&mut P, H),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::VariableChanged { entity, variable }, |p, h| {
            update(p, h.expect("resolved handle"))
        })
    }
    pub fn change_property(
        &mut self,
        kind: ProblemObjectKind,
        key: K,
        update: impl FnOnce(&mut P, H),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::PropertyChanged { kind, key }, |p, h| {
            update(p, h.expect("resolved handle"))
        })
    }
    pub fn add_fact(
        &mut self,
        key: K,
        update: impl FnOnce(&mut P),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::FactAdded(key), |p, _| update(p))
    }
    pub fn remove_fact(
        &mut self,
        key: K,
        update: impl FnOnce(&mut P, H),
    ) -> Result<(), ProblemEditError> {
        self.apply(ProblemEdit::FactRemoved(key), |p, h| {
            update(p, h.expect("resolved handle"))
        })
    }
    /// Propagate only pending changes. Entity membership requires one reset and
    /// expires match registrations; an incremental flush preserves registrations
    /// unless the calculator explicitly cancels them. Neither path scores.
    pub fn flush(&mut self) -> Result<(), ProblemEditError> {
        if self.poisoned {
            return Err(MatchRegistryError::NeedsReset.into());
        }
        if !self.pending {
            return Ok(());
        }
        self.poisoned = true;
        if self.membership {
            self.session.reset_current()?;
        } else {
            self.session.flush_current()?;
        }
        self.pending = false;
        self.membership = false;
        self.poisoned = false;
        Ok(())
    }
}
impl<P, C, K, F, H, L> Drop for ProblemEdits<'_, P, C, K, F, H, L>
where
    C: IncrementalCalculator<P, Change = ProblemEdit<K, F>>,
{
    fn drop(&mut self) {
        if !self.finished {
            self.session.invalidate();
        }
    }
}
