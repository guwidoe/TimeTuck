//! Prepare native local-search policies from explicit or inferred settings.
use crate::{
    Acceptance, AcceptanceError, AdditiveScore, GreatDelugeRise, LocalSearchConfig,
    MixedLocalSearchConfig, ScoreLevel, StepCountingMode,
};
use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum AcceptorKind {
    HillClimbing,
    StepCounting,
    VariableTabu,
    ValueTabu,
    MoveTabu,
    SimulatedAnnealing,
    LateAcceptance,
    DiversifiedLateAcceptance,
    GreatDeluge,
}
impl AcceptorKind {
    pub const ALL: [Self; 9] = [
        Self::HillClimbing,
        Self::StepCounting,
        Self::VariableTabu,
        Self::ValueTabu,
        Self::MoveTabu,
        Self::SimulatedAnnealing,
        Self::LateAcceptance,
        Self::DiversifiedLateAcceptance,
        Self::GreatDeluge,
    ];
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabuSettings {
    pub hard: i64,
    pub fading: Option<i64>,
    pub aspiration: bool,
}
/// Native magnitudes or nonpositive configured losses. Losses are checked before
/// negation; built-in integer minimum values fail instead of wrapping into a
/// negative temperature. Structural values are never discarded silently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnnealingTemperature<S> {
    Magnitude(S),
    Loss(S),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptorProfileError {
    Empty,
    Missing {
        kind: AcceptorKind,
        setting: &'static str,
    },
    InvalidSize {
        kind: AcceptorKind,
        setting: &'static str,
        value: i64,
    },
    DiversifiedDisabled,
    InvalidTemperature,
    Validation {
        kind: AcceptorKind,
        error: AcceptanceError,
    },
}
impl fmt::Display for AcceptorProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                f.write_str("The acceptor does not specify any acceptorType or inferred setting")
            }
            Self::Missing { kind, setting } => write!(f, "{kind:?} requires {setting}"),
            Self::InvalidSize {
                kind,
                setting,
                value,
            } => write!(f, "{kind:?} has invalid {setting}: {value}"),
            Self::DiversifiedDisabled => {
                f.write_str("diversified late acceptance is disabled by the preparation policy")
            }
            Self::InvalidTemperature => f.write_str(
                "annealing loss must have structural zero and nonpositive representable levels",
            ),
            Self::Validation { kind, error } => write!(f, "{kind:?}: {error}"),
        }
    }
}
impl Error for AcceptorProfileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation { error, .. } => Some(error),
            _ => None,
        }
    }
}
/// Configuration only; preparation creates actual Acceptance policies. Explicit
/// types and supplied settings are combined, duplicates removed, and policies
/// emitted in AcceptorKind::ALL order. A late size alone infers ordinary late
/// acceptance, unless diversified late acceptance was explicitly requested.
/// Native defaults: history/step period 100, variable tabu tenure 7, 1000 cooling
/// steps, seed 0, aspiration enabled and great-deluge rain 1/100000. These are
/// native choices; no Java random schedule or default numeric parity is claimed.
#[derive(Clone, Debug)]
pub struct AcceptorProfile<S> {
    pub kinds: Vec<AcceptorKind>,
    pub variable_tabu: Option<TabuSettings>,
    pub value_tabu: Option<TabuSettings>,
    pub move_tabu: Option<TabuSettings>,
    pub late_size: Option<i64>,
    pub step_period: Option<i64>,
    pub step_mode: StepCountingMode,
    pub temperature: Option<AnnealingTemperature<S>>,
    pub cooling_steps: u64,
    pub seed: u64,
    pub deluge_rise: Option<GreatDelugeRise<S>>,
}
impl<S> Default for AcceptorProfile<S> {
    fn default() -> Self {
        Self {
            kinds: vec![],
            variable_tabu: None,
            value_tabu: None,
            move_tabu: None,
            late_size: None,
            step_period: None,
            step_mode: StepCountingMode::Step,
            temperature: None,
            cooling_steps: 1000,
            seed: 0,
            deluge_rise: None,
        }
    }
}
fn size(
    kind: AcceptorKind,
    setting: &'static str,
    value: i64,
    zero: bool,
) -> Result<usize, AcceptorProfileError> {
    if value < 0 || (!zero && value == 0) {
        return Err(AcceptorProfileError::InvalidSize {
            kind,
            setting,
            value,
        });
    }
    usize::try_from(value).map_err(|_| AcceptorProfileError::InvalidSize {
        kind,
        setting,
        value,
    })
}
impl<S: AdditiveScore> AcceptorProfile<S> {
    /// Caller-controlled feature availability replaces reflective preview policy.
    /// Failure never publishes a partial profile or modifies the declaration.
    pub fn prepare(
        &self,
        allow_diversified: bool,
    ) -> Result<PreparedAcceptors<S>, AcceptorProfileError> {
        use AcceptorKind as K;
        let mut selected: BTreeSet<_> = self.kinds.iter().copied().collect();
        for (present, kind) in [
            (self.variable_tabu.is_some(), K::VariableTabu),
            (self.value_tabu.is_some(), K::ValueTabu),
            (self.move_tabu.is_some(), K::MoveTabu),
            (self.step_period.is_some(), K::StepCounting),
            (self.temperature.is_some(), K::SimulatedAnnealing),
            (self.deluge_rise.is_some(), K::GreatDeluge),
        ] {
            if present {
                selected.insert(kind);
            }
        }
        if self.late_size.is_some() && !selected.contains(&K::DiversifiedLateAcceptance) {
            selected.insert(K::LateAcceptance);
        }
        if selected.is_empty() {
            return Err(AcceptorProfileError::Empty);
        }
        if selected.contains(&K::DiversifiedLateAcceptance) && !allow_diversified {
            return Err(AcceptorProfileError::DiversifiedDisabled);
        }
        let mut policies = Vec::with_capacity(selected.len());
        for kind in selected {
            let policy = match kind {
                K::HillClimbing => Acceptance::HillClimbing,
                K::StepCounting => Acceptance::StepCountingHillClimbing {
                    period: size(kind, "step period", self.step_period.unwrap_or(100), false)?
                        as u64,
                    mode: self.step_mode,
                },
                K::LateAcceptance | K::DiversifiedLateAcceptance => {
                    let history_size =
                        size(kind, "late size", self.late_size.unwrap_or(100), false)?;
                    if kind == K::LateAcceptance {
                        Acceptance::LateAcceptance {
                            history_size,
                            hill_climbing: true,
                        }
                    } else {
                        Acceptance::DiversifiedLateAcceptance { history_size }
                    }
                }
                K::VariableTabu | K::ValueTabu | K::MoveTabu => {
                    let settings = match kind {
                        K::VariableTabu => self.variable_tabu.or(Some(TabuSettings {
                            hard: 7,
                            fading: None,
                            aspiration: true,
                        })),
                        K::ValueTabu => self.value_tabu,
                        _ => self.move_tabu,
                    }
                    .ok_or(AcceptorProfileError::Missing {
                        kind,
                        setting: "tabu size",
                    })?;
                    let hard = size(
                        kind,
                        "hard tenure",
                        settings.hard,
                        settings.fading.is_some(),
                    )?;
                    let fading = settings
                        .fading
                        .map(|n| size(kind, "fading tenure", n, true))
                        .transpose()?
                        .unwrap_or(0);
                    let aspiration = settings.aspiration;
                    match (kind, fading) {
                        (K::VariableTabu, 0) => Acceptance::VariableTabu {
                            tenure: hard,
                            aspiration,
                        },
                        (K::ValueTabu, 0) => Acceptance::ValueTabu {
                            tenure: hard,
                            aspiration,
                        },
                        (K::MoveTabu, 0) => Acceptance::MoveTabu {
                            tenure: hard,
                            aspiration,
                        },
                        (K::VariableTabu, _) => Acceptance::FadingVariableTabu {
                            hard_tenure: hard,
                            fading_tenure: fading,
                            aspiration,
                            seed: self.seed,
                        },
                        (K::ValueTabu, _) => Acceptance::FadingValueTabu {
                            hard_tenure: hard,
                            fading_tenure: fading,
                            aspiration,
                            seed: self.seed,
                        },
                        _ => Acceptance::FadingMoveTabu {
                            hard_tenure: hard,
                            fading_tenure: fading,
                            aspiration,
                            seed: self.seed,
                        },
                    }
                }
                K::SimulatedAnnealing => {
                    let input = self
                        .temperature
                        .as_ref()
                        .ok_or(AcceptorProfileError::Missing {
                            kind,
                            setting: "starting temperature",
                        })?;
                    let starting_temperature = match input {
                        AnnealingTemperature::Magnitude(score) => score.clone(),
                        AnnealingTemperature::Loss(score) => {
                            if score.structural() != 0
                                || score
                                    .ordinary_levels()
                                    .any(|n| n.numeric_cmp(&S::Level::zero()).is_gt())
                            {
                                return Err(AcceptorProfileError::InvalidTemperature);
                            }
                            score
                                .checked_scale(-1)
                                .ok_or(AcceptorProfileError::InvalidTemperature)?
                        }
                    };
                    Acceptance::SimulatedAnnealing {
                        starting_temperature,
                        cooling_steps: self.cooling_steps,
                        seed: self.seed,
                    }
                }
                K::GreatDeluge => Acceptance::GreatDeluge {
                    initial_water_level: None,
                    rise: self.deluge_rise.clone().unwrap_or(GreatDelugeRise::Ratio {
                        numerator: 1,
                        denominator: 100000,
                    }),
                },
            };
            policy
                .validate()
                .map_err(|error| AcceptorProfileError::Validation { kind, error })?;
            policies.push(policy);
        }
        Ok(PreparedAcceptors { policies })
    }
}
/// Own validated settings, borrowing them when binding a search configuration.
/// Each real search constructs fresh state through existing native acceptors.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let prepared = AcceptorProfile { late_size: Some(10), ..Default::default() }.prepare(true)?;
/// let r = local_search(&p, &n, prepared.bind(LocalSearchConfig {
///     max_steps: 1, selection: MoveSelection::BestAccepted, ..Default::default()
/// }))?;
/// assert_eq!(r.best.unwrap().score, SimpleScore::new(2));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAcceptors<S> {
    policies: Vec<Acceptance<S>>,
}
impl<S: crate::Score> PreparedAcceptors<S> {
    pub fn policies(&self) -> &[Acceptance<S>] {
        &self.policies
    }
    pub fn is_composite(&self) -> bool {
        self.policies.len() > 1
    }
    pub fn bind<'a>(&'a self, mut config: LocalSearchConfig<'a, S>) -> LocalSearchConfig<'a, S> {
        config.acceptance = self.policies[0].clone();
        config.additional_acceptance = &self.policies[1..];
        config
    }
    pub fn bind_mixed<'a>(
        &'a self,
        mut config: MixedLocalSearchConfig<'a, S>,
    ) -> MixedLocalSearchConfig<'a, S> {
        config.search = self.bind(config.search);
        config
    }
}
