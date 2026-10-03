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

pub(crate) struct TimeBudget<C> {
    limit: Option<ElapsedTimeLimit>,
    unimproved_limit: Option<ElapsedTimeLimit>,
    clock: C,
    last: Duration,
    last_improvement: Duration,
    reason: Option<crate::LocalSearchTermination>,
}
impl<C: FnMut() -> Duration> TimeBudget<C> {
    pub(crate) fn new(limit: Option<Duration>, unimproved: Option<Duration>, clock: C) -> Self {
        Self {
            limit: limit.map(ElapsedTimeLimit::new),
            unimproved_limit: unimproved.map(ElapsedTimeLimit::new),
            clock,
            last: Duration::ZERO,
            last_improvement: Duration::ZERO,
            reason: None,
        }
    }
    fn sample(&mut self) {
        // Also protect tests with injected clocks against backwards samples.
        self.last = self.last.max((self.clock)());
    }
    pub(crate) fn start_search(&mut self) {
        if self.unimproved_limit.is_some() {
            self.sample();
            self.last_improvement = self.last;
        }
    }
    pub(crate) fn record_improvement(&mut self) {
        // Poll before resetting: a late improvement must not revive an expired
        // search. Preserve that improvement in the result regardless of expiry.
        if self.unimproved_limit.is_some() && self.stop().is_none() {
            self.last_improvement = self.last;
        }
    }
    pub(crate) fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
        use crate::LocalSearchTermination::{TimeLimit, UnimprovedTimeLimit};
        if self.reason.is_none() && (self.limit.is_some() || self.unimproved_limit.is_some()) {
            self.sample();
            self.reason = if self.limit.is_some_and(|limit| limit.is_reached(self.last)) {
                Some(TimeLimit)
            } else if self
                .unimproved_limit
                .is_some_and(|limit| limit.is_reached(self.last - self.last_improvement))
            {
                Some(UnimprovedTimeLimit)
            } else {
                None
            };
        }
        self.reason
    }
    pub(crate) fn expired(&mut self) -> bool {
        self.stop().is_some()
    }
}
