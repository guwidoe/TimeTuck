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
    clock: C,
    last: Duration,
    expired: bool,
}
impl<C: FnMut() -> Duration> TimeBudget<C> {
    pub(crate) fn new(limit: Option<Duration>, clock: C) -> Self {
        Self {
            limit: limit.map(ElapsedTimeLimit::new),
            clock,
            last: Duration::ZERO,
            expired: false,
        }
    }
    pub(crate) fn expired(&mut self) -> bool {
        if self.expired {
            return true;
        }
        if let Some(limit) = self.limit {
            // Defensive clamp for an injected clock that moves backwards. Normal
            // solves use Instant; elapsed expiry is irreversible within a phase.
            self.last = self.last.max((self.clock)());
            self.expired = limit.is_reached(self.last);
        }
        self.expired
    }
}
