//! Seeded simulated annealing with step-count or elapsed-time cooling.
use crate::{AcceptanceError, Score, ScoreLevel, random::SplitMix64};

#[derive(Clone, Debug)]
pub struct SimulatedAnnealing<S> {
    starting_temperature: S,
    cooling_steps: u64,
    completed_steps: u64,
    random: SplitMix64,
}

impl<S: Score> SimulatedAnnealing<S> {
    pub(crate) fn validate(temperature: &S, cooling_steps: u64) -> Result<(), AcceptanceError> {
        if temperature.structural() != 0
            || temperature.ordinary_levels().next().is_none()
            || temperature
                .ordinary_levels()
                .any(|level| level.numeric_cmp(&S::Level::zero()).is_lt())
        {
            return Err(AcceptanceError::InvalidTemperature);
        }
        if cooling_steps == 0 {
            return Err(AcceptanceError::ZeroCoolingSteps);
        }
        Ok(())
    }

    /// Zero-temperature levels reject deterioration in those levels. All-zero
    /// temperature is valid and behaves as hill climbing on sound scores.
    pub fn new(temperature: S, cooling_steps: u64, seed: u64) -> Result<Self, AcceptanceError> {
        Self::validate(&temperature, cooling_steps)?;
        Ok(Self {
            starting_temperature: temperature,
            cooling_steps,
            completed_steps: 0,
            random: SplitMix64::new(seed),
        })
    }

    pub fn gradient(&self) -> f64 {
        self.completed_steps as f64 / self.cooling_steps as f64
    }

    /// Probability for a supplied cooling gradient, without consuming randomness.
    /// Sound non-worsening scores have probability one. Otherwise, every declining
    /// ordinary level contributes loss / temperature to an exponential penalty.
    /// Improvements in softer levels do not offset losses. Structural flaws are
    /// never accepted here. Multi-level energy is a native policy; the adapted
    /// upstream annealing assertions cover single-level acceptance only.
    pub fn probability_at(
        &self,
        current: S,
        candidate: S,
        gradient: f64,
    ) -> Result<f64, AcceptanceError> {
        if !gradient.is_finite() || !(0.0..=1.0).contains(&gradient) {
            return Err(AcceptanceError::InvalidGradient);
        }
        if current.structural() != 0 || candidate.structural() != 0 {
            return Ok(0.0);
        }
        if candidate >= current {
            return Ok(1.0);
        }
        let mut energy = 0.0;
        for ((before, after), initial) in current
            .ordinary_levels()
            .zip(candidate.ordinary_levels())
            .zip(self.starting_temperature.ordinary_levels())
        {
            energy += before.cooling_loss(&after, &initial, gradient);
        }
        Ok((-energy).exp())
    }

    /// Replay a supplied uniform draw. This does not mutate the cooling clock or
    /// PRNG, making exact boundary assertions independent of a random algorithm.
    pub fn accepts_at(
        &self,
        current: S,
        candidate: S,
        gradient: f64,
        draw: f64,
    ) -> Result<bool, AcceptanceError> {
        if !draw.is_finite() || !(0.0..1.0).contains(&draw) {
            return Err(AcceptanceError::InvalidRandomDraw);
        }
        Ok(draw < self.probability_at(current, candidate, gradient)?)
    }

    /// Draw only when probability lies strictly between zero and one. Trials
    /// consume randomness but do not cool; selected steps must call step_ended.
    pub fn accepts(&mut self, current: S, candidate: S) -> bool {
        self.accepts_with_gradient(current, candidate, self.gradient())
    }

    pub(crate) fn accepts_with_gradient(
        &mut self,
        current: S,
        candidate: S,
        gradient: f64,
    ) -> bool {
        let probability = self
            .probability_at(current, candidate, gradient)
            .expect("internal gradient stays in [0, 1]");
        probability >= 1.0 || (probability > 0.0 && self.random.unit() < probability)
    }

    pub fn step_ended(&mut self) {
        if self.completed_steps < self.cooling_steps {
            self.completed_steps += 1;
        }
    }
}

/// Annealing cooled by elapsed phase time, frozen between `step_started` calls.
/// Backwards readings never heat the state again. Cloning snapshots the PRNG.
#[derive(Clone, Debug)]
pub struct TimeSimulatedAnnealing<S> {
    inner: SimulatedAnnealing<S>,
    cooling_time: std::time::Duration,
    gradient: f64,
}
impl<S: Score> TimeSimulatedAnnealing<S> {
    pub(crate) fn validate(
        temperature: &S,
        cooling_time: std::time::Duration,
    ) -> Result<(), AcceptanceError> {
        SimulatedAnnealing::validate(temperature, 1)?;
        if cooling_time.is_zero() {
            return Err(AcceptanceError::ZeroCoolingTime);
        }
        Ok(())
    }
    pub fn new(
        temperature: S,
        cooling_time: std::time::Duration,
        seed: u64,
    ) -> Result<Self, AcceptanceError> {
        Self::validate(&temperature, cooling_time)?;
        Ok(Self {
            inner: SimulatedAnnealing::new(temperature, 1, seed)?,
            cooling_time,
            gradient: 0.0,
        })
    }
    /// Freeze this step's gradient; candidate trials do not read a clock.
    pub fn step_started(&mut self, elapsed: std::time::Duration) {
        self.gradient = self
            .gradient
            .max(crate::ElapsedTimeLimit::new(self.cooling_time).progress(elapsed));
    }
    pub fn gradient(&self) -> f64 {
        self.gradient
    }
    pub fn probability(&self, current: S, candidate: S) -> f64 {
        self.inner
            .probability_at(current, candidate, self.gradient)
            .expect("clamped gradient")
    }
    /// Consume a draw only for probabilities strictly between zero and one.
    pub fn accepts(&mut self, current: S, candidate: S) -> bool {
        self.inner
            .accepts_with_gradient(current, candidate, self.gradient)
    }
}
