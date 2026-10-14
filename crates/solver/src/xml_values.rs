//! Typed scalar/property XML values without reflection or new dependencies.
use crate::{
    CivilDate, CivilDateTime, CivilTime, ConfigPropertyError, ConfigPropertyRegistry,
    SolverXmlCause, SolverXmlConfiguration, SolverXmlElement, SolverXmlError, SolverXmlSchemaError,
};
use std::{collections::BTreeMap, error::Error, fmt, str::FromStr, time::Duration};
const BILLION: i128 = 1_000_000_000;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlValueError(pub String);
impl fmt::Display for XmlValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for XmlValueError {}
fn bad(s: &str) -> XmlValueError {
    XmlValueError(s.into())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct XmlDuration {
    nanoseconds: i128,
}
impl XmlDuration {
    pub fn from_nanoseconds(nanoseconds: i128) -> Self {
        Self { nanoseconds }
    }
    pub fn nanoseconds(self) -> i128 {
        self.nanoseconds
    }
    pub fn to_std(self) -> Result<Duration, XmlValueError> {
        let n = u128::try_from(self.nanoseconds)
            .map_err(|_| bad("negative duration cannot be a native time limit"))?;
        Ok(Duration::new(
            u64::try_from(n / BILLION as u128)
                .map_err(|_| bad("duration exceeds native time limit"))?,
            (n % BILLION as u128) as u32,
        ))
    }
    pub fn nullable(text: Option<&str>) -> Result<Option<Self>, XmlValueError> {
        text.filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse())
            .transpose()
    }
}
impl FromStr for XmlDuration {
    type Err = XmlValueError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !text.is_ascii() {
            return Err(bad("duration must be an ASCII ISO seconds-based value"));
        }
        let text = text.to_ascii_uppercase();
        let (negative, text) = if let Some(t) = text.strip_prefix('-') {
            (true, t)
        } else {
            (false, text.strip_prefix('+').unwrap_or(&text))
        };
        let mut rest = text
            .strip_prefix('P')
            .ok_or_else(|| bad("duration requires P prefix"))?;
        let mut time = false;
        let mut time_member = false;
        let mut rank = None;
        let mut total = 0i128;
        let mut members = 0;
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('T') {
                if time {
                    return Err(bad("duplicate duration time separator"));
                }
                time = true;
                rest = r;
                continue;
            }
            let end = rest
                .find(|c: char| c.is_ascii_alphabetic())
                .ok_or_else(|| bad("duration member lacks a unit"))?;
            let number = &rest[..end];
            let unit = rest.as_bytes()[end];
            rest = &rest[end + 1..];
            let (r, multiplier) = match (unit, time) {
                (b'D', false) => (0, 86_400 * BILLION),
                (b'H', true) => (1, 3600 * BILLION),
                (b'M', true) => (2, 60 * BILLION),
                (b'S', true) => (3, BILLION),
                _ => return Err(bad("unsupported or misplaced duration unit")),
            };
            if rank.is_some_and(|old| r <= old) {
                return Err(bad("duration units repeat or are out of order"));
            }
            rank = Some(r);
            let contribution = if r == 3 {
                let num = number.replace(',', ".");
                let (whole, fraction) = num
                    .split_once('.')
                    .map_or((num.as_str(), None), |(a, b)| (a, Some(b)));
                let seconds = whole
                    .parse::<i128>()
                    .map_err(|_| bad("invalid duration seconds"))?;
                let seconds = if negative {
                    seconds
                        .checked_neg()
                        .ok_or_else(|| bad("duration overflow"))?
                } else {
                    seconds
                };
                let nanos = if let Some(f) = fraction {
                    if f.is_empty() || f.len() > 9 || !f.bytes().all(|c| c.is_ascii_digit()) {
                        return Err(bad("duration precision must be one to nine digits"));
                    }
                    f.parse::<i128>().unwrap() * 10i128.pow((9 - f.len()) as u32)
                } else {
                    0
                };
                seconds
                    .checked_mul(BILLION)
                    .and_then(|n| {
                        if negative != whole.starts_with('-') {
                            n.checked_sub(nanos)
                        } else {
                            n.checked_add(nanos)
                        }
                    })
                    .ok_or_else(|| bad("duration overflow"))?
            } else {
                number
                    .parse::<i128>()
                    .map_err(|_| bad("invalid duration number"))?
                    .checked_mul(if negative { -multiplier } else { multiplier })
                    .ok_or_else(|| bad("duration overflow"))?
            };
            total = total
                .checked_add(contribution)
                .ok_or_else(|| bad("duration overflow"))?;
            members += 1;
            time_member |= time;
        }
        if members == 0 || (time && !time_member) {
            return Err(bad("empty duration"));
        }
        Ok(Self::from_nanoseconds(total))
    }
}
impl fmt::Display for XmlDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.nanoseconds.unsigned_abs();
        let seconds = n / BILLION as u128;
        let nanos = n % BILLION as u128;
        if self.nanoseconds < 0 {
            f.write_str("-")?;
        }
        f.write_str("PT")?;
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let sec = seconds % 60;
        if hours != 0 {
            write!(f, "{hours}H")?;
        }
        if minutes != 0 {
            write!(f, "{minutes}M")?;
        }
        if sec != 0 || nanos != 0 || (hours == 0 && minutes == 0) {
            write!(f, "{sec}")?;
            if nanos != 0 {
                write!(f, ".{}", format!("{nanos:09}").trim_end_matches('0'))?;
            }
            f.write_str("S")?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct XmlOffsetDateTime {
    local: CivilDateTime,
    offset_seconds: i32,
}
impl XmlOffsetDateTime {
    pub fn new(local: CivilDateTime, offset_seconds: i32) -> Result<Self, XmlValueError> {
        if !(-64_800..=64_800).contains(&offset_seconds) {
            return Err(bad("UTC offset exceeds eighteen hours"));
        }
        Ok(Self {
            local,
            offset_seconds,
        })
    }
    pub fn local(self) -> CivilDateTime {
        self.local
    }
    pub fn offset_seconds(self) -> i32 {
        self.offset_seconds
    }
    pub fn instant_nanoseconds(self) -> i128 {
        (i128::from(self.local.date().day_number()) * 86400
            + i128::from(self.local.time().hour()) * 3600
            + i128::from(self.local.time().minute()) * 60
            + i128::from(self.local.time().second())
            - i128::from(self.offset_seconds))
            * BILLION
            + i128::from(self.local.time().nanosecond())
    }
    pub fn nullable(text: Option<&str>) -> Result<Option<Self>, XmlValueError> {
        text.filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse())
            .transpose()
    }
}
fn number<T: FromStr>(text: &str) -> Result<T, XmlValueError> {
    text.parse().map_err(|_| bad("invalid date/time number"))
}
impl FromStr for XmlOffsetDateTime {
    type Err = XmlValueError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if !text.is_ascii() {
            return Err(bad("offset date/time must be ASCII"));
        }
        let (date, time) = text
            .split_once('T')
            .ok_or_else(|| bad("date/time requires T separator"))?;
        let (time, offset) = if let Some(t) = time.strip_suffix('Z') {
            (t, 0)
        } else {
            let index = time
                .rfind(['+', '-'])
                .ok_or_else(|| bad("date/time requires an offset"))?;
            let (t, o) = time.split_at(index);
            let negative = o.starts_with('-');
            let fields = o[1..].split(':').collect::<Vec<_>>();
            if !(2..=3).contains(&fields.len())
                || fields
                    .iter()
                    .any(|f| f.len() != 2 || !f.bytes().all(|c| c.is_ascii_digit()))
            {
                return Err(bad("offset requires HH:MM[:SS]"));
            }
            let h = number::<i32>(fields[0])?;
            let m = number::<i32>(fields[1])?;
            let s = if fields.len() == 3 {
                number::<i32>(fields[2])?
            } else {
                0
            };
            if h > 18 || m >= 60 || s >= 60 {
                return Err(bad("invalid UTC offset"));
            }
            let v = h * 3600 + m * 60 + s;
            (t, if negative { -v } else { v })
        };
        let dates = date.rsplitn(3, '-').collect::<Vec<_>>();
        if dates.len() != 3 {
            return Err(bad("date requires year-month-day"));
        }
        let d = CivilDate::new(number(dates[2])?, number(dates[1])?, number(dates[0])?)
            .map_err(|e| XmlValueError(e.to_string()))?;
        let fields = time.split(':').collect::<Vec<_>>();
        if !(2..=3).contains(&fields.len()) {
            return Err(bad("time requires HH:MM[:SS]"));
        }
        let (second, fraction) = if fields.len() == 3 {
            fields[2]
                .split_once('.')
                .map_or((fields[2], None), |(a, b)| (a, Some(b)))
        } else {
            ("0", None)
        };
        let nanos = if let Some(n) = fraction {
            if n.is_empty() || n.len() > 9 || !n.bytes().all(|v| v.is_ascii_digit()) {
                return Err(bad("date/time precision must be one to nine digits"));
            }
            number::<u32>(n)? * 10u32.pow((9 - n.len()) as u32)
        } else {
            0
        };
        let t = CivilTime::new(
            number(fields[0])?,
            number(fields[1])?,
            number(second)?,
            nanos,
        )
        .map_err(|e| XmlValueError(e.to_string()))?;
        Self::new(CivilDateTime::new(d, t), offset)
    }
}
impl fmt::Display for XmlOffsetDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.local.date();
        let t = self.local.time();
        let y = d.year();
        if (0..=9999).contains(&y) {
            write!(f, "{y:04}")?;
        } else if y < 0 {
            write!(f, "-{:04}", y.unsigned_abs())?;
        } else {
            write!(f, "+{y}")?;
        }
        write!(
            f,
            "-{:02}-{:02}T{:02}:{:02}:{:02}",
            d.month(),
            d.day(),
            t.hour(),
            t.minute(),
            t.second()
        )?;
        if t.nanosecond() != 0 {
            write!(
                f,
                ".{}",
                format!("{:09}", t.nanosecond()).trim_end_matches('0')
            )?;
        }
        if self.offset_seconds == 0 {
            f.write_str("Z")
        } else {
            let n = self.offset_seconds.unsigned_abs();
            write!(
                f,
                "{}{:02}:{:02}",
                if self.offset_seconds < 0 { '-' } else { '+' },
                n / 3600,
                (n % 3600) / 60
            )?;
            if !n.is_multiple_of(60) {
                write!(f, ":{:02}", n % 60)?;
            }
            Ok(())
        }
    }
}
pub fn read_xml_properties(
    node: Option<roxmltree::Node<'_, '_>>,
) -> Result<Option<BTreeMap<String, String>>, XmlValueError> {
    let Some(node) = node else { return Ok(None) };
    let mut out = BTreeMap::new();
    for p in node.children().filter(|n| n.is_element()) {
        if p.tag_name().name() != "property"
            || p.tag_name()
                .namespace()
                .is_some_and(|n| n != crate::SOLVER_XML_NAMESPACE)
        {
            return Err(bad("unknown custom property element/namespace"));
        }
        let key = p
            .attribute("name")
            .ok_or_else(|| bad("property name is required"))?;
        let value = p
            .attribute("value")
            .ok_or_else(|| bad("property value is required"))?;
        if out.insert(key.into(), value.into()).is_some() {
            return Err(bad("duplicate property name"));
        }
    }
    Ok(Some(out))
}
pub fn write_xml_properties(values: Option<&BTreeMap<String, String>>) -> Option<SolverXmlElement> {
    values.map(|values| SolverXmlElement {
        name: "customProperties".into(),
        attributes: vec![],
        text: String::new(),
        children: values
            .iter()
            .map(|(k, v)| SolverXmlElement {
                name: "property".into(),
                attributes: vec![("name".into(), k.clone()), ("value".into(), v.clone())],
                text: String::new(),
                children: vec![],
            })
            .collect(),
    })
}
pub fn apply_xml_properties<T: 'static>(
    registry: &ConfigPropertyRegistry<T>,
    target: &mut T,
    values: &BTreeMap<String, String>,
    owner: &str,
) -> Result<(), ConfigPropertyError> {
    registry.apply(
        target,
        owner,
        values.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        "customProperties",
    )
}
impl SolverXmlConfiguration {
    pub fn native_spent_limit(&self) -> Result<Option<Duration>, XmlValueError> {
        self.root()
            .child("termination")
            .map(|n| {
                crate::XmlTerminationSettings::spent_declaration(n)
                    .and_then(|raw| raw.resolve())
                    .map_err(|e| XmlValueError(e.to_string()))
            })
            .transpose()
            .map(Option::flatten)
    }
}
fn xml_error(node: &str, reason: impl Into<String>) -> SolverXmlError {
    SolverXmlError {
        origin: "<typed-object>".into(),
        cause: SolverXmlCause::Schema(SolverXmlSchemaError {
            node: node.into(),
            reason: reason.into(),
        }),
    }
}
type Decode<T> = dyn Fn(roxmltree::Node<'_, '_>) -> Result<T, XmlValueError>;
type Encode<T> = dyn Fn(&T) -> SolverXmlElement;
/// Explicit typed object codecs. No bean introspection or class loading. The
/// decoder owns its returned data; namespace override never enables DTD/entity
/// loading. Syntax errors retain parser causes; object validation uses typed schema errors.
pub struct XmlObjectCodec<T> {
    root: String,
    namespace: Option<String>,
    decode: Box<Decode<T>>,
    encode: Box<Encode<T>>,
}
impl<T> XmlObjectCodec<T> {
    pub fn new(
        root: impl Into<String>,
        namespace: Option<String>,
        decode: impl Fn(roxmltree::Node<'_, '_>) -> Result<T, XmlValueError> + 'static,
        encode: impl Fn(&T) -> SolverXmlElement + 'static,
    ) -> Self {
        Self {
            root: root.into(),
            namespace,
            decode: Box::new(decode),
            encode: Box::new(encode),
        }
    }
    pub fn read(&self, text: &str) -> Result<T, SolverXmlError> {
        self.read_mode(text, false)
    }
    pub fn read_overriding_namespace(&self, text: &str) -> Result<T, SolverXmlError> {
        self.read_mode(text, true)
    }
    fn read_mode(&self, text: &str, override_namespace: bool) -> Result<T, SolverXmlError> {
        let doc = self.document(text, override_namespace)?;
        (self.decode)(doc.root_element()).map_err(|e| xml_error(&self.root, e.to_string()))
    }
    fn document<'a>(
        &self,
        text: &'a str,
        override_namespace: bool,
    ) -> Result<roxmltree::Document<'a>, SolverXmlError> {
        if text.len() > 4 * 1024 * 1024 {
            return Err(xml_error(&self.root, "byte limit exceeded"));
        }
        let doc = roxmltree::Document::parse_with_options(
            text,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: 100_000,
                ..Default::default()
            },
        )
        .map_err(|e| SolverXmlError {
            origin: if matches!(e, roxmltree::Error::DtdDetected) {
                "<typed-object>; disallow-doctype-decl".into()
            } else {
                "<typed-object>".into()
            },
            cause: SolverXmlCause::Syntax(e),
        })?;
        let root = doc.root_element();
        if root.tag_name().name() != self.root
            || (!override_namespace && root.tag_name().namespace() != self.namespace.as_deref())
        {
            return Err(xml_error(
                root.tag_name().name(),
                "unexpected object root/namespace",
            ));
        }
        Ok(doc)
    }
    pub fn write(&self, value: &T) -> Result<String, SolverXmlError> {
        fn escape(s: &str, attribute: bool) -> String {
            let escaped = s
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\r', "&#13;");
            if attribute {
                escaped.replace('\n', "&#10;").replace('\t', "&#9;")
            } else {
                escaped
            }
        }
        fn write(
            e: &SolverXmlElement,
            out: &mut String,
            depth: usize,
        ) -> Result<(), SolverXmlError> {
            if depth > 128 {
                return Err(xml_error(&e.name, "depth limit exceeded"));
            }
            out.push('<');
            out.push_str(&e.name);
            for (k, v) in &e.attributes {
                out.push(' ');
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(&escape(v, true));
                out.push('"');
            }
            out.push('>');
            out.push_str(&escape(&e.text, false));
            for c in &e.children {
                write(c, out, depth + 1)?;
            }
            out.push_str("</");
            out.push_str(&e.name);
            out.push('>');
            Ok(())
        }
        let mut tree = (self.encode)(value);
        if tree.name != self.root {
            return Err(xml_error(&tree.name, "encoder root differs from codec"));
        }
        if let Some(ns) = &self.namespace {
            tree.attributes.push(("xmlns".into(), ns.clone()));
        }
        let mut out = String::new();
        write(&tree, &mut out, 0)?;
        self.document(&out, false)?;
        Ok(out)
    }
}
