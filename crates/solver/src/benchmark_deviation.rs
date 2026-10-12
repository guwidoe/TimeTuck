//! Population score deviations for explicitly successful benchmark observations.
use crate::*;
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BenchmarkDeviationError {
    Count,
    Score(BenchmarkResultError),
    NonFinite,
}
impl fmt::Display for BenchmarkDeviationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "benchmark deviation: {self:?}")
    }
}
impl Error for BenchmarkDeviationError {}
/// RMS deviation around the supplied ordinary-level average. None represents a
/// failed trial; divisor must match successful observations and be nonzero.
/// Structural state is excluded. Exact numeric differences precede conversion;
/// scaled hypot accumulation avoids overflow in squared finite deviations.
/// This does not turn failed/incomplete trials into successful zero samples.
pub fn benchmark_score_deviations<S: Score, A: Score>(
    samples: &[Option<S>],
    average: &A,
    divisor: usize,
) -> Result<Vec<f64>, BenchmarkDeviationError> {
    if divisor == 0 || samples.iter().flatten().count() != divisor {
        return Err(BenchmarkDeviationError::Count);
    }
    let average = BenchmarkScore::from_score(average).map_err(BenchmarkDeviationError::Score)?;
    let mut out = vec![0.0_f64; average.levels().len()];
    let one = Decimal::from(1);
    let scale = (divisor as f64).sqrt();
    for sample in samples.iter().flatten() {
        let sample = BenchmarkScore::from_score(sample).map_err(BenchmarkDeviationError::Score)?;
        if sample.levels().len() != out.len() {
            return Err(BenchmarkDeviationError::Score(
                BenchmarkResultError::DifferentDimensions {
                    left: sample.levels().len(),
                    right: out.len(),
                },
            ));
        }
        for ((out, a), b) in out.iter_mut().zip(sample.levels()).zip(average.levels()) {
            let delta = if a.numeric_cmp(b).is_gt() {
                a.cooling_loss(b, &one, 0.0)
            } else {
                b.cooling_loss(a, &one, 0.0)
            };
            if !delta.is_finite() {
                return Err(BenchmarkDeviationError::NonFinite);
            }
            *out = out.hypot(delta / scale);
            if !out.is_finite() {
                return Err(BenchmarkDeviationError::NonFinite);
            }
        }
    }
    Ok(out)
}
/// Compact native formatting: three significant digits, with scientific notation
/// outside exponents -3..7. Keep at least one fractional digit. None stays absent;
/// invalid deviations return an error. Multiple levels are joined with '/'.
/// No arbitrary Java/locale numeric-formatting parity claim.
pub fn format_benchmark_deviations(
    values: Option<&[f64]>,
) -> Result<Option<String>, BenchmarkDeviationError> {
    let Some(values) = values else {
        return Ok(None);
    };
    let mut parts = Vec::new();
    for &v in values {
        if !v.is_finite() || v < 0.0 {
            return Err(BenchmarkDeviationError::NonFinite);
        }
        let text = format!("{v:.2e}");
        let (mantissa, exponent) = text.split_once('e').expect("scientific format");
        let exponent: i32 = exponent.parse().expect("scientific exponent");
        let rounded = text
            .parse::<f64>()
            .map_err(|_| BenchmarkDeviationError::NonFinite)?;
        if (-3..7).contains(&exponent) {
            let plain = format!("{rounded:.*}", (2 - exponent).max(1) as usize);
            let trimmed = plain.trim_end_matches('0').trim_end_matches('.');
            parts.push(if trimmed.contains('.') {
                trimmed.to_owned()
            } else {
                format!("{trimmed}.0")
            });
        } else {
            let trimmed = mantissa.trim_end_matches('0').trim_end_matches('.');
            let m = if trimmed.contains('.') {
                trimmed.to_owned()
            } else {
                format!("{trimmed}.0")
            };
            parts.push(format!("{m}E{exponent}"));
        }
    }
    Ok(Some(parts.join("/")))
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BenchmarkReportRanking {
    TotalScore,
    TotalRanking,
}
/// Owned report options with explicit native registration keys. These fields
/// describe requested formatting/ranking; they do not instantiate Java classes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BenchmarkReportOptions {
    pub locale: Option<String>,
    pub ranking: Option<BenchmarkReportRanking>,
    pub comparator: Option<PlanningType>,
    pub weight_factory: Option<PlanningType>,
}
impl BenchmarkReportOptions {
    pub fn inherit(&mut self, base: &Self) {
        if self.locale.is_none() {
            self.locale = base.locale.clone();
        }
        self.ranking = self.ranking.or(base.ranking);
        self.comparator = self.comparator.or(base.comparator);
        self.weight_factory = self.weight_factory.or(base.weight_factory);
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BenchmarkBlueprintDeclaration {
    pub kind: Option<BenchmarkBlueprint>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingBenchmarkBlueprint;
impl fmt::Display for MissingBenchmarkBlueprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("solverBenchmarkBluePrintType is missing")
    }
}
impl Error for MissingBenchmarkBlueprint {}
impl BenchmarkBlueprintDeclaration {
    pub fn validate(self) -> Result<BenchmarkBlueprint, MissingBenchmarkBlueprint> {
        self.kind.ok_or(MissingBenchmarkBlueprint)
    }
}
