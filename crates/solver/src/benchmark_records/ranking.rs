use super::*;
use std::{cmp::Ordering, sync::Arc};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordRanking {
    TotalScore,
    TotalRanking,
    WorstScore,
}
impl RecordRanking {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "TOTAL_SCORE" => Self::TotalScore,
            "TOTAL_RANKING" => Self::TotalRanking,
            "WORST_SCORE" => Self::WorstScore,
            _ => return Err(fail(format!("invalid solverRankingType {s}"))),
        })
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::TotalScore => "TOTAL_SCORE",
            Self::TotalRanking => "TOTAL_RANKING",
            Self::WorstScore => "WORST_SCORE",
        }
    }
}
type Comparator = dyn Fn(&BenchmarkSummary, &BenchmarkSummary) -> Ordering + Send + Sync;
type Weight = dyn Fn(&BenchmarkStanding) -> i128 + Send + Sync;
#[derive(Default)]
pub struct BenchmarkRecordReportPolicy {
    pub ranking: Option<RecordRanking>,
    pub comparator: Option<Arc<Comparator>>,
    pub weight: Option<Arc<Weight>>,
}
impl BenchmarkRecordReportPolicy {
    pub fn validate(&self) -> Result<()> {
        let selected = [
            ("solverRankingType", self.ranking.is_some()),
            ("solverRankingComparatorClass", self.comparator.is_some()),
            ("solverRankingWeightFactoryClass", self.weight.is_some()),
        ];
        let used: Vec<_> = selected
            .into_iter()
            .filter(|(_, b)| *b)
            .map(|(n, _)| n)
            .collect();
        if used.len() > 1 {
            return Err(BenchmarkRecordError::ConflictingRanking(used));
        }
        Ok(())
    }
    /// Validate before computing any data or invoking user comparators/weights.
    /// Ranking consumes actual linked observations through native exact summaries
    /// and per-problem standings. Empty/unscored successes are not fabricated scores.
    pub fn rank(&self, records: &BenchmarkRecordSet) -> Result<Vec<usize>> {
        self.validate()?;
        let observations = records.observations()?;
        let standings = rank_benchmarks(records.solvers.len(), &observations)
            .map_err(|e| fail(e.to_string()))?;
        let mut indices: Vec<_> = (0..records.solvers.len()).collect();
        if let Some(weight) = &self.weight {
            let weights: std::collections::BTreeMap<_, _> = standings
                .iter()
                .map(|s| (s.configuration, weight(s)))
                .collect();
            indices.sort_by(|a, b| weights[b].cmp(&weights[a]));
        } else if self.ranking == Some(RecordRanking::TotalRanking) {
            indices = standings.iter().map(|s| s.configuration).collect();
        } else {
            let summaries: std::collections::BTreeMap<_, _> = standings
                .into_iter()
                .map(|s| (s.configuration, s.summary))
                .collect();
            indices.sort_by(|a, b| {
                if let Some(c) = &self.comparator {
                    c(&summaries[b], &summaries[a])
                } else {
                    summaries[b].compare(
                        &summaries[a],
                        if self.ranking == Some(RecordRanking::WorstScore) {
                            BenchmarkRanking::WorstScore
                        } else {
                            BenchmarkRanking::TotalScore
                        },
                    )
                }
            });
        }
        Ok(indices)
    }
}
