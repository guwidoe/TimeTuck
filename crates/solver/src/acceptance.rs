//! Acceptance configurations and per-search state.
use crate::{Score, SimpleScore, SimulatedAnnealing, TabuMemory};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance<S = SimpleScore> {
    /// Linearly cool to zero after this many completed steps. The random stream
    /// is independent of move-order shuffling and does not match Java.
    SimulatedAnnealing {
        starting_temperature: S,
        cooling_steps: u64,
        seed: u64,
    },
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
    InvalidTemperature,
    ZeroCoolingSteps,
    InvalidGradient,
    InvalidRandomDraw,
}
impl fmt::Display for AcceptanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidTemperature => {
                "temperature must have structural zero and nonnegative ordinary levels"
            }
            Self::ZeroCoolingSteps => "annealing cooling steps must be positive",
            Self::InvalidGradient => "annealing gradient must be finite and in [0, 1]",
            Self::InvalidRandomDraw => "random draw must be finite and in [0, 1)",
            Self::EmptyTabu => "tabu tenure must be positive",
            Self::EmptyHistory => "late-acceptance history must not be empty",
            Self::HistoryAllocation => "late-acceptance history could not be allocated",
        })
    }
}
impl Error for AcceptanceError {}
impl<S: Score> Acceptance<S> {
    pub fn validate(&self) -> Result<(), AcceptanceError> {
        if let Self::SimulatedAnnealing {
            starting_temperature,
            cooling_steps,
            ..
        } = self
        {
            SimulatedAnnealing::validate(starting_temperature, *cooling_steps)?;
        }
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
    pub fn start(&self, initial_best: S) -> Result<Acceptor<S>, AcceptanceError> {
        self.validate()?;
        Ok(match self.clone() {
            Self::SimulatedAnnealing {
                starting_temperature,
                cooling_steps,
                seed,
            } => Acceptor::SimulatedAnnealing(SimulatedAnnealing::new(
                starting_temperature,
                cooling_steps,
                seed,
            )?),
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
    SimulatedAnnealing(SimulatedAnnealing<S>),
    StrictImprovement,
    HillClimbing,
    LateAcceptance(LateAcceptance<S>),
    VariableTabu { memory: TabuMemory<usize>, best: S },
}
impl<S: Score> Acceptor<S> {
    /// Supply all variable indices written by the candidate (both sides of swaps).
    /// Score-only acceptors ignore this slice. Annealing trial checks may consume
    /// a random draw; only completed steps advance cooling or acceptance history.
    pub fn accepts(&mut self, current: S, candidate: S, variables: &[usize]) -> bool {
        match self {
            Self::SimulatedAnnealing(state) => state.accepts(current, candidate),
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
            Self::LateAcceptance(state) => state.accepts(current, candidate),
            Self::VariableTabu { memory, best } => {
                memory.accepts(variables, candidate, best.clone())
            }
        }
    }
    /// Record a completed step, never an unselected trial move.
    pub fn step_ended(&mut self, score: S, variables: &[usize]) {
        match self {
            Self::SimulatedAnnealing(state) => state.step_ended(),
            Self::LateAcceptance(state) => state.step_ended(score),
            Self::VariableTabu { memory, best } => {
                memory.step_ended(variables);
                if score >= *best {
                    *best = score;
                }
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
        history.resize(history_size, initial_best.clone());
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
        self.history[self.cursor].clone()
    }
    pub fn accepts(&self, current: S, candidate: S) -> bool {
        candidate >= self.history[self.cursor] || (self.hill_climbing && candidate >= current)
    }
    pub fn step_ended(&mut self, score: S) {
        if score > self.best {
            if score.hard_cmp(&self.best) == Ordering::Greater {
                self.history.fill(score.clone());
            }
            self.best = score.clone();
        }
        self.history[self.cursor] = score;
        self.cursor = if self.cursor + 1 == self.history.len() {
            0
        } else {
            self.cursor + 1
        };
    }
}
