//! Optional benchmark plot data, exact ticks and locale-independent elapsed labels.
//! Triangle selection follows Steinarsson (2013), thesis algorithm 4.2, written
//! independently from the mathematical specification (no implementation copied):
//! https://ggalmazor.com/assets/SS_MSthesis.pdf, section 4.2, pages 20–22.
use crate::Decimal;
use num_bigint::BigInt;
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartPoint {
    pub x: f64,
    pub y: Option<f64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChartDataError {
    InvalidPoint { index: usize },
    Unordered { index: usize },
    BudgetTooSmall { minimum: usize },
    InvalidTickRange,
    TickScaleOverflow,
}
impl fmt::Display for ChartDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "benchmark chart data: {self:?}")
    }
}
impl Error for ChartDataError {}
/// Exact power-of-ten tick near one hundred intervals. Select the nearest decade
/// of the positive span, comparing its squared coefficient to the decade midpoint
/// exactly. No binary logarithm/underflow is used. Reversed/equal ranges fail.
pub fn benchmark_tick_step(
    minimum: &Decimal,
    maximum: &Decimal,
) -> Result<Decimal, ChartDataError> {
    if maximum.numeric_cmp(minimum) != std::cmp::Ordering::Greater {
        return Err(ChartDataError::InvalidTickRange);
    }
    let span = maximum.subtract(minimum);
    let digits = span.coefficient().to_string().len();
    let floor = digits as i64 - 1 - i64::from(span.scale());
    let midpoint = BigInt::from(10).pow(
        u32::try_from(
            digits
                .checked_mul(2)
                .and_then(|n| n.checked_sub(1))
                .ok_or(ChartDataError::TickScaleOverflow)?,
        )
        .map_err(|_| ChartDataError::TickScaleOverflow)?,
    );
    let round_up = span.coefficient() * span.coefficient() >= midpoint;
    let exponent = floor + i64::from(round_up) - 2;
    let scale = i32::try_from(-exponent).map_err(|_| ChartDataError::TickScaleOverflow)?;
    Ok(Decimal::new(BigInt::from(1), scale))
}
/// Validated chronological data. None is an explicit discontinuity, never zero.
/// Downsampling applies triangle selection separately to each non-gap run;
/// run endpoints and gap boundaries survive. Budget counts non-gap points only.
/// Too small a budget returns an error rather than silently bridging gaps.
#[derive(Clone, Debug)]
pub struct BenchmarkSeries {
    points: Vec<ChartPoint>,
}
impl BenchmarkSeries {
    pub fn new(points: Vec<ChartPoint>) -> Result<Self, ChartDataError> {
        for (i, p) in points.iter().enumerate() {
            if !p.x.is_finite() || p.y.is_some_and(|y| !y.is_finite()) {
                return Err(ChartDataError::InvalidPoint { index: i });
            }
            if i > 0 && p.x <= points[i - 1].x {
                return Err(ChartDataError::Unordered { index: i });
            }
        }
        Ok(Self { points })
    }
    pub fn points(&self) -> &[ChartPoint] {
        &self.points
    }
    pub fn downsample(&self, maximum_non_gap: usize) -> Result<Vec<ChartPoint>, ChartDataError> {
        let mut runs = vec![];
        let mut i = 0;
        while i < self.points.len() {
            let start = i;
            let gap = self.points[i].y.is_none();
            while i < self.points.len() && self.points[i].y.is_none() == gap {
                i += 1;
            }
            runs.push((start, i, gap));
        }
        let minimum: usize = runs
            .iter()
            .filter(|r| !r.2)
            .map(|(s, e, _)| (e - s).min(2))
            .sum();
        if maximum_non_gap < minimum {
            return Err(ChartDataError::BudgetTooSmall { minimum });
        }
        let count: usize = runs.iter().filter(|r| !r.2).map(|(s, e, _)| e - s).sum();
        let target = maximum_non_gap.min(count);
        let capacity = count - minimum;
        let extra = target - minimum;
        let mut budgets: Vec<_> = runs
            .iter()
            .map(|&(s, e, gap)| {
                if gap {
                    0
                } else {
                    let len = e - s;
                    let base = len.min(2);
                    base + if capacity == 0 {
                        0
                    } else {
                        ((extra as u128 * (len - base) as u128) / capacity as u128) as usize
                    }
                }
            })
            .collect();
        let mut remaining = target - budgets.iter().sum::<usize>();
        for ((s, e, gap), b) in runs.iter().zip(&mut budgets) {
            if remaining > 0 && !*gap && *b < e - s {
                *b += 1;
                remaining -= 1;
            }
        }
        let mut out = Vec::new();
        for ((s, e, gap), budget) in runs.into_iter().zip(budgets) {
            let points = &self.points[s..e];
            if gap {
                out.push(points[0]);
                if points.len() > 1 {
                    out.push(*points.last().unwrap());
                }
            } else {
                triangle_run(points, budget, &mut out);
            }
        }
        Ok(out)
    }
}
fn triangle_run(points: &[ChartPoint], budget: usize, out: &mut Vec<ChartPoint>) {
    if budget >= points.len() {
        out.extend_from_slice(points);
        return;
    }
    out.push(points[0]);
    if budget == 2 {
        out.push(*points.last().unwrap());
        return;
    }
    let every = (points.len() - 2) as f64 / (budget - 2) as f64;
    let mut selected = 0;
    for bucket in 0..budget - 2 {
        let average_start =
            (((bucket + 1) as f64 * every).floor() as usize + 1).min(points.len() - 1);
        let average_end = (((bucket + 2) as f64 * every).floor() as usize + 1).min(points.len());
        let average = &points[average_start..average_end];
        let x = average
            .iter()
            .map(|p| p.x / average.len() as f64)
            .sum::<f64>();
        let y = average
            .iter()
            .map(|p| p.y.unwrap() / average.len() as f64)
            .sum::<f64>();
        let start = (bucket as f64 * every).floor() as usize + 1;
        let end = (((bucket + 1) as f64 * every).floor() as usize + 1).min(points.len() - 1);
        let a = points[selected];
        let mut area = -1.0;
        let mut best = start;
        for (offset, b) in points[start..end].iter().enumerate() {
            let current = ((a.x - x) * (b.y.unwrap() - a.y.unwrap())
                - (a.x - b.x) * (y - a.y.unwrap()))
            .abs();
            if current > area {
                area = current;
                best = start + offset;
            }
        }
        out.push(points[best]);
        selected = best;
    }
    out.push(*points.last().unwrap());
}
/// Deterministic en-US integer grouping and elapsed fields, independent of process
/// locale. None denotes an absent measurement. Milliseconds use unsigned native
/// values; no Java TemplateNumberModel/Freemarker API or negative-time contract.
pub fn format_benchmark_millis(millis: Option<u64>) -> String {
    let Some(ms) = millis else {
        return "None.".into();
    };
    let digits = ms.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    if ms < 1000 {
        return format!("{grouped} ms.");
    }
    let seconds = ms / 1000;
    let fraction = ms % 1000;
    let elapsed = if seconds < 60 {
        format!("{seconds}.{fraction:03}")
    } else if seconds < 3600 {
        format!("{:02}:{:02}.{fraction:03}", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!(
            "{:02}:{:02}:{:02}.{fraction:03}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!(
            "{:02}:{:02}:{:02}:{:02}.{fraction:03}",
            seconds / 86400,
            seconds / 3600 % 24,
            seconds / 60 % 60,
            seconds % 60
        )
    };
    format!("{elapsed} s. ({grouped} ms.)")
}
