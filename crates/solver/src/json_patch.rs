//! Atomic JSON model edits, with strict pointers or explicit model selectors.
use serde_json::Value;
use std::{error::Error, fmt};

/// Strict RFC 6902 paths, or the Community model-update selector extensions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum JsonPatchMode {
    #[default]
    Strict,
    /// Array selectors `[id=value]`, nested `[skills/id=value]`, `[value]`,
    /// and bare scalar names. Selectors address every match; no matches is a no-op.
    /// Add may create a missing array immediately before `/-`. Replacing a
    /// selected array element removes it and appends its replacement. Numeric
    /// array indices retain standard in-place replacement semantics.
    Model,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JsonPatchErrorKind {
    InvalidDocument,
    InvalidOperation,
    MissingField(&'static str),
    InvalidPath,
    MissingTarget,
    InvalidIndex,
    ExpectedContainer,
    AmbiguousSource,
    MoveIntoDescendant,
    TestFailed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonPatchError {
    pub operation: usize,
    pub kind: JsonPatchErrorKind,
}
impl fmt::Display for JsonPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON patch operation {}: {:?}",
            self.operation, self.kind
        )
    }
}
impl Error for JsonPatchError {}
#[derive(Clone, Debug)]
enum Token {
    Key(String),
    Match(Vec<String>, String),
}
#[derive(Clone, Debug)]
enum Action {
    Add(Value),
    Remove,
    Replace(Value),
    Test(Value),
    Copy(Vec<Token>),
    Move(Vec<Token>),
}
#[derive(Clone, Debug)]
struct Operation {
    path: Vec<Token>,
    action: Action,
}
/// A validated, reusable patch. Parsing compiles paths once. Applying edits only
/// addressed branches and keeps an undo journal of overwritten/removed values;
/// it does not clone the whole input. Any returned error rolls back the entire
/// patch. This edits JSON only; callers must validate and bind their domain model
/// before submitting a solver problem change.
///
/// Copy/move require one source; move also requires one destination. Model-mode
/// tests require at least one match and check every match. Removing the document
/// root leaves JSON null, since this API always retains a JSON value. Selector
/// queries scan their array; numeric pointers do not scan unrelated rows.
///
/// ```
/// use timetuck::{JsonPatch, JsonPatchMode, JsonValue};
/// let mut input: JsonValue = serde_json::from_str(r#"{"employees":[{"id":"Ann","tags":[]}]}"#)?;
/// let patch = JsonPatch::parse(r#"[{"op":"add","path":"/employees/[id=Ann]/tags/-","value":"Night"}]"#, JsonPatchMode::Model)?;
/// patch.apply(&mut input)?;
/// assert_eq!(input["employees"][0]["tags"][0], "Night");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug)]
pub struct JsonPatch {
    mode: JsonPatchMode,
    operations: Vec<Operation>,
}
impl JsonPatch {
    pub fn parse(text: &str, mode: JsonPatchMode) -> Result<Self, JsonPatchError> {
        let value = serde_json::from_str(text).map_err(|_| JsonPatchError {
            operation: 0,
            kind: JsonPatchErrorKind::InvalidDocument,
        })?;
        Self::from_value(&value, mode)
    }
    pub fn from_value(value: &Value, mode: JsonPatchMode) -> Result<Self, JsonPatchError> {
        let rows = value.as_array().ok_or(JsonPatchError {
            operation: 0,
            kind: JsonPatchErrorKind::InvalidDocument,
        })?;
        let operations = rows
            .iter()
            .enumerate()
            .map(|(operation, row)| {
                let build = || -> Result<Operation, JsonPatchErrorKind> {
                    let field = |name| {
                        row.get(name)
                            .and_then(Value::as_str)
                            .ok_or(JsonPatchErrorKind::MissingField(name))
                    };
                    let path = parse_path(field("path")?, mode)?;
                    let value = || {
                        row.get("value")
                            .cloned()
                            .ok_or(JsonPatchErrorKind::MissingField("value"))
                    };
                    let action = match field("op")? {
                        "add" => Action::Add(value()?),
                        "remove" => Action::Remove,
                        "replace" => Action::Replace(value()?),
                        "test" => Action::Test(value()?),
                        "copy" => Action::Copy(parse_path(field("from")?, mode)?),
                        "move" => Action::Move(parse_path(field("from")?, mode)?),
                        _ => return Err(JsonPatchErrorKind::InvalidOperation),
                    };
                    Ok(Operation { path, action })
                };
                build().map_err(|kind| JsonPatchError { operation, kind })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { mode, operations })
    }
    pub fn len(&self) -> usize {
        self.operations.len()
    }
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
    pub fn apply(&self, input: &mut Value) -> Result<(), JsonPatchError> {
        let mut journal = Vec::new();
        for (operation, op) in self.operations.iter().enumerate() {
            if let Err(kind) = apply_operation(input, op, self.mode, &mut journal) {
                while let Some(undo) = journal.pop() {
                    undo.restore(input);
                }
                return Err(JsonPatchError { operation, kind });
            }
        }
        Ok(())
    }
    /// Preserve the caller's input by cloning it once, then apply the patch.
    pub fn applied(&self, input: &Value) -> Result<Value, JsonPatchError> {
        let mut result = input.clone();
        self.apply(&mut result)?;
        Ok(result)
    }
}
fn decode(s: &str) -> Result<String, JsonPatchErrorKind> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        out.push(if c == '~' {
            match chars.next() {
                Some('0') => '~',
                Some('1') => '/',
                _ => return Err(JsonPatchErrorKind::InvalidPath),
            }
        } else {
            c
        });
    }
    Ok(out)
}
fn parse_path(path: &str, mode: JsonPatchMode) -> Result<Vec<Token>, JsonPatchErrorKind> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let path = path
        .strip_prefix('/')
        .ok_or(JsonPatchErrorKind::InvalidPath)?;
    let mut pieces = vec![];
    let mut start = 0;
    let mut bracket = false;
    for (i, ch) in path.char_indices() {
        if mode == JsonPatchMode::Model {
            if ch == '[' && i == start {
                bracket = true;
            } else if ch == ']' && bracket {
                bracket = false;
            }
        }
        if ch == '/' && !bracket {
            pieces.push(&path[start..i]);
            start = i + 1;
        }
    }
    if bracket {
        return Err(JsonPatchErrorKind::InvalidPath);
    }
    pieces.push(&path[start..]);
    pieces
        .into_iter()
        .map(|piece| {
            if mode == JsonPatchMode::Model && piece.starts_with('[') {
                let text = piece
                    .strip_prefix('[')
                    .and_then(|v| v.strip_suffix(']'))
                    .ok_or(JsonPatchErrorKind::InvalidPath)?;
                let (keys, expected) = if let Some((keys, expected)) = text.split_once('=') {
                    (
                        keys.split('/').map(decode).collect::<Result<Vec<_>, _>>()?,
                        decode(expected)?,
                    )
                } else {
                    (Vec::new(), decode(text)?)
                };
                Ok(Token::Match(keys, expected))
            } else {
                Ok(Token::Key(decode(piece)?))
            }
        })
        .collect()
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Location {
    Key(String),
    Index(usize),
}
type Path = Vec<Location>;
fn node<'a>(mut value: &'a Value, path: &[Location]) -> &'a Value {
    for item in path {
        value = match item {
            Location::Key(k) => &value[k],
            Location::Index(i) => &value[*i],
        };
    }
    value
}
fn node_mut<'a>(mut value: &'a mut Value, path: &[Location]) -> &'a mut Value {
    for item in path {
        value = match item {
            Location::Key(k) => value.get_mut(k).expect("resolved key"),
            Location::Index(i) => value.get_mut(*i).expect("resolved index"),
        };
    }
    value
}
#[derive(Debug)]
enum Undo {
    Root(Value),
    Object(Path, String, Option<Value>),
    Insert(Path, usize),
    Remove(Path, usize, Value),
    Replace(Path, usize, Value),
}
impl Undo {
    fn restore(self, root: &mut Value) {
        match self {
            Self::Root(old) => *root = old,
            Self::Object(path, key, old) => {
                let map = node_mut(root, &path).as_object_mut().expect("object undo");
                if let Some(old) = old {
                    map.insert(key, old);
                } else {
                    map.remove(&key);
                }
            }
            Self::Insert(path, i) => {
                node_mut(root, &path)
                    .as_array_mut()
                    .expect("array undo")
                    .remove(i);
            }
            Self::Remove(path, i, old) => node_mut(root, &path)
                .as_array_mut()
                .expect("array undo")
                .insert(i, old),
            Self::Replace(path, i, old) => {
                node_mut(root, &path).as_array_mut().expect("array undo")[i] = old
            }
        }
    }
}
fn text_matches(value: &Value, text: &str) -> bool {
    match value {
        Value::String(s) => s == text,
        Value::Number(n) => n.to_string() == text,
        Value::Bool(b) => {
            if *b {
                text == "true"
            } else {
                text == "false"
            }
        }
        Value::Null => text == "null",
        _ => false,
    }
}
fn matches(value: &Value, keys: &[String], text: &str) -> bool {
    if let Value::Array(rows) = value {
        return rows.iter().any(|v| matches(v, keys, text));
    }
    match keys.split_first() {
        None => text_matches(value, text),
        Some((key, rest)) => value.get(key).is_some_and(|v| matches(v, rest, text)),
    }
}
fn index(text: &str) -> Result<usize, JsonPatchErrorKind> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(JsonPatchErrorKind::InvalidIndex);
    }
    text.parse().map_err(|_| JsonPatchErrorKind::InvalidIndex)
}
fn slots(
    value: &Value,
    token: &Token,
    mode: JsonPatchMode,
    add: bool,
) -> Result<Vec<Location>, JsonPatchErrorKind> {
    match (value, token) {
        (Value::Object(map), Token::Key(key)) => {
            if !add && !map.contains_key(key) {
                return Err(JsonPatchErrorKind::MissingTarget);
            }
            Ok(vec![Location::Key(key.clone())])
        }
        (Value::Array(rows), Token::Key(key)) if key == "-" && add => {
            Ok(vec![Location::Index(rows.len())])
        }
        (Value::Array(rows), Token::Key(key)) => {
            if let Ok(i) = index(key) {
                if i > rows.len() || (!add && i == rows.len()) {
                    return Err(JsonPatchErrorKind::InvalidIndex);
                }
                Ok(vec![Location::Index(i)])
            } else if mode == JsonPatchMode::Model && key != "-" {
                Ok(rows
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| text_matches(v, key))
                    .map(|(i, _)| Location::Index(i))
                    .collect())
            } else {
                Err(JsonPatchErrorKind::InvalidIndex)
            }
        }
        (Value::Array(rows), Token::Match(keys, expected)) => Ok(rows
            .iter()
            .enumerate()
            .filter(|(_, v)| matches(v, keys, expected))
            .map(|(i, _)| Location::Index(i))
            .collect()),
        _ => Err(JsonPatchErrorKind::ExpectedContainer),
    }
}
fn parents(
    root: &mut Value,
    tokens: &[Token],
    mode: JsonPatchMode,
    create: bool,
    journal: &mut Vec<Undo>,
) -> Result<Vec<Path>, JsonPatchErrorKind> {
    let mut paths = vec![Vec::new()];
    for (i, token) in tokens
        .iter()
        .enumerate()
        .take(tokens.len().saturating_sub(1))
    {
        let mut next = vec![];
        for path in paths {
            if create
                && mode == JsonPatchMode::Model
                && matches!(tokens.get(i + 1), Some(Token::Key(k)) if k == "-")
                && let (Value::Object(map), Token::Key(key)) = (node_mut(root, &path), token)
                && !map.contains_key(key)
            {
                map.insert(key.clone(), Value::Array(Vec::new()));
                journal.push(Undo::Object(path.clone(), key.clone(), None));
            }
            for slot in slots(node(root, &path), token, mode, false)? {
                let mut p = path.clone();
                p.push(slot);
                next.push(p);
            }
        }
        paths = next;
    }
    Ok(paths)
}
fn targets(
    root: &mut Value,
    tokens: &[Token],
    mode: JsonPatchMode,
    add: bool,
    journal: &mut Vec<Undo>,
) -> Result<Vec<Path>, JsonPatchErrorKind> {
    let Some(last) = tokens.last() else {
        return Ok(vec![Vec::new()]);
    };
    let mut out = vec![];
    for path in parents(root, tokens, mode, add, journal)? {
        for slot in slots(node(root, &path), last, mode, add)? {
            let mut p = path.clone();
            p.push(slot);
            out.push(p);
        }
    }
    Ok(out)
}
fn remove(root: &mut Value, path: Path, journal: &mut Vec<Undo>) {
    let Some((slot, parent)) = path.split_last() else {
        journal.push(Undo::Root(root.take()));
        return;
    };
    match slot {
        Location::Key(key) => {
            let old = node_mut(root, parent).as_object_mut().unwrap().remove(key);
            journal.push(Undo::Object(parent.to_vec(), key.clone(), old));
        }
        Location::Index(i) => {
            let old = node_mut(root, parent).as_array_mut().unwrap().remove(*i);
            journal.push(Undo::Remove(parent.to_vec(), *i, old));
        }
    }
}
fn put(root: &mut Value, path: Path, value: Value, add: bool, journal: &mut Vec<Undo>) {
    let Some((slot, parent)) = path.split_last() else {
        journal.push(Undo::Root(std::mem::replace(root, value)));
        return;
    };
    match slot {
        Location::Key(key) => {
            let old = node_mut(root, parent)
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), value);
            journal.push(Undo::Object(parent.to_vec(), key.clone(), old));
        }
        Location::Index(i) => {
            let rows = node_mut(root, parent).as_array_mut().unwrap();
            if add {
                rows.insert(*i, value);
                journal.push(Undo::Insert(parent.to_vec(), *i));
            } else {
                let old = std::mem::replace(&mut rows[*i], value);
                journal.push(Undo::Replace(parent.to_vec(), *i, old));
            }
        }
    }
}
fn numeric_equal(a: &serde_json::Number, b: &serde_json::Number) -> bool {
    if a == b {
        return true;
    }
    if !a.is_f64() && !b.is_f64() {
        return false;
    }
    if a.is_f64() && b.is_f64() {
        return a.as_f64() == b.as_f64();
    }
    let (float, int) = if a.is_f64() {
        (a.as_f64().unwrap(), b)
    } else {
        (b.as_f64().unwrap(), a)
    };
    if float.fract() != 0.0 {
        return false;
    }
    if let Some(i) = int.as_i64() {
        float >= i64::MIN as f64 && float < -(i64::MIN as f64) && float as i64 == i
    } else {
        (0.0..18446744073709551616.0).contains(&float) && int.as_u64() == Some(float as u64)
    }
}
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => numeric_equal(a, b),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|b| equal(v, b)))
        }
        _ => a == b,
    }
}
fn apply_operation(
    root: &mut Value,
    op: &Operation,
    mode: JsonPatchMode,
    journal: &mut Vec<Undo>,
) -> Result<(), JsonPatchErrorKind> {
    match &op.action {
        Action::Copy(from) | Action::Move(from) => {
            let source = targets(root, from, mode, false, journal)?;
            if source.len() != 1 {
                return Err(JsonPatchErrorKind::AmbiguousSource);
            }
            let source = &source[0];
            let value = node(root, source).clone();
            if matches!(op.action, Action::Move(_)) {
                let dest = targets(root, &op.path, mode, true, journal)?;
                if dest.len() != 1 {
                    return Err(JsonPatchErrorKind::AmbiguousSource);
                }
                if dest[0] == *source {
                    return Ok(());
                }
                if dest[0].starts_with(source) {
                    return Err(JsonPatchErrorKind::MoveIntoDescendant);
                }
                remove(root, source.clone(), journal);
            }
            let dest = targets(root, &op.path, mode, true, journal)?;
            for path in dest.into_iter().rev() {
                put(root, path, value.clone(), true, journal);
            }
        }
        Action::Test(expected) => {
            let paths = targets(root, &op.path, mode, false, journal)?;
            if paths.is_empty() || paths.iter().any(|path| !equal(node(root, path), expected)) {
                return Err(JsonPatchErrorKind::TestFailed);
            }
        }
        action => {
            let paths = targets(
                root,
                &op.path,
                mode,
                matches!(action, Action::Add(_)),
                journal,
            )?;
            let selector = mode == JsonPatchMode::Model
                && op.path.last().is_some_and(|t| match t {
                    Token::Match(..) => true,
                    Token::Key(k) => k != "-" && index(k).is_err(),
                });
            for path in paths.into_iter().rev() {
                match action {
                    Action::Remove => remove(root, path, journal),
                    Action::Add(value) => put(root, path, value.clone(), true, journal),
                    Action::Replace(value)
                        if selector && matches!(path.last(), Some(Location::Index(_))) =>
                    {
                        remove(root, path.clone(), journal);
                        let parent = &path[..path.len() - 1];
                        let len = node(root, parent).as_array().unwrap().len();
                        let mut end = parent.to_vec();
                        end.push(Location::Index(len));
                        put(root, end, value.clone(), true, journal);
                    }
                    Action::Replace(value) => put(root, path, value.clone(), false, journal),
                    _ => unreachable!(),
                }
            }
        }
    }
    Ok(())
}
