//! Explicit lifecycle for time since the last retained-best improvement.
use crate::ElapsedTimeLimit;
use std::{error::Error, fmt, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegativeUnimprovedTime;
impl fmt::Display for NegativeUnimprovedTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unimproved time limit cannot be negative")
    }
}
impl Error for NegativeUnimprovedTime {}

/// A cooperative timer armed explicitly after construction/initialization.
/// Before start, expiry is false and progress is zero, even for a zero limit.
/// Improvements reset the anchor only before expiry; late improvements cannot
/// revive a stopped search. Backward timestamps clamp to the latest observation.
/// Use reset during construction, then start when improvement search begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnimprovedTimeLimit {
    limit: ElapsedTimeLimit,
    anchor: Option<Duration>,
    last: Duration,
    reached: bool,
}
impl UnimprovedTimeLimit {
    pub const fn new(limit: Duration) -> Self {
        Self {
            limit: ElapsedTimeLimit::new(limit),
            anchor: None,
            last: Duration::ZERO,
            reached: false,
        }
    }
    pub fn from_millis(limit: i64) -> Result<Self, NegativeUnimprovedTime> {
        let millis = u64::try_from(limit).map_err(|_| NegativeUnimprovedTime)?;
        Ok(Self::new(Duration::from_millis(millis)))
    }
    /// Begin a new active interval. An explicit start may restart an expired
    /// timer; ordinary improvements never do. Timestamp monotonicity is retained.
    pub fn start(&mut self, now: Duration) {
        self.last = self.last.max(now);
        self.anchor = Some(self.last);
        self.reached = false;
    }
    /// Return to the inactive state, also resetting the timestamp origin.
    pub fn reset(&mut self) {
        self.anchor = None;
        self.last = Duration::ZERO;
        self.reached = false;
    }
    pub fn is_started(&self) -> bool {
        self.anchor.is_some()
    }
    fn sample(&mut self, now: Duration) {
        self.last = self.last.max(now);
        if let Some(anchor) = self.anchor {
            self.reached |= self.limit.is_reached(self.last - anchor);
        }
    }
    pub fn is_reached(&mut self, now: Duration) -> bool {
        self.sample(now);
        self.reached
    }
    /// Notify a strictly better retained score, using its observation time.
    /// Callers decide score improvement; ties must not call this method.
    pub fn record_improvement(&mut self, now: Duration) {
        self.sample(now);
        if !self.reached && self.anchor.is_some() {
            self.anchor = Some(self.last);
        }
    }
    pub fn progress(&mut self, now: Duration) -> f64 {
        self.sample(now);
        self.anchor
            .map_or(0.0, |anchor| self.limit.progress(self.last - anchor))
    }
}

#[cfg(test)]
#[path = "unimproved_time_tests.rs"]
mod tests;
