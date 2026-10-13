//! Optional typed benchmark declarations and linked historical records.
//! These are native data/control adapters, not Java JAXB or report-renderer ABI.
mod config;
mod ranking;
mod results;
use crate::*;
pub use config::{
    BenchmarkRecordConfig, BenchmarkRecordFileIo, RecordProblemConfig, RecordSolverConfig,
    RecordStatistic, RecordedNearbySelection,
};
pub use ranking::{BenchmarkRecordReportPolicy, RecordRanking};
pub use results::{
    BenchmarkRecordSet, RecordCounter, RecordCounters, RecordProblem, RecordProblemStatistic,
    RecordScore, RecordSelection, RecordSingleRun, RecordSolver, RecordSubRun,
    RecordSystemMetadata,
};
use std::{error::Error, fmt};
#[derive(Debug)]
pub enum BenchmarkRecordError {
    Schema(String),
    XmlSyntax(roxmltree::Error),
    SolverXml(SolverXmlError),
    ConflictingRanking(Vec<&'static str>),
}
impl fmt::Display for BenchmarkRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(s) => f.write_str(s),
            Self::XmlSyntax(e) => e.fmt(f),
            Self::SolverXml(e) => e.fmt(f),
            Self::ConflictingRanking(fields) => write!(f, "conflicting {}", fields.join(" and ")),
        }
    }
}
impl Error for BenchmarkRecordError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::XmlSyntax(e) => Some(e),
            Self::SolverXml(e) => Some(e),
            _ => None,
        }
    }
}
type Result<T> = std::result::Result<T, BenchmarkRecordError>;
fn fail(message: impl Into<String>) -> BenchmarkRecordError {
    BenchmarkRecordError::Schema(message.into())
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn field(out: &mut String, name: &str, value: impl fmt::Display) {
    out.push_str(&format!("<{name}>{}</{name}>", escape(&value.to_string())));
}
fn optional<T: fmt::Display>(out: &mut String, name: &str, value: &Option<T>) {
    if let Some(v) = value {
        field(out, name, v);
    }
}
fn children<'a, 'i>(n: roxmltree::Node<'a, 'i>) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    n.children().filter(|n| n.is_element())
}
fn text<'a>(n: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    children(n)
        .find(|n| n.has_tag_name(name))
        .and_then(|n| n.text())
        .map(str::trim)
}
fn number<T: std::str::FromStr>(n: roxmltree::Node<'_, '_>, name: &str) -> Result<Option<T>> {
    text(n, name)
        .map(|s| {
            s.parse()
                .map_err(|_| fail(format!("{name}: invalid value {s}")))
        })
        .transpose()
}
fn check(n: roxmltree::Node<'_, '_>, allowed: &[&str]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for c in children(n) {
        let name = c.tag_name().name();
        if ![
            "solverBenchmark",
            "solverBenchmarkResult",
            "unifiedProblemBenchmarkResult",
            "singleBenchmarkResult",
            "subSingleBenchmarkResult",
            "problemStatisticType",
            "singleStatisticType",
            "inputSolutionFile",
        ]
        .contains(&name)
            && !seen.insert(name)
        {
            return Err(fail(format!("duplicate element {name}")));
        }
        if !allowed.contains(&c.tag_name().name()) {
            return Err(fail(format!(
                "{}: unknown element {}",
                n.tag_name().name(),
                c.tag_name().name()
            )));
        }
    }
    Ok(())
}
fn document(input: &str) -> Result<roxmltree::Document<'_>> {
    if input.len() > 4 * 1024 * 1024 {
        return Err(fail("XML byte limit"));
    }
    let d = roxmltree::Document::parse_with_options(
        input,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(BenchmarkRecordError::XmlSyntax)?;
    for n in d.descendants().filter(|n| n.is_element()) {
        if n.ancestors().count() > 128 {
            return Err(fail("XML depth limit"));
        }
        if n.tag_name()
            .namespace()
            .is_some_and(|s| s != BENCHMARK_XML_NAMESPACE && s != SOLVER_XML_NAMESPACE)
        {
            return Err(fail("foreign XML namespace"));
        }
    }
    Ok(d)
}
fn solver(n: roxmltree::Node<'_, '_>) -> Result<SolverXmlConfiguration> {
    // Existing validated solver XML parser consumes all nested phase/config data.
    let mut xml = n.document().input_text()[n.range()].to_owned();
    if n.has_tag_name("solverConfig") {
        xml = xml.replacen("<solverConfig>", "<solver>", 1);
        if let Some(i) = xml.rfind("</solverConfig>") {
            xml.replace_range(i.., "</solver>");
        }
    }
    let parsed = SolverXmlConfiguration::parse(&xml).map_err(BenchmarkRecordError::SolverXml)?;
    parsed.recorded_nearby_selections()?;
    Ok(parsed)
}
fn solver_xml(config: &SolverXmlConfiguration, name: &str) -> String {
    let mut xml = config.to_xml();
    if let Some(i) = xml.find("<solver")
        && let Some(start) = xml[i..].find('>')
    {
        let end = i + start;
        xml.replace_range(i..=end, &format!("<{name}>"));
    }
    if let Some(i) = xml.rfind("</solver>") {
        xml.replace_range(i..i + 9, &format!("</{name}>"));
    }
    xml
}
