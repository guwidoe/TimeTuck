//! Bounded XML configuration declarations and explicit native factory registrations.
use crate::{EnvironmentMode, SolverFactory, SolverFactoryError};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::Path,
};
pub const SOLVER_XML_NAMESPACE: &str = "https://timefold.ai/xsd/solver";
#[derive(Clone, Copy, Debug)]
pub struct SolverXmlLimits {
    pub bytes: usize,
    pub nodes: u32,
    pub depth: usize,
}
impl Default for SolverXmlLimits {
    fn default() -> Self {
        Self {
            bytes: 4 * 1024 * 1024,
            nodes: 100_000,
            depth: 128,
        }
    }
}
#[derive(Debug)]
pub struct SolverXmlSchemaError {
    pub node: String,
    pub reason: String,
}
impl fmt::Display for SolverXmlSchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node: {}; {}", self.node, self.reason)
    }
}
impl Error for SolverXmlSchemaError {}
#[derive(Debug)]
pub enum SolverXmlCause {
    Io(io::Error),
    Syntax(roxmltree::Error),
    Schema(SolverXmlSchemaError),
}
#[derive(Debug)]
pub struct SolverXmlError {
    pub origin: String,
    pub cause: SolverXmlCause,
}
impl fmt::Display for SolverXmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "solver XML {}: ", self.origin)?;
        match &self.cause {
            SolverXmlCause::Io(e) => e.fmt(f),
            SolverXmlCause::Syntax(e) => e.fmt(f),
            SolverXmlCause::Schema(e) => e.fmt(f),
        }
    }
}
impl Error for SolverXmlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match &self.cause {
            SolverXmlCause::Io(e) => e,
            SolverXmlCause::Syntax(e) => e,
            SolverXmlCause::Schema(e) => e,
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverXmlElement {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<SolverXmlElement>,
}
impl SolverXmlElement {
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|e| e.name == name)
    }
    pub fn value(&self, name: &str) -> Option<&str> {
        self.child(name).map(|e| e.text.trim())
    }
}
// Supported configuration vocabulary is declared independently from Community
// TEST documents. Parsing declarations is separate from executing their options.
const ROOT_FIELDS: &str = "environmentMode randomSeed moveThreadCount solutionClass entityClass scoreDirectorFactory nearbyDistanceMeterClass enablePreviewFeature termination constructionHeuristic localSearch customPhase exhaustiveSearch partitionedSearch ruinRecreate";
const NESTED_FIELDS: &str = "easyScoreCalculatorClass constraintProviderClass incrementalScoreCalculatorClass initializingScoreTrend constraintStreamImplType constraintStreamProfilingEnabled easyScoreCalculatorCustomProperties incrementalScoreCalculatorCustomProperties constraintProviderCustomProperties property constructionHeuristicType queuedEntityPlacer queuedValuePlacer pooledEntityPlacer entitySelector valueSelector entityClass downcastEntityClass cacheType selectionOrder filterClass sorterManner comparatorClass comparatorFactoryClass id mimicSelectorRef variableNameIncludes variableNameInclude cartesianProductMoveSelector changeMoveSelector swapMoveSelector unionMoveSelector pillarChangeMoveSelector pillarSwapMoveSelector subPillarType pillarSelector minimumSubPillarSize maximumSubPillarSize moveIteratorFactory moveIteratorFactoryClass moveIteratorFactoryCustomProperties moveListFactory moveListFactoryClass moveListFactoryCustomProperties fixedProbabilityWeight selectedCountLimit customPhaseCommandClass exhaustiveSearchType nodeExplorationType localSearchType termination terminationCompositionStyle spentLimit secondsSpentLimit minutesSpentLimit millisecondsSpentLimit unimprovedSpentLimit unimprovedSecondsSpentLimit unimprovedMillisecondsSpentLimit unimprovedMinutesSpentLimit unimprovedHoursSpentLimit unimprovedDaysSpentLimit hoursSpentLimit daysSpentLimit diminishedReturns slidingWindowDuration slidingWindowMilliseconds slidingWindowSeconds slidingWindowMinutes slidingWindowHours slidingWindowDays minimumImprovementRatio unimprovedStepCountLimit stepCountLimit bestScoreLimit acceptor acceptorType entityTabuSize entityTabuRatio valueTabuSize lateAcceptanceSize stepCountingHillClimbingSize stepCountingHillClimbingType simulatedAnnealingStartingTemperature forager pickEarlyType acceptedCountLimit finalistPodiumType breakTieRandomly solutionPartitionerClass solutionPartitionerCustomProperties constructionHeuristic localSearch environmentMode nearbySelection originEntitySelector secondaryEntitySelector nearbyDistanceMeterClass parabolicDistributionSizeMaximum";
fn schema(node: &str, reason: impl Into<String>) -> SolverXmlCause {
    SolverXmlCause::Schema(SolverXmlSchemaError {
        node: node.into(),
        reason: reason.into(),
    })
}
fn element(
    node: roxmltree::Node<'_, '_>,
    depth: usize,
    limits: SolverXmlLimits,
) -> Result<SolverXmlElement, SolverXmlCause> {
    let name = node.tag_name().name();
    if depth > limits.depth {
        return Err(schema(name, "configuration depth limit exceeded"));
    }
    if node
        .tag_name()
        .namespace()
        .is_some_and(|n| n != SOLVER_XML_NAMESPACE)
    {
        return Err(schema(name, "unsupported namespace"));
    }
    if depth == 0 && name != "solver" {
        return Err(schema(name, "expected solver root"));
    }
    if depth > 0 {
        let parent = node.parent_element().unwrap().tag_name().name();
        let known = if parent == "solver" {
            ROOT_FIELDS
        } else {
            NESTED_FIELDS
        };
        if !known.split_whitespace().any(|n| n == name)
            || (parent == "valueSelector" && name == "variableName")
        {
            return Err(schema(
                name,
                "element is not allowed in this configuration location",
            ));
        }
    }
    if (name.ends_with("SpentLimit") || matches!(name, "spentLimit" | "unimprovedSpentLimit"))
        && node.parent_element().unwrap().tag_name().name() != "termination"
    {
        return Err(schema(name, "spent setting belongs under termination"));
    }
    if name == "diminishedReturns"
        && node.parent_element().unwrap().tag_name().name() != "termination"
    {
        return Err(schema(name, "diminishedReturns belongs under termination"));
    }
    if (name.starts_with("slidingWindow") || name == "minimumImprovementRatio")
        && node.parent_element().unwrap().tag_name().name() != "diminishedReturns"
    {
        return Err(schema(
            name,
            "diminished-return setting belongs under diminishedReturns",
        ));
    }
    let mut attributes = Vec::new();
    for a in node.attributes() {
        // External schema hints are not execution declarations. The native
        // schema validator never dereferences them; the executable tree omits
        // them while source-backed serialization may preserve the raw hint.
        if a.namespace() == Some("http://www.w3.org/2001/XMLSchema-instance")
            && matches!(a.name(), "schemaLocation" | "noNamespaceSchemaLocation")
        {
            continue;
        }
        if a.namespace().is_some()
            || !matches!(
                a.name(),
                "id" | "mimicSelectorRef" | "variableName" | "name" | "value"
            )
        {
            return Err(schema(name, format!("unknown attribute {}", a.name())));
        }
        if a.name() == "variableName" && name != "valueSelector" {
            return Err(schema(name, "variableName is a valueSelector attribute"));
        }
        attributes.push((a.name().into(), a.value().into()));
    }
    let children = node
        .children()
        .filter(|n| n.is_element())
        .map(|n| element(n, depth + 1, limits))
        .collect::<Result<Vec<_>, _>>()?;
    let text = node
        .children()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect::<String>();
    if !children.is_empty() && !text.trim().is_empty() {
        return Err(schema(
            name,
            "mixed text and configuration elements are not supported",
        ));
    }
    if matches!(
        name,
        "lateAcceptanceSize" | "parabolicDistributionSizeMaximum"
    ) {
        let parent = node.parent_element().unwrap().tag_name().name();
        let expected = if name == "lateAcceptanceSize" {
            "acceptor"
        } else {
            "nearbySelection"
        };
        if parent != expected || text.trim().parse::<usize>().ok().is_none_or(|n| n == 0) {
            return Err(schema(
                name,
                format!("requires positive size under {expected}"),
            ));
        }
    }
    if name == "nearbySelection" {
        let meter = children
            .iter()
            .find(|c| c.name == "nearbyDistanceMeterClass")
            .is_some_and(|c| !c.text.trim().is_empty());
        let origin = children
            .iter()
            .find(|c| c.name == "originEntitySelector")
            .is_some_and(|c| {
                c.attributes
                    .iter()
                    .any(|(k, v)| k == "mimicSelectorRef" && !v.trim().is_empty())
            });
        if !meter || !origin {
            return Err(schema(
                name,
                "requires nearbyDistanceMeterClass and originEntitySelector mimicSelectorRef",
            ));
        }
    }
    if name.ends_with("Class") && text.trim().is_empty() {
        return Err(schema(name, "component registration key is empty"));
    }
    Ok(SolverXmlElement {
        name: name.into(),
        attributes,
        text,
        children,
    })
}
fn omit_root_namespace(text: &str, start: usize) -> String {
    let bytes = text.as_bytes();
    let mut i = start + 1;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
        i += 1;
    }
    loop {
        let whitespace = i;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || matches!(bytes[i], b'>' | b'/') {
            return text.into();
        }
        let name_start = i;
        while !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' {
            i += 1;
        }
        let name = &text[name_start..i];
        while bytes[i] != b'=' {
            i += 1;
        }
        i += 1;
        while bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = bytes[i];
        i += 1;
        while bytes[i] != quote {
            i += 1;
        }
        i += 1;
        if name == "xmlns" {
            let mut out = text[..whitespace].to_owned();
            out.push_str(&text[i..]);
            return out;
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverXmlConfiguration {
    root: SolverXmlElement,
    source: Option<String>,
}
impl Default for SolverXmlConfiguration {
    fn default() -> Self {
        Self {
            root: SolverXmlElement {
                name: "solver".into(),
                attributes: vec![],
                text: String::new(),
                children: vec![],
            },
            source: None,
        }
    }
}
impl SolverXmlConfiguration {
    pub fn parse(text: &str) -> Result<Self, SolverXmlError> {
        Self::parse_with_limits(text, "<string>", SolverXmlLimits::default())
    }
    pub fn parse_with_limits(
        text: &str,
        origin: &str,
        limits: SolverXmlLimits,
    ) -> Result<Self, SolverXmlError> {
        let parse = || {
            if text.len() > limits.bytes {
                return Err(schema("solver", "configuration byte limit exceeded"));
            }
            let doc = roxmltree::Document::parse_with_options(
                text,
                roxmltree::ParsingOptions {
                    allow_dtd: false,
                    nodes_limit: limits.nodes,
                    ..Default::default()
                },
            )
            .map_err(SolverXmlCause::Syntax)?;
            let root = element(doc.root_element(), 0, limits)?;
            let config = Self {
                root,
                source: Some(
                    if doc.root_element().lookup_namespace_uri(None) == Some(SOLVER_XML_NAMESPACE) {
                        omit_root_namespace(text, doc.root_element().range().start)
                    } else {
                        text.into()
                    },
                ),
            };
            config
                .environment_mode()
                .map_err(|reason| schema("environmentMode", reason))?;
            Ok(config)
        };
        parse().map_err(|cause| SolverXmlError {
            origin: origin.into(),
            cause,
        })
    }
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, SolverXmlError> {
        let path = path.as_ref();
        let origin = path.display().to_string();
        let limits = SolverXmlLimits::default();
        let read = || {
            let mut text = String::new();
            File::open(path)?
                .take(limits.bytes as u64 + 1)
                .read_to_string(&mut text)?;
            Ok::<_, io::Error>(text)
        };
        let text = read().map_err(|e| SolverXmlError {
            origin: origin.clone(),
            cause: SolverXmlCause::Io(e),
        })?;
        Self::parse_with_limits(&text, &origin, limits)
    }
    pub fn from_resource(
        name: &str,
        mut resolve: impl FnMut(&str) -> io::Result<String>,
    ) -> Result<Self, SolverXmlError> {
        let text = resolve(name).map_err(|e| SolverXmlError {
            origin: name.into(),
            cause: SolverXmlCause::Io(e),
        })?;
        Self::parse_with_limits(&text, name, SolverXmlLimits::default())
    }
    pub fn root(&self) -> &SolverXmlElement {
        &self.root
    }
    pub fn solution_key(&self) -> Option<&str> {
        self.root.value("solutionClass")
    }
    pub fn entity_keys(&self) -> Vec<&str> {
        self.root
            .children
            .iter()
            .filter(|e| e.name == "entityClass")
            .map(|e| e.text.trim())
            .collect()
    }
    pub fn phase_names(&self) -> Vec<&str> {
        self.root
            .children
            .iter()
            .map(|e| e.name.as_str())
            .filter(|n| {
                matches!(
                    *n,
                    "constructionHeuristic"
                        | "localSearch"
                        | "customPhase"
                        | "exhaustiveSearch"
                        | "partitionedSearch"
                )
            })
            .collect()
    }
    pub fn environment_mode(&self) -> Result<Option<EnvironmentMode>, String> {
        self.root
            .value("environmentMode")
            .map(|n| match n {
                "NON_REPRODUCIBLE" => Ok(EnvironmentMode::NonReproducible),
                "NO_ASSERT" => Ok(EnvironmentMode::NoAssert),
                "PHASE_ASSERT" => Ok(EnvironmentMode::PhaseAssert),
                "STEP_ASSERT" => Ok(EnvironmentMode::StepAssert),
                "NON_INTRUSIVE_FULL_ASSERT" => Ok(EnvironmentMode::NonIntrusiveFullAssert),
                "FULL_ASSERT" => Ok(EnvironmentMode::FullAssert),
                "TRACKED_FULL_ASSERT" => Ok(EnvironmentMode::TrackedFullAssert),
                _ => Err(format!("unknown environment mode {n}")),
            })
            .transpose()
    }
    pub fn preview_features(&self) -> Option<Vec<&str>> {
        let v = self
            .root
            .children
            .iter()
            .filter(|e| e.name == "enablePreviewFeature")
            .map(|e| e.text.trim())
            .collect::<BTreeSet<_>>();
        (!v.is_empty()).then(|| v.into_iter().collect())
    }
    pub fn with_preview_feature(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        if !self
            .preview_features()
            .is_some_and(|v| v.contains(&key.as_str()))
        {
            self.root.children.push(SolverXmlElement {
                name: "enablePreviewFeature".into(),
                attributes: vec![],
                text: key,
                children: vec![],
            });
            self.source = None;
        }
        self
    }
    /// Child declarations win for existing fields. Missing fields copy from the
    /// parent; an empty child inherits the whole owned document, without aliases.
    pub fn inherit(mut self, parent: &Self) -> Self {
        if self.root.children.is_empty() {
            return parent.clone();
        }
        let names = self
            .root
            .children
            .iter()
            .map(|n| n.name.clone())
            .collect::<BTreeSet<_>>();
        self.root.children.extend(
            parent
                .root
                .children
                .iter()
                .filter(|e| !names.contains(&e.name))
                .cloned(),
        );
        self.source = None;
        self
    }
    pub fn referenced_components(&self) -> Vec<&str> {
        fn visit<'a>(e: &'a SolverXmlElement, out: &mut Vec<&'a str>) {
            if e.name.ends_with("Class") && !out.contains(&e.text.trim()) {
                out.push(e.text.trim());
            }
            for c in &e.children {
                visit(c, out);
            }
        }
        let mut out = vec![];
        visit(&self.root, &mut out);
        out
    }
    /// Reading/writing unchanged declarations preserves source spelling/comments;
    /// the default solver namespace is omitted. Mutated declarations serialize
    /// their owned tree; no external resources or Java reflection are consulted.
    pub fn to_xml(&self) -> String {
        if let Some(source) = &self.source {
            return source.clone();
        }
        fn escape(v: &str) -> String {
            v.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        }
        fn write(e: &SolverXmlElement, out: &mut String) {
            out.push('<');
            out.push_str(&e.name);
            for (k, v) in &e.attributes {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape(v));
                out.push('"');
            }
            out.push('>');
            out.push_str(&escape(&e.text));
            for c in &e.children {
                write(c, out);
            }
            out.push_str("</");
            out.push_str(&e.name);
            out.push('>');
        }
        let mut out = String::new();
        write(&self.root, &mut out);
        out
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct XmlFactorySignature {
    pub solution: String,
    pub entities: Vec<String>,
    pub scoring: String,
}
impl XmlFactorySignature {
    pub fn from_configuration(c: &SolverXmlConfiguration) -> Result<Self, XmlFactoryError> {
        let solution = c
            .solution_key()
            .ok_or_else(|| XmlFactoryError::Signature("solutionClass is required".into()))?
            .into();
        let entities = c
            .entity_keys()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if entities.is_empty() {
            return Err(XmlFactoryError::Signature("entityClass is required".into()));
        }
        let score = c
            .root
            .child("scoreDirectorFactory")
            .ok_or_else(|| XmlFactoryError::Signature("scoreDirectorFactory is required".into()))?;
        let keys = score
            .children
            .iter()
            .filter(|n| {
                matches!(
                    n.name.as_str(),
                    "easyScoreCalculatorClass"
                        | "constraintProviderClass"
                        | "incrementalScoreCalculatorClass"
                )
            })
            .collect::<Vec<_>>();
        if keys.len() != 1 {
            return Err(XmlFactoryError::Signature(
                "exactly one scoring component is required for native factory binding".into(),
            ));
        }
        Ok(Self {
            solution,
            entities,
            scoring: keys[0].text.trim().into(),
        })
    }
}
#[derive(Debug)]
pub enum XmlFactoryError {
    Xml(SolverXmlError),
    Signature(String),
    Unregistered(XmlFactorySignature),
    Duplicate(XmlFactorySignature),
    Build(SolverFactoryError),
}
impl fmt::Display for XmlFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(e) => e.fmt(f),
            Self::Signature(e) => f.write_str(e),
            Self::Unregistered(s) => write!(f, "unregistered native XML factory {s:?}"),
            Self::Duplicate(s) => write!(f, "duplicate native XML factory {s:?}"),
            Self::Build(e) => e.fmt(f),
        }
    }
}
impl Error for XmlFactoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Xml(e) => Some(e),
            Self::Build(e) => Some(e),
            _ => None,
        }
    }
}
type XmlFactoryBuilder<P, R, E, S> =
    dyn Fn(&SolverXmlConfiguration) -> Result<SolverFactory<P, R, E, S>, SolverFactoryError>;
/// Explicit typed callbacks resolve a complete solution/entity/scoring signature.
/// A key alone never stands in for an actual solver. Callbacks build existing
/// native SolverFactory objects and must interpret/reject runtime options.
/// Parsing arbitrary declarations does not certify runtime support for them.
pub struct XmlSolverFactoryRegistry<P, R, E, S: 'static> {
    builders: BTreeMap<XmlFactorySignature, Box<XmlFactoryBuilder<P, R, E, S>>>,
}
impl<P, R, E, S: 'static> Default for XmlSolverFactoryRegistry<P, R, E, S> {
    fn default() -> Self {
        Self {
            builders: BTreeMap::new(),
        }
    }
}
impl<P, R, E, S: 'static> XmlSolverFactoryRegistry<P, R, E, S> {
    pub fn register(
        &mut self,
        key: XmlFactorySignature,
        build: impl Fn(&SolverXmlConfiguration) -> Result<SolverFactory<P, R, E, S>, SolverFactoryError>
        + 'static,
    ) -> Result<(), XmlFactoryError> {
        if self.builders.contains_key(&key) {
            return Err(XmlFactoryError::Duplicate(key));
        }
        self.builders.insert(key, Box::new(build));
        Ok(())
    }
    pub fn prepare(
        &self,
        c: &SolverXmlConfiguration,
    ) -> Result<SolverFactory<P, R, E, S>, XmlFactoryError> {
        let key = XmlFactorySignature::from_configuration(c)?;
        self.builders
            .get(&key)
            .ok_or(XmlFactoryError::Unregistered(key))?(c)
        .map_err(XmlFactoryError::Build)
    }
    pub fn from_file(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<SolverFactory<P, R, E, S>, XmlFactoryError> {
        self.prepare(&SolverXmlConfiguration::from_file(path).map_err(XmlFactoryError::Xml)?)
    }
    pub fn from_resource(
        &self,
        name: &str,
        resolve: impl FnMut(&str) -> io::Result<String>,
    ) -> Result<SolverFactory<P, R, E, S>, XmlFactoryError> {
        self.prepare(
            &SolverXmlConfiguration::from_resource(name, resolve).map_err(XmlFactoryError::Xml)?,
        )
    }
}
