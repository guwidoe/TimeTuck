//! Acceptance configurations and per-search state.
use crate::{Score, TabuMemory};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance {
    StrictImprovement,
    HillClimbing,
    /// Forbid touching variables from recent completed steps, unless optional
    /// aspiration beats the global best. Non-tabu worsening moves are allowed.
    VariableTabu {
        tenure: usize,
        aspiration: bool,
    },
    LateAcceptance {
        history_size: usize,
        hill_climbing: bool,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceError {
    EmptyHistory,
    EmptyTabu,
    HistoryAllocation,
}
impl fmt::Display for AcceptanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyTabu => "tabu tenure must be positive",
            Self::EmptyHistory => "late-acceptance history must not be empty",
            Self::HistoryAllocation => "late-acceptance history could not be allocated",
        })
    }
}
impl Error for AcceptanceError {}
impl Acceptance {
    pub fn validate(self) -> Result<(), AcceptanceError> {
        if matches!(self, Self::VariableTabu { tenure: 0, .. }) {
            return Err(AcceptanceError::EmptyTabu);
        }
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
            Self::VariableTabu { tenure, aspiration } => Acceptor::VariableTabu {
                memory: TabuMemory::new(tenure, aspiration)?,
                best: initial_best,
            },
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
    VariableTabu { memory: TabuMemory<usize>, best: S },
}
impl<S: Score> Acceptor<S> {
    /// Supply all variable indices written by the candidate (both sides of swaps).
    /// Score-only acceptors ignore this slice. Trial checks never update state.
    pub fn accepts(&self, current: S, candidate: S, variables: &[usize]) -> bool {
        match self {
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
            Self::LateAcceptance(state) => state.accepts(current, candidate),
            Self::VariableTabu { memory, best } => memory.accepts(variables, candidate, *best),
        }
    }
    /// Record a completed step, never an unselected trial move.
    pub fn step_ended(&mut self, score: S, variables: &[usize]) {
        match self {
            Self::LateAcceptance(state) => state.step_ended(score),
            Self::VariableTabu { memory, best } => {
                memory.step_ended(variables);
                *best = (*best).max(score);
            }
            _ => {}
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
