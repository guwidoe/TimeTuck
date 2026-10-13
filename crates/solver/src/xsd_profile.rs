//! Independent, explicit XSD profile compiler/merger and bounded validator.
//! Unsupported declarations fail compilation; no external resource loading.
use roxmltree::{Document, Node};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
const XS: &str = "http://www.w3.org/2001/XMLSchema";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XsdProfileError(pub String);
impl fmt::Display for XsdProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for XsdProfileError {}
type R<T> = Result<T, XsdProfileError>;
fn err(s: impl Into<String>) -> XsdProfileError {
    XsdProfileError(s.into())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XsdMergePolicy {
    RejectConflicts,
    OverlayTypes,
}
#[derive(Clone, Copy, Debug)]
pub struct XsdValidationLimits {
    pub max_bytes: usize,
    pub max_nodes: usize,
    pub max_steps: usize,
    pub max_depth: usize,
}
impl Default for XsdValidationLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_nodes: 100_000,
            max_steps: 1_000_000,
            max_depth: 128,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Name {
    builtin: bool,
    local: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Attribute {
    name: String,
    kind: Name,
    required: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Kind {
    Simple {
        base: Name,
        values: Vec<String>,
    },
    Complex {
        base: Option<Name>,
        particle: Option<Particle>,
        attributes: Vec<Attribute>,
        abstract_type: bool,
        final_extension: bool,
        final_restriction: bool,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Particle {
    min: usize,
    max: Option<usize>,
    kind: ParticleKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum ParticleKind {
    Element {
        name: String,
        kind: Name,
    },
    InlineComplex {
        name: String,
        particle: Option<Box<Particle>>,
        attributes: Vec<Attribute>,
    },
    Sequence(Vec<Particle>),
    Choice(Vec<Particle>),
}
#[derive(Clone, Debug)]
pub struct XsdProfile {
    namespace: String,
    types: BTreeMap<String, Kind>,
    roots: BTreeMap<String, Name>,
}
fn children<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(Node::is_element)
}
fn check(n: Node<'_, '_>, allowed: &[&str]) -> R<()> {
    if n.tag_name().namespace() != Some(XS) {
        return Err(err("Foreign XSD declaration namespace"));
    }
    for a in n.attributes() {
        if a.namespace().is_some() || !allowed.contains(&a.name()) {
            return Err(err(format!(
                "Unsupported XSD attribute {} on {}",
                a.name(),
                n.tag_name().name()
            )));
        }
    }
    Ok(())
}
fn required(n: Node<'_, '_>, key: &str) -> R<String> {
    n.attribute(key)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| err(format!("Missing XSD {key}")))
}
fn name(n: Node<'_, '_>, key: &str, target: &str) -> R<Name> {
    let raw = required(n, key)?;
    let (prefix, local) = raw
        .split_once(':')
        .map_or((None, raw.as_str()), |(p, l)| (Some(p), l));
    let namespace = n.lookup_namespace_uri(prefix).unwrap_or("");
    if namespace != XS && namespace != target {
        return Err(err(format!("Unresolved namespace/type reference {raw}")));
    }
    Ok(Name {
        builtin: namespace == XS,
        local: local.into(),
    })
}
fn flag(n: Node<'_, '_>, key: &str) -> R<bool> {
    match n.attribute(key) {
        None | Some("false") | Some("0") => Ok(false),
        Some("true") | Some("1") => Ok(true),
        _ => Err(err(format!("Invalid XSD boolean {key}"))),
    }
}
fn particle(n: Node<'_, '_>, target: &str) -> R<Particle> {
    let element = n.tag_name().name() == "element";
    check(
        n,
        if element {
            &["name", "type", "minOccurs", "maxOccurs"]
        } else {
            &["minOccurs", "maxOccurs"]
        },
    )?;
    let min = n
        .attribute("minOccurs")
        .unwrap_or("1")
        .parse::<usize>()
        .map_err(|_| err("Invalid minOccurs"))?;
    let max = match n.attribute("maxOccurs") {
        Some("unbounded") => None,
        v => Some(
            v.unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| err("Invalid maxOccurs"))?,
        ),
    };
    if max.is_some_and(|m| m < min) {
        return Err(err("maxOccurs smaller than minOccurs"));
    }
    let kind = match n.tag_name().name() {
        "element" => {
            let nested = children(n).collect::<Vec<_>>();
            if nested.is_empty() {
                ParticleKind::Element {
                    name: required_name(n)?,
                    kind: name(n, "type", target)?,
                }
            } else {
                if n.attribute("type").is_some()
                    || nested.len() != 1
                    || nested[0].tag_name().name() != "complexType"
                {
                    return Err(err("Unsupported inline XSD element declaration"));
                }
                check(nested[0], &[])?;
                let (particle, attributes) = body(nested[0], target)?;
                ParticleKind::InlineComplex {
                    name: required_name(n)?,
                    particle: particle.map(Box::new),
                    attributes,
                }
            }
        }
        "sequence" => {
            ParticleKind::Sequence(children(n).map(|c| particle(c, target)).collect::<R<_>>()?)
        }
        "choice" => {
            ParticleKind::Choice(children(n).map(|c| particle(c, target)).collect::<R<_>>()?)
        }
        v => return Err(err(format!("Unsupported XSD particle {v}"))),
    };
    Ok(Particle { min, max, kind })
}
fn body(n: Node<'_, '_>, target: &str) -> R<(Option<Particle>, Vec<Attribute>)> {
    let mut p = None;
    let mut attributes = vec![];
    for c in children(n) {
        match c.tag_name().name() {
            "sequence" | "choice" => {
                if p.is_some() {
                    return Err(err("Multiple XSD content particles"));
                }
                p = Some(particle(c, target)?);
            }
            "attribute" => {
                check(c, &["name", "type", "use"])?;
                if children(c).next().is_some() {
                    return Err(err("Unsupported attribute child"));
                }
                let required = match c.attribute("use") {
                    None | Some("optional") => false,
                    Some("required") => true,
                    _ => return Err(err("Unsupported attribute use")),
                };
                attributes.push(Attribute {
                    name: required_name(c)?,
                    kind: name(c, "type", target)?,
                    required,
                });
            }
            v => return Err(err(format!("Unsupported XSD content {v}"))),
        }
    }
    Ok((p, attributes))
}
fn required_name(n: Node<'_, '_>) -> R<String> {
    let name = required(n, "name")?;
    if name.contains(':') {
        return Err(err("Declaration name must be NCName"));
    }
    let probe = format!("<{name}/>");
    let doc = Document::parse(&probe).map_err(|_| err("Invalid declaration NCName"))?;
    if doc.root_element().tag_name().name() != name || doc.root_element().attributes().len() != 0 {
        return Err(err("Invalid declaration NCName"));
    }
    Ok(name)
}
impl XsdProfile {
    pub fn compile(input: &str) -> R<Self> {
        if input.len() > 2 * 1024 * 1024 {
            return Err(err("Schema byte budget exceeded"));
        }
        let doc = Document::parse(input).map_err(|e| err(e.to_string()))?;
        if doc
            .descendants()
            .any(|n| n.is_text() && n.text().is_some_and(|v| !v.trim().is_empty()))
        {
            return Err(err("Unsupported XSD character content"));
        }
        if doc.descendants().count() > 20_000 {
            return Err(err("Schema node budget exceeded"));
        }
        if doc
            .descendants()
            .filter(Node::is_element)
            .any(|node| node.ancestors().take(129).count() > 128)
        {
            return Err(err("Schema declaration depth budget exceeded"));
        }
        let root = doc.root_element();
        check(root, &["targetNamespace", "elementFormDefault", "version"])?;
        if root.tag_name().name() != "schema"
            || root.attribute("elementFormDefault") != Some("qualified")
        {
            return Err(err("Profile requires qualified schema elements"));
        }
        let namespace = required(root, "targetNamespace")?;
        let mut profile = Self {
            namespace: namespace.clone(),
            types: BTreeMap::new(),
            roots: BTreeMap::new(),
        };
        for n in children(root) {
            match n.tag_name().name() {
                "element" => {
                    check(n, &["name", "type"])?;
                    if children(n).next().is_some() {
                        return Err(err("Unsupported root element content"));
                    }
                    let key = required_name(n)?;
                    if profile
                        .roots
                        .insert(key, name(n, "type", &namespace)?)
                        .is_some()
                    {
                        return Err(err("Duplicate root declaration"));
                    }
                }
                "simpleType" => {
                    check(n, &["name"])?;
                    let restrictions = children(n).collect::<Vec<_>>();
                    if restrictions.len() != 1 || restrictions[0].tag_name().name() != "restriction"
                    {
                        return Err(err("Unsupported simpleType content"));
                    }
                    let r = restrictions[0];
                    check(r, &["base"])?;
                    let mut values = vec![];
                    for v in children(r) {
                        if v.tag_name().name() != "enumeration" {
                            return Err(err(format!(
                                "Unsupported XSD facet {}",
                                v.tag_name().name()
                            )));
                        }
                        check(v, &["value"])?;
                        if children(v).next().is_some() {
                            return Err(err("Unsupported enumeration content"));
                        }
                        values.push(required(v, "value")?);
                    }
                    profile.insert(
                        required_name(n)?,
                        Kind::Simple {
                            base: name(r, "base", &namespace)?,
                            values,
                        },
                    )?;
                }
                "complexType" => {
                    check(n, &["name", "abstract", "final"])?;
                    let final_tokens = n
                        .attribute("final")
                        .unwrap_or("")
                        .split_whitespace()
                        .collect::<Vec<_>>();
                    if final_tokens
                        .iter()
                        .any(|v| !matches!(*v, "extension" | "restriction" | "#all"))
                    {
                        return Err(err("Unsupported final derivation token"));
                    }
                    let content = children(n).collect::<Vec<_>>();
                    let (base, p, attributes) = if content
                        .first()
                        .is_some_and(|c| c.tag_name().name() == "complexContent")
                    {
                        if content.len() != 1 {
                            return Err(err("Invalid complexContent siblings"));
                        }
                        let c = content[0];
                        check(c, &[])?;
                        let e = children(c).collect::<Vec<_>>();
                        if e.len() != 1 || e[0].tag_name().name() != "extension" {
                            return Err(err("Unsupported complexContent derivation"));
                        }
                        check(e[0], &["base"])?;
                        let (p, a) = body(e[0], &namespace)?;
                        (Some(name(e[0], "base", &namespace)?), p, a)
                    } else {
                        let (p, a) = body(n, &namespace)?;
                        (None, p, a)
                    };
                    profile.insert(
                        required_name(n)?,
                        Kind::Complex {
                            base,
                            particle: p,
                            attributes,
                            abstract_type: flag(n, "abstract")?,
                            final_extension: final_tokens.contains(&"extension")
                                || final_tokens.contains(&"#all"),
                            final_restriction: final_tokens.contains(&"restriction")
                                || final_tokens.contains(&"#all"),
                        },
                    )?;
                }
                v => return Err(err(format!("Unsupported XSD top-level declaration {v}"))),
            }
        }
        profile.resolve()?;
        Ok(profile)
    }
    fn insert(&mut self, key: String, kind: Kind) -> R<()> {
        if self.types.insert(key, kind).is_some() {
            return Err(err("Duplicate type declaration"));
        }
        Ok(())
    }
    /// Retargets overlay declarations/references to primary namespace explicitly.
    pub fn merge(primary: &str, overlays: &[&str], policy: XsdMergePolicy) -> R<Self> {
        if overlays.len() > 32 {
            return Err(err("Schema overlay count budget exceeded"));
        }
        let mut result = Self::compile(primary)?;
        for input in overlays {
            let overlay = Self::compile(input)?;
            for (k, v) in overlay.types {
                if result.types.get(&k).is_some_and(|existing| existing != &v)
                    && policy == XsdMergePolicy::RejectConflicts
                {
                    return Err(err(format!("Conflicting schema type {k}")));
                }
                result.types.insert(k, v);
            }
            for (k, v) in overlay.roots {
                if result.roots.get(&k).is_some_and(|existing| existing != &v) {
                    return Err(err(format!("Conflicting root element {k}")));
                }
                result.roots.insert(k, v);
            }
        }
        result.resolve()?;
        Ok(result)
    }
    fn reference(&self, n: &Name) -> R<()> {
        if n.builtin {
            if !matches!(
                n.local.as_str(),
                "string" | "boolean" | "int" | "long" | "double"
            ) {
                return Err(err(format!("Unsupported XSD builtin {}", n.local)));
            }
        } else if !self.types.contains_key(&n.local) {
            return Err(err(format!("Unresolved type reference {}", n.local)));
        }
        Ok(())
    }
    fn resolve(&self) -> R<()> {
        for n in self.roots.values() {
            self.reference(n)?;
        }
        for (key, kind) in &self.types {
            match kind {
                Kind::Simple { base, .. } => {
                    self.reference(base)?;
                    if base.builtin && base.local != "string" {
                        return Err(err(
                            "Unsupported restriction base: profile restricts strings only",
                        ));
                    }
                    if !base.builtin
                        && !matches!(self.types.get(&base.local), Some(Kind::Simple { .. }))
                    {
                        return Err(err("Simple restriction requires simple base"));
                    }
                }
                Kind::Complex {
                    base,
                    particle,
                    attributes,
                    ..
                } => {
                    if let Some(base) = base {
                        self.reference(base)?;
                        if !matches!(
                            self.types.get(&base.local),
                            Some(Kind::Complex {
                                final_extension: false,
                                ..
                            })
                        ) {
                            return Err(err("Complex extension requires non-final complex base"));
                        }
                    }
                    if let Some(p) = particle {
                        self.resolve_particle(p)?;
                    }
                    for a in attributes {
                        self.reference(&a.kind)?;
                        if !a.kind.builtin
                            && !matches!(self.types.get(&a.kind.local), Some(Kind::Simple { .. }))
                        {
                            return Err(err("Attribute requires simple type"));
                        }
                    }
                }
            }
            let mut path = BTreeSet::new();
            let mut at = Some(Name {
                builtin: false,
                local: key.clone(),
            });
            while let Some(n) = at {
                if n.builtin {
                    break;
                }
                if !path.insert(n.local.clone()) {
                    return Err(err("Cyclic schema type derivation"));
                }
                if path.len() > 128 {
                    return Err(err("Schema inheritance depth budget exceeded"));
                }
                at = match self.types.get(&n.local) {
                    Some(Kind::Simple { base, .. }) => Some(base.clone()),
                    Some(Kind::Complex { base, .. }) => base.clone(),
                    None => None,
                };
            }
        }
        Ok(())
    }
    fn resolve_particle(&self, p: &Particle) -> R<()> {
        match &p.kind {
            ParticleKind::Element { kind, .. } => self.reference(kind),
            ParticleKind::InlineComplex {
                particle,
                attributes,
                ..
            } => {
                if let Some(p) = particle {
                    self.resolve_particle(p)?;
                }
                for a in attributes {
                    self.reference(&a.kind)?;
                    if !a.kind.builtin
                        && !matches!(self.types.get(&a.kind.local), Some(Kind::Simple { .. }))
                    {
                        return Err(err("Attribute requires simple type"));
                    }
                }
                Ok(())
            }
            ParticleKind::Sequence(v) | ParticleKind::Choice(v) => {
                for p in v {
                    self.resolve_particle(p)?;
                }
                Ok(())
            }
        }
    }
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn read_benchmark_configuration(
        &self,
        input: &str,
        limits: XsdValidationLimits,
    ) -> R<crate::BenchmarkRecordConfig> {
        self.validate(input, limits)?;
        crate::BenchmarkRecordConfig::parse(input).map_err(|error| err(error.to_string()))
    }
    pub fn validate(&self, input: &str, limits: XsdValidationLimits) -> R<()> {
        if input.len() > limits.max_bytes {
            return Err(err("XML byte budget exceeded"));
        }
        let doc = Document::parse(input).map_err(|e| err(e.to_string()))?;
        if doc.descendants().count() > limits.max_nodes {
            return Err(err("XML node budget exceeded"));
        }
        let root = doc.root_element();
        if root.tag_name().namespace() != Some(self.namespace.as_str()) {
            return Err(err("Wrong root namespace"));
        }
        let kind = self
            .roots
            .get(root.tag_name().name())
            .ok_or_else(|| err("Unknown root element"))?;
        let mut budget = limits.max_steps;
        self.validate_node(root, kind, 0, limits, &mut budget)
    }
    fn validate_node(
        &self,
        node: Node<'_, '_>,
        kind: &Name,
        depth: usize,
        limits: XsdValidationLimits,
        budget: &mut usize,
    ) -> R<()> {
        step(budget)?;
        if depth > limits.max_depth {
            return Err(err("XML depth budget exceeded"));
        }
        let descendants = children(node).collect::<Vec<_>>();
        if kind.builtin || matches!(self.types.get(&kind.local), Some(Kind::Simple { .. })) {
            if !descendants.is_empty()
                || node.attributes().any(|a| {
                    a.namespace() != Some("http://www.w3.org/2001/XMLSchema-instance")
                        || a.name() != "schemaLocation"
                })
            {
                return Err(err("Simple value has children/unsupported attributes"));
            }
            let text = node
                .children()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect::<String>();
            return self.simple(kind, &text);
        }
        let mut attributes = Vec::new();
        let mut particles = Vec::new();
        self.complex_chain(kind, &mut attributes, &mut particles)?;
        if matches!(
            self.types.get(&kind.local),
            Some(Kind::Complex {
                abstract_type: true,
                ..
            })
        ) {
            return Err(err("Cannot instantiate abstract type"));
        }
        for t in node.children().filter(Node::is_text) {
            if t.text().is_some_and(|s| !s.trim().is_empty()) {
                return Err(err("Complex element contains non-whitespace text"));
            }
        }
        for a in node.attributes() {
            if a.namespace() == Some("http://www.w3.org/2001/XMLSchema-instance")
                && a.name() == "schemaLocation"
            {
                continue;
            }
            let spec = attributes
                .iter()
                .find(|s| a.namespace().is_none() && s.name == a.name())
                .ok_or_else(|| {
                    err(format!(
                        "Unknown/unsupported instance attribute {}",
                        a.name()
                    ))
                })?;
            self.simple(&spec.kind, a.value())?;
        }
        for a in &attributes {
            if a.required && node.attribute(a.name.as_str()).is_none() {
                return Err(err("Missing required attribute"));
            }
        }
        let mut positions = BTreeSet::from([0]);
        for p in particles {
            let mut next = BTreeSet::new();
            for pos in positions {
                next.extend(self.match_particle(
                    p,
                    &descendants,
                    pos,
                    depth + 1,
                    limits,
                    budget,
                )?);
            }
            positions = next;
        }
        if !positions.contains(&descendants.len()) {
            return Err(err(format!(
                "Invalid sequence/cardinality/type within {}",
                node.tag_name().name()
            )));
        }
        Ok(())
    }
    fn complex_chain<'a>(
        &'a self,
        n: &Name,
        attributes: &mut Vec<&'a Attribute>,
        particles: &mut Vec<&'a Particle>,
    ) -> R<()> {
        let Some(Kind::Complex {
            base,
            particle,
            attributes: own,
            ..
        }) = self.types.get(&n.local)
        else {
            return Err(err("Expected complex type"));
        };
        if let Some(base) = base {
            self.complex_chain(base, attributes, particles)?;
        }
        attributes.extend(own);
        if let Some(p) = particle {
            particles.push(p);
        }
        Ok(())
    }
    fn match_particle(
        &self,
        p: &Particle,
        nodes: &[Node<'_, '_>],
        start: usize,
        depth: usize,
        limits: XsdValidationLimits,
        budget: &mut usize,
    ) -> R<BTreeSet<usize>> {
        step(budget)?;
        let max = p
            .max
            .unwrap_or(nodes.len().saturating_add(1))
            .min(nodes.len().saturating_add(p.min).saturating_add(1));
        if p.min > limits.max_nodes {
            return Err(err("Occurrence budget exceeded"));
        }
        let mut current = BTreeSet::from([start]);
        let mut accepted = if p.min == 0 {
            current.clone()
        } else {
            BTreeSet::new()
        };
        for count in 1..=max {
            let mut next = BTreeSet::new();
            for pos in &current {
                next.extend(self.match_one(&p.kind, nodes, *pos, depth, limits, budget)?);
            }
            if count >= p.min {
                accepted.extend(&next);
            }
            if next.is_empty() {
                break;
            }
            if next == current {
                if max >= p.min {
                    accepted.extend(next);
                }
                break;
            }
            current = next;
        }
        Ok(accepted)
    }
    fn match_one(
        &self,
        p: &ParticleKind,
        nodes: &[Node<'_, '_>],
        pos: usize,
        depth: usize,
        limits: XsdValidationLimits,
        budget: &mut usize,
    ) -> R<BTreeSet<usize>> {
        step(budget)?;
        match p {
            ParticleKind::Element { name, kind } => {
                let Some(n) = nodes.get(pos) else {
                    return Ok(BTreeSet::new());
                };
                if n.tag_name().name() != name
                    || n.tag_name().namespace() != Some(self.namespace.as_str())
                {
                    return Ok(BTreeSet::new());
                }
                match self.validate_node(*n, kind, depth, limits, budget) {
                    Ok(()) => Ok(BTreeSet::from([pos + 1])),
                    Err(e) if e.0.contains("budget exceeded") => Err(e),
                    Err(_) => Ok(BTreeSet::new()),
                }
            }
            ParticleKind::InlineComplex {
                name,
                particle,
                attributes,
            } => {
                let Some(n) = nodes.get(pos) else {
                    return Ok(BTreeSet::new());
                };
                if n.tag_name().name() != name
                    || n.tag_name().namespace() != Some(self.namespace.as_str())
                {
                    return Ok(BTreeSet::new());
                }
                if depth > limits.max_depth {
                    return Err(err("XML depth budget exceeded"));
                }
                if n.children()
                    .filter(Node::is_text)
                    .any(|t| t.text().is_some_and(|v| !v.trim().is_empty()))
                {
                    return Ok(BTreeSet::new());
                }
                for a in n.attributes() {
                    let Some(spec) = attributes
                        .iter()
                        .find(|s| a.namespace().is_none() && s.name == a.name())
                    else {
                        return Ok(BTreeSet::new());
                    };
                    if self.simple(&spec.kind, a.value()).is_err() {
                        return Ok(BTreeSet::new());
                    }
                }
                if attributes
                    .iter()
                    .any(|a| a.required && n.attribute(a.name.as_str()).is_none())
                {
                    return Ok(BTreeSet::new());
                }
                let children = children(*n).collect::<Vec<_>>();
                let valid = if let Some(p) = particle {
                    self.match_particle(p, &children, 0, depth + 1, limits, budget)?
                        .contains(&children.len())
                } else {
                    children.is_empty()
                };
                Ok(if valid {
                    BTreeSet::from([pos + 1])
                } else {
                    BTreeSet::new()
                })
            }
            ParticleKind::Sequence(v) => {
                let mut positions = BTreeSet::from([pos]);
                for child in v {
                    let mut next = BTreeSet::new();
                    for pos in positions {
                        next.extend(self.match_particle(child, nodes, pos, depth, limits, budget)?);
                    }
                    positions = next;
                }
                Ok(positions)
            }
            ParticleKind::Choice(v) => {
                let mut positions = BTreeSet::new();
                for child in v {
                    positions
                        .extend(self.match_particle(child, nodes, pos, depth, limits, budget)?);
                }
                Ok(positions)
            }
        }
    }
    fn simple(&self, n: &Name, text: &str) -> R<()> {
        if !n.builtin {
            let Some(Kind::Simple { base, values }) = self.types.get(&n.local) else {
                return Err(err("Expected simple type"));
            };
            self.simple(base, text)?;
            if !values.is_empty() && !values.iter().any(|v| v == text) {
                return Err(err("Value outside enumeration"));
            }
            return Ok(());
        }
        let value = text.trim_matches([' ', '\t', '\n', '\r']);
        let valid = match n.local.as_str() {
            "string" => true,
            "boolean" => matches!(value, "true" | "false" | "0" | "1"),
            "int" => integer_lexical(value) && value.parse::<i32>().is_ok(),
            "long" => integer_lexical(value) && value.parse::<i64>().is_ok(),
            "double" => {
                matches!(value, "INF" | "-INF" | "NaN")
                    || (double_lexical(value) && value.parse::<f64>().is_ok())
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(err(format!("Invalid {} lexical value", n.local)))
        }
    }
    pub fn to_xml(&self) -> String {
        let mut s = format!(
            "<xs:schema xmlns:xs=\"{XS}\" xmlns:tns=\"{}\" targetNamespace=\"{}\" elementFormDefault=\"qualified\">",
            escape(&self.namespace),
            escape(&self.namespace)
        );
        for (k, n) in &self.roots {
            s.push_str(&format!(
                "<xs:element name=\"{}\" type=\"{}\"/>",
                escape(k),
                qname(n)
            ));
        }
        for (k, t) in &self.types {
            match t {
                Kind::Simple { base, values } => {
                    s.push_str(&format!(
                        "<xs:simpleType name=\"{}\"><xs:restriction base=\"{}\">",
                        escape(k),
                        qname(base)
                    ));
                    for v in values {
                        s.push_str(&format!("<xs:enumeration value=\"{}\"/>", escape(v)));
                    }
                    s.push_str("</xs:restriction></xs:simpleType>");
                }
                Kind::Complex {
                    base,
                    particle,
                    attributes,
                    abstract_type,
                    final_extension,
                    final_restriction,
                } => {
                    let tokens = [
                        (*final_extension, "extension"),
                        (*final_restriction, "restriction"),
                    ]
                    .into_iter()
                    .filter_map(|(b, s)| b.then_some(s))
                    .collect::<Vec<_>>()
                    .join(" ");
                    s.push_str(&format!(
                        "<xs:complexType name=\"{}\" abstract=\"{}\" final=\"{tokens}\">",
                        escape(k),
                        abstract_type
                    ));
                    if let Some(b) = base {
                        s.push_str(&format!(
                            "<xs:complexContent><xs:extension base=\"{}\">",
                            qname(b)
                        ));
                    }
                    if let Some(p) = particle {
                        write_particle(&mut s, p);
                    }
                    for a in attributes {
                        s.push_str(&format!(
                            "<xs:attribute name=\"{}\" type=\"{}\" use=\"{}\"/>",
                            escape(&a.name),
                            qname(&a.kind),
                            if a.required { "required" } else { "optional" }
                        ));
                    }
                    if base.is_some() {
                        s.push_str("</xs:extension></xs:complexContent>");
                    }
                    s.push_str("</xs:complexType>");
                }
            }
        }
        s.push_str("</xs:schema>");
        s
    }
}
fn step(budget: &mut usize) -> R<()> {
    *budget = budget
        .checked_sub(1)
        .ok_or_else(|| err("Validation step budget exceeded"))?;
    Ok(())
}
fn integer_lexical(v: &str) -> bool {
    let v = v.strip_prefix(['+', '-']).unwrap_or(v);
    !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit())
}
fn double_lexical(v: &str) -> bool {
    let parts = v.split(['e', 'E']).collect::<Vec<_>>();
    if parts.len() > 2 || parts.get(1).is_some_and(|e| !integer_lexical(e)) {
        return false;
    }
    let mantissa = parts[0].strip_prefix(['+', '-']).unwrap_or(parts[0]);
    let mut dot = false;
    let mut digit = false;
    for c in mantissa.bytes() {
        if c == b'.' && !dot {
            dot = true;
        } else if c.is_ascii_digit() {
            digit = true;
        } else {
            return false;
        }
    }
    digit
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('\n', "&#10;")
        .replace('\r', "&#13;")
        .replace('\t', "&#9;")
}
fn qname(n: &Name) -> String {
    format!(
        "{}:{}",
        if n.builtin { "xs" } else { "tns" },
        escape(&n.local)
    )
}
fn write_particle(s: &mut String, p: &Particle) {
    let (tag, extra) = match &p.kind {
        ParticleKind::Element { name, kind } => (
            "element",
            format!(" name=\"{}\" type=\"{}\"", escape(name), qname(kind)),
        ),
        ParticleKind::InlineComplex { name, .. } => {
            ("element", format!(" name=\"{}\"", escape(name)))
        }
        ParticleKind::Sequence(_) => ("sequence", String::new()),
        ParticleKind::Choice(_) => ("choice", String::new()),
    };
    s.push_str(&format!(
        "<xs:{tag}{extra} minOccurs=\"{}\" maxOccurs=\"{}\">",
        p.min,
        p.max.map_or("unbounded".into(), |m| m.to_string())
    ));
    if let ParticleKind::InlineComplex {
        particle,
        attributes,
        ..
    } = &p.kind
    {
        s.push_str("<xs:complexType>");
        if let Some(p) = particle {
            write_particle(s, p);
        }
        for a in attributes {
            s.push_str(&format!(
                "<xs:attribute name=\"{}\" type=\"{}\" use=\"{}\"/>",
                escape(&a.name),
                qname(&a.kind),
                if a.required { "required" } else { "optional" }
            ));
        }
        s.push_str("</xs:complexType>");
    }
    if let ParticleKind::Sequence(v) | ParticleKind::Choice(v) = &p.kind {
        for child in v {
            write_particle(s, child);
        }
    }
    s.push_str(&format!("</xs:{tag}>"));
}
