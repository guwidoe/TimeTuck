//! Optional archive and aggregation of native benchmark reports.
use serde_json::Value;
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum BenchmarkArchiveError {
    EvaluationOverflow,
    Configuration(String),
    MissingDirectory,
    NoResults,
    NoSelectedResults,
    InvalidTimestamp,
    InvalidReport,
    Io(io::Error),
}
impl fmt::Display for BenchmarkArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(e) => write!(f, "benchmark archive configuration: {e}"),
            Self::EvaluationOverflow => f.write_str("aggregate evaluation count exceeds u64"),
            Self::MissingDirectory => f.write_str("Benchmark directory does not exist"),
            Self::NoResults => f.write_str("No benchmark results found in directory"),
            Self::NoSelectedResults => {
                f.write_str("No valid benchmark results found in the selected directories")
            }
            Self::InvalidTimestamp => f.write_str("invalid benchmark archive timestamp"),
            Self::InvalidReport => f.write_str("invalid native benchmark report"),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl Error for BenchmarkArchiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<io::Error> for BenchmarkArchiveError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Caller-supplied timestamp names are explicit; this layer neither measures time
/// nor changes solver scheduling. Publication copies an actual runner report.
/// Schema 1 journals from XmlBenchmarkRun are validated and retained verbatim.
/// Aggregation combines measured trial counts/work, preserving failures and
/// incomplete runs; warm-ups remain in data but are excluded from summaries.
/// Score strings are displayed without parsing or comparing unrelated objectives.
#[derive(Debug)]
pub struct BenchmarkArchive {
    root: PathBuf,
    report_locale: Option<String>,
}
impl BenchmarkArchive {
    pub fn at_directory(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            report_locale: None,
        }
    }
    /// Parse native solver configuration through the existing XML validator while
    /// retaining the optional report locale. No Java class loading is performed.
    pub fn from_xml_archive_file(path: impl AsRef<Path>) -> Result<Self, BenchmarkArchiveError> {
        let text = fs::read_to_string(path.as_ref())?;
        if text.len() > crate::SolverXmlLimits::default().bytes {
            return Err(BenchmarkArchiveError::Configuration(
                "byte limit exceeded".into(),
            ));
        }
        let doc = roxmltree::Document::parse(&text)
            .map_err(|e| BenchmarkArchiveError::Configuration(e.to_string()))?;
        let root = doc.root_element();
        let reports = root
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "benchmarkReport")
            .collect::<Vec<_>>();
        if reports.len() > 1 {
            return Err(BenchmarkArchiveError::Configuration(
                "duplicate benchmarkReport".into(),
            ));
        }
        let mut filtered = text.clone();
        let mut locale = None;
        if let Some(report) = reports.first() {
            if report.tag_name().namespace() != root.tag_name().namespace()
                || report.attributes().len() != 0
            {
                return Err(BenchmarkArchiveError::Configuration(
                    "invalid benchmarkReport".into(),
                ));
            }
            for child in report.children().filter(roxmltree::Node::is_element) {
                if child.tag_name().name() != "locale"
                    || child.tag_name().namespace() != root.tag_name().namespace()
                    || child.children().any(|n| n.is_element())
                    || child.attributes().len() != 0
                    || locale.is_some()
                {
                    return Err(BenchmarkArchiveError::Configuration(
                        "unsupported report setting".into(),
                    ));
                }
                let value = child.text().unwrap_or("").trim();
                if value.is_empty()
                    || !value
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
                {
                    return Err(BenchmarkArchiveError::Configuration(
                        "invalid report locale".into(),
                    ));
                }
                locale = Some(value.to_owned());
            }
            filtered.replace_range(report.range(), "");
        }
        let config = crate::BenchmarkXmlConfiguration::parse(
            &filtered,
            &path.as_ref().display().to_string(),
        )
        .map_err(|e| BenchmarkArchiveError::Configuration(e.to_string()))?;
        Ok(Self {
            root: config.directory,
            report_locale: locale,
        })
    }
    pub fn archive_report_locale(&self) -> Option<&str> {
        self.report_locale.as_deref()
    }
    pub fn archive_completed_report(
        &self,
        run: &Path,
        timestamp: &str,
    ) -> Result<PathBuf, BenchmarkArchiveError> {
        if !archive_valid_timestamp(timestamp, false) {
            return Err(BenchmarkArchiveError::InvalidTimestamp);
        }
        let report =
            archive_read_native_report(run)?.ok_or(BenchmarkArchiveError::InvalidReport)?;
        let bytes =
            serde_json::to_vec_pretty(&report).map_err(|_| BenchmarkArchiveError::InvalidReport)?;
        fs::create_dir_all(&self.root)?;
        let output = archive_reserve_run_dir(&self.root, timestamp)?;
        if let Err(e) = fs::write(output.join("summary.pending"), bytes)
            .and_then(|_| fs::rename(output.join("summary.pending"), output.join("summary.json")))
        {
            // This newly reserved directory is owned by this failed publication.
            let _ = fs::remove_file(output.join("summary.pending"));
            let _ = fs::remove_dir(&output);
            return Err(e.into());
        }
        Ok(output)
    }
    pub fn available_archived_runs(&self) -> Result<Vec<String>, BenchmarkArchiveError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(vec![]);
            }
            Err(e) => return Err(e.into()),
        };
        let mut names = vec![];
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if entry.file_type()?.is_dir()
                && archive_valid_timestamp(name, true)
                && archive_read_native_report(&entry.path())?.is_some()
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }
    pub fn aggregate_archived_runs(&self) -> Result<PathBuf, BenchmarkArchiveError> {
        if !self.root.is_dir() {
            return Err(BenchmarkArchiveError::MissingDirectory);
        }
        let selected = self.available_archived_runs()?;
        if selected.is_empty() {
            return Err(BenchmarkArchiveError::NoResults);
        }
        self.aggregate_selected_archives(&selected)
    }
    pub fn aggregate_selected_archives(
        &self,
        selected: &[String],
    ) -> Result<PathBuf, BenchmarkArchiveError> {
        if !self.root.is_dir() {
            return Err(BenchmarkArchiveError::NoResults);
        }
        let mut reports = vec![];
        let mut seen = std::collections::BTreeSet::new();
        for name in selected {
            // Grammar rejects absolute paths, separators and traversal components.
            if !archive_valid_timestamp(name, true) || !seen.insert(name) {
                continue;
            }
            let path = self.root.join(name);
            if !fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_dir()) {
                continue;
            }
            if let Some(report) = archive_read_native_report(&path)? {
                reports.push((name, report));
            }
        }
        if reports.is_empty() {
            return Err(BenchmarkArchiveError::NoSelectedResults);
        }
        let mut merged = vec![];
        let mut successful = 0usize;
        let mut failed = 0usize;
        let mut warmups = 0usize;
        let mut uninitialized = 0usize;
        let mut evaluations = 0u64;
        let mut html = String::from(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Benchmark aggregation</title></head><body><h1>Benchmark aggregation</h1><table><caption>Measured trials; warm-ups excluded</caption><thead><tr><th scope=\"col\">Run</th><th scope=\"col\">Configuration</th><th scope=\"col\">Problem</th><th scope=\"col\">Repetition</th><th scope=\"col\">Score or error</th><th scope=\"col\">Initialized</th><th scope=\"col\">Evaluations</th></tr></thead><tbody>",
        );
        for (name, report) in &reports {
            for trial in report["trials"].as_array().expect("validated trials") {
                merged.push(serde_json::json!({"run":name,"trial":trial}));
                if trial["warmup"] == true {
                    warmups += 1;
                    continue;
                }
                let result = &trial["result"];
                if result["error"].is_string() {
                    failed += 1;
                } else {
                    successful += 1;
                    uninitialized += usize::from(result["initialized"] == false);
                    evaluations = evaluations
                        .checked_add(result["evaluations"].as_u64().expect("validated counter"))
                        .ok_or(BenchmarkArchiveError::EvaluationOverflow)?;
                }
                html.push_str("<tr>");
                for cell in [
                    (*name).to_owned(),
                    trial["configuration"].to_string(),
                    trial["problem"].to_string(),
                    trial["repetition"].to_string(),
                    result["error"]
                        .as_str()
                        .or(result["score"].as_str())
                        .unwrap_or("")
                        .to_owned(),
                    result["initialized"]
                        .as_bool()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "—".into()),
                    result["evaluations"]
                        .as_u64()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "—".into()),
                ] {
                    html.push_str("<td>");
                    html.push_str(&archive_escape_html(&cell));
                    html.push_str("</td>");
                }
                html.push_str("</tr>");
            }
        }
        if let Some(locale) = &self.report_locale {
            html = html.replacen(
                "lang=\"en\"",
                &format!(
                    "lang=\"{}\"",
                    archive_escape_html(&locale.replace('_', "-"))
                ),
                1,
            );
        }
        html.push_str("</tbody></table>");
        html.push_str(&format!("<p>{} runs; {successful} successful measured trials; {failed} failures; {uninitialized} uninitialized results; {warmups} warm-ups excluded.</p><p>Scores retain their native debug representation. Objective values from different models are not combined.</p><a href=\"aggregate.json\">Download aggregate data</a></body></html>",reports.len()));
        let combined = serde_json::json!({"schema_version":1,"runs":reports.iter().map(|(name,report)|serde_json::json!({"name":name,"report":report})).collect::<Vec<_>>(),"trials":merged,"summary":{"runs":reports.len(),"complete_runs":reports.iter().filter(|(_,r)|r["complete"]==true).count(),"successful":successful,"failed":failed,"warmups":warmups,"uninitialized":uninitialized,"evaluations":evaluations}});
        let output = archive_reserve_aggregation_dir(&self.root)?;
        let path = output.join("index.html");
        fs::write(
            output.join("aggregate.json"),
            serde_json::to_vec_pretty(&combined)
                .map_err(|_| BenchmarkArchiveError::InvalidReport)?,
        )?;
        fs::write(&path, html)?;
        Ok(path)
    }
    /// Report creation succeeds even when the caller's browser launcher fails.
    pub fn aggregate_and_open_archives(
        &self,
        open: impl FnOnce(&Path) -> io::Result<()>,
    ) -> Result<PathBuf, BenchmarkArchiveError> {
        let path = self.aggregate_archived_runs()?;
        let _ = open(&path);
        Ok(path)
    }
}
fn archive_reserve_run_dir(root: &Path, base: &str) -> io::Result<PathBuf> {
    for suffix in 0u64.. {
        let name = if suffix == 0 {
            base.to_owned()
        } else {
            format!("{base}_{suffix}")
        };
        let path = root.join(name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("archive names exhausted"))
}
fn archive_reserve_aggregation_dir(root: &Path) -> io::Result<PathBuf> {
    for suffix in 0u64.. {
        let name = if suffix == 0 {
            "aggregation".to_owned()
        } else {
            format!("{suffix}-aggregation")
        };
        let path = root.join(name);
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("aggregation names exhausted"))
}
fn archive_valid_timestamp(name: &str, suffix: bool) -> bool {
    let b = name.as_bytes();
    if b.len() < 17 || b[4] != b'-' || b[7] != b'-' || b[10] != b'_' {
        return false;
    }
    if !(0..17).all(|i| matches!(i, 4 | 7 | 10) || b[i].is_ascii_digit()) {
        return false;
    }
    if b.len() != 17
        && !(suffix && b.len() > 18 && b[17] == b'_' && b[18..].iter().all(u8::is_ascii_digit))
    {
        return false;
    }
    let number = |s: &[u8]| s.iter().fold(0u32, |n, d| n * 10 + u32::from(d - b'0'));
    let (year, month, day) = (number(&b[..4]), number(&b[5..7]), number(&b[8..10]));
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    year > 0
        && day > 0
        && day <= days
        && number(&b[11..13]) < 24
        && number(&b[13..15]) < 60
        && number(&b[15..17]) < 60
}
fn archive_read_native_report(path: &Path) -> Result<Option<Value>, BenchmarkArchiveError> {
    let file = path.join("summary.json");
    match fs::symlink_metadata(&file) {
        Ok(m) if !m.file_type().is_file() => return Ok(None),
        Ok(_) => {}
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    }
    let bytes = fs::read(file)?;
    let Ok(report) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(None);
    };
    let Some(trials) = report["trials"].as_array() else {
        return Ok(None);
    };
    if report["schema_version"] != 1
        || !report["complete"].is_boolean()
        || (trials.is_empty() && report["complete"] == true)
    {
        return Ok(None);
    }
    for trial in trials {
        let result = &trial["result"];
        let success = result["score"].is_string()
            && result["initialized"].is_boolean()
            && result["evaluations"].as_u64().is_some()
            && result.get("error").is_none();
        let failure = result["error"].is_string() && result.get("score").is_none();
        if !["configuration", "problem", "repetition"]
            .iter()
            .all(|k| trial[*k].as_u64().is_some())
            || !trial["warmup"].is_boolean()
            || !(success || failure)
        {
            return Ok(None);
        }
    }
    Ok(Some(report))
}
fn archive_escape_html(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            '\'' => "&#39;".to_owned(),
            c => c.to_string(),
        })
        .collect()
}
