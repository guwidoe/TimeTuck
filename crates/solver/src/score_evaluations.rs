//! Optional live accounting for composed score-evaluation termination.
use crate::time_limit::SearchControl;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Default)]
pub(crate) struct ScoreCounter(Arc<AtomicU64>);
impl ScoreCounter {
    fn increment(&self) {
        let _ = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(1))
            });
    }
    pub(crate) fn total(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}
pub(crate) fn from_control<S>(shared: &Option<&mut dyn SearchControl<S>>) -> Option<ScoreCounter> {
    shared.as_ref().and_then(|s| s.score_counter())
}
pub(crate) fn record(counter: &Option<ScoreCounter>) {
    if let Some(counter) = counter {
        counter.increment();
    }
}

/// Rebuilds contribute scores to the solve, but their placements are not outer
/// construction steps and their moves keep the existing reconstruction family.
/// Install this proxy for live score termination or an injected solve clock.
pub(crate) fn rebuilding<S, R>(
    shared: &mut Option<&mut dyn SearchControl<S>>,
    run: impl FnOnce(Option<&mut dyn SearchControl<S>>) -> R,
) -> R {
    rebuilding_inner(shared, false, run)
}

/// Make successfully scored candidates visible to expressions while retaining
/// transactional family publication: the outer repair publishes only on success.
pub(crate) fn reconstructing<S, R>(
    shared: &mut Option<&mut dyn SearchControl<S>>,
    run: impl FnOnce(Option<&mut dyn SearchControl<S>>) -> R,
) -> R {
    rebuilding_inner(shared, true, run)
}
fn rebuilding_inner<S, R>(
    shared: &mut Option<&mut dyn SearchControl<S>>,
    reconstruction: bool,
    run: impl FnOnce(Option<&mut dyn SearchControl<S>>) -> R,
) -> R {
    let moves = (reconstruction
        && shared
            .as_ref()
            .is_some_and(|s| s.monitors_reconstruction_moves()))
    .then(crate::move_metrics::MoveCounter::default);
    if moves.is_none()
        && from_control(shared).is_none()
        && shared.as_ref().and_then(|s| s.clock_session()).is_none()
    {
        return run(None);
    }
    struct Rebuild<'a, S>(
        &'a mut dyn SearchControl<S>,
        Option<crate::move_metrics::MoveCounter>,
    );
    impl<S> SearchControl<S> for Rebuild<'_, S> {
        fn clock_session(&self) -> Option<&crate::solver_clock::ClockSession> {
            self.0.clock_session()
        }
        fn score_counter(&self) -> Option<ScoreCounter> {
            self.0.score_counter()
        }
        fn move_counter(&self) -> Option<crate::move_metrics::MoveCounter> {
            self.1.clone()
        }
        fn construction_stopped(&mut self) -> bool {
            self.stop().is_some()
        }
        fn start_search(&mut self) {}
        fn record_improvement(&mut self) {}
        fn complete_step(&mut self, _: bool) {}
        fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
            match &self.1 {
                Some(moves) => self
                    .0
                    .stop_with_reconstruction_moves(moves.snapshot().total()),
                None => self.0.stop(),
            }
        }
    }
    let mut monitor = Rebuild(
        &mut **shared.as_mut().expect("counter has a monitor"),
        moves,
    );
    run(Some(&mut monitor))
}
