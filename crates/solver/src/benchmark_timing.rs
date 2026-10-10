//! Timing comparisons matched by problem, repetition and seed.
use crate::{BenchmarkTrialResult, Score};
use std::{cmp::Ordering, collections::BTreeMap};

/// Two measured trials from the same problem, repetition and seed. Raw outcomes,
/// elapsed times and work remain available, including errors and unequal quality.
#[derive(Debug)]
pub struct BenchmarkTimingPair<'a, S, E> {
    pub baseline: &'a BenchmarkTrialResult<S, E>,
    pub candidate: &'a BenchmarkTrialResult<S, E>,
}
impl<S: Score, E> BenchmarkTimingPair<'_, S, E> {
    /// Candidate quality relative to baseline: initialized beats uninitialized,
    /// then ordinary score ordering applies (including structural score levels).
    /// None means at least one trial failed; inspect the original typed results.
    pub fn quality_order(&self) -> Option<Ordering> {
        let baseline = self.baseline.result.as_ref().ok()?;
        let candidate = self.candidate.result.as_ref().ok()?;
        Some(
            candidate
                .initialized
                .cmp(&baseline.initialized)
                .then_with(|| candidate.score.cmp(&baseline.score)),
        )
    }
    /// Whether successful callbacks reported identical work. This does not imply
    /// identical algorithms or traces; work counters retain native definitions.
    pub fn equal_work(&self) -> Option<bool> {
        Some(self.baseline.result.as_ref().ok()?.work == self.candidate.result.as_ref().ok()?.work)
    }
    /// Candidate elapsed time / baseline elapsed time. Below one means less time.
    /// Only successful, initialized, equal-score pairs with nonzero elapsed times
    /// qualify. Unequal work is permitted and separately visible via equal_work.
    /// This is an observed ratio, not a confidence interval or a causal claim.
    pub fn candidate_over_baseline(&self) -> Option<f64> {
        let a = &self.baseline.trial;
        let b = &self.candidate.trial;
        if a.warmup
            || b.warmup
            || (a.problem, a.repetition, a.seed) != (b.problem, b.repetition, b.seed)
        {
            return None;
        }
        let baseline = self.baseline.result.as_ref().ok()?;
        let candidate = self.candidate.result.as_ref().ok()?;
        if !baseline.initialized
            || !candidate.initialized
            || baseline.score != candidate.score
            || self.baseline.elapsed.is_zero()
            || self.candidate.elapsed.is_zero()
        {
            return None;
        }
        Some(self.candidate.elapsed.as_secs_f64() / self.baseline.elapsed.as_secs_f64())
    }
}

/// Comparison for one problem in a native benchmark report. Warm-ups are excluded.
/// Pairs are ordered by repetition and seed. Missing counterparts remain visible;
/// cancelled schedules may have unequal counts and schedule_complete will be false.
/// Matching inputs/seeds does not establish equivalent solver work or random traces.
#[derive(Debug)]
pub struct BenchmarkTimingComparison<'a, S, E> {
    pub baseline_configuration: usize,
    pub candidate_configuration: usize,
    pub problem: usize,
    pub schedule_complete: bool,
    pub pairs: Vec<BenchmarkTimingPair<'a, S, E>>,
    pub baseline_unpaired: Vec<&'a BenchmarkTrialResult<S, E>>,
    pub candidate_unpaired: Vec<&'a BenchmarkTrialResult<S, E>>,
}
impl<'a, S: Score, E> BenchmarkTimingComparison<'a, S, E> {
    pub(crate) fn from_trials(
        baseline_configuration: usize,
        candidate_configuration: usize,
        problem: usize,
        schedule_complete: bool,
        trials: &'a [BenchmarkTrialResult<S, E>],
    ) -> Self {
        let selected = |configuration| {
            trials
                .iter()
                .filter(move |t| {
                    !t.trial.warmup
                        && t.trial.problem == problem
                        && t.trial.configuration == configuration
                })
                .map(|t| ((t.trial.repetition, t.trial.seed), t))
                .collect::<BTreeMap<_, _>>()
        };
        let mut candidate = selected(candidate_configuration);
        let mut pairs = Vec::new();
        let mut baseline_unpaired = Vec::new();
        for (key, baseline) in selected(baseline_configuration) {
            if let Some(candidate) = candidate.remove(&key) {
                pairs.push(BenchmarkTimingPair {
                    baseline,
                    candidate,
                });
            } else {
                baseline_unpaired.push(baseline);
            }
        }
        Self {
            baseline_configuration,
            candidate_configuration,
            problem,
            schedule_complete,
            pairs,
            baseline_unpaired,
            candidate_unpaired: candidate.into_values().collect(),
        }
    }
    /// Number of pairs contributing to median_time_ratio. All omitted pairs and
    /// unmatched trials remain inspectable; always report this sample coverage.
    pub fn comparable_pairs(&self) -> usize {
        self.pairs
            .iter()
            .filter(|p| p.candidate_over_baseline().is_some())
            .count()
    }
    /// Median of eligible per-repetition ratios, not a ratio of aggregate times.
    /// For an even count, average the two middle ratios. None means no eligible
    /// pairs. Excluding failures/unequal quality can select a biased subset; this
    /// statistic does not summarize those omitted runs or certify a full schedule.
    pub fn median_time_ratio(&self) -> Option<f64> {
        let mut ratios: Vec<_> = self
            .pairs
            .iter()
            .filter_map(|p| p.candidate_over_baseline())
            .collect();
        if ratios.is_empty() {
            return None;
        }
        ratios.sort_by(f64::total_cmp);
        let middle = ratios.len() / 2;
        Some(if ratios.len().is_multiple_of(2) {
            (ratios[middle - 1] + ratios[middle]) / 2.0
        } else {
            ratios[middle]
        })
    }
}
