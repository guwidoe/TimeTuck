//! Threshold-based acceptance with a counter reset at completed step boundaries.
use crate::{AcceptanceError, Score};

/// What counts toward the next threshold replacement. Trial modes count every
/// acceptor check (or every accepted check), including candidates not selected.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StepCountingMode {
    /// Every scored candidate submitted to accepts, including rejected trials.
    SelectedMove,
    /// Every trial accepted by this policy, including unselected accepted trials.
    AcceptedMove,
    #[default]
    Step,
    EqualOrImprovingStep,
    ImprovingStep,
}

/// Accept non-worsening candidates or candidates at least as good as the retained
/// threshold. At the end of a step reaching the configured count, replace the
/// threshold with that step's score and reset the count to zero. Excess trial
/// counts do not carry into the next interval. Global-best score is independent.
///
/// Counts and threshold state are private to one phase. Starting a fresh session
/// resets them to the supplied initial score, including for owned decimal scores.
///
/// ```
/// use timetuck::{SimpleScore as S, StepCountingHillClimbing, StepCountingMode};
/// let mut a = StepCountingHillClimbing::new(2, StepCountingMode::Step, S::new(-10))?;
/// assert!(a.accepts(S::new(-10), S::new(-5)));
/// a.step_ended(S::new(-5));
/// assert!(a.accepts(S::new(-5), S::new(-7))); // Meets the old threshold.
/// a.step_ended(S::new(-7));
/// assert_eq!(a.threshold(), S::new(-7));
/// assert!(!a.accepts(S::new(-7), S::new(-8)));
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct StepCountingHillClimbing<S> {
    period: u64,
    mode: StepCountingMode,
    count: u64,
    threshold: S,
    previous: S,
}
impl<S: Score> StepCountingHillClimbing<S> {
    pub fn new(period: u64, mode: StepCountingMode, initial: S) -> Result<Self, AcceptanceError> {
        if period == 0 {
            return Err(AcceptanceError::ZeroStepCountingPeriod);
        }
        Ok(Self {
            period,
            mode,
            count: 0,
            threshold: initial.clone(),
            previous: initial,
        })
    }
    pub fn threshold(&self) -> S {
        self.threshold.clone()
    }
    /// Count toward the next reset, capped at the period. Trial counts may reach
    /// the period during a scan; the threshold stays fixed until step_ended.
    pub fn count(&self) -> u64 {
        self.count
    }
    fn increment(&mut self) {
        if self.count < self.period {
            self.count += 1;
        }
    }
    pub fn accepts(&mut self, current: S, candidate: S) -> bool {
        let accepted = candidate >= current || candidate >= self.threshold;
        if self.mode == StepCountingMode::SelectedMove
            || (self.mode == StepCountingMode::AcceptedMove && accepted)
        {
            self.increment();
        }
        accepted
    }
    /// The score must be that of the completed step, including a selected rejected
    /// fallback. Step modes compare with the previous completed step, not the best.
    /// Call only after committing a step; unselected trials cannot replace a threshold.
    pub fn step_ended(&mut self, score: S) {
        let counted = match self.mode {
            StepCountingMode::Step => true,
            StepCountingMode::EqualOrImprovingStep => score >= self.previous,
            StepCountingMode::ImprovingStep => score > self.previous,
            StepCountingMode::SelectedMove | StepCountingMode::AcceptedMove => false,
        };
        if counted {
            self.increment();
        }
        if self.count == self.period {
            self.threshold = score.clone();
            self.count = 0;
        }
        self.previous = score;
    }
}
