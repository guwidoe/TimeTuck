//! Exact count thresholds and bounded progress estimates.

/// A nonnegative count limit, independent of what is being counted. In native
/// search, score calculations include initialization; evaluated moves do not. Neither count
/// includes diagnostic score recomputations. Compare exact counts for stopping:
/// floating progress can round up near large limits before they are reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountLimit(u64);
impl CountLimit {
    pub const fn new(limit: u64) -> Self {
        Self(limit)
    }
    pub const fn is_reached(self, count: u64) -> bool {
        count >= self.0
    }
    pub const fn remaining(self, count: u64) -> u64 {
        self.0.saturating_sub(count)
    }
    /// Capped fraction in [0,1]. A zero limit has progress 1, including at count 0.
    pub fn progress(self, count: u64) -> f64 {
        if self.is_reached(count) {
            1.0
        } else {
            count as f64 / self.0 as f64
        }
    }
}
impl TryFrom<i64> for CountLimit {
    type Error = std::num::TryFromIntError;
    fn try_from(limit: i64) -> Result<Self, Self::Error> {
        u64::try_from(limit).map(Self::new)
    }
}

// The reason is fixed before selection. Recomputing it from post-selection counts
// would lose which limit was tighter, since both remaining counts may now be zero.
#[derive(Clone, Copy)]
pub(crate) struct CandidateBudget {
    pub remaining: u64,
    move_limited: bool,
}
impl CandidateBudget {
    pub fn new(max_evaluations: u64, max_moves: Option<u64>, evaluations: u64) -> Self {
        let scores = CountLimit::new(max_evaluations).remaining(evaluations);
        let moves =
            max_moves.map(|limit| CountLimit::new(limit).remaining(evaluations.saturating_sub(1)));
        Self {
            remaining: moves.map_or(scores, |moves| moves.min(scores)),
            move_limited: moves.is_some_and(|moves| moves < scores),
        }
    }
    pub fn local_reason(self) -> crate::LocalSearchTermination {
        if self.move_limited {
            crate::LocalSearchTermination::MoveEvaluationLimit
        } else {
            crate::LocalSearchTermination::EvaluationLimit
        }
    }
    pub fn construction_reason(self) -> crate::ConstructionTermination {
        if self.move_limited {
            crate::ConstructionTermination::MoveEvaluationLimit
        } else {
            crate::ConstructionTermination::EvaluationLimit
        }
    }
}
