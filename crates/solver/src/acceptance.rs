//! Acceptance configurations and per-search state.
use crate::{
    FadingTabuMemory, GreatDeluge, GreatDelugeRise, Score, SimpleScore, SimulatedAnnealing,
    StepCountingHillClimbing, StepCountingMode, TabuMemory,
};
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
    GreatDeluge {
        initial_water_level: Option<S>,
        rise: GreatDelugeRise<S>,
    },
    StepCountingHillClimbing {
        period: u64,
        mode: StepCountingMode,
    },
    StrictImprovement,
    HillClimbing,
    /// Forbid touching variables from recent completed steps, unless optional
    /// aspiration beats the global best. Non-tabu worsening moves are allowed.
    VariableTabu {
        tenure: usize,
        aspiration: bool,
    },
    /// Variable tabu followed by seeded probabilistic fading. Keys are basic
    /// variable indices or list entity indices, as with VariableTabu.
    FadingVariableTabu {
        hard_tenure: usize,
        fading_tenure: usize,
        aspiration: bool,
        seed: u64,
    },
    LateAcceptance {
        history_size: usize,
        hill_climbing: bool,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptanceError {
    UnsupportedScoreArithmetic,
    InvalidWaterLevel,
    InvalidWaterLevelIncrement,
    ZeroStepCountingPeriod,
    EmptyHistory,
    EmptyTabu,
    TabuTenureOverflow,
    HistoryAllocation,
    InvalidTemperature,
    ZeroCoolingSteps,
    InvalidGradient,
    InvalidRandomDraw,
}
impl fmt::Display for AcceptanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedScoreArithmetic => {
                "score levels do not support exact threshold arithmetic"
            }
            Self::InvalidWaterLevel => "water levels and increments require structural zero",
            Self::InvalidWaterLevelIncrement => {
                "rain requires nonnegative score levels or a positive ratio"
            }
            Self::ZeroStepCountingPeriod => "step-counting hill-climbing period must be positive",
            Self::InvalidTemperature => {
                "temperature must have structural zero and nonnegative ordinary levels"
            }
            Self::ZeroCoolingSteps => "annealing cooling steps must be positive",
            Self::InvalidGradient => "annealing gradient must be finite and in [0, 1]",
            Self::InvalidRandomDraw => "random draw must be finite and in [0, 1)",
            Self::TabuTenureOverflow => "combined tabu tenure exceeds usize",
            Self::EmptyTabu => "tabu tenure must be positive",
            Self::EmptyHistory => "late-acceptance history must not be empty",
            Self::HistoryAllocation => "late-acceptance history could not be allocated",
        })
    }
}
impl Error for AcceptanceError {}
impl<S: Score> Acceptance<S> {
    pub fn validate(&self) -> Result<(), AcceptanceError> {
        if let Self::FadingVariableTabu {
            hard_tenure,
            fading_tenure,
            ..
        } = self
        {
            FadingTabuMemory::<usize>::validate(*hard_tenure, *fading_tenure)?;
        }
        if let Self::GreatDeluge {
            initial_water_level,
            rise,
        } = self
        {
            GreatDeluge::validate(initial_water_level.as_ref(), rise)?;
        }
        if matches!(self, Self::StepCountingHillClimbing { period: 0, .. }) {
            return Err(AcceptanceError::ZeroStepCountingPeriod);
        }
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
            Self::GreatDeluge {
                initial_water_level,
                rise,
            } => Acceptor::GreatDeluge(GreatDeluge::new(initial_water_level, rise, initial_best)?),
            Self::StepCountingHillClimbing { period, mode } => Acceptor::StepCountingHillClimbing(
                StepCountingHillClimbing::new(period, mode, initial_best)?,
            ),
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
            Self::FadingVariableTabu {
                hard_tenure,
                fading_tenure,
                aspiration,
                seed,
            } => Acceptor::FadingVariableTabu {
                memory: FadingTabuMemory::new(hard_tenure, fading_tenure, aspiration, seed)?,
                best: initial_best,
            },
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
    GreatDeluge(GreatDeluge<S>),
    StepCountingHillClimbing(StepCountingHillClimbing<S>),
    SimulatedAnnealing(SimulatedAnnealing<S>),
    StrictImprovement,
    HillClimbing,
    LateAcceptance(LateAcceptance<S>),
    VariableTabu {
        memory: TabuMemory<usize>,
        best: S,
    },
    FadingVariableTabu {
        memory: FadingTabuMemory<usize>,
        best: S,
    },
}
impl<S: Score> Acceptor<S> {
    /// Supply all variable indices written by the candidate (both sides of swaps).
    /// Score-only acceptors ignore this slice. Annealing and fading-tabu trial checks may consume
    /// a random draw. Step-counting trial modes count these checks, while only
    /// completed steps replace their threshold or advance cooling/history.
    pub fn accepts(&mut self, current: S, candidate: S, variables: &[usize]) -> bool {
        match self {
            Self::GreatDeluge(state) => state.accepts(current, candidate),
            Self::StepCountingHillClimbing(state) => state.accepts(current, candidate),
            Self::SimulatedAnnealing(state) => state.accepts(current, candidate),
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
            Self::LateAcceptance(state) => state.accepts(current, candidate),
            Self::FadingVariableTabu { memory, best } => {
                memory.accepts(variables, candidate, best.clone())
            }
            Self::VariableTabu { memory, best } => {
                memory.accepts(variables, candidate, best.clone())
            }
        }
    }
    /// Record a completed step, never an unselected trial move.
    pub fn step_ended(&mut self, score: S, variables: &[usize]) {
        match self {
            Self::GreatDeluge(state) => state.step_ended(),
            Self::StepCountingHillClimbing(state) => state.step_ended(score),
            Self::SimulatedAnnealing(state) => state.step_ended(),
            Self::LateAcceptance(state) => state.step_ended(score),
            Self::FadingVariableTabu { memory, best } => {
                memory.step_ended(variables);
                if score > *best {
                    *best = score;
                }
            }
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
