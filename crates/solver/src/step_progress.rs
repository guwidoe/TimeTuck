//! Deterministic phase progress measured in completed steps.

/// A completed-step budget. Zero is immediately reached, with progress 1.0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepCountLimit(u64);
impl StepCountLimit {
    pub const fn new(limit: u64) -> Self {
        Self(limit)
    }
    pub const fn is_reached(self, steps: u64) -> bool {
        steps >= self.0
    }
    /// Fraction in [0,1], capped at the limit. Large counts use f64 precision.
    pub fn progress(self, steps: u64) -> f64 {
        if self.is_reached(steps) {
            1.0
        } else {
            steps as f64 / self.0 as f64
        }
    }
}
/// Counts completed steps and steps since the last strict global-best improvement.
/// The initial solution precedes step 1. Ties, worsening steps, and recovery to an
/// earlier best do not reset stagnation. A selected rejected fallback still counts.
///
/// ```
/// use timetuck::{StepProgress,StepCountLimit};
/// let mut p=StepProgress::default();
/// p.complete_step(true);
/// p.complete_step(false);
/// assert_eq!(p.completed_steps(),2);
/// assert_eq!(p.unimproved_steps(),1);
/// assert_eq!(StepCountLimit::new(4).progress(p.unimproved_steps()),0.25);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StepProgress {
    completed: u64,
    best_step: u64,
}
impl StepProgress {
    pub const fn completed_steps(self) -> u64 {
        self.completed
    }
    pub const fn best_step(self) -> u64 {
        self.best_step
    }
    pub const fn unimproved_steps(self) -> u64 {
        self.completed - self.best_step
    }
    pub(crate) fn record_improvement(&mut self) {
        self.best_step = self.completed;
    }
    /// Record whether this completed step strictly improved the global best.
    /// Panics without changing state if the completed count would overflow u64.
    pub fn complete_step(&mut self, improved: bool) {
        let next = self
            .completed
            .checked_add(1)
            .expect("completed step count overflow");
        self.completed = next;
        if improved {
            self.best_step = next;
        }
    }
}
