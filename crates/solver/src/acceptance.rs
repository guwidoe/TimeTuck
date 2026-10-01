//! Acceptance configurations and per-search state.
use crate::Score;
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance {
    StrictImprovement,
    HillClimbing,
    LateAcceptance {
        history_size: usize,
        hill_climbing: bool,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceError {
    EmptyHistory,
    HistoryAllocation,
}
impl fmt::Display for AcceptanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyHistory => "late-acceptance history must not be empty",
            Self::HistoryAllocation => "late-acceptance history could not be allocated",
        })
    }
}
impl Error for AcceptanceError {}
impl Acceptance {
    pub fn validate(self) -> Result<(), AcceptanceError> {
        if matches!(
            self,
            Self::LateAcceptance {
                history_size: 0,
                ..
            }
        ) {
            Err(AcceptanceError::EmptyHistory)
        } else {
            Ok(())
        }
    }
    /// Start a fresh acceptance session. No state is shared between solver runs.
    pub fn start<S: Score>(self, initial_best: S) -> Result<Acceptor<S>, AcceptanceError> {
        self.validate()?;
        Ok(match self {
            Self::StrictImprovement => Acceptor::StrictImprovement,
            Self::HillClimbing => Acceptor::HillClimbing,
            Self::LateAcceptance {
                history_size,
                hill_climbing,
            } => Acceptor::LateAcceptance(LateAcceptance::new(
                history_size,
                hill_climbing,
                initial_best,
            )?),
        })
    }
}
#[derive(Clone, Debug)]
pub enum Acceptor<S> {
    StrictImprovement,
    HillClimbing,
    LateAcceptance(LateAcceptance<S>),
}
impl<S: Score> Acceptor<S> {
    pub fn accepts(&self, current: S, candidate: S) -> bool {
        match self {
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
            Self::LateAcceptance(state) => state.accepts(current, candidate),
        }
    }
    /// Record a completed step, never an unselected trial move.
    pub fn step_ended(&mut self, score: S) {
        if let Self::LateAcceptance(state) = self {
            state.step_ended(score);
        }
    }
}

/// A circular history initialized to the phase's best score. History updates
/// only on completed steps, and stores that step's score even when it worsens.
/// A new best with better hard levels resets every slot to the new best score.
/// Structural components are not hard levels; use fully assigned phase scores.
#[derive(Clone, Debug)]
pub struct LateAcceptance<S> {
    history: Vec<S>,
    cursor: usize,
    hill_climbing: bool,
    best: S,
}
impl<S: Score> LateAcceptance<S> {
    pub fn new(
        history_size: usize,
        hill_climbing: bool,
        initial_best: S,
    ) -> Result<Self, AcceptanceError> {
        if history_size == 0 {
            return Err(AcceptanceError::EmptyHistory);
        }
        let mut history = Vec::new();
        history
            .try_reserve_exact(history_size)
            .map_err(|_| AcceptanceError::HistoryAllocation)?;
        history.resize(history_size, initial_best);
        Ok(Self {
            history,
            cursor: 0,
            hill_climbing,
            best: initial_best,
        })
    }
    pub fn history(&self) -> &[S] {
        &self.history
    }
    pub fn threshold(&self) -> S {
        self.history[self.cursor]
    }
    pub fn accepts(&self, current: S, candidate: S) -> bool {
        candidate >= self.threshold() || (self.hill_climbing && candidate >= current)
    }
    pub fn step_ended(&mut self, score: S) {
        if score > self.best {
            if score.hard_cmp(self.best) == Ordering::Greater {
                self.history.fill(score);
            }
            self.best = score;
        }
        self.history[self.cursor] = score;
        self.cursor = if self.cursor + 1 == self.history.len() {
            0
        } else {
            self.cursor + 1
        };
    }
}
