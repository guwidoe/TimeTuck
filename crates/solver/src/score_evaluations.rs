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
/// Only install this proxy when live score termination needs candidate polling.
pub(crate) fn rebuilding<S, R>(
    shared: &mut Option<&mut dyn SearchControl<S>>,
    run: impl FnOnce(Option<&mut dyn SearchControl<S>>) -> R,
) -> R {
    if from_control(shared).is_none() {
        return run(None);
    }
    struct Rebuild<'a, S>(&'a mut dyn SearchControl<S>);
    impl<S> SearchControl<S> for Rebuild<'_, S> {
        fn score_counter(&self) -> Option<ScoreCounter> {
            self.0.score_counter()
        }
        fn construction_stopped(&mut self) -> bool {
            self.0.stop().is_some()
        }
        fn start_search(&mut self) {}
        fn record_improvement(&mut self) {}
        fn complete_step(&mut self, _: bool) {}
        fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
            self.0.stop()
        }
    }
    let mut monitor = Rebuild(&mut **shared.as_mut().expect("counter has a monitor"));
    run(Some(&mut monitor))
}
