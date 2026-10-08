//! Relative rankings compare outcomes only within matching problem keys.
use crate::{BenchmarkOutcome, BenchmarkRanking, BenchmarkResultError, BenchmarkSummary};
use std::{cmp::Ordering, collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Debug)]
pub struct BenchmarkComparison<K> {
    pub configuration: usize,
    pub problem: K,
    pub outcome: BenchmarkOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BenchmarkStandingError {
    UnknownConfiguration { configuration: usize, count: usize },
    IncompleteRun,
    Score(BenchmarkResultError),
}
impl fmt::Display for BenchmarkStandingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownConfiguration {
                configuration,
                count,
            } => write!(
                f,
                "benchmark configuration {configuration} is outside 0..{count}"
            ),
            Self::IncompleteRun => f.write_str("cannot rank an incomplete benchmark schedule"),
            Self::Score(error) => error.fmt(f),
        }
    }
}
impl Error for BenchmarkStandingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Score(error) => Some(error),
            _ => None,
        }
    }
}
impl From<BenchmarkResultError> for BenchmarkStandingError {
    fn from(value: BenchmarkResultError) -> Self {
        Self::Score(value)
    }
}
#[derive(Clone, Debug)]
pub struct BenchmarkStanding {
    pub configuration: usize,
    /// One-based competition rank: tied leaders receive 1, the next receives 3.
    pub rank: usize,
    pub wins: u128,
    pub ties: u128,
    pub losses: u128,
    pub summary: BenchmarkSummary,
}
impl BenchmarkStanding {
    /// Greater means better: more wins, then more ties, then fewer losses.
    /// Equal counts use the existing total-score summary ranking as a tie-breaker.
    /// Configuration identity and display rank do not affect quality comparison.
    pub fn compare(&self, other: &Self) -> Ordering {
        self.wins
            .cmp(&other.wins)
            .then_with(|| self.ties.cmp(&other.ties))
            .then_with(|| other.losses.cmp(&self.losses))
            .then_with(|| {
                self.summary
                    .compare(&other.summary, BenchmarkRanking::TotalScore)
            })
    }
}

/// Compare every pair of outcomes from different configurations with equal problem
/// keys. Repeated keys contribute all cross-configuration pairs; include repetition
/// in the key to pair only matching repetitions. Outcomes from the same configuration
/// never compete. Failures lose to successes; incomplete successes lose to complete
/// successes, then exact numeric score ordering decides. Missing problems create no
/// synthetic losses. Unequal sample counts can bias totals; use matched sets.
///
/// Results are best-first, stable by configuration index on a quality tie. Configs
/// with no outcomes are included. Disjoint sets have zero comparisons and fall back
/// to numeric summary ordering; this fallback is not evidence of a matched comparison.
/// Group sorting and prefix counts avoid materializing or iterating every pair:
/// O(N log N + C log C) comparisons and O(N + C) storage for N observations
/// and C configurations (excluding the cost of comparing score levels and keys).
///
/// ```
/// use timetuck::*;
/// let entries=[(0,"a",-10),(1,"a",-9),(0,"b",-10000),(1,"b",-9999)]
///     .map(|(configuration,problem,score)|BenchmarkComparison {
///         configuration,problem,outcome:BenchmarkOutcome::success(&SimpleScore::new(score),true).unwrap()
///     });
/// let standings=rank_benchmarks(2,&entries)?;
/// assert_eq!(standings[0].configuration,1);
/// assert_eq!(standings[0].wins,2);
/// # Ok::<(), BenchmarkStandingError>(())
/// ```
pub fn rank_benchmarks<K: Ord>(
    configuration_count: usize,
    observations: &[BenchmarkComparison<K>],
) -> Result<Vec<BenchmarkStanding>, BenchmarkStandingError> {
    let mut outcomes = vec![vec![]; configuration_count];
    let mut groups: BTreeMap<&K, Vec<&BenchmarkComparison<K>>> = BTreeMap::new();
    for observation in observations {
        let Some(config) = outcomes.get_mut(observation.configuration) else {
            return Err(BenchmarkStandingError::UnknownConfiguration {
                configuration: observation.configuration,
                count: configuration_count,
            });
        };
        config.push(observation.outcome.clone());
        groups
            .entry(&observation.problem)
            .or_default()
            .push(observation);
    }
    let mut standings: Vec<_> = outcomes
        .into_iter()
        .enumerate()
        .map(|(configuration, outcomes)| BenchmarkStanding {
            configuration,
            rank: 0,
            wins: 0,
            ties: 0,
            losses: 0,
            summary: BenchmarkSummary::new(outcomes),
        })
        .collect();
    for group in groups.values_mut() {
        group.sort_by(|a, b| a.outcome.compare(&b.outcome));
        let mut totals = BTreeMap::<usize, u128>::new();
        for entry in group.iter() {
            *totals.entry(entry.configuration).or_default() += 1;
        }
        let mut lower = BTreeMap::<usize, u128>::new();
        let mut start = 0;
        while start < group.len() {
            let mut end = start + 1;
            while end < group.len() && group[start].outcome.compare(&group[end].outcome).is_eq() {
                end += 1;
            }
            let mut band = BTreeMap::<usize, u128>::new();
            for entry in &group[start..end] {
                *band.entry(entry.configuration).or_default() += 1;
            }
            for (&configuration, &count) in &band {
                let own_lower = lower.get(&configuration).copied().unwrap_or(0);
                let own_higher = totals[&configuration] - own_lower - count;
                let standing = &mut standings[configuration];
                standing.wins += count * (start as u128 - own_lower);
                standing.ties += count * ((end - start) as u128 - count);
                standing.losses += count * ((group.len() - end) as u128 - own_higher);
                *lower.entry(configuration).or_default() += count;
            }
            start = end;
        }
    }
    standings.sort_by(|a, b| {
        b.compare(a)
            .then_with(|| a.configuration.cmp(&b.configuration))
    });
    for i in 0..standings.len() {
        standings[i].rank = if i > 0 && standings[i].compare(&standings[i - 1]).is_eq() {
            standings[i - 1].rank
        } else {
            i + 1
        };
    }
    Ok(standings)
}
