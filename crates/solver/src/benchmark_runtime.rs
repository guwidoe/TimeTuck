//! Optional typed statistic persistence and native benchmark data artifacts.
use crate::*;
use serde_json::{Value, json};
use std::{fs, io, path::Path, time::Instant};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BenchmarkStatisticPoint {
    Long {
        time: i64,
        value: i64,
    },
    Score {
        time: i64,
        score: SimpleScore,
        initialized: bool,
    },
    Moves {
        time: i64,
        accepted: i64,
        selected: i64,
    },
    MoveType {
        name: String,
        count: i64,
    },
    Memory {
        time: i64,
        used: u64,
        maximum: u64,
    },
    Constraint {
        time: i64,
        name: String,
        count: i64,
        score: SimpleScore,
    },
    MoveScore {
        time: i64,
        name: String,
        score: SimpleScore,
    },
}
fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}
impl BenchmarkStatisticPoint {
    /// Linux resident bytes and physical memory are actual native readings,
    /// not JVM heap counters. Other platforms can supply measured Memory points.
    pub fn linux_memory(time: i64) -> io::Result<Self> {
        let read = |path: &str, key: &str| -> io::Result<u64> {
            let data = fs::read_to_string(path)?;
            let line = data
                .lines()
                .find(|l| l.starts_with(key))
                .ok_or_else(|| invalid("missing memory metric"))?;
            line.split_whitespace()
                .nth(1)
                .ok_or_else(|| invalid("missing memory value"))?
                .parse::<u64>()
                .map_err(invalid)?
                .checked_mul(1024)
                .ok_or_else(|| invalid("memory overflow"))
        };
        Ok(Self::Memory {
            time,
            used: read("/proc/self/status", "VmRSS:")?,
            maximum: read("/proc/meminfo", "MemTotal:")?,
        })
    }
    fn fields(&self) -> Vec<String> {
        match self {
            Self::Long { time, value } => vec![time.to_string(), value.to_string()],
            Self::Score {
                time,
                score,
                initialized,
            } => vec![time.to_string(), score.to_string(), initialized.to_string()],
            Self::Moves {
                time,
                accepted,
                selected,
            } => vec![time.to_string(), accepted.to_string(), selected.to_string()],
            Self::MoveType { name, count } => vec![name.clone(), count.to_string()],
            Self::Memory {
                time,
                used,
                maximum,
            } => vec![time.to_string(), used.to_string(), maximum.to_string()],
            Self::Constraint {
                time,
                name,
                count,
                score,
            } => vec![
                time.to_string(),
                name.clone(),
                count.to_string(),
                score.to_string(),
            ],
            Self::MoveScore { time, name, score } => {
                vec![time.to_string(), name.clone(), score.to_string()]
            }
        }
    }
}
fn header(kind: RecordStatistic) -> &'static [&'static str] {
    match kind {
        RecordStatistic::BestScore | RecordStatistic::StepScore => {
            &["timeMillisSpent", "score", "initialized"]
        }
        RecordStatistic::MoveCountPerStep => {
            &["timeMillisSpent", "acceptedMoveCount", "selectedMoveCount"]
        }
        RecordStatistic::MoveCountPerType => &["moveType", "count"],
        RecordStatistic::MemoryUse => &["timeMillisSpent", "usedMemory", "maxMemory"],
        RecordStatistic::ConstraintMatchTotalBestScore
        | RecordStatistic::ConstraintMatchTotalStepScore => &[
            "timeMillisSpent",
            "constraintId",
            "constraintMatchCount",
            "scoreTotal",
        ],
        RecordStatistic::PickedMoveTypeBestScoreDiff
        | RecordStatistic::PickedMoveTypeStepScoreDiff => {
            &["timeMillisSpent", "moveType", "scoreDiff"]
        }
        _ => &["timeMillisSpent", "value"],
    }
}
fn parse_point(kind: RecordStatistic, f: &[String]) -> io::Result<BenchmarkStatisticPoint> {
    if f.len() != header(kind).len() {
        return Err(invalid("point field count"));
    }
    let number = |i: usize| f[i].parse::<i64>().map_err(invalid);
    Ok(match kind {
        RecordStatistic::BestScore | RecordStatistic::StepScore => BenchmarkStatisticPoint::Score {
            time: number(0)?,
            score: f[1].parse().map_err(invalid)?,
            initialized: f[2].parse().map_err(invalid)?,
        },
        RecordStatistic::MoveCountPerStep => BenchmarkStatisticPoint::Moves {
            time: number(0)?,
            accepted: number(1)?,
            selected: number(2)?,
        },
        RecordStatistic::MoveCountPerType => BenchmarkStatisticPoint::MoveType {
            name: f[0].clone(),
            count: number(1)?,
        },
        RecordStatistic::MemoryUse => BenchmarkStatisticPoint::Memory {
            time: number(0)?,
            used: f[1].parse().map_err(invalid)?,
            maximum: f[2].parse().map_err(invalid)?,
        },
        RecordStatistic::ConstraintMatchTotalBestScore
        | RecordStatistic::ConstraintMatchTotalStepScore => BenchmarkStatisticPoint::Constraint {
            time: number(0)?,
            name: f[1].clone(),
            count: number(2)?,
            score: f[3].parse().map_err(invalid)?,
        },
        RecordStatistic::PickedMoveTypeBestScoreDiff
        | RecordStatistic::PickedMoveTypeStepScoreDiff => BenchmarkStatisticPoint::MoveScore {
            time: number(0)?,
            name: f[1].clone(),
            score: f[2].parse().map_err(invalid)?,
        },
        _ => BenchmarkStatisticPoint::Long {
            time: number(0)?,
            value: number(1)?,
        },
    })
}
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn csv_rows(s: &str) -> io::Result<Vec<Vec<String>>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut field = String::new();
    let mut chars = s.chars().peekable();
    let mut quoted = false;
    let mut closed = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' if field.is_empty() && !closed => quoted = true,
                ',' => {
                    row.push(std::mem::take(&mut field));
                    closed = false;
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                    closed = false;
                }
                '\r' if chars.peek() == Some(&'\n') => {}
                _ if closed => return Err(invalid("text after quoted CSV field")),
                _ => field.push(c),
            }
        }
    }
    if quoted {
        return Err(invalid("unterminated CSV quote"));
    }
    if !field.is_empty() || !row.is_empty() || closed {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkStatistic {
    pub kind: RecordStatistic,
    points: Option<Vec<BenchmarkStatisticPoint>>,
}
impl BenchmarkStatistic {
    pub fn new(kind: RecordStatistic, points: Vec<BenchmarkStatisticPoint>) -> io::Result<Self> {
        for point in &points {
            if parse_point(kind, &point.fields())? != *point {
                return Err(invalid("point incompatible with statistic"));
            }
        }
        Ok(Self {
            kind,
            points: Some(points),
        })
    }
    pub fn points(&self) -> Option<&[BenchmarkStatisticPoint]> {
        self.points.as_deref()
    }
    pub fn to_csv(&self) -> io::Result<String> {
        let points = self
            .points
            .as_ref()
            .ok_or_else(|| invalid("hibernated point list"))?;
        let mut csv = header(self.kind)
            .iter()
            .map(|s| quote(s))
            .collect::<Vec<_>>()
            .join(",")
            + "\n";
        for p in points {
            csv.push_str(
                &p.fields()
                    .iter()
                    .map(|s| quote(s))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
        }
        Ok(csv)
    }
    pub fn read_csv(kind: RecordStatistic, path: impl AsRef<Path>) -> io::Result<Self> {
        let input = fs::read_to_string(path)?;
        if input.len() > 16 * 1024 * 1024 {
            return Err(invalid("statistic byte limit"));
        }
        let mut rows = csv_rows(&input)?.into_iter();
        if rows.next() != Some(header(kind).iter().map(|s| s.to_string()).collect()) {
            return Err(invalid("statistic CSV header"));
        }
        Self::new(
            kind,
            rows.map(|r| parse_point(kind, &r))
                .collect::<io::Result<_>>()?,
        )
    }
    /// Release resident points only after successfully publishing their file.
    pub fn hibernate(&mut self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::write(path, self.to_csv()?)?;
        self.points = None;
        Ok(())
    }
    pub fn to_xml(&self) -> io::Result<String> {
        let mut out = format!("<statistic type=\"{}\">", self.kind.label());
        for p in self
            .points
            .as_ref()
            .ok_or_else(|| invalid("hibernated point list"))?
        {
            out.push_str("<point>");
            for f in p.fields() {
                out.push_str("<field>");
                out.push_str(
                    &f.replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;"),
                );
                out.push_str("</field>");
            }
            out.push_str("</point>");
        }
        out.push_str("</statistic>");
        Ok(out)
    }
    pub fn from_xml(s: &str) -> io::Result<Self> {
        if s.len() > 16 * 1024 * 1024 {
            return Err(invalid("statistic byte limit"));
        }
        let doc = roxmltree::Document::parse(s).map_err(invalid)?;
        let root = doc.root_element();
        if !root.has_tag_name("statistic") || root.attributes().len() != 1 {
            return Err(invalid("statistic schema"));
        }
        let kind = RecordStatistic::parse(
            root.attribute("type")
                .ok_or_else(|| invalid("statistic type"))?,
        )
        .map_err(invalid)?;
        let mut points = vec![];
        for n in root.children().filter(|n| n.is_element()) {
            if !n.has_tag_name("point") || n.attributes().len() != 0 {
                return Err(invalid("point schema"));
            }
            let mut fields = vec![];
            for f in n.children().filter(|n| n.is_element()) {
                if !f.has_tag_name("field")
                    || f.attributes().len() != 0
                    || f.children().any(|c| c.is_element())
                {
                    return Err(invalid("field schema"));
                }
                fields.push(f.text().unwrap_or("").to_owned());
            }
            points.push(parse_point(kind, &fields)?);
        }
        Self::new(kind, points)
    }
}
#[derive(Clone, Debug)]
pub struct BenchmarkStatisticRun {
    pub label: String,
    pub successful: bool,
    pub statistic: BenchmarkStatistic,
}
#[derive(Clone, Debug)]
pub enum BenchmarkChartGeometry {
    Lines(Vec<(String, BenchmarkSeries)>),
    Bars(Vec<(String, String, i64)>),
}
#[derive(Clone, Debug)]
pub struct BenchmarkStatisticChart {
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub geometry: BenchmarkChartGeometry,
}
/// Build actual retained point data, excluding failed runs. Native line series
/// validate monotonic time; bar counts retain exact signed integers.
pub fn benchmark_statistic_charts(
    problem: &str,
    kind: RecordStatistic,
    runs: &[BenchmarkStatisticRun],
) -> io::Result<Vec<BenchmarkStatisticChart>> {
    let (title, x, y) = match kind {
        RecordStatistic::ScoreCalculationSpeed => (
            "score calculation speed statistic",
            "Time spent",
            "Score calculation speed per second",
        ),
        RecordStatistic::MoveEvaluationSpeed => (
            "move evaluation speed statistic",
            "Time spent",
            "Move evaluation speed per second",
        ),
        RecordStatistic::MoveCountPerType => ("move count per type statistic", "Type", "Count"),
        _ => return Err(invalid("unsupported chart geometry")),
    };
    let geometry = if kind == RecordStatistic::MoveCountPerType {
        let mut bars = vec![];
        for r in runs.iter().filter(|r| r.successful) {
            if r.statistic.kind != kind {
                return Err(invalid("bar statistic kind"));
            }
            for p in r
                .statistic
                .points()
                .ok_or_else(|| invalid("hibernated chart input"))?
            {
                if let BenchmarkStatisticPoint::MoveType { name, count } = p {
                    bars.push((r.label.clone(), name.clone(), *count));
                } else {
                    return Err(invalid("bar input type"));
                }
            }
        }
        if bars.is_empty() {
            return Ok(vec![]);
        }
        BenchmarkChartGeometry::Bars(bars)
    } else {
        let mut lines = vec![];
        for r in runs.iter().filter(|r| r.successful) {
            if r.statistic.kind != kind {
                return Err(invalid("line statistic kind"));
            }
            let mut points = vec![];
            for p in r
                .statistic
                .points()
                .ok_or_else(|| invalid("hibernated chart input"))?
            {
                if let BenchmarkStatisticPoint::Long { time, value } = p {
                    points.push(ChartPoint {
                        x: *time as f64,
                        y: Some(*value as f64),
                    });
                } else {
                    return Err(invalid("line input type"));
                }
            }
            if !points.is_empty() {
                lines.push((
                    r.label.clone(),
                    BenchmarkSeries::new(points).map_err(invalid)?,
                ));
            }
        }
        if lines.is_empty() {
            return Ok(vec![]);
        }
        BenchmarkChartGeometry::Lines(lines)
    };
    Ok(vec![BenchmarkStatisticChart {
        title: format!("{problem} {title}"),
        x_label: x.into(),
        y_label: y.into(),
        geometry,
    }])
}
impl BenchmarkStatisticChart {
    pub fn write_data(&self, path: impl AsRef<Path>, budget: usize) -> io::Result<()> {
        let data = match &self.geometry {
            BenchmarkChartGeometry::Bars(b) => json!({"bars":b}),
            BenchmarkChartGeometry::Lines(lines) => {
                let mut series = vec![];
                for (label, line) in lines {
                    let points = line.downsample(budget).map_err(invalid)?;
                    series.push(json!({"label":label,"points":points.iter().map(|p|json!([p.x,p.y])).collect::<Vec<_>>()}));
                }
                json!({"series":series})
            }
        };
        fs::write(path,serde_json::to_vec_pretty(&json!({"title":self.title,"xLabel":self.x_label,"yLabel":self.y_label,"data":data})).map_err(invalid)?)
    }
}
/// Collect the actual best/initial complete score on local-search entry and
/// every retained LS improvement. Construction notifications do not become chart points.
/// A step limit/clock is supplied through the real solver configuration.
pub fn benchmark_solve_with_best_csv<V: Eq + Clone>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, SimpleScore>,
    config: SolverConfig,
    path: impl AsRef<Path>,
) -> io::Result<SolverResult<Assignment<V>, SimpleScore>> {
    let metrics = SolverMetrics::new(None);
    let origin = Instant::now();
    let points = std::cell::RefCell::new(Vec::new());
    let append = |score: SimpleScore, complete: bool| {
        let mut points = points.borrow_mut();
        if complete
            && points.last().is_none_or(
                |p| !matches!(p, BenchmarkStatisticPoint::Score {score: old,..} if *old == score),
            )
        {
            points.push(BenchmarkStatisticPoint::Score {
                time: i64::try_from(origin.elapsed().as_millis()).unwrap_or(i64::MAX),
                score,
                initialized: complete,
            });
        }
    };
    let mut listener = |event| {
        if matches!(
            event,
            SolverLifecycleEvent::PhaseStarted {
                kind: SolverPhaseKind::LocalSearch,
                ..
            }
        ) {
            let snapshot = metrics.snapshot();
            if let Some(active) = snapshot.active.first()
                && let Some(score) = active.best_score
            {
                append(score, active.complete);
            }
        }
    };
    let mut events = |event: SolverEvent<'_, Assignment<V>, SimpleScore>| {
        if let SolverEvent::Best(update) = event
            && update.kind == SolverPhaseKind::LocalSearch
        {
            append(update.score, update.complete);
        }
    };
    let result = solve_with_extensions(
        problem,
        network,
        config,
        SolverExtensions {
            metrics: Some(&metrics),
            lifecycle: Some(&mut listener),
            events: Some(&mut events),
            ..Default::default()
        },
    )
    .map_err(invalid)?;
    let stat = BenchmarkStatistic::new(RecordStatistic::BestScore, points.into_inner())?;
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, stat.to_csv()?)?;
    Ok(result)
}
#[derive(Clone, Debug)]
pub struct BenchmarkRecordAggregation {
    pub solver_summaries: Vec<BenchmarkSummary>,
    pub problem_count: usize,
    pub failure_count: usize,
}
impl BenchmarkRecordAggregation {
    pub fn from_records(records: &BenchmarkRecordSet) -> io::Result<Self> {
        records.validate().map_err(invalid)?;
        let observations = records.observations().map_err(invalid)?;
        let solver_summaries: Vec<_> = (0..records.solvers.len())
            .map(|configuration| {
                BenchmarkSummary::new(
                    observations
                        .iter()
                        .filter(|o| o.configuration == configuration)
                        .map(|o| o.outcome.clone()),
                )
            })
            .collect();
        let failure_count = solver_summaries
            .iter()
            .map(BenchmarkSummary::failure_count)
            .sum();
        Ok(Self {
            solver_summaries,
            problem_count: records.problems.len(),
            failure_count,
        })
    }
    pub fn write_summary(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let data: Value = json!({"solverCount":self.solver_summaries.len(),"problemCount":self.problem_count,"failureCount":self.failure_count,"runs":self.solver_summaries.iter().map(BenchmarkSummary::run_count).collect::<Vec<_>>()});
        fs::write(path, serde_json::to_vec_pretty(&data).map_err(invalid)?)
    }
}
