//! Transparent bounded decorators for problem-specific move providers.
use crate::{MoveProvider, MoveProviderEvent, NeighborhoodCountError, ProviderCacheScope};

/// Limit proposals per opening, including proposals a downstream filter rejects.
/// Zero skips the child factory. Dropping or exhausting a cursor never reopens it.
/// Lifecycle and construction-completion notifications remain delegated.
pub struct LimitedMoveProvider<Q> {
    provider: Q,
    maximum: usize,
}
impl<Q> LimitedMoveProvider<Q> {
    pub fn new(provider: Q, maximum: usize) -> Self {
        Self { provider, maximum }
    }
    pub fn provider(&self) -> &Q {
        &self.provider
    }
    pub fn provider_mut(&mut self) -> &mut Q {
        &mut self.provider
    }
    pub fn into_inner(self) -> Q {
        self.provider
    }
}
impl<P, A, M, Q: MoveProvider<P, A, M>> MoveProvider<P, A, M> for LimitedMoveProvider<Q> {
    fn neighborhood_count(&self) -> Result<usize, NeighborhoodCountError> {
        self.provider.neighborhood_count()
    }
    fn event(&mut self, event: MoveProviderEvent) {
        self.provider.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.provider.construction_placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        if self.maximum == 0 {
            Box::new(std::iter::empty())
        } else {
            Box::new(self.provider.moves(p, a, step).take(self.maximum))
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSnapshotError {
    pub maximum: usize,
}
impl std::fmt::Display for ProviderSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "move provider exceeds snapshot limit {}", self.maximum)
    }
}
impl std::error::Error for ProviderSnapshotError {}

/// Cache a finite stream of owned moves in original order, including duplicates.
/// Select this adapter only when moves remain valid throughout the chosen scope.
/// For keys mapped against changing assignments use CachedMoveProvider instead:
/// cached list positions, dependency-dependent moves and pinned targets can stale.
/// Step scope rebuilds for every engine scan; phase/solve scopes explicitly retain
/// the first opening's moves until their respective boundary. Higher boundaries
/// also invalidate shorter-lived caches. Direct moves calls synthesize no events;
/// call reset between scopes when driving the adapter manually.
///
/// Collection uses at most maximum+1 reads and publishes only complete snapshots;
/// overflow returns a typed error from try_moves and panics at the infallible
/// provider boundary. Child callbacks/reads cannot be rolled back. Zero disables
/// collection without opening the child. Outputs clone lazily on consumption.
pub struct CachedProviderMoves<Q, M> {
    provider: Q,
    scope: ProviderCacheScope,
    maximum: usize,
    cached: Option<Vec<M>>,
}
impl<Q, M> CachedProviderMoves<Q, M> {
    pub fn new(provider: Q, scope: ProviderCacheScope, maximum: usize) -> Self {
        Self {
            provider,
            scope,
            maximum,
            cached: None,
        }
    }
    pub fn provider(&self) -> &Q {
        &self.provider
    }
    /// Editing the child invalidates the retained snapshot first.
    pub fn provider_mut(&mut self) -> &mut Q {
        self.reset();
        &mut self.provider
    }
    pub fn into_inner(self) -> Q {
        self.provider
    }
    pub fn reset(&mut self) {
        drop(self.cached.take());
    }
    pub fn cached(&self) -> Option<&[M]> {
        self.cached.as_deref()
    }
    pub fn try_moves<'a, P, A>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = M> + 'a>, ProviderSnapshotError>
    where
        Q: MoveProvider<P, A, M>,
        M: Clone + 'a,
    {
        if self.maximum == 0 {
            return Ok(Box::new(std::iter::empty()));
        }
        if self.cached.is_none() {
            let mut moves = Vec::new();
            for movement in self.provider.moves(p, a, step) {
                if moves.len() == self.maximum {
                    return Err(ProviderSnapshotError {
                        maximum: self.maximum,
                    });
                }
                moves.push(movement);
            }
            self.cached = Some(moves);
        }
        Ok(Box::new(
            self.cached
                .as_ref()
                .expect("complete snapshot")
                .iter()
                .cloned(),
        ))
    }
}
impl<P, A, M: Clone, Q: MoveProvider<P, A, M>> MoveProvider<P, A, M> for CachedProviderMoves<Q, M> {
    fn neighborhood_count(&self) -> Result<usize, NeighborhoodCountError> {
        self.provider.neighborhood_count()
    }
    fn event(&mut self, event: MoveProviderEvent) {
        let invalidate = match event {
            MoveProviderEvent::SolveStarted | MoveProviderEvent::SolveEnded => true,
            MoveProviderEvent::PhaseStarted | MoveProviderEvent::PhaseEnded => {
                self.scope != ProviderCacheScope::Solve
            }
            MoveProviderEvent::StepStarted | MoveProviderEvent::StepEnded => {
                self.scope == ProviderCacheScope::Step
            }
        };
        if invalidate {
            self.reset();
        }
        self.provider.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.provider.construction_placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.try_moves(p, a, step)
            .unwrap_or_else(|error| panic!("{error}"))
    }
}
