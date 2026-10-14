//! Reduced JSON snapshots of real native consecutive-sequence topology.
//!
//! Format specified by pinned Community SequenceRoundTripTest (Apache-2.0);
//! see compat/provenance.json and THIRD_PARTY_NOTICES.md. This independent native
//! implementation neither reconstructs missing metric/topology data nor claims
//! Jackson, Java interface, reflection or general object-graph compatibility.
use crate::{ConsecutiveSequence, SequenceBreak, SequenceChain};
use serde_json::{Map, Value};
use std::{collections::HashMap, error::Error, fmt, sync::Arc};

#[derive(Debug)]
pub enum SequenceJsonError {
    Json(serde_json::Error),
    Shape(&'static str),
    UnknownProperty(String),
    Item(String),
    ConflictingIdentity(String),
    UnknownIdentity(String),
    IndexOutOfBounds,
}
impl fmt::Display for SequenceJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sequence snapshot JSON: {self:?}")
    }
}
impl Error for SequenceJsonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}
impl From<serde_json::Error> for SequenceJsonError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReducedChainBreaksUnavailable;
impl fmt::Display for ReducedChainBreaksUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("breaks are unavailable in a reduced sequence-chain snapshot")
    }
}
impl Error for ReducedChainBreaksUnavailable {}

/// The chain representation carries flags/items, with no neighboring break data.
#[derive(Debug)]
pub struct ReducedChainSequence<T> {
    items: Vec<Arc<T>>,
    first: bool,
    last: bool,
}
impl<T> ReducedChainSequence<T> {
    pub fn items(&self) -> &[Arc<T>] {
        &self.items
    }
    pub fn count(&self) -> usize {
        self.items.len()
    }
    pub fn is_first(&self) -> bool {
        self.first
    }
    pub fn is_last(&self) -> bool {
        self.last
    }
}
#[derive(Debug)]
pub struct ReducedSequenceChain<T> {
    sequences: Vec<ReducedChainSequence<T>>,
}
impl<T> ReducedSequenceChain<T> {
    pub fn sequences(&self) -> &[ReducedChainSequence<T>] {
        &self.sequences
    }
    pub fn breaks(&self) -> Result<&[ReducedSequenceBreak<T>], ReducedChainBreaksUnavailable> {
        Err(ReducedChainBreaksUnavailable)
    }
}
#[derive(Debug)]
pub struct ReducedSequenceBreak<T> {
    first: bool,
    last: bool,
    previous: Arc<T>,
    next: Arc<T>,
}
impl<T> ReducedSequenceBreak<T> {
    pub fn is_first(&self) -> bool {
        self.first
    }
    pub fn is_last(&self) -> bool {
        self.last
    }
    pub fn previous_sequence_end(&self) -> &Arc<T> {
        &self.previous
    }
    pub fn next_sequence_start(&self) -> &Arc<T> {
        &self.next
    }
}
#[derive(Debug)]
pub struct ReducedSequence<T> {
    sequence: ReducedChainSequence<T>,
    previous_break: Option<ReducedSequenceBreak<T>>,
    next_break: Option<ReducedSequenceBreak<T>>,
}
impl<T> ReducedSequence<T> {
    pub fn items(&self) -> &[Arc<T>] {
        self.sequence.items()
    }
    pub fn count(&self) -> usize {
        self.sequence.count()
    }
    pub fn is_first(&self) -> bool {
        self.sequence.is_first()
    }
    pub fn is_last(&self) -> bool {
        self.sequence.is_last()
    }
    pub fn previous_break(&self) -> Option<&ReducedSequenceBreak<T>> {
        self.previous_break.as_ref()
    }
    pub fn next_break(&self) -> Option<&ReducedSequenceBreak<T>> {
        self.next_break.as_ref()
    }
}

type ReadId<'a, T> = dyn for<'i> Fn(&'i T) -> &'i str + 'a;
type EncodeItem<'a, T> = dyn Fn(&T) -> Result<Value, String> + 'a;
type DecodeItem<'a, T> = dyn Fn(&Value) -> Result<T, String> + 'a;
/// An explicit immutable item adapter, following native typed JSON registration.
/// Full objects must carry the matching string `id` property. First occurrences
/// encode an object; subsequent occurrences encode that ID. Each document has
/// its own identity scope. Decode resolves previously declared IDs to the same
/// Arc allocation; unknown/forward IDs and conflicting ID payloads reject.
/// Callbacks should be pure. Duplicate JSON keys use serde_json's last value.
pub struct SequenceJsonCodec<'a, T> {
    id: Box<ReadId<'a, T>>,
    encode_item: Box<EncodeItem<'a, T>>,
    decode_item: Box<DecodeItem<'a, T>>,
}
impl<'a, T> SequenceJsonCodec<'a, T> {
    pub fn new(
        id: impl for<'i> Fn(&'i T) -> &'i str + 'a,
        encode_item: impl Fn(&T) -> Result<Value, String> + 'a,
        decode_item: impl Fn(&Value) -> Result<T, String> + 'a,
    ) -> Self {
        Self {
            id: Box::new(id),
            encode_item: Box::new(encode_item),
            decode_item: Box::new(decode_item),
        }
    }
    pub fn encode_chain<I, P, V: AsRef<T>, D>(
        &self,
        chain: &SequenceChain<I, P, V, D>,
    ) -> Result<String, SequenceJsonError> {
        let mut encoder = Encoder {
            codec: self,
            identities: HashMap::new(),
        };
        let sequences = chain
            .sequences()
            .iter()
            .map(|sequence| encoder.sequence_core(sequence).map(Value::Object))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::to_string(
            &serde_json::json!({ "sequences": sequences }),
        )?)
    }
    pub fn encode_sequence<I, P, V: AsRef<T>, D>(
        &self,
        chain: &SequenceChain<I, P, V, D>,
        index: usize,
    ) -> Result<String, SequenceJsonError> {
        let sequence = chain
            .sequences()
            .get(index)
            .ok_or(SequenceJsonError::IndexOutOfBounds)?;
        let mut encoder = Encoder {
            codec: self,
            identities: HashMap::new(),
        };
        let mut object = encoder.sequence_core(sequence)?;
        // Identity traversal is part of the wire contract: items, next, previous.
        if let Some(index) = sequence.next_break_index() {
            let gap = chain
                .breaks()
                .get(index)
                .ok_or(SequenceJsonError::IndexOutOfBounds)?;
            object.insert("next_break".into(), encoder.sequence_break(gap)?);
        }
        if let Some(index) = sequence.previous_break_index() {
            let gap = chain
                .breaks()
                .get(index)
                .ok_or(SequenceJsonError::IndexOutOfBounds)?;
            object.insert("previous_break".into(), encoder.sequence_break(gap)?);
        }
        Ok(serde_json::to_string(&object)?)
    }
    pub fn encode_break<I, P, V: AsRef<T>, D>(
        &self,
        chain: &SequenceChain<I, P, V, D>,
        index: usize,
    ) -> Result<String, SequenceJsonError> {
        let gap = chain
            .breaks()
            .get(index)
            .ok_or(SequenceJsonError::IndexOutOfBounds)?;
        let mut encoder = Encoder {
            codec: self,
            identities: HashMap::new(),
        };
        Ok(serde_json::to_string(&encoder.sequence_break(gap)?)?)
    }
    pub fn decode_chain(&self, input: &str) -> Result<ReducedSequenceChain<T>, SequenceJsonError> {
        let value: Value = serde_json::from_str(input)?;
        let root = object(&value, &["sequences"])?;
        let sequences = array(root, "sequences")?;
        let mut decoder = Decoder {
            codec: self,
            identities: HashMap::new(),
        };
        Ok(ReducedSequenceChain {
            sequences: sequences
                .iter()
                .map(|sequence| {
                    decoder.sequence_core(object(sequence, &["first", "items", "last"])?)
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
    pub fn decode_sequence(&self, input: &str) -> Result<ReducedSequence<T>, SequenceJsonError> {
        let value: Value = serde_json::from_str(input)?;
        let object = object(
            &value,
            &["first", "items", "last", "next_break", "previous_break"],
        )?;
        let mut decoder = Decoder {
            codec: self,
            identities: HashMap::new(),
        };
        let sequence = decoder.sequence_core(object)?;
        let next_break = object
            .get("next_break")
            .map(|gap| decoder.sequence_break(gap))
            .transpose()?;
        let previous_break = object
            .get("previous_break")
            .map(|gap| decoder.sequence_break(gap))
            .transpose()?;
        if sequence.first != previous_break.is_none() || sequence.last != next_break.is_none() {
            return Err(SequenceJsonError::Shape(
                "sequence boundary flags disagree with neighboring break presence",
            ));
        }
        Ok(ReducedSequence {
            sequence,
            previous_break,
            next_break,
        })
    }
    pub fn decode_break(&self, input: &str) -> Result<ReducedSequenceBreak<T>, SequenceJsonError> {
        let value: Value = serde_json::from_str(input)?;
        Decoder {
            codec: self,
            identities: HashMap::new(),
        }
        .sequence_break(&value)
    }
}
struct Encoder<'c, 'a, T> {
    codec: &'c SequenceJsonCodec<'a, T>,
    identities: HashMap<String, Value>,
}
impl<T> Encoder<'_, '_, T> {
    fn item(&mut self, item: &T) -> Result<Value, SequenceJsonError> {
        let id = (self.codec.id)(item);
        let object = (self.codec.encode_item)(item).map_err(SequenceJsonError::Item)?;
        if object.get("id").and_then(Value::as_str) != Some(id) || !object.is_object() {
            return Err(SequenceJsonError::Shape(
                "item encoder must return an object with its declared string id",
            ));
        }
        if let Some(previous) = self.identities.get(id) {
            if previous != &object {
                return Err(SequenceJsonError::ConflictingIdentity(id.into()));
            }
            return Ok(Value::String(id.into()));
        }
        self.identities.insert(id.into(), object.clone());
        Ok(object)
    }
    fn sequence_core<I, P, V: AsRef<T>, D>(
        &mut self,
        sequence: &ConsecutiveSequence<I, P, V, D>,
    ) -> Result<Map<String, Value>, SequenceJsonError> {
        let mut object = Map::new();
        object.insert("first".into(), Value::Bool(sequence.is_first()));
        object.insert(
            "items".into(),
            Value::Array(
                sequence
                    .items()
                    .iter()
                    .map(|item| self.item(item.value.as_ref()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        );
        object.insert("last".into(), Value::Bool(sequence.is_last()));
        Ok(object)
    }
    fn sequence_break<I, P, V: AsRef<T>, D>(
        &mut self,
        gap: &SequenceBreak<I, P, V, D>,
    ) -> Result<Value, SequenceJsonError> {
        let mut object = Map::new();
        object.insert("first".into(), Value::Bool(gap.is_first()));
        object.insert("last".into(), Value::Bool(gap.is_last()));
        object.insert(
            "next_sequence_start".into(),
            self.item(gap.next_sequence_start().value.as_ref())?,
        );
        object.insert(
            "previous_sequence_end".into(),
            self.item(gap.previous_sequence_end().value.as_ref())?,
        );
        Ok(Value::Object(object))
    }
}
struct Decoder<'c, 'a, T> {
    codec: &'c SequenceJsonCodec<'a, T>,
    identities: HashMap<String, (Value, Arc<T>)>,
}
impl<T> Decoder<'_, '_, T> {
    fn item(&mut self, value: &Value) -> Result<Arc<T>, SequenceJsonError> {
        if let Some(id) = value.as_str() {
            return self
                .identities
                .get(id)
                .map(|(_, item)| item.clone())
                .ok_or_else(|| SequenceJsonError::UnknownIdentity(id.into()));
        }
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|_| value.is_object())
            .ok_or(SequenceJsonError::Shape(
                "expected item object or prior string identity",
            ))?;
        if let Some((previous, item)) = self.identities.get(id) {
            if previous != value {
                return Err(SequenceJsonError::ConflictingIdentity(id.into()));
            }
            return Ok(item.clone());
        }
        let item = Arc::new((self.codec.decode_item)(value).map_err(SequenceJsonError::Item)?);
        if (self.codec.id)(&item) != id {
            return Err(SequenceJsonError::Shape(
                "decoded item id differs from wire identity",
            ));
        }
        self.identities
            .insert(id.into(), (value.clone(), item.clone()));
        Ok(item)
    }
    fn sequence_core(
        &mut self,
        object: &Map<String, Value>,
    ) -> Result<ReducedChainSequence<T>, SequenceJsonError> {
        let first = flag(object, "first")?;
        let items = array(object, "items")?
            .iter()
            .map(|value| self.item(value))
            .collect::<Result<Vec<_>, _>>()?;
        if items.is_empty() {
            return Err(SequenceJsonError::Shape(
                "individual sequence must have at least one item",
            ));
        }
        Ok(ReducedChainSequence {
            items,
            first,
            last: flag(object, "last")?,
        })
    }
    fn sequence_break(
        &mut self,
        value: &Value,
    ) -> Result<ReducedSequenceBreak<T>, SequenceJsonError> {
        let object = object(
            value,
            &[
                "first",
                "last",
                "next_sequence_start",
                "previous_sequence_end",
            ],
        )?;
        let first = flag(object, "first")?;
        let last = flag(object, "last")?;
        let next = self.item(
            object
                .get("next_sequence_start")
                .ok_or(SequenceJsonError::Shape("missing next sequence start"))?,
        )?;
        let previous = self.item(
            object
                .get("previous_sequence_end")
                .ok_or(SequenceJsonError::Shape("missing previous sequence end"))?,
        )?;
        Ok(ReducedSequenceBreak {
            first,
            last,
            previous,
            next,
        })
    }
}
fn object<'v>(
    value: &'v Value,
    allowed: &[&str],
) -> Result<&'v Map<String, Value>, SequenceJsonError> {
    let object = value
        .as_object()
        .ok_or(SequenceJsonError::Shape("expected object"))?;
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(SequenceJsonError::UnknownProperty(name.clone()));
    }
    Ok(object)
}
fn flag(object: &Map<String, Value>, name: &str) -> Result<bool, SequenceJsonError> {
    object
        .get(name)
        .and_then(Value::as_bool)
        .ok_or(SequenceJsonError::Shape("missing or invalid boolean flag"))
}
fn array<'v>(object: &'v Map<String, Value>, name: &str) -> Result<&'v [Value], SequenceJsonError> {
    object
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(SequenceJsonError::Shape("missing or invalid array"))
}
