//! Sliding-window termination over retained best scores.
use crate::{AdditiveScore, BigInt, Decimal, Score, ScoreLevel};
use std::{collections::VecDeque, error::Error, fmt, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiminishedReturnsError {
    ZeroWindow,
    ZeroRatio,
    UnsupportedScoreArithmetic,
    NoScoreLevels,
}
impl fmt::Display for DiminishedReturnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroWindow => "diminished-returns window must be positive",
            Self::ZeroRatio => {
                "diminished-returns ratio numerator and denominator must be positive"
            }
            Self::UnsupportedScoreArithmetic => {
                "diminished returns requires exact decimal score levels"
            }
            Self::NoScoreLevels => "diminished returns requires at least one ordinary score level",
        })
    }
}
impl Error for DiminishedReturnsError {}

/// Stop when the recent improvement rate is strictly below the given fraction
/// of the initial rate. Equality continues. There is no floating-point conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiminishedReturnsConfig {
    window: Duration,
    numerator: u64,
    denominator: u64,
}
impl DiminishedReturnsConfig {
    pub fn new(
        window: Duration,
        numerator: u64,
        denominator: u64,
    ) -> Result<Self, DiminishedReturnsError> {
        if window.is_zero() {
            return Err(DiminishedReturnsError::ZeroWindow);
        }
        if numerator == 0 || denominator == 0 {
            return Err(DiminishedReturnsError::ZeroRatio);
        }
        Ok(Self {
            window,
            numerator,
            denominator,
        })
    }
    pub fn window(self) -> Duration {
        self.window
    }
    pub fn ratio(self) -> (u64, u64) {
        (self.numerator, self.denominator)
    }
}
#[derive(Clone, Debug)]
struct Active<S> {
    start: Duration,
    last_time: Duration,
    baseline: S,
    reference: Option<(Decimal, Duration)>,
    history: VecDeque<(Duration, S)>,
}
impl<S: Clone> Active<S> {
    fn new(now: Duration, score: S) -> Self {
        let history = VecDeque::from([(now, score.clone())]);
        Self {
            start: now,
            last_time: now,
            baseline: score,
            reference: None,
            history,
        }
    }
}

/// A deterministic policy with an explicit monotonic elapsed-time input. Pass
/// retained best scores, not worsening current solutions. Initialization may be
/// arbitrarily long: phase_started does not arm the grace period; step_started does.
/// A direct start is also available. Expiry latches until start/phase_started.
///
/// Improvements in structural state or any hard/higher-priority ordinary level
/// reset the grace period. The lowest-priority ordinary level measures the rate.
/// Scores must use structural-first, lexicographic numeric ordering. Built-in
/// integer/decimal families are supported; exact_decimal is required for custom
/// levels. Large decimal scale gaps have the same arithmetic storage cost as Decimal.
/// History retains changes within the window plus one predecessor, not every step.
///
/// ```
/// use timetuck::{DiminishedReturns, DiminishedReturnsConfig, SimpleScore};
/// use std::time::Duration;
/// let ms = Duration::from_millis;
/// let mut limit = DiminishedReturns::<SimpleScore>::new(
///     DiminishedReturnsConfig::new(ms(10), 1, 2)?)?;
/// limit.start(ms(0), SimpleScore::new(0));
/// assert!(!limit.is_terminated(ms(10), &SimpleScore::new(10)));
/// limit.step(ms(15), &SimpleScore::new(12));
/// assert!(limit.is_terminated(ms(20), &SimpleScore::new(12)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct DiminishedReturns<S> {
    config: DiminishedReturnsConfig,
    reset_levels: usize,
    pending: Option<S>,
    active: Option<Active<S>>,
    terminated: bool,
}
impl<S: AdditiveScore> DiminishedReturns<S> {
    pub fn new(config: DiminishedReturnsConfig) -> Result<Self, DiminishedReturnsError> {
        if S::Level::zero().exact_decimal().is_none() {
            return Err(DiminishedReturnsError::UnsupportedScoreArithmetic);
        }
        let levels = S::ZERO.ordinary_levels().count();
        if levels == 0 {
            return Err(DiminishedReturnsError::NoScoreLevels);
        }
        Ok(Self {
            config,
            reset_levels: S::HARD_LEVEL_COUNT.max(levels - 1),
            pending: None,
            active: None,
            terminated: false,
        })
    }
    pub fn phase_started(&mut self, best: S) {
        self.pending = Some(best);
        self.active = None;
        self.terminated = false;
    }
    pub fn step_started(&mut self, now: Duration) {
        if self.active.is_none()
            && let Some(best) = self.pending.as_ref()
        {
            self.start(now, best.clone());
        }
    }
    pub fn start(&mut self, now: Duration, best: S) {
        let active = Active::new(now, best);
        self.active = Some(active);
        self.pending = None;
        self.terminated = false;
    }
    pub fn grace_started(&self) -> bool {
        self.active.is_some()
    }
    pub fn grace_start(&self) -> Option<Duration> {
        self.active.as_ref().map(|a| a.start)
    }
    pub fn retained_samples(&self) -> usize {
        self.active.as_ref().map_or(0, |a| a.history.len())
    }
    fn observe(&mut self, now: Duration, best: &S) {
        let Some(active) = &mut self.active else {
            return;
        };
        let now = now.max(active.last_time);
        let stronger = best
            .structural()
            .cmp(&active.baseline.structural())
            .then_with(|| {
                best.ordinary_levels()
                    .zip(active.baseline.ordinary_levels())
                    .take(self.reset_levels)
                    .map(|(a, b)| a.numeric_cmp(&b))
                    .find(|o| !o.is_eq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .is_gt();
        if stronger {
            *active = Active::new(now, best.clone());
            return;
        }
        if active.history.back().is_none_or(|(_, old)| best != old) {
            let value = best.clone();
            if active.history.back().is_some_and(|(time, _)| *time == now) {
                active.history.pop_back();
            }
            active.history.push_back((now, value));
        }
        active.last_time = now;
    }
    pub fn step(&mut self, now: Duration, best: &S) {
        if !self.terminated {
            self.observe(now, best);
            self.trim();
        }
    }
    fn trim(&mut self) {
        if let Some(a) = &mut self.active {
            let cutoff = a.last_time.saturating_sub(self.config.window);
            while a.history.len() > 1 && a.history[1].0 <= cutoff {
                a.history.pop_front();
            }
        }
    }
    pub fn is_terminated(&mut self, now: Duration, best: &S) -> bool {
        if self.terminated {
            return true;
        }
        self.observe(now, best);
        let Some(active) = &mut self.active else {
            return false;
        };
        let elapsed = active.last_time.saturating_sub(active.start);
        if elapsed < self.config.window {
            return false;
        }
        if active.reference.is_none() {
            let gain = soft_gain(best, &active.baseline);
            if !gain.numeric_cmp(&Decimal::ZERO).is_gt() {
                self.terminated = true;
                return true;
            }
            active.reference = Some((gain, elapsed));
            self.trim();
            return false;
        }
        self.trim();
        let active = self.active.as_ref().expect("active window");
        let (reference, period) = active.reference.as_ref().expect("finished grace");
        let gain = soft_gain(best, &active.history.front().expect("initial sample").1);
        let scale = |value: &Decimal, factor: BigInt| {
            Decimal::new(value.coefficient() * factor, value.scale())
        };
        let current = scale(
            &gain,
            BigInt::from(period.as_nanos()) * self.config.denominator,
        );
        let required = scale(
            reference,
            BigInt::from(self.config.window.as_nanos()) * self.config.numerator,
        );
        self.terminated = current.numeric_cmp(&required).is_lt();
        self.terminated
    }
}
fn soft_gain<S: Score>(to: &S, from: &S) -> Decimal {
    let soft = |s: &S| {
        s.ordinary_levels()
            .last()
            .expect("validated levels")
            .exact_decimal()
            .expect("validated arithmetic")
    };
    soft(to).subtract(&soft(from))
}

/// Existing time/stagnation controls retain precedence. Diminished returns arms
/// at the first candidate-boundary poll, after initial scoring and notification.
pub(crate) struct LocalTimeBudget<'a, S, C> {
    base: crate::time_limit::TimeBudget<'a, C>,
    rate: Option<DiminishedReturns<S>>,
    significant: Option<crate::SignificantImprovement<S>>,
}
impl<'a, S: AdditiveScore, C: FnMut() -> Duration> LocalTimeBudget<'a, S, C> {
    pub(crate) fn new(
        config: &crate::LocalSearchConfig<S>,
        clock: C,
        shared: Option<&'a mut dyn crate::time_limit::SearchControl>,
    ) -> Result<Self, DiminishedReturnsError> {
        Ok(Self {
            significant: config
                .significant_improvement
                .cloned()
                .map(crate::SignificantImprovement::new),
            base: crate::time_limit::TimeBudget::new(
                config.max_time,
                config.max_unimproved_time,
                clock,
            )
            .with_shared(shared),
            rate: config
                .diminished_returns
                .map(DiminishedReturns::new)
                .transpose()?,
        })
    }
    pub(crate) fn start_search(&mut self) {
        self.base.start_search();
    }
    pub(crate) fn record_improvement(&mut self) {
        self.base.record_improvement();
    }
    pub(crate) fn start_step(&mut self) {
        self.base.start_step();
    }
    pub(crate) fn end_step(&mut self, completed: bool) {
        self.base.end_step(completed);
    }
    pub(crate) fn complete_step(&mut self, improved: bool, best: &S) {
        self.base.complete_step(improved);
        if let Some(limit) = &mut self.significant {
            limit.is_terminated(self.base.elapsed(), best);
        }
        if let Some(rate) = &mut self.rate {
            rate.step(self.base.elapsed(), best);
        }
    }
    pub(crate) fn stop(&mut self, best: &S) -> Option<crate::LocalSearchTermination> {
        if let Some(reason) = self.base.stop() {
            return Some(reason);
        }
        if let Some(limit) = &mut self.significant {
            let now = self.base.elapsed();
            if !limit.is_started() {
                limit.start(now, best.clone());
            }
            if limit.is_terminated(now, best) {
                return Some(crate::LocalSearchTermination::SignificantImprovementTimeLimit);
            }
        }
        if let Some(rate) = &mut self.rate {
            let now = self.base.elapsed();
            if !rate.grace_started() {
                rate.start(now, best.clone());
            }
            if rate.is_terminated(now, best) {
                return Some(crate::LocalSearchTermination::DiminishedReturns);
            }
        }
        None
    }
    pub(crate) fn expired(&mut self, best: &S) -> bool {
        self.stop(best).is_some()
    }
}
