//! Explicit clocks for complete solver invocations.
use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

/// Optional time source for [`crate::SolverExtensions`]. Readings are arbitrary
/// ticks, rebased independently at each solve entry. All phase/rebuild deadlines
/// and stagnation windows use the same source with their own elapsed origins.
/// Backwards readings are clamped to the greatest elapsed reading in that solve.
///
/// Time advances only when the callback's source advances; a frozen clock does
/// not stop unbounded search. Limits remain cooperative, checked between solver
/// operations. Setup, scoring, diagnostics and observer time are charged as with
/// the default wall clock. Callback panics propagate. Metrics retain their own
/// separately configured clock. Standalone search APIs keep their usual clocks.
///
/// The callback may read an `Arc<AtomicU64>` to advance time from another thread.
/// Sharing this handle across solves shares only the source: each invocation has
/// independent origins and monotonic state. No scoring callbacks are added.
#[derive(Clone)]
pub struct SolverClock {
    read: Arc<dyn Fn() -> Duration + Send + Sync>,
}
impl SolverClock {
    pub fn new(read: impl Fn() -> Duration + Send + Sync + 'static) -> Self {
        Self {
            read: Arc::new(read),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ClockSession(Rc<ClockState>);
struct ClockState {
    source: SolverClock,
    origin: Duration,
    last: Cell<Duration>,
}
impl ClockSession {
    pub(crate) fn new(source: &SolverClock) -> Self {
        let origin = (source.read)();
        Self(Rc::new(ClockState {
            source: source.clone(),
            origin,
            last: Cell::new(Duration::ZERO),
        }))
    }
    pub(crate) fn elapsed(&self) -> Duration {
        let now = (self.0.source.read)()
            .saturating_sub(self.0.origin)
            .max(self.0.last.get());
        self.0.last.set(now);
        now
    }
}

/// Capture a new phase/rebuild origin. Disabled clocks never read or clone the
/// source. The ordinary path retains the existing optional Instant behavior.
pub(crate) fn elapsed_clock(
    enabled: bool,
    session: Option<&ClockSession>,
) -> impl Fn() -> Duration + use<> {
    let session = enabled.then(|| session.cloned()).flatten();
    let origin = session.as_ref().map(ClockSession::elapsed);
    let real = (enabled && session.is_none()).then(std::time::Instant::now);
    move || match &session {
        Some(session) => session
            .elapsed()
            .saturating_sub(origin.expect("session origin")),
        None => real.map_or(Duration::ZERO, |start| start.elapsed()),
    }
}

pub(crate) fn monitored_clock<S>(
    enabled: bool,
    shared: &Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> impl Fn() -> Duration + use<S> {
    elapsed_clock(enabled, shared.as_ref().and_then(|s| s.clock_session()))
}
