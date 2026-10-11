//! Acceptance configurations and per-search state.
use crate::{
    DiversifiedLateAcceptance, FadingTabuMemory, GreatDeluge, GreatDelugeRise, Score, SimpleScore,
    SimulatedAnnealing, StepCountingHillClimbing, StepCountingMode, TabuMemory,
    TimeSimulatedAnnealing,
};
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acceptance<S = SimpleScore> {
    /// Linearly cool to zero after this many completed steps. The random stream
    /// is independent of move-order shuffling and does not match Java.
    SimulatedAnnealing {
        starting_temperature: S,
        cooling_steps: u64,
        seed: u64,
    },
    /// Cool to zero over a positive elapsed duration. Integrated plans measure
    /// from phase entry, before PhaseStarted and preparation; standalone searches
    /// measure from kernel entry. Setup is included. Each step samples once and
    /// freezes its temperature throughout candidate scanning. Every phase resets
    /// independently, using SolverExtensions' clock when configured. This is an
    /// explicit duration schedule, not a composed termination gradient.
    SimulatedAnnealingTime {
        starting_temperature: S,
        cooling_time: std::time::Duration,
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
    /// Resolve this size from the model's variable/entity count per phase.
    /// Standalone users call for_population before starting acceptance.
    SizedVariableTabu {
        size: crate::TabuSize,
        aspiration: bool,
    },
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
    /// Reject planning values assigned by recent selected moves, including None.
    ValueTabu {
        tenure: usize,
        aspiration: bool,
    },
    FadingValueTabu {
        hard_tenure: usize,
        fading_tenure: usize,
        aspiration: bool,
        seed: u64,
    },
    /// Reject equal recent selected move descriptors. Use start_with_moves for
    /// standalone state; solver entry points supply typed moves automatically.
    MoveTabu {
        tenure: usize,
        aspiration: bool,
    },
    /// Move tabu with the same seeded expiry policy as FadingVariableTabu.
    FadingMoveTabu {
        hard_tenure: usize,
        fading_tenure: usize,
        aspiration: bool,
        seed: u64,
    },
    /// Accepted trials update history; completed steps advance the history slot.
    DiversifiedLateAcceptance {
        history_size: usize,
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
    InvalidTabuRatio,
    PopulationContextRequired,
    MoveContextRequired,
    ValueContextRequired,
    TabuTenureOverflow,
    HistoryAllocation,
    AcceptorAllocation,
    InvalidTemperature,
    ZeroCoolingSteps,
    ZeroCoolingTime,
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
            Self::ZeroCoolingTime => "annealing cooling time must be positive",
            Self::ZeroCoolingSteps => "annealing cooling steps must be positive",
            Self::InvalidGradient => "annealing gradient must be finite and in [0, 1]",
            Self::InvalidRandomDraw => "random draw must be finite and in [0, 1)",
            Self::TabuTenureOverflow => "tabu tenure exceeds usize",
            Self::ValueContextRequired => {
                "value-based acceptance requires typed value context (start_with_values)"
            }
            Self::MoveContextRequired => {
                "move-based acceptance requires typed move context (start_with_moves)"
            }
            Self::InvalidTabuRatio => "tabu ratio must be finite and in [0, 1]",
            Self::PopulationContextRequired => {
                "sized variable tabu requires population context (for_population)"
            }
            Self::EmptyTabu => "tabu tenure must be positive",
            Self::EmptyHistory => "late-acceptance history must not be empty",
            Self::AcceptorAllocation => "composite acceptors could not be allocated",
            Self::HistoryAllocation => "late-acceptance history could not be allocated",
        })
    }
}
impl Error for AcceptanceError {}
impl<S: Score> Acceptance<S> {
    /// Resolve model-relative policies once per phase, retaining other settings.
    /// Basic models count variables; list models count list-owning entities;
    /// mixed models count both namespaces. Counts include pinned entries.
    pub fn for_population(&self, population: usize) -> Self {
        match self {
            Self::SizedVariableTabu { size, aspiration } => Self::VariableTabu {
                tenure: size.resolve(population),
                aspiration: *aspiration,
            },
            _ => self.clone(),
        }
    }
    pub fn validate(&self) -> Result<(), AcceptanceError> {
        if let Self::SimulatedAnnealingTime {
            starting_temperature,
            cooling_time,
            ..
        } = self
        {
            TimeSimulatedAnnealing::validate(starting_temperature, *cooling_time)?;
        }
        if let Self::FadingVariableTabu {
            hard_tenure,
            fading_tenure,
            ..
        }
        | Self::FadingValueTabu {
            hard_tenure,
            fading_tenure,
            ..
        }
        | Self::FadingMoveTabu {
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
        if matches!(
            self,
            Self::VariableTabu { tenure: 0, .. }
                | Self::MoveTabu { tenure: 0, .. }
                | Self::ValueTabu { tenure: 0, .. }
        ) {
            return Err(AcceptanceError::EmptyTabu);
        }
        if matches!(
            self,
            Self::DiversifiedLateAcceptance { history_size: 0 }
                | Self::LateAcceptance {
                    history_size: 0,
                    ..
                }
        ) {
            Err(AcceptanceError::EmptyHistory)
        } else {
            Ok(())
        }
    }
    /// Start a fresh session that receives actual move identities. Equality must
    /// remain stable while a selected descriptor is retained.
    pub fn start_with_moves<M: Eq + Clone>(
        &self,
        initial_best: S,
    ) -> Result<crate::MoveAcceptor<S, M>, AcceptanceError> {
        crate::MoveAcceptor::new(self, initial_best)
    }
    /// Start a session with independent typed move and planning-value identities.
    pub fn start_with_values<M: Eq + Clone, K: Eq + Clone>(
        &self,
        initial_best: S,
    ) -> Result<crate::ValueAcceptor<S, M, K>, AcceptanceError> {
        crate::ValueAcceptor::new(self, initial_best)
    }
    /// Start a fresh acceptance session. No state is shared between solver runs.
    /// Move/value policies require their context; use start_with_moves or
    /// start_with_values instead.
    pub fn start(&self, initial_best: S) -> Result<Acceptor<S>, AcceptanceError> {
        self.validate()?;
        Ok(match self.clone() {
            Self::SizedVariableTabu { .. } => {
                return Err(AcceptanceError::PopulationContextRequired);
            }
            Self::ValueTabu { .. } | Self::FadingValueTabu { .. } => {
                return Err(AcceptanceError::ValueContextRequired);
            }
            Self::MoveTabu { .. } | Self::FadingMoveTabu { .. } => {
                return Err(AcceptanceError::MoveContextRequired);
            }
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
            Self::SimulatedAnnealingTime {
                starting_temperature,
                cooling_time,
                seed,
            } => Acceptor::SimulatedAnnealingTime(TimeSimulatedAnnealing::new(
                starting_temperature,
                cooling_time,
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
            Self::DiversifiedLateAcceptance { history_size } => {
                Acceptor::DiversifiedLateAcceptance(DiversifiedLateAcceptance::new(
                    history_size,
                    initial_best,
                )?)
            }
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
    /// Ordered conjunction. Trials short-circuit at the first rejecting child;
    /// completed steps update every child, including those not queried.
    All(Vec<Acceptor<S>>),
    GreatDeluge(GreatDeluge<S>),
    StepCountingHillClimbing(StepCountingHillClimbing<S>),
    SimulatedAnnealing(SimulatedAnnealing<S>),
    SimulatedAnnealingTime(TimeSimulatedAnnealing<S>),
    StrictImprovement,
    HillClimbing,
    DiversifiedLateAcceptance(DiversifiedLateAcceptance<S>),
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
    /// Create independent state for an ordered conjunction of policies. Empty
    /// conjunctions accept every candidate. All configurations are validated
    /// before state construction. Each call starts a fresh session. For move-based
    /// policies use [`crate::MoveAcceptor::from_policies`] instead; value policies
    /// require [`crate::ValueAcceptor::from_policies`].
    ///
    /// ```
    /// use timetuck::{Acceptance, Acceptor, SimpleScore};
    /// let policies = [Acceptance::HillClimbing,
    ///     Acceptance::VariableTabu { tenure: 2, aspiration: false }];
    /// let mut state = Acceptor::from_policies(&policies, SimpleScore::ZERO)?;
    /// assert!(state.accepts(SimpleScore::ZERO, SimpleScore::new(1), &[0]));
    /// state.step_ended(SimpleScore::new(1), &[0]);
    /// assert!(!state.accepts(SimpleScore::new(1), SimpleScore::new(2), &[0]));
    /// # Ok::<(), timetuck::AcceptanceError>(())
    /// ```
    pub fn from_policies(
        policies: &[Acceptance<S>],
        initial_best: S,
    ) -> Result<Self, AcceptanceError> {
        Self::start_composite(policies.iter(), initial_best)
    }
    pub(crate) fn start_composite<'a>(
        policies: impl Iterator<Item = &'a Acceptance<S>> + Clone,
        initial_best: S,
    ) -> Result<Self, AcceptanceError>
    where
        S: 'a,
    {
        for policy in policies.clone() {
            policy.validate()?;
        }
        let mut children = Vec::new();
        children
            .try_reserve_exact(policies.size_hint().0)
            .map_err(|_| AcceptanceError::AcceptorAllocation)?;
        for policy in policies {
            children.push(policy.start(initial_best.clone())?);
        }
        Ok(Self::All(children))
    }

    /// Supply all variable indices written by the candidate (both sides of swaps).
    /// Score-only acceptors ignore this slice. Annealing and fading-tabu trial checks may consume
    /// a random draw. Step-counting trial modes count these checks, while only
    /// completed steps replace their threshold or advance cooling/history.
    /// Diversified late acceptance updates its active slot on accepted trials.
    pub fn accepts(&mut self, current: S, candidate: S, variables: &[usize]) -> bool {
        match self {
            Self::All(children) => children
                .iter_mut()
                .all(|child| child.accepts(current.clone(), candidate.clone(), variables)),
            Self::GreatDeluge(state) => state.accepts(current, candidate),
            Self::StepCountingHillClimbing(state) => state.accepts(current, candidate),
            Self::SimulatedAnnealing(state) => state.accepts(current, candidate),
            Self::SimulatedAnnealingTime(state) => state.accepts(current, candidate),
            Self::StrictImprovement => candidate > current,
            Self::HillClimbing => candidate >= current,
            Self::DiversifiedLateAcceptance(state) => state.accepts(current, candidate),
            Self::LateAcceptance(state) => state.accepts(current, candidate),
            Self::FadingVariableTabu { memory, best } => {
                memory.accepts(variables, candidate, best.clone())
            }
            Self::VariableTabu { memory, best } => {
                memory.accepts(variables, candidate, best.clone())
            }
        }
    }
    /// Freeze elapsed-time acceptors before scanning this step's candidates.
    /// Other policies are unchanged; every composite child receives the sample.
    pub fn step_started(&mut self, elapsed: std::time::Duration) {
        match self {
            Self::All(children) => {
                for child in children {
                    child.step_started(elapsed);
                }
            }
            Self::SimulatedAnnealingTime(state) => state.step_started(elapsed),
            _ => {}
        }
    }
    /// Record a completed step, never an unselected trial move.
    pub fn step_ended(&mut self, score: S, variables: &[usize]) {
        match self {
            Self::All(children) => {
                for child in children {
                    child.step_ended(score.clone(), variables);
                }
            }
            Self::GreatDeluge(state) => state.step_ended(),
            Self::StepCountingHillClimbing(state) => state.step_ended(score),
            Self::SimulatedAnnealing(state) => state.step_ended(),
            Self::DiversifiedLateAcceptance(state) => state.step_ended(),
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
/// A new best with better reset levels resets every slot to the new best score.
/// See Score::late_acceptance_cmp: HardMediumSoft includes its medium level.
/// Structural components are not hard levels; use fully assigned phase scores.
#[derive(Clone, Debug)]
pub struct LateAcceptance<S> {
    history: crate::ScoreHistory<S>,
    hill_climbing: bool,
    best: S,
}
impl<S: Score> LateAcceptance<S> {
    pub fn new(
        history_size: usize,
        hill_climbing: bool,
        initial_best: S,
    ) -> Result<Self, AcceptanceError> {
        let history = crate::ScoreHistory::new(history_size, initial_best)?;
        let best = history.current().clone();
        Ok(Self {
            history,
            hill_climbing,
            best,
        })
    }
    /// Materialize and cache the full history until its next mutation. Normal
    /// acceptance checks and resets do not require this O(history size) snapshot.
    pub fn history(&self) -> &[S] {
        self.history.as_slice()
    }
    pub fn threshold(&self) -> S {
        self.history.current().clone()
    }
    pub fn accepts(&self, current: S, candidate: S) -> bool {
        candidate >= *self.history.current() || (self.hill_climbing && candidate >= current)
    }
    pub fn step_ended(&mut self, score: S) {
        let improves = score > self.best;
        let resets = improves && score.late_acceptance_cmp(&self.best).is_gt();
        // Prepare every potentially panicking score clone before mutating history
        // or the retained best. Normal writes/resets move prepared values only.
        let new_best = improves.then(|| score.clone());
        let baseline = resets.then(|| score.clone());
        if let Some(baseline) = baseline {
            self.history.reset(baseline);
        }
        self.history.update(score);
        if let Some(best) = new_best {
            self.best = best;
        }
    }
}

impl<S> Acceptance<S> {
    pub(crate) fn uses_elapsed_time(&self) -> bool {
        matches!(self, Self::SimulatedAnnealingTime { .. })
    }
}
