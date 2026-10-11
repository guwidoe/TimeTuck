//! Cooperative elapsed-time budgets measured by a monotonic clock.
use std::time::{Duration, Instant};

/// An elapsed-time threshold with a capped progress estimate. Zero is immediately
/// reached. Use the exact threshold decision rather than a rounded fraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElapsedTimeLimit(Duration);
impl ElapsedTimeLimit {
    pub const fn new(limit: Duration) -> Self {
        Self(limit)
    }
    pub fn is_reached(self, elapsed: Duration) -> bool {
        elapsed >= self.0
    }
    pub fn progress(self, elapsed: Duration) -> f64 {
        if self.is_reached(elapsed) {
            1.0
        } else {
            elapsed.as_secs_f64() / self.0.as_secs_f64()
        }
    }
}

pub(crate) fn monotonic_clock(enabled: bool) -> impl FnMut() -> Duration {
    let start = enabled.then(Instant::now);
    move || start.map_or(Duration::ZERO, |s| s.elapsed())
}

/// Optional solve-wide observer; phase timers and step counters stay independent.
pub(crate) trait SearchControl<S = crate::SimpleScore> {
    fn clock_session(&self) -> Option<&crate::solver_clock::ClockSession> {
        None
    }

    fn score_counter(&self) -> Option<crate::score_evaluations::ScoreCounter> {
        None
    }
    fn move_counter(&self) -> Option<crate::move_metrics::MoveCounter> {
        None
    }
    fn monitors_step_scores(&self) -> bool {
        false
    }
    fn record_step_score(&mut self, _score: S, _complete: bool) {}
    fn step_started(&mut self) {}
    fn step_ended(&mut self, _completed: bool) {}
    /// Construction does not start/reset search stagnation or advance search steps.
    fn complete_construction_step(&mut self) {}
    fn construction_stopped(&mut self) -> bool {
        false
    }
    /// Whether construction must publish complete scores for a scored condition.
    fn monitors_complete_scores(&self) -> bool {
        false
    }
    /// A retained complete best; ordinary best tracking may include partial custom states.
    fn observe_complete_best_score(&mut self, score: &S) {
        self.observe_goal_score(score);
        self.observe_best_score(score);
    }
    fn observe_goal_score(&mut self, _score: &S) {}
    fn observe_construction_score(&mut self, _score: &S) {}
    /// Observe a retained best, after search starts; no candidate rescoring.
    fn observe_best_score(&mut self, _score: &S) {}
    fn start_search(&mut self);
    fn record_improvement(&mut self);
    fn complete_step(&mut self, improved: bool);
    fn stop(&mut self) -> Option<crate::LocalSearchTermination>;
}

pub(crate) struct TimeBudget<'m, C, S = crate::SimpleScore> {
    shared: Option<&'m mut dyn SearchControl<S>>,
    limit: Option<ElapsedTimeLimit>,
    unimproved_limit: Option<crate::UnimprovedTimeLimit>,
    clock: C,
    last: Duration,
    reason: Option<crate::LocalSearchTermination>,
}
impl<'m, C: FnMut() -> Duration, S> TimeBudget<'m, C, S> {
    pub(crate) fn new(limit: Option<Duration>, unimproved: Option<Duration>, clock: C) -> Self {
        Self {
            shared: None,
            limit: limit.map(ElapsedTimeLimit::new),
            unimproved_limit: unimproved.map(crate::UnimprovedTimeLimit::new),
            clock,
            last: Duration::ZERO,
            reason: None,
        }
    }
    pub(crate) fn with_shared(mut self, shared: Option<&'m mut dyn SearchControl<S>>) -> Self {
        self.shared = shared;
        self
    }
    pub(crate) fn start_step(&mut self) {
        if let Some(shared) = &mut self.shared {
            shared.step_started();
        }
    }
    pub(crate) fn end_step(
        &mut self,
        completed: bool,
        score: impl FnOnce() -> S,
        complete: impl FnOnce() -> bool,
    ) {
        if let Some(shared) = &mut self.shared {
            finish_step(*shared, completed, score, complete);
        }
    }
    pub(crate) fn complete_step(&mut self, improved: bool) {
        if let Some(shared) = &mut self.shared {
            shared.complete_step(improved);
        }
    }
    fn sample(&mut self) {
        // Also protect tests with injected clocks against backwards samples.
        self.last = self.last.max((self.clock)());
    }
    pub(crate) fn elapsed(&mut self) -> Duration {
        self.sample();
        self.last
    }
    pub(crate) fn observe_best_score(&mut self, score: &S) {
        if let Some(shared) = &mut self.shared {
            shared.observe_complete_best_score(score);
        }
    }
    pub(crate) fn start_search(&mut self) {
        if let Some(shared) = &mut self.shared {
            shared.start_search();
        }
        if self.unimproved_limit.is_some() {
            self.sample();
            if let Some(limit) = &mut self.unimproved_limit {
                limit.start(self.last);
            }
        }
    }
    pub(crate) fn record_improvement(&mut self) {
        if let Some(shared) = &mut self.shared {
            shared.record_improvement();
        }
        // Poll before resetting: a late improvement must not revive an expired
        // search. Preserve that improvement in the result regardless of expiry.
        if self.unimproved_limit.is_some()
            && self.stop().is_none()
            && let Some(limit) = &mut self.unimproved_limit
        {
            limit.record_improvement(self.last);
        }
    }
    pub(crate) fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
        use crate::LocalSearchTermination::{TimeLimit, UnimprovedTimeLimit};
        if self.reason.is_none()
            && (self.limit.is_some() || self.unimproved_limit.is_some() || self.shared.is_some())
        {
            if self.limit.is_some() || self.unimproved_limit.is_some() {
                self.sample();
            }
            self.reason = if self.limit.is_some_and(|limit| limit.is_reached(self.last)) {
                Some(TimeLimit)
            } else if self
                .unimproved_limit
                .as_mut()
                .is_some_and(|limit| limit.is_reached(self.last))
            {
                Some(UnimprovedTimeLimit)
            } else {
                self.shared.as_mut().and_then(|shared| shared.stop())
            };
        }
        self.reason
    }
    pub(crate) fn expired(&mut self) -> bool {
        self.stop().is_some()
    }
}

/// Publish only completed steps. Lazy closures avoid score clones and completion
/// scans when a solve has no metrics attached.
pub(crate) fn finish_step<S>(
    control: &mut dyn SearchControl<S>,
    completed: bool,
    score: impl FnOnce() -> S,
    complete: impl FnOnce() -> bool,
) {
    if completed && control.monitors_step_scores() {
        control.record_step_score(score(), complete());
    }
    control.step_ended(completed);
}

/// Construction score getters may clone, so call them only for active scored
/// conditions and complete states. This does not arm search stagnation.
pub(crate) fn observe_construction_score<S>(
    shared: &mut Option<&mut dyn SearchControl<S>>,
    score: impl FnOnce() -> S,
    complete: impl FnOnce() -> bool,
) {
    if let Some(shared) = shared
        && shared.monitors_complete_scores()
        && complete()
    {
        shared.observe_construction_score(&score());
    }
}

#[cfg(test)]
mod step_metric_tests {
    use super::*;
    #[derive(Default)]
    struct Control {
        enabled: bool,
        published: Option<(i32, bool)>,
        ended: bool,
    }
    impl SearchControl<i32> for Control {
        fn monitors_step_scores(&self) -> bool {
            self.enabled
        }
        fn record_step_score(&mut self, score: i32, complete: bool) {
            self.published = Some((score, complete));
        }
        fn step_ended(&mut self, _: bool) {
            self.ended = true;
        }
        fn start_search(&mut self) {}
        fn record_improvement(&mut self) {}
        fn complete_step(&mut self, _: bool) {}
        fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
            None
        }
    }
    #[test]
    fn disabled_or_incomplete_steps_do_not_clone_scores_or_scan_completeness() {
        for (enabled, completed) in [(false, true), (true, false)] {
            let mut c = Control {
                enabled,
                ..Default::default()
            };
            finish_step(
                &mut c,
                completed,
                || panic!("unneeded score"),
                || panic!("unneeded completeness"),
            );
            assert!(c.ended);
            assert!(c.published.is_none());
        }
        let mut c = Control {
            enabled: true,
            ..Default::default()
        };
        finish_step(&mut c, true, || 7, || false);
        assert_eq!(c.published, Some((7, false)));
        assert!(c.ended);
    }
}
