//! Problem-specific candidate generation using the existing local-search engine.

/// Supply candidates for one immutable assignment snapshot at a completed-step
/// boundary. The solver recreates this iterator each step and applies its normal
/// scoring, acceptance, tabu, foraging, rollback and stopping rules.
///
/// Providers replace built-in neighborhoods, union settings and mixed joint-move
/// generation. Variable shuffling does not reorder provider inputs; candidate
/// shuffling and tie breaking still apply. Invalid moves fail the search using
/// the existing model/scoring error. Returning no moves ends an ordinary scan.
/// Providers may retain state, but must not mutate the model or snapshot.
///
/// Iterator work is cooperative: factory/next calls must return promptly. Only
/// scored proposals count toward evaluation limits; candidate shuffling may
/// prefetch a bounded window. Infinite iterators require a search budget.
///
/// Closures returning Vec<M> implement this trait for convenience. Implement the
/// trait directly to return lazy iterators borrowing the current snapshot.
pub trait MoveProvider<P, A, M> {
    /// Receive real engine phase/scan boundaries. Direct calls to moves() do not
    /// synthesize events. Defaults to no state change, preserving resumable queues.
    /// Callbacks must return promptly and should not panic.
    fn event(&mut self, _event: MoveProviderEvent) {}

    /// Construction-only notification after a nonempty completed placement scan.
    /// KeepBestPrefix counts interrupted nonempty scans as completed steps;
    /// discarded atomic scans do not call this hook. Return true to try another
    /// placement when no assignment was installed (optional rejection/no-change).
    /// Default pooled providers stop on rejection. Returning true must make
    /// progress through a bounded placement schedule; score limits still apply.
    fn construction_placement_completed(&mut self, _installed: bool) -> bool {
        false
    }

    fn moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a;
}
impl<P, A, M, F> MoveProvider<P, A, M> for F
where
    F: FnMut(&P, &A, u64) -> Vec<M>,
{
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new(self(p, a, step).into_iter())
    }
}

pub(crate) enum ProvidedMoves<'a, M, I> {
    BuiltIn(I),
    Custom(Box<dyn Iterator<Item = M> + 'a>),
}
impl<M, I: Iterator<Item = M>> Iterator for ProvidedMoves<'_, M, I> {
    type Item = M;
    fn next(&mut self) -> Option<M> {
        match self {
            Self::BuiltIn(i) => i.next(),
            Self::Custom(i) => i.next(),
        }
    }
}

/// Lifecycle of provider-backed solver execution. Solver sequences deliver a
/// single solve scope to each distinct registered provider, starting lazily at the
/// first scoring kernel after plan validation. Runs that never enter a scoring
/// kernel emit no solve events; entering a zero-budget kernel still opens the
/// solve scope without opening a provider phase.
/// Standalone engines emit phase/scan events only. Phase events reach only providers
/// whose construction or local-search phase is entered.
/// No phase/scan events are emitted for failed preflight validation, a zero scoring budget or cancellation
/// detected before initialization. PhaseStarted precedes director creation; errors
/// returned afterward still close the phase. A scored phase may have no scans.
///
/// Step events bracket a candidate scan, including empty, rejected and discarded
/// scans; they do not imply a completed solver step. StepEnded follows iterator
/// destruction, move commit/rollback, consistency checks and construction completion
/// notification, before best-solution observers. A returned error closes an open
/// scan and phase. During panic unwinding no further callbacks run; owners must
/// reset affected provider state before reuse. Ordinary Rust destructors still run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveProviderEvent {
    /// Solver sequence entered its first scoring kernel. Sent once per distinct
    /// binding, including bindings for phases the run may never reach.
    SolveStarted,
    /// Solver sequence returned successfully or with an error. Not sent on panic.
    SolveEnded,
    PhaseStarted,
    StepStarted,
    StepEnded,
    PhaseEnded,
}

/// Reset a provider automatically at each entered phase, before forwarding its
/// PhaseStarted event. This is opt-in: ordinary providers may intentionally retain
/// scheduling state across budget-limited calls. The reset callback can also clear
/// application caches. All move and construction-completion behavior is delegated.
pub struct PhaseResetMoveProvider<Q, F> {
    provider: Q,
    reset: F,
}
impl<Q, F: FnMut(&mut Q)> PhaseResetMoveProvider<Q, F> {
    pub fn new(provider: Q, reset: F) -> Self {
        Self { provider, reset }
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
impl<P, A, M, Q: MoveProvider<P, A, M>, F: FnMut(&mut Q)> MoveProvider<P, A, M>
    for PhaseResetMoveProvider<Q, F>
{
    fn event(&mut self, event: MoveProviderEvent) {
        if event == MoveProviderEvent::PhaseStarted {
            (self.reset)(&mut self.provider);
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
        self.provider.moves(p, a, step)
    }
}

/// Own the callback boundary independently of the borrowed candidate iterator.
/// Dropping an iterator alone is too early: the director has not installed its
/// winner yet. Engines explicitly end scans after selection and accounting.
pub(crate) struct ProviderPhase<'r, P, A, M> {
    provider: Option<crate::MoveProviderBinding<'r, P, A, M>>,
    step: bool,
}
impl<'r, P, A, M> ProviderPhase<'r, P, A, M> {
    pub(crate) fn new(provider: Option<crate::MoveProviderBinding<'r, P, A, M>>) -> Self {
        let mut phase = Self {
            provider,
            step: false,
        };
        phase.event(MoveProviderEvent::PhaseStarted);
        phase
    }
    pub(crate) fn is_none(&self) -> bool {
        self.provider.is_none()
    }
    pub(crate) fn provider(&mut self) -> Option<&mut crate::MoveProviderBinding<'r, P, A, M>> {
        self.provider.as_mut()
    }
    fn event(&mut self, event: MoveProviderEvent) {
        if let Some(p) = &mut self.provider {
            p.event(event);
        }
    }
    pub(crate) fn start_step(&mut self) {
        debug_assert!(!self.step);
        self.step = true;
        self.event(MoveProviderEvent::StepStarted);
    }
    pub(crate) fn end_step(&mut self) {
        if std::mem::take(&mut self.step) {
            self.event(MoveProviderEvent::StepEnded);
        }
    }
}
impl<P, A, M> Drop for ProviderPhase<'_, P, A, M> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.end_step();
            self.event(MoveProviderEvent::PhaseEnded);
        }
    }
}
