//! Native JSON/XML transport for typed basic solution documents and canonical values.
use crate::{
    Assignment, FiniteProblem, JsonScore, ScoreEnvelope, SimpleScore, SolutionCodec, Variable,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedValue {
    pub code: String,
}
#[derive(Clone, Debug)]
pub struct PersistedEntity {
    pub code: String,
    pub value: Option<Arc<PersistedValue>>,
}
#[derive(Clone, Debug)]
pub struct BasicSolutionDocument<S = SimpleScore> {
    pub code: String,
    pub values: Vec<Arc<PersistedValue>>,
    pub entities: Vec<PersistedEntity>,
    pub score: Option<S>,
}
impl<S> BasicSolutionDocument<S> {
    pub fn validate(&self) -> Result<(), String> {
        let mut values = HashMap::new();
        for value in &self.values {
            if values.insert(value.code.as_str(), value).is_some() {
                return Err(format!("duplicate value code ({})", value.code));
            }
        }
        let mut entities = HashSet::new();
        for entity in &self.entities {
            if !entities.insert(&entity.code) {
                return Err(format!("duplicate entity code ({})", entity.code));
            }
            if let Some(value) = &entity.value {
                match values.get(value.code.as_str()) {
                    Some(canonical) if Arc::ptr_eq(canonical, value) => {}
                    Some(_) => {
                        return Err(format!("noncanonical value reference ({})", value.code));
                    }
                    None => return Err(format!("unknown value reference ({})", value.code)),
                }
            }
        }
        Ok(())
    }
    /// Stable domain codes are the actual finite values. The document retains
    /// canonical Arc allocations; committing resolves every code before any write.
    pub fn problem(&self) -> Result<FiniteProblem<String>, String> {
        self.validate()?;
        let domain = self
            .values
            .iter()
            .map(|v| v.code.clone())
            .collect::<Vec<_>>();
        FiniteProblem::new(
            self.entities
                .iter()
                .map(|e| {
                    let v = Variable::new(domain.clone());
                    match &e.value {
                        Some(x) => v.with_initial(x.code.clone()),
                        None => v,
                    }
                })
                .collect(),
        )
        .map_err(|e| e.to_string())
    }
    pub fn commit(&mut self, assignment: &Assignment<String>, score: S) -> Result<(), String> {
        self.problem()?
            .validate(assignment)
            .map_err(|e| e.to_string())?;
        let values: HashMap<_, _> = self
            .values
            .iter()
            .map(|v| (v.code.as_str(), v.clone()))
            .collect();
        let resolved = assignment
            .values()
            .iter()
            .map(|v| v.as_ref().map(|code| values[code.as_str()].clone()))
            .collect::<Vec<_>>();
        for (e, v) in self.entities.iter_mut().zip(resolved) {
            e.value = v
        }
        self.score = Some(score);
        Ok(())
    }
}
fn construct<S>(
    code: String,
    values: Vec<String>,
    entities: Vec<(String, Option<String>)>,
    score: Option<S>,
) -> Result<BasicSolutionDocument<S>, String> {
    let mut lookup = HashMap::new();
    let mut canonical = vec![];
    for code in values {
        let value = Arc::new(PersistedValue { code: code.clone() });
        if lookup.insert(code.clone(), value.clone()).is_some() {
            return Err(format!("duplicate value code ({code})"));
        }
        canonical.push(value)
    }
    let mut rows = vec![];
    for (code, reference) in entities {
        let value = reference
            .map(|r| {
                lookup
                    .get(&r)
                    .cloned()
                    .ok_or_else(|| format!("unknown value reference ({r})"))
            })
            .transpose()?;
        rows.push(PersistedEntity { code, value })
    }
    let document = BasicSolutionDocument {
        code,
        values: canonical,
        entities: rows,
        score,
    };
    document.validate()?;
    Ok(document)
}
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicSolutionJsonCodec;
fn object<'a>(v: &'a Value, keys: &[&str]) -> Result<&'a serde_json::Map<String, Value>, String> {
    let o = v.as_object().ok_or("expected object")?;
    if o.len() != keys.len() || o.keys().any(|k| !keys.contains(&k.as_str())) {
        return Err("missing or unknown JSON member".into());
    }
    Ok(o)
}
fn string(v: &Value) -> Result<String, String> {
    v.as_str()
        .map(str::to_owned)
        .ok_or_else(|| "expected string".into())
}
fn array(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array().ok_or_else(|| "expected array".into())
}
impl<S: JsonScore> SolutionCodec<BasicSolutionDocument<S>> for BasicSolutionJsonCodec {
    fn encode(&self, s: &BasicSolutionDocument<S>) -> Result<Vec<u8>, String> {
        s.validate()?;
        serde_json::to_vec_pretty(&json!({"version":1,"code":s.code,"valueList":s.values.iter().map(|v|json!({"code":v.code})).collect::<Vec<_>>(),"entityList":s.entities.iter().map(|e|json!({"code":e.code,"valueRef":e.value.as_ref().map(|v|&v.code)})).collect::<Vec<_>>(),"score":s.score.as_ref().map(|v|ScoreEnvelope::capture(v).to_json())})).map_err(|e|e.to_string())
    }
    fn decode(&self, input: &[u8]) -> Result<BasicSolutionDocument<S>, String> {
        let v: Value = serde_json::from_slice(input).map_err(|e| e.to_string())?;
        let o = object(&v, &["version", "code", "valueList", "entityList", "score"])?;
        if o["version"].as_u64() != Some(1) {
            return Err("unsupported solution version".into());
        }
        let values = array(&o["valueList"])?
            .iter()
            .map(|v| {
                let o = object(v, &["code"])?;
                string(&o["code"])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let entities = array(&o["entityList"])?
            .iter()
            .map(|v| {
                let o = object(v, &["code", "valueRef"])?;
                Ok((
                    string(&o["code"])?,
                    if o["valueRef"].is_null() {
                        None
                    } else {
                        Some(string(&o["valueRef"])?)
                    },
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let score = if o["score"].is_null() {
            None
        } else {
            Some(
                ScoreEnvelope::from_json(&o["score"])
                    .and_then(|v| v.decode())
                    .map_err(|e| e.to_string())?,
            )
        };
        construct(string(&o["code"])?, values, entities, score)
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicSolutionXmlCodec;
fn escape(text: &str) -> Result<String, String> {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c == '\t'
                || c == '\n'
                || c == '\r'
                || (' '..='\u{d7ff}').contains(&c)
                || ('\u{e000}'..='\u{fffd}').contains(&c)
                || c >= '\u{10000}' =>
            {
                if c == '\r' {
                    out.push_str("&#13;")
                } else {
                    out.push(c)
                }
            }
            _ => return Err("character not representable in XML 1.0".into()),
        }
    }
    Ok(out)
}
fn children<'a, 'i>(
    node: roxmltree::Node<'a, 'i>,
    names: &[&str],
) -> Result<Vec<roxmltree::Node<'a, 'i>>, String> {
    if node.attributes().len() != 0 {
        return Err("unexpected XML attributes".into());
    }
    let mut found = vec![];
    for child in node.children() {
        if child.is_element() {
            if child.tag_name().namespace().is_some() || !names.contains(&child.tag_name().name()) {
                return Err("unexpected XML element or namespace".into());
            }
            found.push(child)
        } else if child.is_text() && child.text().is_some_and(|t| !t.trim().is_empty()) {
            return Err("unexpected XML text".into());
        }
    }
    Ok(found)
}
fn exactly<'a, 'i>(
    nodes: &[roxmltree::Node<'a, 'i>],
    name: &str,
) -> Result<roxmltree::Node<'a, 'i>, String> {
    let matches = nodes
        .iter()
        .filter(|n| n.tag_name().name() == name)
        .copied()
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!("expected one XML element ({name})"));
    }
    Ok(matches[0])
}
fn text(node: roxmltree::Node<'_, '_>) -> Result<String, String> {
    if node.attributes().len() != 0 || node.children().any(|n| n.is_element()) {
        return Err("expected scalar XML text".into());
    }
    Ok(node
        .children()
        .filter_map(|n| if n.is_text() { n.text() } else { None })
        .collect())
}
impl<S: JsonScore> SolutionCodec<BasicSolutionDocument<S>> for BasicSolutionXmlCodec {
    fn encode(&self, s: &BasicSolutionDocument<S>) -> Result<Vec<u8>, String> {
        s.validate()?;
        let mut out = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><solution><version>1</version><code>{}</code><valueList>",
            escape(&s.code)?
        );
        for v in &s.values {
            out.push_str(&format!("<value><code>{}</code></value>", escape(&v.code)?))
        }
        out.push_str("</valueList><entityList>");
        for e in &s.entities {
            out.push_str(&format!("<entity><code>{}</code>", escape(&e.code)?));
            if let Some(v) = &e.value {
                out.push_str(&format!("<valueRef>{}</valueRef>", escape(&v.code)?))
            }
            out.push_str("</entity>")
        }
        out.push_str("</entityList><score>");
        if let Some(score) = &s.score {
            let envelope = ScoreEnvelope::capture(score);
            out.push_str(&format!(
                "<type>{}</type><text>{}</text>",
                envelope.format().name(),
                escape(envelope.text())?
            ));
            if let Some((hard, soft)) = envelope.format().dimensions() {
                out.push_str(&format!(
                    "<hardLevels>{hard}</hardLevels><softLevels>{soft}</softLevels>"
                ))
            }
        }
        out.push_str("</score></solution>");
        Ok(out.into_bytes())
    }
    fn decode(&self, input: &[u8]) -> Result<BasicSolutionDocument<S>, String> {
        let input = std::str::from_utf8(input).map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(input).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        if root.tag_name().name() != "solution" || root.tag_name().namespace().is_some() {
            return Err("expected native solution root".into());
        }
        let fields = children(
            root,
            &["version", "code", "valueList", "entityList", "score"],
        )?;
        if fields.len() != 5 || text(exactly(&fields, "version")?)? != "1" {
            return Err("invalid native solution version/members".into());
        }
        let values = children(exactly(&fields, "valueList")?, &["value"])?
            .iter()
            .map(|&v| {
                let parts = children(v, &["code"])?;
                text(exactly(&parts, "code")?)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let entities = children(exactly(&fields, "entityList")?, &["entity"])?
            .iter()
            .map(|&e| {
                let parts = children(e, &["code", "valueRef"])?;
                let refs = parts
                    .iter()
                    .filter(|n| n.tag_name().name() == "valueRef")
                    .copied()
                    .collect::<Vec<_>>();
                if refs.len() > 1 {
                    return Err("duplicate valueRef".into());
                }
                Ok((
                    text(exactly(&parts, "code")?)?,
                    refs.first().map(|&n| text(n)).transpose()?,
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let parts = children(
            exactly(&fields, "score")?,
            &["type", "text", "hardLevels", "softLevels"],
        )?;
        let score = if parts.is_empty() {
            None
        } else {
            let mut o = serde_json::Map::new();
            o.insert("type".into(), text(exactly(&parts, "type")?)?.into());
            o.insert("score".into(), text(exactly(&parts, "text")?)?.into());
            for (tag, key) in [("hardLevels", "hard_levels"), ("softLevels", "soft_levels")] {
                if parts.iter().any(|n| n.tag_name().name() == tag) {
                    let n = text(exactly(&parts, tag)?)?
                        .parse::<usize>()
                        .map_err(|e| e.to_string())?;
                    o.insert(key.into(), n.into());
                }
            }
            Some(
                ScoreEnvelope::from_json(&Value::Object(o))
                    .and_then(|e| e.decode())
                    .map_err(|e| e.to_string())?,
            )
        };
        construct(text(exactly(&fields, "code")?)?, values, entities, score)
    }
}
